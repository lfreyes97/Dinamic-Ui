//! Previsualizador interactivo de Dinamic UI (feature `previewer`): una ventana
//! normal con los componentes compilados, datos de ejemplo y registro de acciones.
//! Ver `docs/PREVIEWER.md`. No liga el socket de control, no reclama el nombre
//! D-Bus y no ejecuta nada: solo registra lo que haría la instancia real.
//!
//! `just previewer` (perfil rápido). El sidebar ofrece solo lo compilado, y los
//! botones usan `dispatch_command`, el mismo camino que el CLI y los keybinds.
//!
//! Fase 2: vigila `config.toml` (mtime) y recarga en vivo; los controles de
//! apariencia/tamaños escriben con `config::set` (memoria, sin tocar el archivo).

use std::time::{Duration, SystemTime};

use dinamic_ui::config::{Config, Side};
#[cfg(feature = "notifications")]
use dinamic_ui::ui::sample::sample_notifications;
use dinamic_ui::ui::sample::{fake_desktop, sample_state, sample_system};
use dinamic_ui::{config, gfx, theme::Palette, ui};
use egui_phosphor::regular as ic;
use smithay_client_toolkit::seat::keyboard::Keysym;

/// Posiciones candidatas para los controles de modo (sandbox de diseño).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlPlacement {
    Derecha,
    Izquierda,
    Reloj,
    Notch,
    Hueco,
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 720.0])
            .with_title("Dinamic UI — Previewer"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "dinamic-ui-previewer",
        options,
        Box::new(|cc| {
            let ctx = cc.egui_ctx.clone();
            gfx::install_fonts(&ctx);
            Ok(Box::new(Previewer::new(ctx)))
        }),
    )
}

struct Previewer {
    /// Contexto de egui: reaplica las fuentes cuando cambia la config en vivo.
    ctx: egui::Context,
    state: ui::UiState,
    /// Acciones que la instancia real ejecutaría (dispatches, spawns, reload, quit).
    log: Vec<String>,
    /// mtime del config.toml en el último chequeo (Fase 2a).
    config_mtime: Option<SystemTime>,
    /// Sandbox de diseño: posición de los controles de modo (no ejecuta nada).
    design_controls: bool,
    control_placement: ControlPlacement,
    /// Sidebar colapsable (F1 o botón flotante) para no estorbar al diseñar.
    sidebar_visible: bool,
}

impl Previewer {
    fn new(ctx: egui::Context) -> Self {
        // La config real se lee (solo lectura): temas y tamaños son los del usuario.
        config::init();
        let mut p = Self {
            ctx,
            state: sample_state(1.0),
            log: Vec::new(),
            config_mtime: Self::config_mtime(),
            design_controls: std::env::var_os("DINAMIC_UI_DESIGN_CONTROLS").is_some(),
            sidebar_visible: std::env::var_os("DINAMIC_UI_HIDE_SIDEBAR").is_none(),
            control_placement: match std::env::var("DINAMIC_UI_DESIGN_PLACEMENT").as_deref() {
                Ok("Izquierda") => ControlPlacement::Izquierda,
                Ok("Reloj") => ControlPlacement::Reloj,
                Ok("Notch") => ControlPlacement::Notch,
                Ok("Hueco") => ControlPlacement::Hueco,
                _ => ControlPlacement::Derecha,
            },
        };
        p.init_state();
        p
    }

