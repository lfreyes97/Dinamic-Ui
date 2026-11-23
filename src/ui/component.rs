//! Registro único de componentes del shell (Fase 2 del plan).
//!
//! Cada componente expone tres operaciones: dibujar, declarar sus rects de
//! entrada y atender comandos. `build_components()` es el único lugar donde se
//! decide qué componentes existen; el orden del vector es el z-order.

use super::*;
#[allow(unused_imports)]
use crate::{approach, config::BarMaxState, needs::Needs, theme::Palette};
use smithay_client_toolkit::seat::keyboard::Keysym;

/// Geometría animada que comparten los componentes en un frame.
#[derive(Clone, Copy)]
pub struct DrawCtx {
    pub t: f32,
    pub panel_t: f32,
    pub pill_w: f32,
}

/// Datos de cada frame que reciben los componentes en `update`.
#[derive(Clone, Copy)]
pub struct UpdateCtx {
    pub dt: f32,
    pub screen_w: i32,
    pub screen_h: i32,
    /// Posición del cursor; solo en los frames en que el núcleo la consultó.
    pub cursor: Option<(i32, i32)>,
}

/// Zona que reclama entrada mientras el dock está visible (misma que `input_rects`).
#[cfg(feature = "dock")]
const DOCK_HIT_H: i32 = 72;
/// Franja inferior que DESPLEGA el dock cuando está oculto.
#[cfg(feature = "dock")]
const DOCK_ACTIVATE_H: i32 = 24;

/// Histéresis del autohide: entrar cuesta menos que salir. Once visible, el cursor
/// puede estar sobre cualquier parte del dock (72 px), no solo en la franja de 24.
#[cfg(feature = "dock")]
fn dock_should_show(visible: bool, forced: bool, y: i32, screen_h: i32) -> bool {
    if forced {
        return true;
    }
    if visible {
        y >= screen_h - DOCK_HIT_H
    } else {
        y >= screen_h - DOCK_ACTIVATE_H
    }
}

/// Un componente del shell: dibujo, entrada, comandos y su propia lógica de host.
pub trait Component {
    #[allow(dead_code)]
    fn name(&self) -> &'static str;

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, ctx: DrawCtx, palette: Palette);

    /// Rects en coordenadas del surface que deben recibir eventos.
    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        let _ = (state, screen_w, screen_h);
        Vec::new()
    }

    /// Atiende un comando; devuelve `true` si lo consumió.
    fn on_command(&mut self, command: &str, state: &mut UiState) -> bool {
        let _ = (command, state);
        false
    }

    /// Lógica por frame (animaciones, servicios, acciones diferidas).
    /// Devuelve `true` si hay que repintar.
    fn update(&mut self, state: &mut UiState, ctx: &UpdateCtx) -> bool {
        let _ = (state, ctx);
        false
    }

    /// Tecla pulsada; devuelve `true` si la consumió (se prueba de arriba abajo).
    fn on_key(&mut self, state: &mut UiState, key: Keysym) -> bool {
        let _ = (state, key);
        false
    }

    /// `true` si necesita el foco exclusivo del teclado mientras está activo.
    fn wants_keyboard(&self, state: &UiState) -> bool {
        let _ = state;
        false
    }

    /// `true` si ahora mismo muestra datos del sistema (wifi, volumen, brillo…): el hilo de
    /// `sysinfo` solo consulta mientras alguien los pide. Primer paso del `needs()` de la
    /// Fase 6.0 de `docs/WIDGETS_PLAN.md`.
    fn wants_system(&self, state: &UiState) -> bool {
        let _ = state;
        false
    }

    /// Datos concretos que necesita ahora. Por defecto se deriva del booleano
    /// `wants_system`, pero los componentes con vistas parciales lo afinan.
    fn needs(&self, state: &UiState) -> Needs {
        if self.wants_system(state) {
            Needs::SYSTEM
        } else {
            Needs::NONE
        }
    }
}

