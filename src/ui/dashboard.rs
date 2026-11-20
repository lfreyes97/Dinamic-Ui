//! Dashboard: rejilla bento de widgets a pantalla completa (feature `dashboard`, comando
//! `--dashboard`). Ver `docs/WIDGETS_PLAN.md` (Fase 6.2) y `docs/dashboard-mockup.html`.

use smithay_client_toolkit::seat::keyboard::Keysym;

use super::component::{close_overlays, Component, DrawCtx, UpdateCtx};
use super::*;
use crate::approach;

/// Estado de el dashboard.
#[derive(Default)]
pub struct DashboardState {
    /// Objetivo: abierto o cerrado. La animación lo sigue con `progress`.
    pub open: bool,
    /// 0 (cerrado) a 1 (abierto); permite desvanecer también al cerrar.
    pub progress: f32,
}

impl DashboardState {
    /// ¿Hay algo que dibujar? (también mientras se desvanece al cerrar).
    pub fn visible(&self) -> bool {
        self.open || self.progress > 0.005
    }
}

/// Dibuja el dashboard a pantalla completa. **MAQUETA PENDIENTE**: hoy solo pinta el fondo y un
/// rótulo; el contenido real se maqueta aquí, con los tokens de `theme.rs` (`palette`, `pal()`).
pub(super) fn draw(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let screen = ui.max_rect();
    ui.painter()
        .rect_filled(screen, CornerRadius::same(0), Palette::scrim(200));
    // Registrado antes que el contenido: un clic en un widget gana sobre el fondo.
    let backdrop = ui.interact(
        screen,
        ui.scope_id().with("dashboard-backdrop"),
        egui::Sense::click(),
    );
    if backdrop.clicked() {
        state.dashboard.open = false;
    }
    text_at(
        ui,
        screen.center(),
        egui::Align2::CENTER_CENTER,
        "el dashboard (maqueta pendiente)",
        TEXT_TITLE,
        palette.muted,
    );
}

pub struct Dashboard;

impl Component for Dashboard {
    fn name(&self) -> &'static str {
        "dashboard"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, _ctx: DrawCtx, palette: Palette) {
        if !state.dashboard.visible() {
            return;
        }
        // Fundido de entrada y salida; se restaura la opacidad para los componentes siguientes.
        let fade = crate::smootherstep(state.dashboard.progress);
        let previous = ui.opacity();
        ui.set_opacity(previous * fade);
        draw(ui, state, palette);
        ui.set_opacity(previous);
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        if state.dashboard.open {
            vec![(0, 0, screen_w, screen_h)]
        } else {
            Vec::new()
        }
    }

    fn update(&mut self, state: &mut UiState, ctx: &UpdateCtx) -> bool {
        let s = &mut state.dashboard;
        let before = s.progress;
        s.progress = approach(s.progress, if s.open { 1.0 } else { 0.0 }, ctx.dt * 7.0);
        (s.progress - before).abs() > 0.0005
    }

    fn on_key(&mut self, state: &mut UiState, key: Keysym) -> bool {
        if state.dashboard.open && key == Keysym::Escape {
            state.dashboard.open = false;
            return true;
        }
        false
    }

    fn wants_keyboard(&self, state: &UiState) -> bool {
        state.dashboard.open
    }

    fn on_command(&mut self, command: &str, state: &mut UiState) -> bool {
        if command != "dashboard" {
            return false;
        }
        // El mismo atajo abre y cierra. Solo hay un overlay a pantalla completa a la vez.
        let was_open = state.dashboard.open;
        close_overlays(state);
        state.dashboard.open = !was_open;
        state.bar.show_panel = false;
        true
    }
}
