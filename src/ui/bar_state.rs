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
