// El bucle de eventos Wayland del shell. La lógica vive en la lib (`src/lib.rs`);
// el previewer interactivo (feature `previewer`) la reutiliza sin compositor.

use dinamic_ui::{config, control, gfx, ipc, needs, power, sysinfo, theme, ui};

use std::time::{Duration, Instant};

use chrono::Local;
use control::ControlServer;
use gfx::Gfx;
use ipc::HyprlandIpc;
use needs::{Need, NeedTracker};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_registry,
    globals::GlobalData,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{generic::Generic, timer::Timer, EventLoop, Interest, Mode, PostAction},
        calloop_wayland_source::WaylandSource,
        protocols::wp::cursor_shape::v1::client::{
            wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1},
            wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerData, PointerEvent, PointerEventKind, PointerHandler},
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
use ui::{metrics, reservations, UiState};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_region, wl_seat, wl_surface},
    Connection, Proxy, QueueHandle,
};

type InputRects = Vec<(i32, i32, i32, i32)>;

/// Zonas reservadas por borde: arriba, derecha, abajo, izquierda.
type Zones = [i32; 4];

/// Cuatro layer surfaces auxiliares que solo exponen `exclusive_zone`: no llevan
/// buffer, no dibujan y su `input_region` está vacía. El surface principal sigue
/// siendo el único que renderiza; las reservas viven aparte para no violar la
/// regla del protocolo que anula el zone en superficies ancladas a los 4 bordes.
struct Reservations {
    edges: [LayerSurface; 4],
    zones: Zones,
}

impl Reservations {
    fn new(
        layer_shell: &LayerShell,
        compositor: &CompositorState,
        qh: &QueueHandle<DinamicUi>,
    ) -> Self {
        // Región vacía: `set_input_region(None)` significaría "toda la superficie"
        // y un auxiliar invisible se comería los clics del borde.
        let empty = compositor.wl_compositor().create_region(qh, ());
        let make = |namespace: &str, anchor: Anchor, size: (u32, u32)| {
            let surface = compositor.create_surface(qh);
            let reserved =
                layer_shell.create_layer_surface(qh, surface, Layer::Top, Some(namespace), None);
            reserved.set_anchor(anchor);
            reserved.set_keyboard_interactivity(KeyboardInteractivity::None);
            reserved.set_size(size.0, size.1);
            reserved.set_input_region(Some(&empty));
            reserved.set_exclusive_zone(0);
            reserved.commit();
            reserved
        };
        let edges = [
            make(
                "dinamic-ui-reserve-top",
                Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
                (0, 1),
            ),
            make(
                "dinamic-ui-reserve-right",
                Anchor::RIGHT | Anchor::TOP | Anchor::BOTTOM,
                (1, 0),
            ),
            make(
                "dinamic-ui-reserve-bottom",
                Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                (0, 1),
            ),
            make(
                "dinamic-ui-reserve-left",
                Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM,
                (1, 0),
            ),
        ];
        empty.destroy();
        Self {
            edges,
            zones: [0; 4],
        }
    }

    fn sync(&mut self, zones: Zones) {
        if self.zones == zones {
            return;
        }
        self.zones = zones;
        for (i, (surface, zone)) in self.edges.iter().zip(zones).enumerate() {
            // Un borde y los dos perpendiculares: el caso en el que el protocolo
            // considera significativo un zone positivo. Al soltar la zona se
            // restaura el tamaño de arranque: si no, el auxiliar conserva una
            // geometría "fantasma" aunque ya no reserve nada.
            let vertical = i == 1 || i == 3;
            let size = if zone > 0 {
                if vertical {
                    (zone as u32, 0)
                } else {
                    (0, zone as u32)
                }
            } else if vertical {
                (1, 0)
            } else {
                (0, 1)
            };
            surface.set_size(size.0, size.1);
            surface.set_exclusive_zone(zone);
            surface.commit();
        }
    }

    fn is_edge(&self, layer: &LayerSurface) -> bool {
        self.edges.iter().any(|edge| edge == layer)
    }
}

/// Contribución de cada componente a la reserva por borde. El dock no aporta:
/// es autohide por diseño (`docs/DOCK.md`), así que no reserva hueco permanente.
struct DinamicUi {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,