    fn config_mtime() -> Option<SystemTime> {
        config::path()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok())
    }

    /// Copiar la config global (archivo o sliders) al estado que dibuja la UI.
    fn sync_config(&mut self) {
        let cfg = config::get();
        self.state.shared.config = cfg.clone();
        self.state.shared.palette = Palette::load();
        #[cfg(feature = "taskbar")]
        {
            self.state.taskbar.enabled = cfg.taskbar.enabled;
            self.state.taskbar.visible = cfg.taskbar.enabled;
        }
        #[cfg(feature = "topbar")]
        {
            self.state.topbar.enabled = cfg.topbar.enabled;
        }
    }

    fn init_state(&mut self) {
        sample_system(&mut self.state);
        self.sync_config();
    }

    /// Fase 2a: volver a leer el archivo. No escribe nada.
    fn reload_from_disk(&mut self) {
        match config::reload() {
            Ok(()) => {
                self.sync_config();
                gfx::install_fonts(&self.ctx);
                self.log.push("config.toml recargado del disco".into());
            }
            Err(e) => self.log.push(format!("config: {e}")),
        }
        self.config_mtime = Self::config_mtime();
    }

    /// Fase 2b: aplicar la config de los sliders en memoria, sin tocar el archivo.
    fn apply_config(&mut self, cfg: Config) {
        config::set(cfg);
        self.sync_config();
        gfx::install_fonts(&self.ctx);
    }

    /// Llamado cada frame: si el archivo cambió, recarga. Poll de mtime, sin deps nuevas.
    fn watch_config(&mut self) {
        let now = Self::config_mtime();
        if now != self.config_mtime {
            self.config_mtime = now;
            self.reload_from_disk();
        }
    }

    fn reset(&mut self) {
        self.state = sample_state(1.0);
        self.init_state();
        self.config_mtime = Self::config_mtime();
        self.log.push("— estado reiniciado —".into());
    }

    /// Botón que despacha un comando por el mismo camino que el CLI.
    /// Sin features de componentes (matriz `previewer` sola) no tiene llamadas.
    #[allow(dead_code)]
    fn command(&mut self, ui: &mut egui::Ui, command: &str, label: &str) {
        if ui.button(label).clicked() {
            ui::dispatch_command(command, &mut self.state);
            self.log.push(format!("comando: {command}"));
        }
    }

    /// Fase 2b: temas y tamaños con efecto inmediato en el centro.
    fn config_ui(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.strong("Apariencia y tamaños (en vivo)");
        let mut cfg = (*self.state.shared.config).clone();
        let mut changed = false;

        let current = cfg
            .theme
            .scheme
            .clone()
            .or_else(|| cfg.theme.name.clone())
            .unwrap_or_else(|| "black-metal".into());
        let mut picked = current.clone();
        egui::ComboBox::from_label("Esquema")
            .selected_text(&picked)
            .show_ui(ui, |ui| {
                for s in ["black-metal", "bathory", "venom", "marduk", "mayhem"] {
                    if ui.selectable_value(&mut picked, s.to_string(), s).changed() {
                        changed = true;
                    }
                }
            });
        if picked != current {
            cfg.theme.scheme = Some(picked);
            changed = true;
        }

        let mut accent = cfg.theme.accent.clone().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Acento");
            if ui.text_edit_singleline(&mut accent).changed() {
                changed = true;
            }
        });
        cfg.theme.accent = if accent.trim().is_empty() {
            None
        } else {
            Some(accent.trim().to_string())
        };

        ui.separator();
        ui.strong("Fuente (en vivo)");
        let mut regular = cfg.font.regular.clone().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Regular");
            if ui.text_edit_singleline(&mut regular).changed() {
                changed = true;
            }
        });
        cfg.font.regular = if regular.trim().is_empty() {
            None
        } else {
            Some(regular.trim().to_string())
        };
        let mut semibold = cfg.font.semibold.clone().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Semibold");
            if ui.text_edit_singleline(&mut semibold).changed() {
                changed = true;
            }
        });
        cfg.font.semibold = if semibold.trim().is_empty() {
            None
        } else {
            Some(semibold.trim().to_string())
        };
        ui.label("Vacío = autodetectar Inter en XDG_DATA_HOME");

        if ui
            .add(egui::Slider::new(&mut cfg.sizes.bar_h, 24..=48).text("bar_h"))
            .changed()
        {
            changed = true;
        }
        if ui
            .add(egui::Slider::new(&mut cfg.sizes.panel_h, 80..=480).text("panel_h"))
            .changed()
        {
            changed = true;
        }
        if ui
            .add(egui::Slider::new(&mut cfg.sizes.island_w, 120..=520).text("island_w"))
            .changed()
        {
            changed = true;
        }
        if ui
            .add(egui::Slider::new(&mut cfg.sizes.bar_w, 320..=1200).text("bar_w"))
            .changed()
        {
            changed = true;
        }
        if ui
            .add(egui::Slider::new(&mut cfg.workspaces.count, 4..=20).text("workspaces"))
            .changed()
        {
            changed = true;
        }

        #[cfg(feature = "taskbar")]
        {
            ui.separator();
            ui.strong("Taskbar");
            if ui.checkbox(&mut cfg.taskbar.enabled, "Activada").changed() {
                changed = true;
            }
            if side_combo(ui, "Lado", &mut cfg.taskbar.side) {
                changed = true;
            }
            if ui.checkbox(&mut cfg.taskbar.autohide, "Autohide").changed() {
                changed = true;
            }
            if ui
                .add(egui::Slider::new(&mut cfg.taskbar.size, 40..=72).text("size"))
                .changed()
            {
                changed = true;
            }
        }

        #[cfg(feature = "topbar")]
        {
            ui.separator();
            ui.strong("Topbar");
            if ui.checkbox(&mut cfg.topbar.enabled, "Activada").changed() {
                changed = true;
            }
            if ui
                .add(egui::Slider::new(&mut cfg.topbar.height, 24..=48).text("height"))
                .changed()
            {
                changed = true;
            }
        }

        #[cfg(feature = "overview")]
        {
            ui.separator();
            ui.strong("Overviews");
            if side_combo(ui, "Franja H", &mut cfg.overview.strip_h_side) {
                changed = true;
            }
            if side_combo(ui, "Franja V", &mut cfg.overview.strip_v_side) {
                changed = true;
            }
        }

        if changed {
            self.apply_config(cfg);
        }
    }

    /// Sandbox temporal: elegir dónde aparecerían los controles de modo.
    fn design_ui(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.strong("Modo diseño · controles de modo");
        ui.checkbox(&mut self.design_controls, "Mostrar mock en la barra/topbar");
        if self.design_controls {
            ui.radio_value(
                &mut self.control_placement,
                ControlPlacement::Derecha,
                "Derecha (reveal al hover)",
            );
            ui.radio_value(
                &mut self.control_placement,
                ControlPlacement::Izquierda,
                "Izquierda (junto a workspaces)",
            );
            ui.radio_value(
                &mut self.control_placement,
                ControlPlacement::Reloj,
                "Junto al reloj",
            );
            ui.radio_value(
                &mut self.control_placement,
                ControlPlacement::Notch,
                "Dentro del notch activo",
            );
            ui.radio_value(
                &mut self.control_placement,
                ControlPlacement::Hueco,
                "Hueco reloj ↔ sistema",
            );
            ui.small("Solo visual: no envía comandos ni cambia el shell.");
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        if ui.button("Ocultar sidebar (F1)").clicked() {
            self.sidebar_visible = false;
        }
        ui.heading("Dinamic UI");
        ui.label("Previsualizador · datos de ejemplo");
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.strong("Vistas");
            #[cfg(feature = "bar")]
            {
                self.command(ui, "toggle", "Isla ↔ barra");
                self.command(ui, "expand", "Expandir barra");
                self.command(ui, "collapse", "Plegar a isla");
                self.command(ui, "compact", "Isla compacta");
                self.command(ui, "notch", "Notch");
                self.command(ui, "panel", "Panel de control");
            }
            #[cfg(feature = "topbar")]
            self.command(ui, "topbar", "Topbar GNOME-like");
            #[cfg(feature = "taskbar")]
            self.command(ui, "taskbar", "Taskbar");
            #[cfg(feature = "overview")]
            {
                self.command(ui, "overview", "Overview de workspaces");
                self.command(ui, "overview-strip-h", "Franja horizontal");
                self.command(ui, "overview-strip-v", "Franja vertical");
            }
            #[cfg(feature = "activities")]
            self.command(ui, "activities", "Actividades (lanzador)");
            #[cfg(feature = "dashboard")]
            self.command(ui, "dashboard", "Dashboard");
            #[cfg(feature = "config-panel")]
            self.command(ui, "config-panel", "Panel de configuración");
            #[cfg(feature = "command-menu")]
            self.command(ui, "menu", "Command menu");
            #[cfg(feature = "notifications")]
            if ui.button("Notificación de prueba").clicked() {
                if let Some(n) = sample_notifications(1).pop() {
                    self.state.notifications.service.push_preview(n);
                    self.log.push("notificación de ejemplo inyectada".into());
                }
            }
            ui.separator();
            if ui.button("Reiniciar estado").clicked() {
                self.reset();
            }

            self.config_ui(ui);
            self.design_ui(ui);

            ui.separator();
            ui.strong("Registro (nada se ejecuta)");
            let skip = self.log.len().saturating_sub(30);
            for line in &self.log[skip..] {
                ui.monospace(line);
            }
        });
    }

    /// La pantalla simulada: escritorio falso + el `draw` real de los componentes.
    fn screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, dt: f32) {
        let screen = ui.max_rect();
        if !self.sidebar_visible {
            let btn = egui::Rect::from_min_size(
                screen.min + egui::vec2(6.0, 6.0),
                egui::vec2(26.0, 20.0),
            );
            if ui
                .put(btn, egui::Button::new("☰").corner_radius(6.0))
                .on_hover_text("Mostrar sidebar (F1)")
                .clicked()
            {
                self.sidebar_visible = true;
            }
        }
        let (w, h) = (
            screen.width().round() as i32,
            screen.height().round() as i32,
        );
        // El cursor de egui, en coordenadas de la pantalla simulada: alimenta el
        // hover del notch y el autohide de dock y taskbar (como el IPC cursorpos).
        let cursor = ctx
            .input(|i| i.pointer.hover_pos())
            .filter(|p| screen.contains(*p))
            .map(|p| ((p.x - screen.min.x) as i32, (p.y - screen.min.y) as i32));
        let update_ctx = ui::UpdateCtx {
            dt,
            screen_w: w,
            screen_h: h,
            cursor,
        };
        let animating = ui::update_components(&mut self.state, &update_ctx);
        self.drain_actions();

        let (t, panel_t, pill_w) = ui::draw_params(&self.state);
        let mut full = ui.new_child(egui::UiBuilder::new().max_rect(screen));
        fake_desktop(&full, true);
        ui::draw(
            &mut full,
            &mut self.state,
            Palette::load(),
            t,
            panel_t,
            pill_w,
        );
        self.draw_design_controls(&full, screen);

        if animating {
            ctx.request_repaint();
        }
    }

    /// Dibuja los candidatos a controles de modo sobre la superficie real del
    /// shell (barra o topbar), con los tokens del tema actual. Es solo un mock.
    fn draw_design_controls(&self, ui: &egui::Ui, screen: egui::Rect) {
        if !self.design_controls {
            return;
        }
        let p = Palette::load();
        let (t, _, pill_w) = ui::draw_params(&self.state);
        let m = ui::metrics();
        let bar = if t < 0.5 {
            let ip = self.state.bar.island_progress * self.state.bar.island_progress;
            ui::island_rect(screen.width(), ip)
        } else {
            egui::Rect::from_min_size(
                egui::pos2((screen.width() - pill_w) / 2.0, 0.0),
                egui::vec2(pill_w, m.bar_h as f32),
            )
        };
        let topbar_on = {
            #[cfg(feature = "topbar")]
            {
                self.state.topbar.enabled
            }
            #[cfg(not(feature = "topbar"))]
            {
                false
            }
        };
        let surface = if topbar_on {
            egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(
                    screen.width(),
                    self.state.shared.config.topbar.height as f32,
                ),
            )
        } else {
            bar
        };

        const SIZE: f32 = 16.0;
        let cy = surface.center().y - SIZE / 2.0;
        let chip = |rect: egui::Rect, label: &str| draw_chip(ui, rect, label, p);
        match self.control_placement {
            ControlPlacement::Derecha => {
                let x = surface.max.x - 4.0 - SIZE * 2.0 - 2.0;
                chip(
                    egui::Rect::from_min_size(egui::pos2(x, cy), egui::vec2(SIZE, SIZE)),
                    ic::CARET_UP,
                );
                chip(
                    egui::Rect::from_min_size(
                        egui::pos2(x + SIZE + 2.0, cy),
                        egui::vec2(SIZE, SIZE),
                    ),
                    ic::CARET_DOWN,
                );
            }
            ControlPlacement::Izquierda => {
                let x = surface.min.x + 4.0;
                chip(
                    egui::Rect::from_min_size(egui::pos2(x, cy), egui::vec2(SIZE, SIZE)),
                    ic::CARET_UP,
                );
                chip(
                    egui::Rect::from_min_size(
                        egui::pos2(x + SIZE + 2.0, cy),
                        egui::vec2(SIZE, SIZE),
                    ),
                    ic::CARET_DOWN,
                );
            }
            ControlPlacement::Reloj => {
                let cx = screen.width() / 2.0;
                chip(
                    egui::Rect::from_min_size(egui::pos2(cx - 52.0, cy), egui::vec2(SIZE, SIZE)),
                    ic::CARET_UP,
                );
                chip(
                    egui::Rect::from_min_size(egui::pos2(cx + 36.0, cy), egui::vec2(SIZE, SIZE)),
                    ic::CARET_DOWN,
                );
            }
            ControlPlacement::Notch => {
                if t < 0.5 && self.state.bar.island_progress > 0.1 {
                    let c = bar.center();
                    chip(
                        egui::Rect::from_min_size(
                            egui::pos2(c.x + 34.0, c.y - SIZE / 2.0),
                            egui::vec2(SIZE, SIZE),
                        ),
                        ic::CARET_UP,
                    );
                }
            }
            ControlPlacement::Hueco => {
                let cx = screen.width() / 2.0;
                let x = cx + 80.0;
                chip(
                    egui::Rect::from_min_size(egui::pos2(x, cy), egui::vec2(SIZE, SIZE)),
                    ic::CARET_UP,
                );
                chip(
                    egui::Rect::from_min_size(
                        egui::pos2(x + SIZE + 2.0, cy),
                        egui::vec2(SIZE, SIZE),
                    ),
                    ic::CARET_DOWN,
                );
            }
        }
    }

    /// Las acciones diferidas se registran, no se ejecutan: ni Hyprland, ni apps,
    /// ni `awww`. `quit` no termina; `reload` sí vuelve a leer el archivo (Fase 2a).
    fn drain_actions(&mut self) {
        for command in std::mem::take(&mut self.state.shared.pending_commands) {
            self.log.push(format!("hyprctl dispatch {command}"));
        }
        for argv in std::mem::take(&mut self.state.shared.pending_spawns) {
            self.log.push(format!("spawn: {}", argv.join(" ")));
        }
        for action in std::mem::take(&mut self.state.shared.settings.queue) {
            self.log.push(format!("setting: {action:?}"));
        }
        // Sin sistema real que confirme, el previewer cierra el semáforo al instante.
        self.state.shared.settings.clear_pending();
        let reload = std::mem::take(&mut self.state.shared.reload_requested);
        if self.state.should_exit {
            self.state.should_exit = false;
            self.log
                .push("quit pedido (ignorado: el previewer no termina)".into());
        }
        if reload {
            self.log.push("reload pedido por un componente".into());
            self.reload_from_disk();
        }
    }

    /// Teclado básico: las teclas de los componentes (Esc, flechas, Enter, Tab,
    /// Backspace) y texto para la búsqueda. El mapa completo llega en la Fase 3.
    fn handle_keys(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::Key {
                    key: egui::Key::F1,
                    pressed: true,
                    ..
                } => self.sidebar_visible = !self.sidebar_visible,
                egui::Event::Key {
                    key, pressed: true, ..
                } => {
                    if let Some(keysym) = map_key(key) {
                        ui::dispatch_key(&mut self.state, keysym);
                    }
                }
                egui::Event::Text(text) => {
                    for ch in text.chars() {
                        ui::dispatch_key(&mut self.state, Keysym::from_char(ch));
                    }
                }
                _ => {}
            }
        }
    }
}

