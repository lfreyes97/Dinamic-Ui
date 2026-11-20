//! Panel de configuración: páginas de ajustes con navegación lateral (feature `config-panel`,
//! comando `--config-panel`). Ver `docs/WIDGETS_PLAN.md` (Fase 6.5) y `docs/config-panel-mockup.html`.

use smithay_client_toolkit::seat::keyboard::Keysym;

use super::component::{close_overlays, Component, DrawCtx, UpdateCtx};
use super::*;
use crate::approach;

/// Estado de el panel de configuración.
#[derive(Default)]
pub struct ConfigPanelState {
    /// Objetivo: abierto o cerrado. La animación lo sigue con `progress`.
    pub open: bool,
    /// 0 (cerrado) a 1 (abierto); permite desvanecer también al cerrar.
    pub progress: f32,
    /// Página seleccionada en la navegación lateral.
    #[allow(dead_code)] // lo usará la maqueta de las páginas
    pub page: usize,
}

impl ConfigPanelState {
    /// ¿Hay algo que dibujar? (también mientras se desvanece al cerrar).
    pub fn visible(&self) -> bool {
        self.open || self.progress > 0.005
    }
}

/// Dibuja el panel de configuración a pantalla completa. **MAQUETA PENDIENTE**: hoy solo pinta el fondo y un
/// rótulo; el contenido real se maqueta aquí, con los tokens de `theme.rs` (`palette`, `pal()`).
pub(super) fn draw(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let screen = ui.max_rect();
    ui.painter()
        .rect_filled(screen, CornerRadius::same(0), Palette::scrim(200));
    // Registrado antes que el contenido: un clic en un widget gana sobre el fondo.
    let backdrop = ui.interact(
        screen,
        ui.scope_id().with("config_panel-backdrop"),
        egui::Sense::click(),
    );
    if backdrop.clicked() {
        state.config_panel.open = false;
    }
    text_at(
        ui,
        screen.center(),
        egui::Align2::CENTER_CENTER,
        "el panel de configuración (maqueta pendiente)",
        TEXT_TITLE,
        palette.muted,
    );
}

pub struct ConfigPanel;

impl Component for ConfigPanel {
    fn name(&self) -> &'static str {
        "config_panel"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, _ctx: DrawCtx, palette: Palette) {
        if !state.config_panel.visible() {
            return;
        }
        // Fundido de entrada y salida; se restaura la opacidad para los componentes siguientes.
        let fade = crate::smootherstep(state.config_panel.progress);
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
        if state.config_panel.open {
            vec![(0, 0, screen_w, screen_h)]
        } else {
            Vec::new()
        }
    }

    fn update(&mut self, state: &mut UiState, ctx: &UpdateCtx) -> bool {
        let s = &mut state.config_panel;
        let before = s.progress;
        s.progress = approach(s.progress, if s.open { 1.0 } else { 0.0 }, ctx.dt * 7.0);
        (s.progress - before).abs() > 0.0005
    }

    fn on_key(&mut self, state: &mut UiState, key: Keysym) -> bool {
        if state.config_panel.open && key == Keysym::Escape {
            state.config_panel.open = false;
            return true;
        }
        false
    }

    fn wants_keyboard(&self, state: &UiState) -> bool {
        state.config_panel.open
    }

    fn on_command(&mut self, command: &str, state: &mut UiState) -> bool {
        if command != "config-panel" {
            return false;
        }
        // El mismo atajo abre y cierra. Solo hay un overlay a pantalla completa a la vez.
        let was_open = state.config_panel.open;
        close_overlays(state);
        state.config_panel.open = !was_open;
        state.bar.show_panel = false;
        true
    }
}
