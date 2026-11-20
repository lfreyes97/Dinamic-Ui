//! Vista previa del panel de configuración (solo desarrollo): `just preview` deja las imágenes
//! en `target/ui-preview/config-*.png`. Usa los datos de ejemplo de `preview.rs` y, como
//! configuración, el `config.example.toml` del repo (parseado con el mismo `Config` del shell).

use std::sync::Arc;

use super::config_panel::{ConfigFile, FileState, Page, PAGES};
use super::preview::{render_sized, sample_state};
use super::UiState;
use crate::config::Config;

/// Ejemplo del repo: los valores activos son los que el shell usaría con ese archivo.
const EXAMPLE_TOML: &str = include_str!("../../config.example.toml");

/// Estado del panel en una página, con la configuración de ejemplo y su archivo.
fn panel_state(k: f32, page: Page) -> UiState {
    let mut state = sample_state(k);
    let cfg: Config = toml::from_str(EXAMPLE_TOML).expect("config.example.toml válido");
    state.shared.config = Arc::new(cfg);
    state.config_panel.open = true;
    state.config_panel.page = PAGES.iter().position(|p| *p == page).unwrap_or(0);
    state.config_panel.file = Some(ConfigFile {
        path: "~/.config/dinamic-ui/config.toml".into(),
        state: FileState::Text(EXAMPLE_TOML.to_string()),
    });
    state
}

#[test]
#[ignore = "genera PNG en target/ui-preview (necesita GPU)"]
fn ui_preview_config_panel() {
    // 1366×768 es la pantalla del R3; 1080p comprueba que las medidas escalan.
    for (name, size, k) in [
        ("config", (1366.0, 768.0), 1366.0 / 1280.0),
        ("config-1080p", (1920.0, 1080.0), 1.5),
        // Más alta que la pantalla real: deja ver la paleta completa sin desplazar la página.
        ("config-paleta", (1366.0, 2000.0), 1366.0 / 1280.0),
    ] {
        render_sized(name, size, true, panel_state(k, Page::Theme));
    }
}

#[test]
#[ignore = "genera PNG en target/ui-preview (necesita GPU)"]
fn ui_preview_config_pages() {
    let k = 1366.0 / 1280.0;
    for (name, page) in [
        ("config-bars", Page::Bars),
        ("config-overviews", Page::Overviews),
        ("config-dashboard", Page::Dashboard),
        ("config-workspaces", Page::Workspaces),
        ("config-notifications", Page::Notifications),
        ("config-dock", Page::Dock),
        ("config-keys", Page::Keys),
        ("config-toml", Page::Toml),
    ] {
        render_sized(name, (1366.0, 768.0), true, panel_state(k, page));
    }
}