/// Único lugar donde se decide qué componentes existen según las features
/// compiladas. El orden del vector es el z-order (de abajo arriba).
#[allow(clippy::vec_init_then_push, unused_mut)] // con `--no-default-features` no hay ningún push
pub fn build_components() -> Vec<Box<dyn Component>> {
    let mut components: Vec<Box<dyn Component>> = Vec::new();
    #[cfg(feature = "bar")]
    components.push(Box::new(BarShell));
    #[cfg(feature = "notifications")]
    components.push(Box::new(NotificationCard));
    #[cfg(feature = "bar")]
    components.push(Box::new(Panel));
    #[cfg(feature = "wallpaper")]
    components.push(Box::new(WallpaperHost));
    #[cfg(feature = "topbar")]
    components.push(Box::new(Topbar));
    #[cfg(feature = "taskbar")]
    components.push(Box::new(Taskbar));
    #[cfg(feature = "dock")]
    components.push(Box::new(Dock));
    #[cfg(feature = "overview")]
    components.push(Box::new(Overview));
    #[cfg(feature = "activities")]
    components.push(Box::new(Activities));
    #[cfg(feature = "dashboard")]
    components.push(Box::new(dashboard::Dashboard));
    #[cfg(feature = "config-panel")]
    components.push(Box::new(config_panel::ConfigPanel));
    #[cfg(feature = "command-menu")]
    components.push(Box::new(CommandMenu));
    components
}

/// Cierra todos los overlays a pantalla completa. Quien abre uno lo llama antes: solo hay uno a
/// la vez (rejilla, actividades, dashboard, panel de configuración y command menu).
pub(super) fn close_overlays(state: &mut UiState) {
    #[cfg(feature = "overview")]
    {
        state.overview.overview = false;
    }
    #[cfg(feature = "activities")]
    {
        state.activities.open = false;
    }
    #[cfg(feature = "command-menu")]
    {
        state.menu.menu_open = false;
    }
    #[cfg(feature = "dashboard")]
    {
        state.dashboard.open = false;
    }
    #[cfg(feature = "config-panel")]
    {
        state.config_panel.open = false;
    }
    let _ = state;
}

/// Unión de los rects de entrada de todos los componentes.
pub fn input_rects(state: &UiState, screen_w: i32, screen_h: i32) -> Vec<(i32, i32, i32, i32)> {
    build_components()
        .iter()
        .flat_map(|c| c.input_rects(state, screen_w, screen_h))
        .collect()
}

/// Despacha un comando al primer componente que lo atiende.
pub fn dispatch_command(command: &str, state: &mut UiState) -> bool {
    for component in build_components().iter_mut() {
        if component.on_command(command, state) {
            return true;
        }
    }
    false
}

/// Actualiza todos los componentes; `true` si alguno pide repintar.
pub fn update_components(state: &mut UiState, ctx: &UpdateCtx) -> bool {
    let mut dirty = false;
    for component in build_components().iter_mut() {
        dirty |= component.update(state, ctx);
    }
    dirty
}

/// Reparte una tecla de arriba abajo (z-order inverso) hasta que alguien la consume.
pub fn dispatch_key(state: &mut UiState, key: Keysym) -> bool {
    for component in build_components().iter_mut().rev() {
        if component.on_key(state, key) {
            return true;
        }
    }
    false
}

/// Unión de los datos que piden todos los componentes en este frame.
pub fn needs(state: &UiState) -> Needs {
    build_components()
        .iter()
        .fold(Needs::NONE, |acc, c| acc.union(c.needs(state)))
}

/// ¿Algún componente necesita el teclado en exclusiva?
pub fn wants_keyboard(state: &UiState) -> bool {
    build_components().iter().any(|c| c.wants_keyboard(state))
}

// ── Rect helpers ────────────────────────────────────────────────────────

#[allow(dead_code)]
fn rect_of(r: egui::Rect) -> (i32, i32, i32, i32) {
    (
        r.min.x as i32,
        r.min.y as i32,
        r.width() as i32,
        r.height().max(1.0) as i32,
    )
}

#[cfg(feature = "bar")]
fn bar_rect(state: &UiState, w: i32) -> (i32, i32, i32, i32) {
    let t = crate::smootherstep(state.bar.expansion);
    let pill_w = crate::lerp(metrics().island_w as f32, metrics().bar_w as f32, t);
    if t < 0.5 {
        let p = state.bar.island_progress;
        rect_of(island_rect(w as f32, p * p))
    } else {
        (
            ((w as f32 - pill_w) / 2.0).round() as i32,
            0,
            pill_w.round() as i32,
            metrics().bar_h as i32,
        )
    }
}

#[cfg(feature = "bar")]
fn card_rect(state: &UiState, w: i32) -> (i32, i32, i32, i32) {
    let t = crate::smootherstep(state.bar.expansion);
    let panel_t = crate::smootherstep(state.bar.panel_progress);
    let card_w = panel_width(t).round();
    (
        ((w as f32 - card_w) / 2.0).round() as i32,
        (metrics().bar_h as f32 + PANEL_GAP) as i32,
        card_w as i32,
        (metrics().panel_h as f32 * panel_t).round() as i32,
    )
}

