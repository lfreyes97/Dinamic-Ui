//! Registro único de componentes del shell (Fase 2 del plan).
//!
//! Cada componente expone tres operaciones: dibujar, declarar sus rects de
//! entrada y atender comandos. `build_components()` es el único lugar donde se
//! decide qué componentes existen; el orden del vector es el z-order.

use super::*;
use crate::{config::BarMaxState, theme::Palette};

/// Geometría animada que comparten los componentes en un frame.
#[derive(Clone, Copy)]
pub struct DrawCtx {
    pub t: f32,
    pub panel_t: f32,
    pub pill_w: f32,
}

/// Un componente del shell: dibujo, entrada y comandos propios.
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
}

/// Orden de dibujo: barra, notificación, panel, dock, overview y menú.
pub fn build_components() -> Vec<Box<dyn Component>> {
    vec![
        Box::new(BarShell),
        Box::new(NotificationCard),
        Box::new(Panel),
        Box::new(Dock),
        Box::new(Overview),
        Box::new(CommandMenu),
    ]
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

// ── Rect helpers ────────────────────────────────────────────────────────

fn rect_of(r: egui::Rect) -> (i32, i32, i32, i32) {
    (
        r.min.x as i32,
        r.min.y as i32,
        r.width() as i32,
        r.height().max(1.0) as i32,
    )
}

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

struct BarShell;
impl Component for BarShell {
    fn name(&self) -> &'static str {
        "bar"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, ctx: DrawCtx, palette: Palette) {
        bar::draw_bar_shell(ui, state, palette, ctx.t, ctx.pill_w);
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        _screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
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
}

struct NotificationCard;
impl Component for NotificationCard {
    fn name(&self) -> &'static str {
        "notifications"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, ctx: DrawCtx, palette: Palette) {
        notifications::draw_notification(ui, state, palette, ctx.panel_t);
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        _screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        let panel_t = crate::smootherstep(state.bar.panel_progress);
        if panel_t >= 0.5 || state.notifications.notification.is_none() {
            return Vec::new();
        }
        let r = notification_card(screen_w as f32);
        let mut rect = rect_of(r);
        if state.notifications.notification_progress <= 0.01 {
            rect.3 = 0;
        }
        vec![rect]
    }
}

struct Panel;
impl Component for Panel {
    fn name(&self) -> &'static str {
        "panel"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, ctx: DrawCtx, palette: Palette) {
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
}

struct Dock;
impl Component for Dock {
    fn name(&self) -> &'static str {
        "dock"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, _ctx: DrawCtx, palette: Palette) {
        dock::draw_dock(ui, state, palette);
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        if state.dock.dock_progress <= 0.01 {
            Vec::new()
        } else {
            vec![(0, screen_h - 72, screen_w, 72)]
        }
    }
}

struct Overview;
impl Component for Overview {
    fn name(&self) -> &'static str {
        "overview"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, _ctx: DrawCtx, palette: Palette) {
        if state.overview.overview {
            overview::draw_overview(ui, state, palette);
        }
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        if state.overview.overview {
            vec![(0, 0, screen_w, screen_h)]
        } else {
            Vec::new()
        }
    }

    fn on_command(&mut self, command: &str, state: &mut UiState) -> bool {
        if command == "overview" {
            state.overview.overview = !state.overview.overview;
            if state.overview.overview {
                state.bar.show_panel = false;
            }
            true
        } else {
            false
        }
    }
}

struct CommandMenu;
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
                state.overview.overview = false;
                state.bar.show_panel = false;
                menu.menu_query.clear();
                menu.menu_selected = 0;
            }
            true
        } else {
            false
        }
    }
}