fn draw_chip(ui: &egui::Ui, rect: egui::Rect, icon: &str, p: Palette) {
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(7),
        Palette::sheen(10),
        egui::Stroke::new(1.0, p.border),
        egui::StrokeKind::Middle,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::new(12.0, egui::FontFamily::Name("phosphor-bold".into())),
        p.text,
    );
}

/// Selector de `Side`. Solo lo llaman los bloques con feature `taskbar` / `overview`;
/// con `previewer` sola no tiene llamadas.
#[allow(dead_code)]
fn side_combo(ui: &mut egui::Ui, label: &str, value: &mut Side) -> bool {
    let mut picked = *value;
    let mut changed = false;
    egui::ComboBox::from_label(label)
        .selected_text(format!("{picked:?}"))
        .show_ui(ui, |ui| {
            for s in [Side::Bottom, Side::Top, Side::Left, Side::Right] {
                if ui
                    .selectable_value(&mut picked, s, format!("{s:?}"))
                    .changed()
                {
                    changed = true;
                }
            }
        });
    if changed {
        *value = picked;
    }
    changed
}

fn map_key(key: egui::Key) -> Option<Keysym> {
    Some(match key {
        egui::Key::Escape => Keysym::Escape,
        egui::Key::ArrowLeft => Keysym::Left,
        egui::Key::ArrowRight => Keysym::Right,
        egui::Key::ArrowUp => Keysym::Up,
        egui::Key::ArrowDown => Keysym::Down,
        egui::Key::Enter => Keysym::Return,
        egui::Key::Tab => Keysym::Tab,
        egui::Key::Backspace => Keysym::BackSpace,
        egui::Key::Home => Keysym::Home,
        egui::Key::End => Keysym::End,
        egui::Key::Delete => Keysym::Delete,
        _ => return None,
    })
}

impl eframe::App for Previewer {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.watch_config();
        self.handle_keys(&ctx);

        // ui::draw deja visuales transparentes en el ctx (el shell es un layer
        // sobre el escritorio). Aquí somos una ventana normal: el root vuelve a
        // los visuales opacos por defecto y el sidebar hereda fondo. El centro
        // queda así en las mismas condiciones que las vistas previas PNG.
        *ui.visuals_mut() = egui::Visuals::dark();

        if self.sidebar_visible {
            egui::Panel::left("previewer-nav")
                .resizable(false)
                .default_size(190.0)
                .show(ui, |ui| self.sidebar(ui));
        }

        egui::CentralPanel::default().show(ui, |ui| self.screen(ui, &ctx, dt));

        // El reloj avanza solo: refresca aunque no haya entrada ni animación.
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}
