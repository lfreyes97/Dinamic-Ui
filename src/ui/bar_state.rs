//! Estado de la barra. Vive en el núcleo (no depende de la feature `bar`): lo leen
//! overview, menú y notificaciones para cerrar el panel o posicionar su tarjeta.

/// Fases de la barra, de menor a mayor expansión.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BarPhase {
    Notch,
    Compact,
    Bar,
    Panel,
}

/// Estado de la barra: isla → píldora → barra, con panel acoplado.
#[derive(Default)]
pub struct BarState {
    pub expansion: f32,
    pub expanded: bool,
    pub show_panel: bool,
    pub panel_progress: f32,
    pub panel_tab: usize,
    pub island_progress: f32,
    pub island_active: bool,
    pub island_compact: bool,
    pub layout_preset: usize,
    /// Otro componente (el topbar) ocupa el borde superior: la barra no se dibuja ni reclama
    /// entrada, pero el panel sigue disponible. Lo fija ese componente en cada frame.
    pub suppressed: bool,
    /// Hay un desplegable abierto bajo el borde superior (calendario, resumen del sistema…):
    /// el aviso emergente de notificaciones se aparta para no solaparse. Lo fija el topbar.
    pub popover_open: bool,
    /// Última distribución de teclado vista, para aplicar el preset solo al cambiar.
    pub last_layout: Option<String>,
}

impl BarState {
    /// Fase actual, derivada de los booleanos hasta que Fase 3 los sustituya.
    #[allow(dead_code)]
    pub fn phase(&self) -> BarPhase {
        if self.show_panel {
            BarPhase::Panel
        } else if self.expanded {
            BarPhase::Bar
        } else if self.island_compact {
            BarPhase::Compact
        } else {
            BarPhase::Notch
        }
    }

    pub fn new() -> Self {
        let panel = std::env::var_os("DINAMIC_UI_PANEL").is_some();
        Self {
            expanded: std::env::var_os("DINAMIC_UI_EXPANDED").is_some(),
            show_panel: panel,
            panel_progress: if panel { 1.0 } else { 0.0 },
            panel_tab: std::env::var("DINAMIC_UI_PANEL_TAB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            island_active: std::env::var_os("DINAMIC_UI_ISLAND").is_some(),
            ..Self::default()
        }
    }
}

/// Datos que pide la barra en su fase actual, acotados por `bar.max_state`.
/// El notch en reposo no pide nada; compacto pide la hora; la barra añade
/// batería/red/audio y el panel, pantalla y layout.
pub fn bar_needs(phase: BarPhase, max: crate::config::BarMaxState) -> crate::needs::Needs {
    use crate::{config::BarMaxState, needs::Need};
    let mut out = crate::needs::Needs::NONE;
    if phase >= BarPhase::Compact && max >= BarMaxState::Compact {
        out.set(Need::Time, true);
    }
    if phase >= BarPhase::Bar && max >= BarMaxState::Bar {
        out.set(Need::Battery, true);
        out.set(Need::Network, true);
        out.set(Need::Audio, true);
    }
    if phase >= BarPhase::Panel && max >= BarMaxState::Panel {
        out.set(Need::Display, true);
        out.set(Need::Layout, true);
        out.set(Need::Power, true);
    }
    out
}

#[cfg(test)]
mod needs_tests {
    use super::*;
    use crate::{
        config::BarMaxState,
        needs::{Need, Needs},
    };

    #[test]
    fn notch_requests_nothing() {
        assert_eq!(bar_needs(BarPhase::Notch, BarMaxState::Panel), Needs::NONE);
    }

    #[test]
    fn compact_only_requests_time() {
        let n = bar_needs(BarPhase::Compact, BarMaxState::Panel);
        assert!(n.get(Need::Time));
        assert!(!n.get(Need::Battery));
    }

    #[test]
    fn bar_and_panel_add_their_data() {
        let bar = bar_needs(BarPhase::Bar, BarMaxState::Panel);
        assert!(bar.get(Need::Battery) && bar.get(Need::Network) && bar.get(Need::Audio));
        assert!(!bar.get(Need::Layout));
        let panel = bar_needs(BarPhase::Panel, BarMaxState::Panel);
        assert!(panel.get(Need::Display) && panel.get(Need::Layout));
    }

    #[test]
    fn max_state_caps_the_requests() {
        assert_eq!(bar_needs(BarPhase::Panel, BarMaxState::Notch), Needs::NONE);
        let compact = bar_needs(BarPhase::Panel, BarMaxState::Compact);
        assert!(compact.get(Need::Time));
        assert!(!compact.get(Need::Battery) && !compact.get(Need::Layout));
    }
}