// ── Componentes ────────────────────────────────────────────────────────

#[cfg(feature = "bar")]
struct BarShell;
#[cfg(feature = "bar")]
impl Component for BarShell {
    fn name(&self) -> &'static str {
        "bar"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, ctx: DrawCtx, palette: Palette) {
        #[cfg(feature = "taskbar")]
        if super::taskbar_active(state) {
            return;
        }
        if state.bar.suppressed {
            return;
        }
        bar::draw_bar_shell(ui, state, palette, ctx.t, ctx.pill_w);
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        _screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        #[cfg(feature = "taskbar")]
        if super::taskbar_active(state) {
            return Vec::new();
        }
        if state.bar.suppressed {
            return vec![card_rect(state, screen_w)];
        }
        vec![bar_rect(state, screen_w), card_rect(state, screen_w)]
    }

    fn on_command(&mut self, command: &str, state: &mut UiState) -> bool {
        let bar = &mut state.bar;
        match command {
            "toggle" => {
                if state.shared.config.bar.max_state >= BarMaxState::Bar {
                    bar.expanded = !bar.expanded;
                }
                true
            }
            "expand" => {
                if state.shared.config.bar.max_state >= BarMaxState::Bar {
                    bar.expanded = true;
                }
                true
            }
            "collapse" => {
                bar.expanded = false;
                bar.show_panel = false;
                true
            }
            "compact" => {
                if state.shared.config.bar.max_state >= BarMaxState::Compact {
                    bar.expanded = false;
                    bar.island_compact = true;
                }
                true
            }
            "notch" => {
                bar.expanded = false;
                bar.island_compact = false;
                bar.island_active = false;
                true
            }
            _ => false,
        }
    }

    fn update(&mut self, state: &mut UiState, ctx: &UpdateCtx) -> bool {
        let mut dirty = false;
        let bar = &mut state.bar;
        // Zona del notch: franja superior centrada.
        if bar.suppressed {
            bar.island_active = false;
        } else if let Some((x, y)) = ctx.cursor {
            let near_top = std::env::var_os("DINAMIC_UI_ISLAND").is_some()
                || (y <= 18 && (x - ctx.screen_w / 2).abs() <= 100);
            if near_top != bar.island_active {
                bar.island_active = near_top;
                dirty = true;
            }
        }
        // La distribución real manda: se aplica el preset solo cuando cambia.
        if state.shared.system.layout != bar.last_layout {
            bar.last_layout = state.shared.system.layout.clone();
            if let Some(preset) = bar
                .last_layout
                .as_deref()
                .and_then(panel::layout_preset_index)
            {
                bar.layout_preset = preset;
            }
        }
        // Animación isla <-> barra.
        let before = bar.expansion;
        let target = if bar.expanded { 1.0 } else { 0.0 };
        bar.expansion = approach(bar.expansion, target, ctx.dt * 4.5);
        dirty |= (bar.expansion - before).abs() > 0.0005;

        let island_target = if bar.island_active || bar.island_compact {
            1.0
        } else {
            0.0
        };
        let before = bar.island_progress;
        bar.island_progress = approach(bar.island_progress, island_target, ctx.dt * 6.0);
        dirty |= (bar.island_progress - before).abs() > 0.0005;
        dirty
    }

    fn wants_system(&self, state: &UiState) -> bool {
        state.bar.expanded || state.bar.show_panel
    }

    fn needs(&self, state: &UiState) -> Needs {
        bar_state::bar_needs(state.bar.phase(), state.shared.config.bar.max_state)
    }

    fn on_key(&mut self, state: &mut UiState, key: Keysym) -> bool {
        // Esc nunca cierra el proceso: solo deshace la capa superior.
        if key == Keysym::Escape && state.bar.expanded {
            state.bar.expanded = false;
            return true;
        }
        false
    }
}

