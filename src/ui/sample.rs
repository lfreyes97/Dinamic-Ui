//! Datos de ejemplo para las vistas previas PNG (`preview.rs`) y el previewer
//! interactivo (feature `previewer`). Sin compositor ni D-Bus: todo es de mentira,
//! pero con las formas de los datos reales. El escritorio falso pinta colores fijos
//! a propósito (está exento de `lint-colors`, como `theme.rs`).

use super::*;
use crate::ipc::ClientInfo;

fn client(class: &str, title: &str, address: &str, ws: i32, g: (i32, i32, i32, i32)) -> ClientInfo {
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

#[cfg(feature = "icons")]
pub fn apps() -> Vec<crate::desktop::DesktopApp> {
    let entry = |id: &str, name: &str, comment: &str, exec: &str| {
        crate::desktop::parse(
            id,
            &format!("[Desktop Entry]\nName={name}\nComment={comment}\nExec={exec}\nIcon={id}\nType=Application\n"),
        )
        .unwrap()
    };
    vec![
        entry("firefox", "Firefox", "Navegador web", "firefox %u"),
        entry("code", "VS Code", "Editor de código", "code"),
        entry("kitty", "Kitty", "Terminal", "kitty"),
        entry(
            "org.gnome.Nautilus",
            "Archivos",
            "Gestor de archivos",
            "nautilus",
        ),
        entry("spotify", "Spotify", "Música", "spotify"),
        entry(
            "gnome-control-center",
            "Ajustes",
            "Configuración",
            "gnome-control-center",
        ),
        entry("alacritty", "Alacritty", "Terminal", "alacritty"),
        entry("obsidian", "Obsidian", "Notas", "obsidian"),
        entry("google-chrome", "Chrome", "Navegador web", "chrome"),
        entry("gimp", "GIMP", "Editor de imágenes", "gimp"),
        entry("obs", "OBS Studio", "Grabación", "obs"),
        entry("vlc", "VLC", "Reproductor", "vlc"),
    ]
}

/// Estado de ejemplo para una pantalla `k` veces mayor que 1280×720.
/// Sin servidor D-Bus: las notificaciones se inyectan con `push_preview`.
pub fn sample_state(k: f32) -> UiState {
    let g = |x: i32, y: i32, w: i32, h: i32| {
        let s = |v: i32| (v as f32 * k).round() as i32;
        (s(x), s(y), s(w), s(h))
    };
    let mut state = UiState::new_preview();
    let s = &mut state.shared;
    s.snapshot.workspace = "1".into();
    s.snapshot.window_address = "0x1".into();
    s.snapshot.occupied = vec![1, 2, 3, 5];
    s.clients = vec![
        client(
            "firefox",
            "Rust — Mozilla Firefox",
            "0x1",
            1,
            g(40, 60, 700, 560),
        ),
        client("kitty", "zsh — ~/Projects", "0x2", 1, g(760, 60, 480, 300)),
        client("kitty", "htop", "0x3", 1, g(760, 380, 480, 260)),
        client(
            "code",
            "main.rs — Dinamic-Ui",
            "0x4",
            2,
            g(80, 50, 1120, 620),
        ),
        client(
            "spotify",
            "Spotify Premium",
            "0x5",
            3,
            g(200, 120, 880, 480),
        ),
        client("firefox", "Hyprland wiki", "0x6", 5, g(60, 60, 600, 540)),
        client("code", "Cargo.toml", "0x7", 5, g(700, 60, 540, 540)),
    ];
    state
}

/// Datos del sistema de ejemplo (los que leería `sysinfo`) y batería de muestra.
pub fn sample_system(state: &mut UiState) {
    let system = &mut state.shared.system;
    system.wifi = Some(crate::sysinfo::Wifi {
        enabled: true,
        ssid: Some("RAAH-5G".into()),
    });
    system.bluetooth = Some(crate::sysinfo::Bluetooth {
        powered: true,
        connected: 1,
    });
    system.volume = Some(crate::sysinfo::Volume {
        percent: 64,
        muted: false,
    });
    system.brightness = Some(80);
    system.audio_output = Some("Altavoces internos".into());
    state.shared.battery = crate::power::BatteryState {
        present: true,
        percent: 64,
        charging: false,
        minutes_left: Some(215),
    };
}

#[cfg(feature = "notifications")]
pub fn sample_notifications(n: usize) -> Vec<crate::notifications::Notification> {
    let items = [
        ("Firefox", "Descarga completada", "14:52"),
        ("Kitty", "El proceso terminó con código 0", "14:40"),
        ("Spotify", "Siguiente: Mastodon — Blood Mountain", "14:21"),
        ("Hyprland", "Configuración recargada", "13:58"),
        ("Sistema", "Batería al 64 %", "13:30"),
    ];
    items
        .iter()
        .take(n)
        .enumerate()
        .map(
            |(i, (app, summary, time))| crate::notifications::Notification {
                id: i as u32 + 1,
                app: app.to_string(),
                summary: summary.to_string(),
                body: String::new(),
                time: time.to_string(),
                received: std::time::Instant::now(),
            },
        )
        .collect()
}

/// Fondo de escritorio de mentira, para ver bien las transparencias.
/// Usa el rect del `Ui` (no el de la ventana): en el previewer el centro no ocupa
/// toda la ventana; en las vistas previas PNG ambos coinciden.
pub fn fake_desktop(ui: &egui::Ui, with_windows: bool) {
    const W: f32 = 1280.0;
    let screen = ui.max_rect();
    let (w, h) = (screen.width(), screen.height());
    let p = ui.painter();
    p.rect_filled(screen, 0.0, egui::Color32::from_rgb(0x14, 0x17, 0x1f));
    p.circle_filled(
        egui::pos2(160.0, 20.0),
        420.0,
        egui::Color32::from_rgba_unmultiplied(221, 153, 153, 26),
    );
    p.circle_filled(
        egui::pos2(w - 120.0 * (w / W), h),
        380.0,
        egui::Color32::from_rgba_unmultiplied(127, 200, 255, 22),
    );
    if with_windows {
        for (x, y, w, h) in [(260.0, 170.0, 420.0, 300.0), (720.0, 140.0, 340.0, 320.0)] {
            let r = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h));
            p.rect(
                r,
                egui::CornerRadius::same(6),
                egui::Color32::from_rgba_unmultiplied(30, 31, 36, 220),
                egui::Stroke::new(1.0, egui::Color32::from_white_alpha(24)),
                egui::StrokeKind::Middle,
            );
            p.line_segment(
                [
                    r.left_top() + egui::vec2(0.0, 18.0),
                    r.right_top() + egui::vec2(0.0, 18.0),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_white_alpha(20)),
            );
        }
    }
}
