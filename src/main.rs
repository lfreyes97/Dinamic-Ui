mod appicons;
mod config;
mod control;
mod gfx;
mod ipc;
mod launcher;
mod notifications;
mod power;
mod sysinfo;
mod theme;
mod thumbs;
mod ui;

use std::time::{Duration, Instant};

use chrono::Local;
use control::ControlServer;
use gfx::Gfx;
use ipc::HyprlandIpc;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_registry,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{generic::Generic, timer::Timer, EventLoop, Interest, Mode, PostAction},
        calloop_wayland_source::WaylandSource,
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
};
use theme::Palette;
use ui::{metrics, UiState, PANEL_GAP};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_region, wl_seat, wl_surface},
    Connection, QueueHandle,
};

type InputRects = (
    (i32, i32, i32, i32),
    (i32, i32, i32, i32),
    (i32, i32, i32, i32),
    (i32, i32, i32, i32),
);

struct DinamicUi {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,

    layer: LayerSurface,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,

    gfx: Gfx,
    ui_state: UiState,
    ipc: HyprlandIpc,
    notifications: notifications::Notifications,
    last_notification_revision: u64,
    sysinfo: sysinfo::SystemMonitor,
    last_sysinfo_revision: u64,
    last_thumb_revision: u64,
    control: ControlServer,
    palette: Palette,
    last_snapshot: ipc::HyprlandSnapshot,
    last_minute: String,
    pending_events: Vec<egui::Event>,

    width: u32,
    height: u32,
    /// Último rect del input region (x, w, h) para no recrearlo igual.
    last_input_region: Option<InputRects>,
    scale: i32,
    last_frame: Instant,
    last_battery: Instant,
    last_dock_query: Instant,
    menu_focus: bool,
    idle_frames: u32,
    /// Comandos del socket de control (entregados por la fuente calloop).
    pending_control: Vec<control::Command>,
    /// Manejador de la cola Wayland (para `draw` desde el timer).
    qh: QueueHandle<Self>,
    /// El timer de calloop pone `tick` y se reprograma con `tick_interval`.
    tick: bool,
    tick_interval: Duration,
    last_rendered: bool,
    exit: bool,
}