#[cfg(feature = "notifications")]
struct NotificationCard;
#[cfg(feature = "notifications")]
impl Component for NotificationCard {
    fn name(&self) -> &'static str {
        "notifications"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, ctx: DrawCtx, palette: Palette) {
        // Con un desplegable abierto bajo el borde superior, el aviso espera (no se pierde:
        // sigue en la lista de notificaciones del calendario).
        if state.bar.popover_open {
            return;
        }
        notifications::draw_notification(ui, state, palette, ctx.panel_t);
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        _screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        let panel_t = crate::smootherstep(state.bar.panel_progress);
        if panel_t >= 0.5 || state.bar.popover_open || state.notifications.notification.is_none() {
            return Vec::new();
        }
        let r = notification_card(screen_w as f32);
        let mut rect = rect_of(r);
        if state.notifications.notification_progress <= 0.01 {
            rect.3 = 0;
        }
        vec![rect]
    }

    fn update(&mut self, state: &mut UiState, ctx: &UpdateCtx) -> bool {
        // Notificaciones DBus (cola alimentada por un hilo propio).
        let n = &mut state.notifications;
        let mut dirty = false;
        let (notification, _count, revision) = n.service.poll(std::time::Duration::from_secs(
            config::get().notifications.ttl_secs,
        ));
        if revision != n.last_revision {
            n.last_revision = revision;
            dirty = true;
        }
        if n.dismiss_notification {
            n.dismiss_notification = false;
            n.service.dismiss();
            dirty = true;
        }
        n.notification = notification;
        n.notification_count = n.service.session_count() as usize;
        n.notification_list = n.service.list();
        n.dnd = n.service.dnd();
        if let Some(id) = n.dismiss_notification_id.take() {
            n.service.dismiss_id(id);
            dirty = true;
        }
        if n.clear_all_notifications {
            n.clear_all_notifications = false;
            n.service.clear_all();
            dirty = true;
        }
        if n.toggle_dnd {
            n.toggle_dnd = false;
            let dnd = !n.service.dnd();
            n.service.set_dnd(dnd);
            dirty = true;
        }
        // La notificación entra/sale con su propio progreso.
        let before = n.notification_progress;
        let target = if n.notification.is_some() { 1.0 } else { 0.0 };
        n.notification_progress = approach(n.notification_progress, target, ctx.dt * 2.0);
        dirty || (n.notification_progress - before).abs() > 0.0005
    }
}

#[cfg(feature = "bar")]
struct Panel;
#[cfg(feature = "bar")]
impl Component for Panel {
    fn name(&self) -> &'static str {
        "panel"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, ctx: DrawCtx, palette: Palette) {
        #[cfg(feature = "taskbar")]
        if super::taskbar_active(state) {
            return;
        }
        panel::draw_panel_card(ui, state, palette, ctx.t, ctx.panel_t);
    }

    fn on_command(&mut self, command: &str, state: &mut UiState) -> bool {
        if command == "panel" {
            if state.shared.config.bar.max_state >= BarMaxState::Panel {
                state.bar.show_panel = !state.bar.show_panel;
            }
            true
        } else {
            false
        }
    }

    fn update(&mut self, state: &mut UiState, ctx: &UpdateCtx) -> bool {
        // El panel tiene su propio progreso: así se cierra animado.
        let bar = &mut state.bar;
        let before = bar.panel_progress;
        let target = if bar.show_panel { 1.0 } else { 0.0 };
        bar.panel_progress = approach(bar.panel_progress, target, ctx.dt * 6.0);
        (bar.panel_progress - before).abs() > 0.0005
    }

    fn on_key(&mut self, state: &mut UiState, key: Keysym) -> bool {
        if key == Keysym::Escape && state.bar.show_panel {
            state.bar.show_panel = false;
            return true;
        }
        false
    }
}

/// Lógica de host de la pestaña Wallpaper: aplicar fondos y sincronizar con `awww`.
#[cfg(feature = "wallpaper")]
struct WallpaperHost;
#[cfg(feature = "wallpaper")]
impl Component for WallpaperHost {
    fn name(&self) -> &'static str {
        "wallpaper"
    }

    fn draw(&mut self, _ui: &mut egui::Ui, _state: &mut UiState, _ctx: DrawCtx, _p: Palette) {}

    fn update(&mut self, state: &mut UiState, _ctx: &UpdateCtx) -> bool {
        let mut dirty = false;
        let w = &mut state.wallpaper;
        // El demonio `awww` debe estar en marcha. El color persiste aunque se cierre la UI.
        if let Some(color) = w.wallpaper_color.take() {
            state
                .shared
                .pending_spawns
                .push(vec!["awww".into(), "clear".into(), color]);
            dirty = true;
        }
        if let Some(path) = w.wallpaper_image_request.take() {
            state.shared.pending_spawns.push(vec![
                "awww".into(),
                "img".into(),
                path.to_string_lossy().into_owned(),
            ]);
            dirty = true;
        }
        // Una miniatura terminó de generarse: repintar para mostrarla.
        let revision = w.thumbs.revision();
        if revision != w.last_thumb_revision {
            w.last_thumb_revision = revision;
            dirty = true;
        }
        // El fondo real manda sobre la selección optimista de la rejilla.
        if state.shared.system_revision != w.last_system_revision {
            w.last_system_revision = state.shared.system_revision;
            let current = &state.shared.system.wallpaper;
            w.wallpaper_selected = match current {
                Some(crate::sysinfo::Wallpaper::Color(hex)) => {
                    wallpaper::wallpaper_swatch_index(hex)
                }
                _ => None,
            };
            w.wallpaper_image = match current {
                Some(crate::sysinfo::Wallpaper::Image(path)) => {
                    Some(std::path::PathBuf::from(path))
                }
                _ => None,
            };
        }
        dirty
    }
}

