//! Reservas de layout por borde: única fuente para los surfaces auxiliares y
//! para la geometría interna de los overlays.
//!
//! Orden: arriba, derecha, abajo, izquierda.

use super::UiState;

pub type Edges = [i32; 4];

/// Zonas que reservan los componentes activos: topbar y taskbar (esta última
/// solo si no es autohide y ya pasó la mitad de su animación de entrada).
#[cfg_attr(
    not(any(feature = "topbar", feature = "taskbar")),
    allow(unused_variables, unused_mut)
)]
pub fn reservations(state: &UiState) -> Edges {
    let mut zones = [0i32; 4];
    #[cfg(feature = "topbar")]
    if state.topbar.enabled {
        zones[0] = state.shared.config.topbar.height as i32;
    }
    #[cfg(feature = "taskbar")]
    if state.taskbar.enabled
        && !state.shared.config.taskbar.autohide
        && state.taskbar.progress > 0.5
    {
        use crate::config::Side;
        let size = state.shared.config.taskbar.size as i32;
        let edge = match state.shared.config.taskbar.side {
            Side::Top => 0,
            Side::Right => 1,
            Side::Bottom => 2,
            Side::Left => 3,
        };
        zones[edge] = zones[edge].max(size);
    }
    zones
}

/// Rect útil dentro de la pantalla, descontando las zonas reservadas. El rect
/// nunca se invierte aunque las reservas sumen más que la pantalla.
pub fn safe_rect(state: &UiState, screen: egui::Rect) -> egui::Rect {
    let [top, right, bottom, left] = reservations(state);
    let min = egui::pos2(
        screen.min.x + left.max(0) as f32,
        screen.min.y + top.max(0) as f32,
    );
    let max = egui::pos2(
        (screen.max.x - right.max(0) as f32).max(min.x),
        (screen.max.y - bottom.max(0) as f32).max(min.y),
    );
    egui::Rect::from_min_max(min, max)
}

#[cfg(test)]
mod tests {
    #[cfg(all(feature = "topbar", feature = "taskbar"))]
    use super::*;
    #[cfg(all(feature = "topbar", feature = "taskbar"))]
    use crate::config::{Config, Side, TaskbarConfig, TopbarConfig};

    #[cfg(all(feature = "topbar", feature = "taskbar"))]
    #[test]
    fn reservations_topbar_y_taskbar_por_borde() {
        let cfg = Config {
            topbar: TopbarConfig {
                enabled: true,
                height: 34,
                mode_control: true,
            },
            taskbar: TaskbarConfig {
                enabled: true,
                side: Side::Bottom,
                size: 48,
                autohide: false,
            },
            ..Default::default()
        };
        crate::config::set(cfg);

        let mut state = UiState::new_preview();
        state.shared.config = crate::config::get();
        state.topbar.enabled = true;
        state.taskbar.enabled = true;
        state.taskbar.progress = 1.0;
        assert_eq!(
            reservations(&state),
            [34, 0, 48, 0],
            "topbar + taskbar abajo"
        );

        let mut cfg = crate::config::get().as_ref().clone();
        cfg.taskbar.side = Side::Right;
        crate::config::set(cfg);
        state.shared.config = crate::config::get();
        assert_eq!(reservations(&state), [34, 48, 0, 0], "taskbar a la derecha");

        let mut cfg = crate::config::get().as_ref().clone();
        cfg.taskbar.autohide = true;
        crate::config::set(cfg);
        state.shared.config = crate::config::get();
        assert_eq!(reservations(&state), [34, 0, 0, 0], "autohide no reserva");

        let mut cfg = crate::config::get().as_ref().clone();
        cfg.taskbar.autohide = false;
        crate::config::set(cfg);
        state.shared.config = crate::config::get();
        state.taskbar.progress = 0.4;
        assert_eq!(reservations(&state), [34, 0, 0, 0], "progress 0.4");
        state.taskbar.progress = 0.6;
        assert_eq!(reservations(&state), [34, 48, 0, 0], "progress 0.6");
    }

    #[cfg(all(feature = "topbar", feature = "taskbar"))]
    #[test]
    fn safe_rect_descuenta_las_reservas() {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1366.0, 768.0));
        crate::config::set(Config::default());
        let mut state = UiState::new_preview();
        state.shared.config = crate::config::get();
        state.topbar.enabled = false;
        state.taskbar.enabled = false;
        assert_eq!(safe_rect(&state, screen), screen);
    }
}