    layer: LayerSurface,
    /// Reservas de layout por borde (topbar y taskbar).
    reservations: Reservations,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    /// `wp_cursor_shape_manager_v1`, si el compositor lo ofrece.
    cursor_shape: Option<WpCursorShapeManagerV1>,
    shape_device: Option<WpCursorShapeDeviceV1>,
    /// Último icono pedido por egui, para no repetir `set_shape` igual.
    cursor_icon: egui::CursorIcon,
    /// Un `Enter` cambia el serial: hay que reaplicar el shape aunque el icono sea el mismo.
    cursor_dirty: bool,

    gfx: Gfx,
    ui_state: UiState,
    ipc: HyprlandIpc,
    sysinfo: sysinfo::SystemMonitor,
    last_sysinfo_revision: u64,
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
    /// Histéresis de los datos pedidos por los componentes.
    need_tracker: NeedTracker,
    /// Última consulta de la posición del cursor (la comparten bar y dock).
    last_cursor_query: Instant,
    /// Clientes de Hyprland, consultados por el núcleo (no por el dock).
    last_clients_query: Instant,
    /// ¿El layer tiene el teclado en exclusiva? (lo pide un componente, p. ej. el menú).
    keyboard_exclusive: bool,
    idle_frames: u32,
    /// Comandos del socket de control (entregados por la fuente calloop).
    pending_control: Vec<control::Command>,
    /// Manejador de la cola Wayland (para `draw` desde el timer).
    qh: QueueHandle<Self>,
    /// El timer de calloop pone `tick` y se reprograma con `tick_interval`.
    tick: bool,
    tick_interval: Duration,
    /// El teclado no genera eventos de egui: los componentes cambian el estado
    /// directamente. Esto fuerza un frame en el mismo tick de la tecla, sin
    /// esperar al timer de 250 ms ni a un evento de IPC.
    input_dirty: bool,
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
    let cursor_shape = globals
        .bind::<WpCursorShapeManagerV1, DinamicUi, GlobalData>(&qh, 1..=2, GlobalData)
        .ok();