#[cfg(feature = "topbar")]
struct Topbar;
#[cfg(feature = "topbar")]
impl Component for Topbar {
    fn name(&self) -> &'static str {
        "topbar"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, _ctx: DrawCtx, palette: Palette) {
        topbar::draw_topbar(ui, state, palette);
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        if !state.topbar.enabled {
            return Vec::new();
        }
        // Con un desplegable abierto, toda la pantalla recibe el clic: así se cierra al pulsar
        // fuera, como un menú. (Sin esto el clic iría a la ventana de debajo y el desplegable
        // se quedaría abierto.)
        if state.topbar.quick_open || state.topbar.calendar_open {
            return vec![(0, 0, screen_w, screen_h)];
        }
        let mut rects = vec![topbar::topbar_rect(state, screen_w)];
        // Las mismas funciones que usa el dibujo: no pueden desincronizarse.
        if state.topbar.quick_open {
            rects.push(topbar::quick_rect(state, screen_w));
        }
        if state.topbar.calendar_open {
            rects.push(topbar::calendar_rect(state, screen_w));
        }
        rects
    }

    fn on_command(&mut self, command: &str, state: &mut UiState) -> bool {
        if command == "topbar" {
            let enabled = !state.topbar.enabled;
            state.topbar.enabled = enabled;
            state.topbar.quick_open = false;
            state.topbar.calendar_open = false;
            // Si la taskbar no está compilada, ningún componente atiende el comando.
            if enabled {
                dispatch_command("taskbar-off", state);
            }
            true
        } else {
            false
        }
    }

    fn update(&mut self, state: &mut UiState, _ctx: &UpdateCtx) -> bool {
        // Mientras el topbar está activo, la barra (notch/isla) cede el borde superior.
        state.bar.suppressed = state.topbar.enabled;
        state.bar.popover_open =
            state.topbar.enabled && (state.topbar.quick_open || state.topbar.calendar_open);
        false
    }

    fn wants_system(&self, state: &UiState) -> bool {
        state.topbar.enabled
    }

    fn needs(&self, state: &UiState) -> Needs {
        if !state.topbar.enabled {
            return Needs::NONE;
        }
        use crate::needs::Need;
        Needs::NONE
            .with(Need::Time, true)
            .with(Need::Battery, true)
            .with(Need::Network, true)
            .with(Need::Audio, true)
            .with(Need::Display, true)
    }

    fn on_key(&mut self, state: &mut UiState, key: Keysym) -> bool {
        if key == Keysym::Escape && (state.topbar.quick_open || state.topbar.calendar_open) {
            state.topbar.quick_open = false;
            state.topbar.calendar_open = false;
            return true;
        }
        false
    }
}