fn main() {
    if let Some(code) = control::handle_cli() {
        std::process::exit(code);
    }
    if let Err(e) = run() {
        eprintln!("[dinamic-ui] error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    config::init();
    let conn = Connection::connect_to_env()?;
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    let compositor_state = CompositorState::bind(&globals, &qh)?;
    let layer_shell = LayerShell::bind(&globals, &qh)?;

    let surface = compositor_state.create_surface(&qh);
    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Top, Some("dinamic-ui"), None);
    // Surface fijo a TODA la pantalla: arriba van isla/barra/panel y abajo el
    // dock. El input region limita los clics a los elementos dibujados.
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    layer.set_size(0, 0);
    layer.commit(); // configure inicial

    let backend = conn.backend();
    let gfx = Gfx::new(&backend, layer.wl_surface(), 1.0)?;

    let mut app = DinamicUi {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        compositor_state,
        layer,
        keyboard: None,
        pointer: None,
        gfx,
        ui_state: UiState::new(),
        ipc: HyprlandIpc::new(),
        notifications: notifications::Notifications::new(),
        last_notification_revision: 0,
        sysinfo: sysinfo::SystemMonitor::new(),
        last_sysinfo_revision: 0,
        last_thumb_revision: 0,
        control: ControlServer::new()?,
        palette: Palette::load(),
        last_snapshot: ipc::HyprlandSnapshot::default(),
        last_minute: String::new(),
        pending_events: Vec::new(),
        width: 1920,
        height: metrics().surface_h,
        last_input_region: None,
        scale: 1,
        last_frame: Instant::now(),
        last_battery: Instant::now() - std::time::Duration::from_secs(5),
        last_dock_query: Instant::now() - std::time::Duration::from_secs(2),
        menu_focus: false,
        idle_frames: 0,
        pending_control: Vec::new(),
        qh: qh.clone(),
        tick: false,
        tick_interval: Duration::from_millis(16),
        last_rendered: false,
        exit: false,
    };

    // Event loop único: eventos Wayland + un timer que marca los ticks de UI.
    let mut event_loop: EventLoop<'static, DinamicUi> = EventLoop::try_new()?;
    WaylandSource::new(conn, event_queue).insert(event_loop.handle())?;

    // El socket de control es una fuente calloop: los keybinds de Hyprland
    // despiertan el loop al instante, sin esperar al intervalo del timer.
    let control_listener = app.control.try_clone_listener()?;
    event_loop.handle().insert_source(
        Generic::new(control_listener, Interest::READ, Mode::Level),
        |_, _, app: &mut DinamicUi| {
            let commands = app.control.poll();
            if !commands.is_empty() {
                app.pending_control.extend(commands);
                app.tick = true;
            }
            Ok(PostAction::Continue)
        },
    )?;

    event_loop.handle().insert_source(
        Timer::from_duration(Duration::from_millis(16)),
        |_, _, app: &mut DinamicUi| {
            app.tick = true;
            smithay_client_toolkit::reexports::calloop::timer::TimeoutAction::ToDuration(
                app.tick_interval,
            )
        },
    )?;

    while !app.exit {
        event_loop.dispatch(Some(Duration::from_secs(1)), &mut app)?;
        if app.exit {
            break;
        }
        if app.tick {
            app.tick = false;
            let qh = app.qh.clone();
            app.draw(&qh);
            app.tick_interval = if app.last_rendered || app.gfx.needs_repaint() {
                Duration::from_millis(16)
            } else {
                Duration::from_millis(250)
            };
        }
    }

    println!("[dinamic-ui] saliendo");
    Ok(())
}

impl DinamicUi {
    fn refresh_menu_entries(&mut self) {
        use launcher::MenuMode;
        self.ui_state.menu_entries = match self.ui_state.menu_mode {
            MenuMode::Apps => launcher::apps(),
            MenuMode::Files => launcher::files(&config::get().menu.file_dirs),
            MenuMode::Workspaces => launcher::workspaces(&self.ui_state.snapshot.occupied),
            MenuMode::Commands => launcher::commands(),
        };
        self.ui_state.menu_selected = 0;
    }

    /// Aplica una imagen con `awww img` (su demonio debe estar en marcha).
    fn change_wallpaper_image(&mut self, path: &std::path::Path) {
        sysinfo::spawn_detached(vec![
            "awww".to_string(),
            "img".to_string(),
            path.to_string_lossy().into_owned(),
        ]);
    }

    /// Pinta un color sólido con `awww` (su demonio debe estar en marcha).
    /// El fondo persiste aunque se cierre la UI.
    fn change_wallpaper(&mut self, color: &str) {
        sysinfo::spawn_detached(vec![
            "awww".to_string(),
            "clear".to_string(),
            color.to_string(),
        ]);
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) {
        let mut dirty = false;

        // Comandos del socket de control (keybinds de Hyprland).
        let pending_control = std::mem::take(&mut self.pending_control);
        for command in pending_control {
            dirty = true;
            match command {
                control::Command::Toggle => self.ui_state.expanded = !self.ui_state.expanded,
                control::Command::Expand => self.ui_state.expanded = true,
                control::Command::Collapse => {
                    self.ui_state.expanded = false;
                    self.ui_state.show_panel = false;
                }
                control::Command::Panel => self.ui_state.show_panel = !self.ui_state.show_panel,
                control::Command::Compact => {
                    self.ui_state.expanded = false;
                    self.ui_state.island_compact = true;
                }
                control::Command::Notch => {
                    self.ui_state.expanded = false;
                    self.ui_state.island_compact = false;
                    self.ui_state.island_active = false;
                }
                control::Command::Overview => {
                    self.ui_state.overview = !self.ui_state.overview;
                    if self.ui_state.overview {
                        self.ui_state.show_panel = false;
                    }
                }
                control::Command::Menu => {
                    self.ui_state.menu_open = !self.ui_state.menu_open;
                    let focus = if self.ui_state.menu_open {
                        KeyboardInteractivity::Exclusive
                    } else {
                        KeyboardInteractivity::OnDemand
                    };
                    self.layer.set_keyboard_interactivity(focus);
                    self.layer.commit();
                    if self.ui_state.menu_open {
                        self.ui_state.overview = false;
                        self.ui_state.show_panel = false;
                        self.ui_state.menu_query.clear();
                        self.ui_state.menu_selected = 0;
                        self.refresh_menu_entries();
                    }
                }
                control::Command::Quit => self.exit = true,
                control::Command::Reload => {
                    if let Err(e) = config::reload() {
                        eprintln!("[dinamic-ui] reload: {e}");
                    }
                    self.palette = Palette::load();
                }
            }
        }

        // Eventos de Hyprland: destello breve y consulta inmediata.
        if self.ipc.poll_events() {
            self.ui_state.pulse = 1.0;
            self.ipc.force_refresh();
            dirty = true;
        }
        self.ipc.poll();
        let snapshot = self.ipc.snapshot().clone();
        if snapshot != self.last_snapshot {
            self.last_snapshot = snapshot.clone();
            self.ui_state.snapshot = snapshot;
            dirty = true;
        }

        // Notificaciones DBus (cola alimentada por un hilo propio).
        let (notification, _count, revision) = self.notifications.poll(
            std::time::Duration::from_secs(config::get().notifications.ttl_secs),
        );
        if revision != self.last_notification_revision {
            self.last_notification_revision = revision;
            dirty = true;
        }
        if self.ui_state.dismiss_notification {
            self.ui_state.dismiss_notification = false;
            self.notifications.dismiss();
            dirty = true;
        }
        self.ui_state.notification = notification;
        self.ui_state.notification_count = self.notifications.session_count() as usize;
        self.ui_state.notification_list = self.notifications.list();
        self.ui_state.dnd = self.notifications.dnd();

        if let Some(id) = self.ui_state.dismiss_notification_id.take() {
            self.notifications.dismiss_id(id);
            dirty = true;
        }
        if self.ui_state.clear_all_notifications {
            self.ui_state.clear_all_notifications = false;
            self.notifications.clear_all();
            dirty = true;
        }
        if self.ui_state.toggle_dnd {
            self.ui_state.toggle_dnd = false;
            let dnd = !self.notifications.dnd();
            self.notifications.set_dnd(dnd);
            dirty = true;
        }
        if let Some(color) = self.ui_state.wallpaper_color.take() {
            self.change_wallpaper(&color);
            dirty = true;
        }
        if let Some(path) = self.ui_state.wallpaper_image_request.take() {
            self.change_wallpaper_image(&path);
            dirty = true;
        }

        for command in self.ui_state.pending_commands.drain(..) {
            self.ipc.dispatch(&command);
        }
        // Comandos externos (hyprctl eval, xdg-open…) fuera del hilo de render.
        for argv in self.ui_state.pending_spawns.drain(..) {
            sysinfo::spawn_detached(argv);
        }

        // Datos del sistema (panel e iconos de la barra): el hilo solo consulta
        // con la barra expandida o el panel abierto.
        self.sysinfo
            .set_active(self.ui_state.show_panel || self.ui_state.expanded);
        // Una miniatura terminó de generarse: repintar para mostrarla.
        let thumb_revision = self.ui_state.thumbs.revision();
        if thumb_revision != self.last_thumb_revision {
            self.last_thumb_revision = thumb_revision;
            dirty = true;
        }
        if let Some((info, revision)) = self.sysinfo.poll() {
            if revision != self.last_sysinfo_revision {
                self.last_sysinfo_revision = revision;
                if info.layout != self.ui_state.system.layout {
                    if let Some(preset) = info
                        .layout
                        .as_deref()
                        .and_then(ui::panel::layout_preset_index)
                    {
                        self.ui_state.layout_preset = preset;
                    }
                }
                // El fondo real manda sobre la selección optimista de la rejilla.
                self.ui_state.wallpaper_selected = match &info.wallpaper {
                    Some(sysinfo::Wallpaper::Color(hex)) => ui::panel::wallpaper_swatch_index(hex),
                    _ => None,
                };
                self.ui_state.wallpaper_image = match &info.wallpaper {
                    Some(sysinfo::Wallpaper::Image(path)) => Some(std::path::PathBuf::from(path)),
                    _ => None,
                };
                self.ui_state.system = info;
                dirty = true;
            }
        }
        if self.ui_state.should_exit {
            self.exit = true;
        }

        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        if self.ui_state.pulse > 0.0 {
            self.ui_state.pulse = (self.ui_state.pulse - dt * 1.6).max(0.0);
            dirty = true;
        }

        // Animación isla <-> barra.
        let previous_exp = self.ui_state.expansion;
        let exp_target = if self.ui_state.expanded { 1.0 } else { 0.0 };
        self.ui_state.expansion = approach(self.ui_state.expansion, exp_target, dt * 4.5);

        // El panel tiene su propio progreso: así se cierra animado.
        let previous_panel = self.ui_state.panel_progress;
        let panel_target = if self.ui_state.show_panel { 1.0 } else { 0.0 };
        self.ui_state.panel_progress =
            approach(self.ui_state.panel_progress, panel_target, dt * 6.0);

        // La notificación entra/sale con su propio progreso.
        let previous_notif = self.ui_state.notification_progress;
        let notif_target = if self.ui_state.notification.is_some() {
            1.0
        } else {
            0.0
        };
        self.ui_state.notification_progress =
            approach(self.ui_state.notification_progress, notif_target, dt * 2.0);

        let animating = (self.ui_state.expansion - previous_exp).abs() > 0.0005
            || (self.ui_state.panel_progress - previous_panel).abs() > 0.0005
            || (self.ui_state.notification_progress - previous_notif).abs() > 0.0005;

        let t = smootherstep(self.ui_state.expansion);
        let panel_t = smootherstep(self.ui_state.panel_progress);
        let m = metrics();
        // La notificación tiene tarjeta propia: la isla no cambia de ancho.
        let pill_w = lerp(m.island_w as f32, m.bar_w as f32, t);
        // El surface no se redimensiona: solo movemos el input region con la
        // barra, el panel y la tarjeta de notificación.
        let bar = if t < 0.5 {
            let r = ui::island_rect(
                self.width as f32,
                self.ui_state.island_progress * self.ui_state.island_progress,
            );
            (
                r.min.x as i32,
                0,
                r.width() as i32,
                r.height().max(8.0) as i32,
            )
        } else {
            (
                ((self.width as f32 - pill_w) / 2.0).round() as i32,
                0,
                pill_w.round() as i32,
                metrics().bar_h as i32,
            )
        };
        let card_w = ui::panel_width(t).round();
        let card_x = ((self.width as f32 - card_w) / 2.0).round() as i32;
        let card = (
            card_x,
            (metrics().bar_h as f32 + PANEL_GAP) as i32,
            card_w as i32,
            (metrics().panel_h as f32 * panel_t).round() as i32,
        );
        let nr = ui::notification_card(self.width as f32);
        let notif = (
            nr.min.x as i32,
            nr.min.y as i32,
            nr.width() as i32,
            if panel_t < 0.5 && self.ui_state.notification.is_some() {
                nr.height() as i32
            } else {
                0
            },
        );
        let dock = if self.ui_state.dock_progress > 0.01 {
            (0, self.height as i32 - 72, self.width as i32, 72)
        } else {
            (0, 0, 0, 0)
        };
        let screen_rect = (0, 0, self.width as i32, self.height as i32);
        let region = if self.ui_state.overview || self.ui_state.menu_open {
            (screen_rect, screen_rect, screen_rect, screen_rect)
        } else {
            (bar, card, notif, dock)
        };
        if self.last_input_region != Some(region) {
            let wl_region = self.compositor_state.wl_compositor().create_region(qh, ());
            wl_region.add(bar.0, bar.1, bar.2, bar.3);
            if card.3 > 0 {
                wl_region.add(card.0, card.1, card.2, card.3);
            }
            if notif.3 > 0 {
                wl_region.add(notif.0, notif.1, notif.2, notif.3);
            }
            if dock.3 > 0 {
                wl_region.add(dock.0, dock.1, dock.2, dock.3);
            }
            self.layer.set_input_region(Some(&wl_region));
            wl_region.destroy();
            self.last_input_region = Some(region);
        }

        let minute = Local::now().format("%H:%M").to_string();
        if minute != self.last_minute {
            self.last_minute = minute;
            dirty = true;
        }

        // Batería: sysfs es barato, pero tampoco hace falta leerlo cada frame.
        if self.last_battery.elapsed() >= std::time::Duration::from_secs(2) {
            self.last_battery = Instant::now();
            let battery = power::read();
            if battery != self.ui_state.battery {
                self.ui_state.battery = battery;
                dirty = true;
            }
        }

        // El command menu necesita el foco de teclado mientras está abierto.
        if self.ui_state.menu_open != self.menu_focus {
            self.menu_focus = self.ui_state.menu_open;
            let focus = if self.menu_focus {
                KeyboardInteractivity::Exclusive
            } else {
                KeyboardInteractivity::OnDemand
            };
            self.layer.set_keyboard_interactivity(focus);
            self.layer.commit();
        }

        // Dock: cursor cerca del borde inferior y clientes para los iconos.
        if self.last_dock_query.elapsed() >= std::time::Duration::from_millis(120) {
            self.last_dock_query = Instant::now();
            if let Some((x, y)) = self.ipc.cursor_pos() {
                let near_bottom =
                    std::env::var_os("DINAMIC_UI_DOCK").is_some() || y >= self.height as i32 - 24;
                if near_bottom != self.ui_state.dock_visible {
                    self.ui_state.dock_visible = near_bottom;
                    dirty = true;
                }
                // Zona del notch: franja superior centrada.
                let near_top = std::env::var_os("DINAMIC_UI_ISLAND").is_some()
                    || (y <= 18 && (x - self.width as i32 / 2).abs() <= 100);
                if near_top != self.ui_state.island_active {
                    self.ui_state.island_active = near_top;
                    dirty = true;
                }
            }
            let clients = self.ipc.clients();
            if clients != self.ui_state.dock_clients {
                self.ui_state.dock_clients = clients;
                dirty = true;
            }
        }
        let island_target = if self.ui_state.island_active || self.ui_state.island_compact {
            1.0
        } else {
            0.0
        };
        let island_before = self.ui_state.island_progress;
        self.ui_state.island_progress =
            approach(self.ui_state.island_progress, island_target, dt * 6.0);
        if (self.ui_state.island_progress - island_before).abs() > 0.0005 {
            dirty = true;
        }

        let dock_target = if self.ui_state.dock_visible { 1.0 } else { 0.0 };
        let dock_before = self.ui_state.dock_progress;
        self.ui_state.dock_progress = approach(self.ui_state.dock_progress, dock_target, dt * 5.0);
        if (self.ui_state.dock_progress - dock_before).abs() > 0.0005 {
            dirty = true;
        }
        if let Some(action) = self.ui_state.dock_action.take() {
            match action {
                ui::DockAction::Focus(address) => {
                    self.ipc.dispatch(&format!("focuswindow address:{address}"));
                }
                ui::DockAction::Launch(command) => {
                    let _ = std::process::Command::new(&command).spawn();
                }
            }
            dirty = true;
        }
        if let Some(action) = self.ui_state.menu_action.take() {
            match action {
                launcher::EntryAction::Launch(command) => {
                    let parts: Vec<&str> = command.split_whitespace().collect();
                    if let Some((first, rest)) = parts.split_first() {
                        let _ = std::process::Command::new(first).args(rest).spawn();
                    }
                }
                launcher::EntryAction::OpenFile(path) => {
                    let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
                }
                launcher::EntryAction::Workspace(id) => {
                    self.ipc.dispatch(&format!("workspace {id}"));
                }
                launcher::EntryAction::Command(command) => match command.as_str() {
                    "panel" => self.ui_state.show_panel = !self.ui_state.show_panel,
                    "overview" => self.ui_state.overview = !self.ui_state.overview,
                    "reload" => {
                        let _ = config::reload();
                        self.palette = Palette::load();
                    }
                    "compact" => self.ui_state.island_compact = true,
                    "quit" => self.exit = true,
                    _ => {}
                },
            }
            dirty = true;
        }
        if let Some(address) = self.ui_state.overview_focus.take() {
            self.ipc.dispatch(&format!("focuswindow address:{address}"));
            self.ui_state.overview = false;
            dirty = true;
        }
        if let Some(id) = self.ui_state.overview_workspace.take() {
            self.ipc.dispatch(&format!("workspace {id}"));
            self.ui_state.overview = false;
            dirty = true;
        }

        let scale = self.scale.max(1) as u32;
        let physical = (self.width * scale, self.height * scale);
        self.gfx.resize(physical.0, physical.1);
        self.gfx.set_scale(scale as f32);
        self.gfx
            .set_logical_size(self.width as f32, self.height as f32);

        // Repintamos solo cuando hay cambios; de vez en cuando para refrescar
        // el reloj y volver a consultar el IPC. Si no hay nada que pintar ni
        // entrada de ratón, nos saltamos incluso `run_ui` y el tesselado.
        self.idle_frames = self.idle_frames.wrapping_add(1);
        let has_input = !self.pending_events.is_empty();
        let render = dirty
            || animating
            || has_input
            || self.gfx.needs_repaint()
            || self.idle_frames.is_multiple_of(15);

        // El ritmo lo marca el timer de calloop; no pedimos frame callbacks
        // Wayland para no despertar 60 veces por segundo en reposo.
        self.last_rendered = render;
        if render {
            for event in self.pending_events.drain(..) {
                self.gfx.push_event(event);
            }
            if let Err(e) =
                self.gfx
                    .draw(&mut self.ui_state, self.palette, t, panel_t, pill_w, render)
            {
                eprintln!("[dinamic-ui] render: {e}");
            }
        }
        self.layer.commit();
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Curva con velocidad cero en ambos extremos: el morph arranca y termina suave.
fn smootherstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn approach(current: f32, target: f32, step: f32) -> f32 {
    if (current - target).abs() <= step {
        target
    } else if current < target {
        current + step
    } else {
        current - step
    }
}

impl CompositorHandler for DinamicUi {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        self.scale = new_factor.max(1);
        self.layer.wl_surface().set_buffer_scale(self.scale);
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.draw(qh);
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for DinamicUi {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl LayerShellHandler for DinamicUi {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        self.width = configure.new_size.0.max(1);
        self.height = configure.new_size.1.max(1);
        self.draw(qh);
    }
}

impl SeatHandler for DinamicUi {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = self.seat_state.get_keyboard(qh, &seat, None).ok();
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer = self.seat_state.get_pointer(qh, &seat).ok();
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            if let Some(k) = self.keyboard.take() {
                k.release();
            }
        }
        if capability == Capability::Pointer {
            if let Some(p) = self.pointer.take() {
                p.release();
            }
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for DinamicUi {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        // Redibujar al instante: el menu y las animaciones no esperan al timer.
        self.tick = true;
        if self.ui_state.menu_open {
            use launcher::MenuMode;
            let count =
                launcher::filter(&self.ui_state.menu_entries, &self.ui_state.menu_query).len();
            match event.keysym {
                Keysym::Escape => {
                    self.ui_state.menu_open = false;
                    self.layer
                        .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                    self.layer.commit();
                }
                Keysym::Up => {
                    self.ui_state.menu_selected = self.ui_state.menu_selected.saturating_sub(1);
                }
                Keysym::Down => {
                    if count > 0 {
                        self.ui_state.menu_selected =
                            (self.ui_state.menu_selected + 1).min(count - 1);
                    }
                }
                Keysym::Tab => {
                    self.ui_state.menu_mode = self.ui_state.menu_mode.next();
                    self.ui_state.menu_query.clear();
                    self.refresh_menu_entries();
                }
                Keysym::BackSpace => {
                    self.ui_state.menu_query.pop();
                    self.ui_state.menu_selected = 0;
                }
                Keysym::Return => {
                    if let Some(entry) =
                        launcher::filter(&self.ui_state.menu_entries, &self.ui_state.menu_query)
                            .into_iter()
                            .nth(self.ui_state.menu_selected)
                    {
                        let action = entry.action;
                        self.ui_state.menu_action = Some(action);
                        self.ui_state.menu_open = false;
                        self.layer
                            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                        self.layer.commit();
                    }
                }
                Keysym::_1 | Keysym::_2 | Keysym::_3 | Keysym::_4 => {
                    let index = match event.keysym {
                        Keysym::_1 => 0,
                        Keysym::_2 => 1,
                        Keysym::_3 => 2,
                        _ => 3,
                    };
                    self.ui_state.menu_mode = MenuMode::ALL[index];
                    self.ui_state.menu_query.clear();
                    self.refresh_menu_entries();
                }
                _ => {
                    let text = xkbcommon::xkb::keysym_to_utf8(event.keysym);
                    if text.chars().count() == 1 && !text.chars().next().unwrap().is_control() {
                        self.ui_state.menu_query.push_str(&text);
                        self.ui_state.menu_selected = 0;
                    }
                }
            }
            return;
        }

        if event.keysym == Keysym::Escape {
            // Esc nunca cierra el proceso: solo deshace la capa superior.
            // Para salir: botón "Salir" del panel o `dinamic-ui --quit`.
            if self.ui_state.overview {
                self.ui_state.overview = false;
            } else if self.ui_state.show_panel {
                self.ui_state.show_panel = false;
            } else if self.ui_state.expanded {
                self.ui_state.expanded = false;
            }
        }
    }

    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
    }
}

impl PointerHandler for DinamicUi {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let mut has_events = false;
        for event in events {
            if event.surface != *self.layer.wl_surface() {
                continue;
            }
            has_events = true;
            let pos = egui::Pos2::new(event.position.0 as f32, event.position.1 as f32);
            match event.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.pending_events.push(egui::Event::PointerMoved(pos));
                }
                PointerEventKind::Leave { .. } => {
                    self.pending_events.push(egui::Event::PointerGone);
                }
                PointerEventKind::Press { button, .. } => {
                    if button == 0x110 {
                        self.pending_events.push(egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: egui::Modifiers::NONE,
                        });
                    }
                }
                PointerEventKind::Release { button, .. } => {
                    if button == 0x110 {
                        self.pending_events.push(egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: false,
                            modifiers: egui::Modifiers::NONE,
                        });
                    }
                }
                // Rueda/touchpad → scroll de egui (las páginas del panel hacen scroll).
                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    let delta =
                        egui::Vec2::new(-horizontal.absolute as f32, -vertical.absolute as f32);
                    if delta != egui::Vec2::ZERO {
                        self.pending_events.push(egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta,
                            phase: egui::TouchPhase::Move,
                            source: egui::MouseWheelSource::Unknown,
                            modifiers: egui::Modifiers::NONE,
                        });
                    }
                }
            }
        }
        if has_events {
            // Responder al ratón de inmediato (dock/isla) sin esperar 250 ms.
            self.tick = true;
        }
    }
}

delegate_registry!(DinamicUi);

impl ProvidesRegistryState for DinamicUi {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

impl smithay_client_toolkit::dispatch2::Dispatch2<wl_region::WlRegion, DinamicUi> for () {
    fn event(
        &self,
        _: &mut DinamicUi,
        _: &wl_region::WlRegion,
        _: wl_region::Event,
        _: &Connection,
        _: &QueueHandle<DinamicUi>,
    ) {
    }
}

smithay_client_toolkit::delegate_dispatch2!(DinamicUi);
