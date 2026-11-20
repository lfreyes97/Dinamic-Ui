//! Vista previa del panel de configuración (solo desarrollo): `just preview` deja las imágenes
//! en `target/ui-preview/config-*.png`. Usa los datos de ejemplo de `preview.rs`.

use super::preview::{render_sized, sample_state};

#[test]
#[ignore = "genera PNG en target/ui-preview (necesita GPU)"]
fn ui_preview_config_panel() {
    // 1366×768 es la pantalla del R3; 1080p comprueba que las medidas escalan.
    for (name, size, k) in [
        ("config", (1366.0, 768.0), 1366.0 / 1280.0),
        ("config-1080p", (1920.0, 1080.0), 1.5),
    ] {
        let mut state = sample_state(k);
        state.config_panel.open = true;
        render_sized(name, size, true, state);
    }
}