#[cfg(feature = "taskbar")]
struct Taskbar;
#[cfg(feature = "taskbar")]
impl Component for Taskbar {
    fn name(&self) -> &'static str {
        "taskbar"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, _ctx: DrawCtx, palette: Palette) {
        taskbar::draw_taskbar(ui, state, palette);
    }

    fn needs(&self, state: &UiState) -> Needs {
        if !taskbar_on(state) {
            return Needs::NONE;
        }
        use crate::needs::Need;
        Needs::NONE
            .with(Need::Time, true)
            .with(Need::Battery, true)
            .with(Need::Network, true)
            .with(Need::Audio, true)
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        if !state.taskbar.enabled {
            return Vec::new();
        }
        // Con el desplegable de apps abierto, toda la pantalla recibe el clic: un clic fuera lo
        // cierra (igual que los desplegables del topbar).
        if taskbar::overflow_active(state) {
            return vec![(0, 0, screen_w, screen_h)];
        }
        if state.taskbar.progress > 0.5 {
            vec![taskbar::taskbar_rect(state, screen_w, screen_h)]
        } else {
            vec![taskbar::hot_rect(state, screen_w, screen_h)]
        }
    }

    fn update(&mut self, state: &mut UiState, ctx: &UpdateCtx) -> bool {
        let cfg = state.shared.config.taskbar;
        let tb = &mut state.taskbar;
        let near = ctx
            .cursor
            .is_some_and(|c| taskbar::near_edge(&cfg, c, ctx.screen_w, ctx.screen_h));
        // Con el desplegable abierto la barra no se oculta aunque el cursor se aleje.
        let visible = tb.enabled && (!cfg.autohide || near || tb.overflow_open);
        let before = tb.visible;
        tb.visible = visible;
        let before_progress = tb.progress;
        let target = if visible { 1.0 } else { 0.0 };
        tb.progress = approach(tb.progress, target, ctx.dt * 6.0);
        before != visible || (tb.progress - before_progress).abs() > 0.0005
    }

    fn on_command(&mut self, command: &str, state: &mut UiState) -> bool {
        match command {
            "taskbar" => {
                let on = !state.taskbar.enabled;
                taskbar::set_enabled(state, on);
                // Cierra la barra y el panel: al volver a apagar la taskbar no reaparecen abiertos.
                dispatch_command("collapse", state);
                true
            }
            "taskbar-off" => {
                taskbar::set_enabled(state, false);
                true
            }
            _ => false,
        }
    }

    fn on_key(&mut self, state: &mut UiState, key: Keysym) -> bool {
        if key == Keysym::Escape && state.taskbar.overflow_open {
            state.taskbar.overflow_open = false;
            return true;
        }
        false
    }
}

/// `true` mientras la taskbar está desplegada. Sin la feature, nunca lo está.
fn taskbar_on(state: &UiState) -> bool {
    #[cfg(feature = "taskbar")]
    {
        super::taskbar_active(state)
    }
    #[cfg(not(feature = "taskbar"))]
    {
        let _ = state;
        false
    }
}

#[cfg(feature = "dock")]
struct Dock;
#[cfg(feature = "dock")]
impl Component for Dock {
    fn name(&self) -> &'static str {
        "dock"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, _ctx: DrawCtx, palette: Palette) {
        #[cfg(feature = "taskbar")]
        if super::taskbar_active(state) {
            return;
        }
        dock::draw_dock(ui, state, palette);
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        // Con la taskbar desplegada el dock no se dibuja ni debe reclamar entrada.
        if state.dock.dock_progress <= 0.01 || taskbar_on(state) {
            Vec::new()
        } else {
            vec![(0, screen_h - DOCK_HIT_H, screen_w, DOCK_HIT_H)]
        }
    }

    fn update(&mut self, state: &mut UiState, ctx: &UpdateCtx) -> bool {
        let mut dirty = false;
        if taskbar_on(state) {
            if state.dock.dock_visible {
                state.dock.dock_visible = false;
                dirty = true;
            }
        } else if let Some((_, y)) = ctx.cursor {
            // El dock no depende de la feature `taskbar`. Con histéresis: aparece al
            // llegar a 24 px del borde, pero no se oculta hasta salir de los 72 px
            // que reclama (así el cursor puede estar sobre el dock sin que se vaya).
            let forced = std::env::var_os("DINAMIC_UI_DOCK").is_some();
            let visible = dock_should_show(state.dock.dock_visible, forced, y, ctx.screen_h);
            if visible != state.dock.dock_visible {
                state.dock.dock_visible = visible;
                dirty = true;
            }
        }
        let target = if state.dock.dock_visible { 1.0 } else { 0.0 };
        let before = state.dock.dock_progress;
        state.dock.dock_progress = approach(state.dock.dock_progress, target, ctx.dt * 5.0);
        dirty |= (state.dock.dock_progress - before).abs() > 0.0005;

        if let Some(action) = state.shared.dock_action.take() {
            match action {
                crate::shared::DockAction::Focus(address) => state
                    .shared
                    .pending_commands
                    .push(format!("focuswindow address:{address}")),
                crate::shared::DockAction::Launch(command) => {
                    state.shared.pending_spawns.push(vec![command])
                }
            }
            dirty = true;
        }
        dirty
    }
}

