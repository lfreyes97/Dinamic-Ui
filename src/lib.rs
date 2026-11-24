//! Dinamic UI: isla / barra / panel para Hyprland (egui + wgpu sobre layer-shell).
//!
//! La lib contiene todo el shell salvo el bucle de eventos Wayland (`src/main.rs`,
//! binario `dinamic-ui`). El previsualizador interactivo (feature `previewer`,
//! binario `previewer`) reutiliza esta lib sin compositor: ver `docs/PREVIEWER.md`.

// El kit de UI compartido (`ui/mod.rs`) tiene piezas que solo usa cada componente: en
// un build parcial es normal que haya código sin usar. Con todo compilado sigue avisando.
#![cfg_attr(
    not(all(
        feature = "bar",
        feature = "dock",
        feature = "overview",
        feature = "command-menu",
        feature = "notifications",
        feature = "wallpaper"
    )),
    allow(dead_code)
)]

pub mod actions;
#[cfg(feature = "icons")]
pub mod appicons;
pub mod config;
pub mod control;
#[cfg(feature = "icons")]
pub mod desktop;
pub mod gfx;
pub mod ipc;
#[cfg(feature = "command-menu")]
pub mod launcher;
pub mod needs;
#[cfg(feature = "notifications")]
pub mod notifications;
pub mod power;
// Criterio de búsqueda compartido por el command menu y Actividades.
#[cfg(any(feature = "command-menu", feature = "activities"))]
pub mod search;
pub mod shared;
pub mod sysinfo;
pub mod theme;
#[cfg(feature = "wallpaper")]
pub mod thumbs;
pub mod ui;

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Curva con velocidad cero en ambos extremos: el morph arranca y termina suave.
pub fn smootherstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

pub fn approach(current: f32, target: f32, step: f32) -> f32 {
    if (current - target).abs() <= step {
        target
    } else if current < target {
        current + step
    } else {
        current - step
    }
}
