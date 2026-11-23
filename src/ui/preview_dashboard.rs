//! Vista previa del dashboard (solo desarrollo): `just preview` deja las imágenes en
//! `target/ui-preview/dashboard-*.png`. Usa los datos de ejemplo de `preview.rs` y, para el
//! sistema, valores de muestra fijos (batería, Wi-Fi, volumen) que solo existen aquí.

use super::preview::render_sized;
use super::sample::sample_state;
use super::*;
use crate::ipc::ClientInfo;
use crate::power::BatteryState;
use crate::sysinfo::{SystemInfo, Volume, Wifi};

/// Datos de sistema de muestra: batería al 72 %, Wi-Fi conectado y volumen al 64 %.
fn datos_de_muestra(state: &mut UiState) {
    state.shared.battery = BatteryState {
        present: true,
        percent: 72,
        charging: false,
        minutes_left: Some(140),
    };
    state.shared.system = SystemInfo {
        wifi: Some(Wifi {
            enabled: true,
            ssid: Some("Casa_5G".into()),
        }),
        volume: Some(Volume {
            percent: 64,
            muted: false,
        }),
        brightness: Some(80),
        ..Default::default()
    };
}

/// Ventana de muestra en el workspace `ws`, con la geometría en un monitor de 1920×1080.
fn ventana(
    class: &str,
    title: &str,
    address: &str,
    ws: i32,
    g: (i32, i32, i32, i32),
) -> ClientInfo {
    ClientInfo {
        class: class.into(),
        title: title.into(),
        address: address.into(),
        mapped: true,
        workspace: ws,
        x: g.0,
        y: g.1,
        w: g.2,
        h: g.3,
    }
}

fn escenario(k: f32, datos: bool) -> UiState {
    let mut state = sample_state(k);
    state.dashboard.open = true;
    if datos {
        datos_de_muestra(&mut state);
    }
    state
}

#[test]
#[ignore = "genera PNG en target/ui-preview (necesita GPU)"]
fn ui_preview_dashboard() {
    // 1366×768 es la pantalla del R3; 1080p comprueba que las medidas escalan.
    for (name, size, k) in [
        ("dashboard", (1366.0, 768.0), 1366.0 / 1280.0),
        ("dashboard-1080p", (1920.0, 1080.0), 1.5),
    ] {
        render_sized(name, size, true, escenario(k, true));
    }
}

#[test]
#[ignore = "genera PNG en target/ui-preview (necesita GPU)"]
fn ui_preview_dashboard_escenarios() {
    let size = (1366.0, 768.0);
    let k = 1366.0 / 1280.0;

    // Sin ventanas abiertas: estados vacíos de Ventanas y Workspaces.
    let mut vacio = escenario(k, true);
    vacio.shared.clients.clear();
    vacio.shared.snapshot.occupied.clear();
    render_sized("dashboard-sin-ventanas", size, false, vacio);

    // Muchas ventanas: comprueba los resúmenes «+N más».
    let mut muchas = escenario(k, true);
    let clases = [
        "firefox",
        "kitty",
        "code",
        "spotify",
        "org.gnome.Nautilus",
        "obsidian",
    ];
    for i in 0..14 {
        let ws = (i % 6) as i32 + 1;
        let clase = clases[i % clases.len()];
        muchas.shared.clients.push(ventana(
            clase,
            &format!("Ventana de muestra {i}"),
            &format!("0x{:x}", 0x100 + i),
            ws,
            (120 + 40 * i as i32, 90 + 25 * i as i32, 640, 420),
        ));
    }
    muchas.shared.snapshot.occupied = vec![1, 2, 3, 4, 5, 6];
    render_sized("dashboard-muchas-ventanas", size, true, muchas);

    // Sin datos de sistema: batería no detectada y Wi-Fi/volumen/brillo sin datos.
    render_sized("dashboard-sin-datos", size, true, escenario(k, false));
}