#[cfg(feature = "overview")]
struct Overview;
#[cfg(feature = "overview")]
impl Component for Overview {
    fn name(&self) -> &'static str {
        "overview"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, _ctx: DrawCtx, palette: Palette) {
        if !state.overview.visible() {
            return;
        }
        // Fundido de entrada y salida. El menú se dibuja después: hay que restaurar la opacidad.
        let fade = crate::smootherstep(state.overview.progress);
        let previous = ui.opacity();
        ui.set_opacity(previous * fade);
        overview::draw_overview(ui, state, palette);
        ui.set_opacity(previous);
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        if !state.overview.overview {
            return Vec::new();
        }
        use super::overview::OverviewKind;
        match state.overview.kind {
            OverviewKind::Workspaces => vec![(0, 0, screen_w, screen_h)],
            OverviewKind::StripH | OverviewKind::StripV => {
                vec![super::overview_strips::strip_rect(
                    state, screen_w, screen_h,
                )]
            }
        }
    }

    fn update(&mut self, state: &mut UiState, ctx: &UpdateCtx) -> bool {
        let mut dirty = false;
        // Animación de entrada/salida; al terminar de cerrar se suelta la búsqueda.
        let ov = &mut state.overview;
        let before = ov.progress;
        ov.progress = approach(
            ov.progress,
            if ov.overview { 1.0 } else { 0.0 },
            ctx.dt * 7.0,
        );
        if !ov.overview && ov.progress <= 0.0 && !ov.query.is_empty() {
            ov.query.clear();
        }
        dirty |= (ov.progress - before).abs() > 0.0005;
        if let Some(address) = state.shared.overview_focus.take() {
            state
                .shared
                .pending_commands
                .push(format!("focuswindow address:{address}"));
            state.overview.overview = false;
            dirty = true;
        }
        if let Some(id) = state.shared.overview_workspace.take() {
            state
                .shared
                .pending_commands
                .push(format!("workspace {id}"));
            state.overview.overview = false;
            dirty = true;
        }
        dirty
    }

    fn on_key(&mut self, state: &mut UiState, key: Keysym) -> bool {
        overview::handle_key(state, key)
    }

    /// Las vistas modales (rejilla y actividades) toman el teclado; las franjas no, para no
    /// quitárselo a la app que hay debajo.
    fn wants_keyboard(&self, state: &UiState) -> bool {
        state.overview.overview && state.overview.kind.is_modal()
    }

    fn on_command(&mut self, command: &str, state: &mut UiState) -> bool {
        use super::overview::OverviewKind;
        let kind = match command {
            "overview" => OverviewKind::Workspaces,
            "overview-strip-h" => OverviewKind::StripH,
            "overview-strip-v" => OverviewKind::StripV,
            _ => return false,
        };
        // El mismo atajo abre y cierra; con otra vista abierta, cambia a esta.
        if state.overview.overview && state.overview.kind == kind {
            state.overview.overview = false;
        } else {
            overview::open(state, kind);
            // Solo un overlay a pantalla completa a la vez.
            #[cfg(feature = "activities")]
            {
                state.activities.open = false;
            }
            #[cfg(feature = "dashboard")]
            {
                state.dashboard.open = false;
            }
            #[cfg(feature = "config-panel")]
            {
                state.config_panel.open = false;
            }
        }
        true
    }
}

/// Actividades: lanzador a pantalla completa (feature `activities`, independiente del overview).
#[cfg(feature = "activities")]
struct Activities;
#[cfg(feature = "activities")]
impl Component for Activities {
    fn name(&self) -> &'static str {
        "activities"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, _ctx: DrawCtx, palette: Palette) {
        if !state.activities.visible() {
            return;
        }
        // Fundido de entrada y salida; se restaura la opacidad para los componentes siguientes.
        let fade = crate::smootherstep(state.activities.progress);
        let previous = ui.opacity();
        ui.set_opacity(previous * fade);
        activities::draw(ui, state, palette);
        ui.set_opacity(previous);
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        if state.activities.open {
            vec![(0, 0, screen_w, screen_h)]
        } else {
            Vec::new()
        }
    }

    fn update(&mut self, state: &mut UiState, ctx: &UpdateCtx) -> bool {
        let a = &mut state.activities;
        let before = a.progress;
        a.progress = approach(a.progress, if a.open { 1.0 } else { 0.0 }, ctx.dt * 7.0);
        // Al terminar de cerrar se sueltan las apps leídas y la búsqueda.
        if !a.open && a.progress <= 0.0 && !a.apps.is_empty() {
            a.apps = Vec::new();
            a.query.clear();
        }
        (a.progress - before).abs() > 0.0005
    }

    fn on_key(&mut self, state: &mut UiState, key: Keysym) -> bool {
        activities::handle_key(state, key)
    }

    fn wants_keyboard(&self, state: &UiState) -> bool {
        state.activities.open
    }

    fn on_command(&mut self, command: &str, state: &mut UiState) -> bool {
        if command != "activities" {
            return false;
        }
        if state.activities.open {
            state.activities.open = false;
        } else {
            activities::open(state);
            #[cfg(feature = "overview")]
            {
                state.overview.overview = false;
            }
            #[cfg(feature = "dashboard")]
            {
                state.dashboard.open = false;
            }
            #[cfg(feature = "config-panel")]
            {
                state.config_panel.open = false;
            }
        }
        true
    }
}