    let surface = compositor_state.create_surface(&qh);
    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Top, Some("dinamic-ui"), None);
    // Surface fijo a TODA la pantalla: arriba van isla/barra/panel y abajo el
    // dock. El input region limita los clics a los elementos dibujados.
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    layer.set_size(0, 0);
    // -1 = extensión total: el compositor no debe mover este surface para
    // acomodar las reservas de los auxiliares. Así overview y menú siguen
    // cubriendo la pantalla y la geometría de la animación no cambia.
    layer.set_exclusive_zone(-1);
    layer.commit(); // configure inicial

    let reservations = Reservations::new(&layer_shell, &compositor_state, &qh);

    let backend = conn.backend();
    let gfx = Gfx::new(&backend, layer.wl_surface(), 1.0)?;

    let mut app = DinamicUi {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        compositor_state,
        layer,
        reservations,
        keyboard: None,
        pointer: None,
        cursor_shape,
        shape_device: None,
        cursor_icon: egui::CursorIcon::Default,
        cursor_dirty: true,
        gfx,
        ui_state: UiState::new(),
        ipc: HyprlandIpc::new(),
        sysinfo: sysinfo::SystemMonitor::new(),
        last_sysinfo_revision: 0,
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
        need_tracker: NeedTracker::new(std::time::Duration::from_secs(5)),
        last_cursor_query: Instant::now() - std::time::Duration::from_secs(2),
        last_clients_query: Instant::now() - std::time::Duration::from_secs(2),
        keyboard_exclusive: false,
        idle_frames: 0,
        pending_control: Vec::new(),
        qh: qh.clone(),
        tick: false,
        tick_interval: Duration::from_millis(16),
        input_dirty: false,
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
            // Con un overlay que tiene el teclado en exclusiva (menú, actividades,
            // overview, switcher) el frame debe ser de 16 ms: así la búsqueda no
            // depende del timer lento de reposo aunque no haya animación activa.
            app.tick_interval = if app.last_rendered
                || app.gfx.needs_repaint()
                || ui::wants_keyboard(&app.ui_state)
            {
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
    /// Recarga config y paleta (comando `reload` o petición de un componente).
    fn reload_config(&mut self) {
        if let Err(e) = config::reload() {
            eprintln!("[dinamic-ui] reload: {e}");
        }
        self.palette = Palette::load();
        // Los componentes leen la config desde `Shared`: hay que refrescarla.
        self.ui_state.shared.config = config::get();
        self.ui_state.shared.palette = self.palette;
        // La fuente también es configuración en vivo: reaplicar al recargar.
        gfx::install_fonts(&self.gfx.ctx);
        #[cfg(feature = "taskbar")]
        {
            let enabled = self.ui_state.shared.config.taskbar.enabled;
            self.ui_state.taskbar.enabled = enabled;
            self.ui_state.taskbar.visible = enabled;
        }
        #[cfg(feature = "topbar")]
        {
            self.ui_state.topbar.enabled = self.ui_state.shared.config.topbar.enabled;
        }
    }

    /// Muestra el OSD y fuerza una lectura inmediata del dato que va a pintar.
    #[cfg(feature = "osd")]
    fn show_osd(&mut self, kind: ui::osd::OsdKind) {
        self.ui_state.osd.show(kind);
        let data = match kind {
            ui::osd::OsdKind::Volume => needs::Needs::NONE.with(Need::Audio, true),
            ui::osd::OsdKind::Brightness => needs::Needs::NONE.with(Need::Display, true),
        };
        self.sysinfo.force_refresh(data);
    }

    /// Reemplaza la config en memoria y refresca el estado, sin leer el archivo
    /// (mismo camino que los sliders del previewer).
    fn set_config(&mut self, cfg: config::Config) {
        config::set(cfg);
        self.ui_state.shared.config = config::get();
        #[cfg(feature = "taskbar")]
        {
            let enabled = self.ui_state.shared.config.taskbar.enabled;
            self.ui_state.taskbar.enabled = enabled;
            self.ui_state.taskbar.visible = enabled;
        }
        #[cfg(feature = "topbar")]
        {
            self.ui_state.topbar.enabled = self.ui_state.shared.config.topbar.enabled;
        }
    }

    /// Cicla el estilo del switcher por el mismo camino que `--tune`.
    fn cycle_switcher_style(&mut self) {
        let style = self.ui_state.shared.config.switcher.style.next();
        let mut cfg = (*self.ui_state.shared.config).clone();
        cfg.switcher.style = style;
        self.set_config(cfg);
    }

    /// Ajustes internos en caliente: `--tune=<clave>=<valor>` (ver `control::TUNE_KEYS`).
    fn apply_tune(&mut self, text: &str) {
        let Some((key, value)) = text.split_once('=') else {
            eprintln!("[dinamic-ui] tune: se esperaba clave=valor ({text})");
            return;
        };
        let outcome: Result<(), String> = (|| {
            match key {
                "taskbar.side" => {
                    let side = match value {
                        "top" => config::Side::Top,
                        "right" => config::Side::Right,
                        "bottom" => config::Side::Bottom,
                        "left" => config::Side::Left,
                        other => return Err(format!("lado desconocido: {other}")),
                    };
                    let mut cfg = (*self.ui_state.shared.config).clone();
                    cfg.taskbar.side = side;
                    self.set_config(cfg);
                }
                "taskbar.size" => {
                    let size = value
                        .parse::<u32>()
                        .map_err(|_| format!("tamaño inválido: {value}"))?;
                    let mut cfg = (*self.ui_state.shared.config).clone();
                    cfg.taskbar.size = size;
                    self.set_config(cfg);
                }
                "taskbar.autohide" => {
                    let on = match value {
                        "on" | "true" | "1" => true,
                        "off" | "false" | "0" => false,
                        other => return Err(format!("booleano inválido: {other}")),
                    };
                    let mut cfg = (*self.ui_state.shared.config).clone();
                    cfg.taskbar.autohide = on;
                    self.set_config(cfg);
                }
                "topbar.height" => {
                    let height = value
                        .parse::<u32>()
                        .map_err(|_| format!("altura inválida: {value}"))?;
                    let mut cfg = (*self.ui_state.shared.config).clone();
                    cfg.topbar.height = height;
                    self.set_config(cfg);
                }
                "dock.activate" => {
                    let margin = value
                        .parse::<u32>()
                        .map_err(|_| format!("margen inválido: {value}"))?;
                    self.ui_state.shared.dock_activate_margin = margin as i32;
                }
                "dock.hold" => {
                    let margin = value
                        .parse::<u32>()
                        .map_err(|_| format!("margen inválido: {value}"))?;
                    self.ui_state.shared.dock_persist_margin = margin as i32;
                }
                "needs.hold" => {
                    let secs = value
                        .parse::<u64>()
                        .map_err(|_| format!("segundos inválidos: {value}"))?;
                    self.need_tracker.set_hold(Duration::from_secs(secs));
                }
                "switcher.style" => {
                    let style = value
                        .parse::<config::WindowSwitcherStyle>()
                        .map_err(|_| format!("estilo desconocido: {value}"))?;
                    let mut cfg = (*self.ui_state.shared.config).clone();
                    cfg.switcher.style = style;
                    self.set_config(cfg);
                }
                other => return Err(format!("clave desconocida: {other}")),
            }
            Ok(())
        })();
        if let Err(error) = outcome {
            eprintln!("[dinamic-ui] tune {key}: {error}");
        }
    }

    /// Aplica al wl_pointer el icono que pidió egui este frame. Sin esto el
    /// surface a pantalla completa no cambia (ni muestra) el cursor del sistema.
    fn apply_cursor(&mut self) {
        let icon = self.gfx.cursor_icon;
        if icon == self.cursor_icon && !self.cursor_dirty {
            return;
        }
        let Some(serial) = self
            .pointer
            .as_ref()
            .and_then(|p| p.data::<PointerData<()>>())
            .and_then(|d| d.latest_enter_serial())
        else {
            return;
        };
        if let Some(device) = &self.shape_device {
            device.set_shape(serial, cursor_shape_for(icon));
            self.cursor_icon = icon;
            self.cursor_dirty = false;
        }
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) {
        let mut dirty = std::mem::take(&mut self.input_dirty);

        // Comandos del socket de control (keybinds de Hyprland).
        let pending_control = std::mem::take(&mut self.pending_control);
        for command in pending_control {
            dirty = true;
            if ui::dispatch_command(command.name(), &mut self.ui_state) {
                continue;
            }
            match command {
                control::Command::Quit => self.exit = true,
                control::Command::Reload => self.reload_config(),
                #[cfg(feature = "osd")]
                control::Command::OsdVolume => self.show_osd(ui::osd::OsdKind::Volume),
                #[cfg(feature = "osd")]
                control::Command::OsdBrightness => self.show_osd(ui::osd::OsdKind::Brightness),
                control::Command::Tune(payload) => self.apply_tune(&payload),
                control::Command::SwitcherStyleNext => self.cycle_switcher_style(),
                other => eprintln!(
                    "[dinamic-ui] comando `{}` no disponible: componente no compilado",
                    other.name()
                ),
            }
        }

        // Eventos de Hyprland: destello breve y consulta inmediata.
        if self.ipc.poll_events() {
            self.ui_state.shared.pulse = 1.0;
            self.ipc.force_refresh();
            dirty = true;
        }
        self.ipc.poll();
        let snapshot = self.ipc.snapshot().clone();
        if snapshot != self.last_snapshot {
            if snapshot.window_address != self.last_snapshot.window_address {
                let address = snapshot.window_address.clone();
                self.ui_state.shared.touch_window(&address);
            }
            if snapshot.workspace != self.last_snapshot.workspace {
                if let Ok(id) = snapshot.workspace.parse::<u32>() {
                    self.ui_state.shared.touch_workspace(id);
                }
            }
            self.last_snapshot = snapshot.clone();
            self.ui_state.shared.snapshot = snapshot;
            dirty = true;
        }

        // Clientes: los consulta el núcleo, no el dock. Así overview y taskbar
        // tienen datos aunque el dock no esté compilado o nunca aparezca.
        if self.last_clients_query.elapsed() >= Duration::from_millis(400) {
            self.last_clients_query = Instant::now();
            let clients = self.ipc.clients();
            if clients != self.ui_state.shared.clients {
                self.ui_state.shared.clients = clients;
                dirty = true;
            }
        }

        // Datos del sistema: el hilo solo consulta mientras algún componente los muestra
        // (barra expandida, panel abierto, topbar…; ver `Component::wants_system`).
        // Los componentes declaran solo lo que muestran; el tracker mantiene
        // cada dato vivo unos segundos tras dejar de pedirlo.
        let needs = self
            .need_tracker
            .update(Instant::now(), ui::needs(&self.ui_state));
        self.sysinfo.set_needs(needs);
        if let Some((info, revision)) = self.sysinfo.poll() {
            if revision != self.last_sysinfo_revision {
                self.last_sysinfo_revision = revision;
                self.ui_state.shared.system = info;
                self.ui_state.shared.system_revision = revision;
                dirty = true;
            }
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        if self.ui_state.shared.pulse > 0.0 {
            self.ui_state.shared.pulse = (self.ui_state.shared.pulse - dt * 1.6).max(0.0);
            dirty = true;
        }

        // Cursor: lo consultan la isla (zona del notch) y el dock (borde inferior).
        let mut cursor = None;
        if self.last_cursor_query.elapsed() >= Duration::from_millis(120) {
            self.last_cursor_query = Instant::now();
            cursor = self.ipc.cursor_pos();
        }

        // Cada componente actualiza su estado, sus servicios y sus acciones diferidas.
        let update_ctx = ui::UpdateCtx {
            dt,
            screen_w: self.width as i32,
            screen_h: self.height as i32,
            cursor,
        };
        let animating = ui::update_components(&mut self.ui_state, &update_ctx);

        // Colas diferidas: órdenes a Hyprland y comandos externos fuera del render.
        for command in self.ui_state.shared.pending_commands.drain(..) {
            self.ipc.dispatch(&command);
        }
        // Drag & drop del overview: la UI solo dejó la pareja (ventana, destino).
        #[cfg(feature = "overview")]
        if let Some((address, ws)) = self.ui_state.shared.overview_move.take() {
            self.ipc
                .dispatch(&format!("movewindow address:{address} {ws}"));
            // Refrescar clientes ya en el próximo frame, no dentro de 400 ms.
            self.last_clients_query = Instant::now() - Duration::from_millis(400);
        }
        // La UI solo encola ajustes: `apply_tune` tiene el `set_config` del núcleo.
        for payload in std::mem::take(&mut self.ui_state.shared.pending_tune) {
            self.apply_tune(&payload);
        }
        for argv in self.ui_state.shared.pending_spawns.drain(..) {
            sysinfo::spawn_detached(argv);
        }
        // QuickSettings: ejecuta las acciones tipadas y hace avanzar el semáforo.
        let setting_actions = std::mem::take(&mut self.ui_state.shared.settings.queue);
        for action in setting_actions {
            if let Some(argv) = action.command() {
                sysinfo::spawn_detached(argv);
            }
            #[cfg(feature = "osd")]
            match action {
                dinamic_ui::actions::SettingAction::Volume(_) => {
                    self.show_osd(ui::osd::OsdKind::Volume)
                }
                dinamic_ui::actions::SettingAction::Brightness(_) => {
                    self.show_osd(ui::osd::OsdKind::Brightness)
                }
                _ => {}
            }
        }
        for p in self.ui_state.shared.settings.pending.values_mut() {
            if p.since_revision == 0 {
                p.since_revision = self.last_sysinfo_revision;
            }
        }
        let now = Instant::now();
        #[cfg(feature = "notifications")]
        let (dnd_revision, dnd) = (
            self.ui_state.notifications.last_revision,
            self.ui_state.notifications.dnd,
        );
        #[cfg(not(feature = "notifications"))]
        let (dnd_revision, dnd) = (0u64, false);
        self.ui_state.shared.settings.reconcile(
            now,
            self.last_sysinfo_revision,
            dnd_revision,
            &self.ui_state.shared.system,
            dnd,
        );
        if std::mem::take(&mut self.ui_state.shared.reload_requested) {
            self.reload_config();
            dirty = true;
        }
        if self.ui_state.should_exit {
            self.exit = true;
        }

        // Reservas: cambian con topbar, taskbar (lado y autohide) y reload.
        // El surface principal no se redimensiona, así que la animación sigue
        // siendo nuestra y el input region solo filtra clics.
        self.reservations.sync(reservations(&self.ui_state));

        let (t, panel_t, pill_w) = ui::draw_params(&self.ui_state);
        // El input region lo declaran los componentes activos. El surface no
        // se redimensiona: solo cambia la zona que acepta eventos.
        let rects = ui::input_rects(&self.ui_state, self.width as i32, self.height as i32);
        if self.last_input_region.as_ref() != Some(&rects) {
            let wl_region = self.compositor_state.wl_compositor().create_region(qh, ());
            for (x, y, w, h) in &rects {
                wl_region.add(*x, *y, *w, *h);
            }
            self.layer.set_input_region(Some(&wl_region));
            wl_region.destroy();
            self.last_input_region = Some(rects);
        }

        let minute = Local::now().format("%H:%M").to_string();
        if minute != self.last_minute {
            self.last_minute = minute;
            dirty = true;
        }

        // Batería: sysfs es barato, pero tampoco hace falta leerlo cada frame.
        if needs.get(Need::Battery)
            && self.last_battery.elapsed() >= std::time::Duration::from_secs(2)
        {
            self.last_battery = Instant::now();
            let battery = power::read();
            if battery != self.ui_state.shared.battery {
                self.ui_state.shared.battery = battery;
                dirty = true;
            }
        }

        // Un componente (p. ej. el command menu) puede pedir el teclado en exclusiva.
        let want_keyboard = ui::wants_keyboard(&self.ui_state);
        if want_keyboard != self.keyboard_exclusive {
            self.keyboard_exclusive = want_keyboard;
            let focus = if want_keyboard {
                KeyboardInteractivity::Exclusive
            } else {
                KeyboardInteractivity::OnDemand
            };
            self.layer.set_keyboard_interactivity(focus);
            self.layer.commit();
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
            self.apply_cursor();
        }
        self.layer.commit();
    }
}

impl CompositorHandler for DinamicUi {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        if surface != self.layer.wl_surface() {
            return;
        }
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
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        if surface != self.layer.wl_surface() {
            return;
        }
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
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        // Solo el surface principal sostiene el proceso; cerrar un auxiliar no
        // tiene que apagar el shell.
        if layer == &self.layer {
            self.exit = true;
        }
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if self.reservations.is_edge(layer) {
            // El estado del auxiliar ya se aplicó en el commit de creación y se
            // reenvía desde `sync()` cuando cambia la zona. Commitear aquí
            // provoca un bucle configure↔commit con el compositor: cada commit
            // sin buffer dispara otro configure (~4.400/s medidos).
            return;
        }
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
            if let Ok(pointer) = self.seat_state.get_pointer(qh, &seat) {
                if let Some(manager) = &self.cursor_shape {
                    self.shape_device = Some(manager.get_pointer(&pointer, qh, GlobalData));
                }
                self.pointer = Some(pointer);
            }
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
        self.input_dirty = true;
        // Los componentes consumen la tecla de arriba abajo. Esc nunca cierra el
        // proceso: solo deshace la capa superior (salir: botón del panel o `--quit`).
        ui::dispatch_key(&mut self.ui_state, event.keysym);
    }

    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        // El autorepetido del compositor llega aquí, no por `press_key`. Sin esto,
        // mantener BackSpace o una letra no seguía escribiendo en el buscador.
        self.tick = true;
        self.input_dirty = true;
        ui::dispatch_key(&mut self.ui_state, event.keysym);
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
        mods: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        // Soltar Alt/Super confirma el switcher (Tab solo cicla la selección).
        self.input_dirty = true;
        let before = ui::wants_keyboard(&self.ui_state);
        ui::switcher_modifiers(&mut self.ui_state, mods);
        if before != ui::wants_keyboard(&self.ui_state) {
            self.tick = true;
        }
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
                PointerEventKind::Enter { .. } => {
                    self.cursor_dirty = true;
                    self.pending_events.push(egui::Event::PointerMoved(pos));
                }
                PointerEventKind::Motion { .. } => {
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

/// Convierte el icono que pide egui a un shape de `wp_cursor_shape_v1`.
fn cursor_shape_for(icon: egui::CursorIcon) -> Shape {
    use egui::CursorIcon as C;
    match icon {
        C::Default => Shape::Default,
        C::ContextMenu => Shape::ContextMenu,
        C::Help => Shape::Help,
        C::PointingHand => Shape::Pointer,
        C::Progress => Shape::Progress,
        C::Wait => Shape::Wait,
        C::Cell => Shape::Cell,
        C::Crosshair => Shape::Crosshair,
        C::Text => Shape::Text,
        C::VerticalText => Shape::VerticalText,
        C::Alias => Shape::Alias,
        C::Copy => Shape::Copy,
        C::Move => Shape::Move,
        C::NoDrop => Shape::NoDrop,
        C::NotAllowed => Shape::NotAllowed,
        C::Grab => Shape::Grab,
        C::Grabbing => Shape::Grabbing,
        C::ResizeEast => Shape::EResize,
        C::ResizeNorth => Shape::NResize,
        C::ResizeNorthEast => Shape::NeResize,
        C::ResizeNorthWest => Shape::NwResize,
        C::ResizeSouth => Shape::SResize,
        C::ResizeSouthEast => Shape::SeResize,
        C::ResizeSouthWest => Shape::SwResize,
        C::ResizeWest => Shape::WResize,
        C::ResizeHorizontal => Shape::EwResize,
        C::ResizeVertical => Shape::NsResize,
        C::ResizeNeSw => Shape::NeswResize,
        C::ResizeNwSe => Shape::NwseResize,
        C::ResizeColumn => Shape::ColResize,
        C::ResizeRow => Shape::RowResize,
        C::AllScroll => Shape::AllScroll,
        C::ZoomIn => Shape::ZoomIn,
        C::ZoomOut => Shape::ZoomOut,
        _ => Shape::Default,
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
