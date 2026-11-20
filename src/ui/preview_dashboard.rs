//! Vista previa del dashboard (solo desarrollo): `just preview` deja las imágenes en
//! `target/ui-preview/dashboard-*.png`. Usa los datos de ejemplo de `preview.rs`.

use super::preview::{render_sized, sample_state};

#[test]
#[ignore = "genera PNG en target/ui-preview (necesita GPU)"]
fn ui_preview_dashboard() {
    // 1366×768 es la pantalla del R3; 1080p comprueba que las medidas escalan.
    for (name, size, k) in [
        ("dashboard", (1366.0, 768.0), 1366.0 / 1280.0),
        ("dashboard-1080p", (1920.0, 1080.0), 1.5),
    ] {
        let mut state = sample_state(k);
        state.dashboard.open = true;
        render_sized(name, size, true, state);
    }
}