#[cfg(feature = "command-menu")]
struct CommandMenu;
#[cfg(feature = "command-menu")]
impl Component for CommandMenu {
    fn name(&self) -> &'static str {
        "command-menu"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, _ctx: DrawCtx, palette: Palette) {
        if state.menu.menu_open {
            menu::draw_command_menu(ui, state, palette);
        }
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        if state.menu.menu_open {
            vec![(0, 0, screen_w, screen_h)]
        } else {
            Vec::new()
        }
    }

    fn on_command(&mut self, command: &str, state: &mut UiState) -> bool {
        if command == "menu" {
            let menu = &mut state.menu;
            menu.menu_open = !menu.menu_open;
            if menu.menu_open {
                #[cfg(feature = "overview")]
                {
                    state.overview.overview = false;
                }
                #[cfg(feature = "activities")]
                {
                    state.activities.open = false;
                }
                #[cfg(feature = "dashboard")]
                {
                    state.dashboard.open = false;
                }
                #[cfg(feature = "config-panel")]
                {
                    state.config_panel.open = false;
                }
                state.bar.show_panel = false;
                menu.menu_query.clear();
                menu.menu_selected = 0;
                menu::refresh_entries(state);
            }
            true
        } else {
            false
        }
    }

    fn update(&mut self, state: &mut UiState, _ctx: &UpdateCtx) -> bool {
        use crate::launcher::EntryAction;
        let Some(action) = state.shared.menu_action.take() else {
            return false;
        };
        match action {
            EntryAction::Launch(command) => {
                let argv = crate::desktop::split_exec(&command);
                state.shared.pending_spawns.push(argv);
            }
            EntryAction::OpenFile(path) => state
                .shared
                .pending_spawns
                .push(vec!["xdg-open".into(), path.to_string_lossy().into_owned()]),
            EntryAction::Workspace(id) => state
                .shared
                .pending_commands
                .push(format!("workspace {id}")),
            EntryAction::Command(command) => match command.as_str() {
                "reload" => state.shared.reload_requested = true,
                "quit" => state.should_exit = true,
                // El resto (panel, overview, compact…) los atiende el registro; si el
                // componente no está compilado, el comando se ignora.
                other => {
                    dispatch_command(other, state);
                }
            },
        }
        true
    }

    fn on_key(&mut self, state: &mut UiState, key: Keysym) -> bool {
        if !state.menu.menu_open {
            return false;
        }
        menu::handle_key(state, key);
        true
    }

    fn wants_keyboard(&self, state: &UiState) -> bool {
        state.menu.menu_open
    }
}

#[cfg(all(test, feature = "dock"))]
mod dock_tests {
    use super::*;

    const H: i32 = 768;

    #[test]
    fn oculto_aparece_solo_en_la_franja_de_24px() {
        assert!(dock_should_show(false, false, H - 23, H));
        assert!(dock_should_show(false, false, H - 24, H));
        assert!(!dock_should_show(false, false, H - 25, H));
    }

    #[test]
    fn visible_se_mantiene_sobre_todo_el_dock_de_72px() {
        // El cursor puede estar en la parte alta del dock (por encima de la franja).
        assert!(dock_should_show(true, false, H - 70, H));
        assert!(dock_should_show(true, false, H - 24, H));
        assert!(!dock_should_show(true, false, H - 73, H));
    }

    #[test]
    fn env_var_fuerza_visible() {
        assert!(dock_should_show(false, true, 0, H));
        assert!(dock_should_show(true, true, 0, H));
    }
}
