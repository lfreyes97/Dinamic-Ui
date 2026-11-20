//! Vista previa de la UI real, renderizada a PNG sin compositor (solo desarrollo).
//!
//! `cargo test ui_preview -- --ignored` deja las imágenes en `target/ui-preview/`.
//! Usa datos de ejemplo y la config por defecto, así que el resultado es reproducible.

use std::sync::Arc;

use egui_kittest::Harness;

use super::*;
use crate::{config::Side, desktop, ipc::ClientInfo};

const W: f32 = 1280.0;
const H: f32 = 720.0;

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

fn apps() -> Vec<desktop::DesktopApp> {
    let entry = |id: &str, name: &str, comment: &str, exec: &str| {
        desktop::parse(
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
fn sample_state(k: f32) -> UiState {
    let g = |x: i32, y: i32, w: i32, h: i32| {
        let s = |v: i32| (v as f32 * k).round() as i32;
        (s(x), s(y), s(w), s(h))
    };
    let mut state = UiState::new();
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

/// Fondo de escritorio de mentira, para ver bien las transparencias.
fn fake_desktop(ui: &egui::Ui, with_windows: bool) {
    let (w, h) = (
        ui.ctx().content_rect().width(),
        ui.ctx().content_rect().height(),
    );
    let p = ui.painter();
    let screen = ui.ctx().content_rect();
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

fn render(name: &str, windows_behind: bool, state: UiState) {
    render_sized(name, (W, H), windows_behind, state);
}

fn render_sized(name: &str, size: (f32, f32), windows_behind: bool, mut state: UiState) {
    let mut fonts_ready = false;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(size.0, size.1))
        .with_pixels_per_point(1.0)
        .build_ui_state(
            move |ui, state: &mut UiState| {
                let ctx = ui.ctx().clone();
                // Solo la primera vez: las fuentes del proyecto (Inter + Phosphor).
                if !fonts_ready {
                    crate::gfx::install_fonts(&ctx);
                    fonts_ready = true;
                }
                let screen = ctx.content_rect();
                let mut full = ui.new_child(egui::UiBuilder::new().max_rect(screen));
                fake_desktop(&full, windows_behind);
                let pill_w = metrics().island_w as f32;
                draw(&mut full, state, Palette::load(), 0.0, 0.0, pill_w);
            },
            {
                state.overview.progress = if state.overview.overview { 1.0 } else { 0.0 };
                state.activities.progress = if state.activities.open { 1.0 } else { 0.0 };
                state
            },
        );
    for _ in 0..4 {
        harness.step();
    }
    let image = harness.render().expect("render");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-preview");
    std::fs::create_dir_all(&dir).unwrap();
    image.save(dir.join(format!("{name}.png"))).unwrap();
    eprintln!(
        "vista previa: {}",
        dir.join(format!("{name}.png")).display()
    );
}

fn open_overview(kind: overview::OverviewKind, query: &str, k: f32) -> UiState {
    let mut state = sample_state(k);
    overview::open(&mut state, kind);
    state.overview.query = query.into();
    state
}

/// Actividades con las apps de ejemplo (no las del sistema).
fn open_activities(query: &str, k: f32) -> UiState {
    let mut state = sample_state(k);
    activities::open(&mut state);
    state.activities.apps = apps();
    state.activities.query = query.into();
    state
}

fn strip_state(kind: overview::OverviewKind, h: Side, v: Side, k: f32) -> UiState {
    let mut state = open_overview(kind, "", k);
    let mut cfg = crate::config::Config::default();
    cfg.overview.strip_h_side = h;
    cfg.overview.strip_v_side = v;
    state.shared.config = Arc::new(cfg);
    state
}

#[test]
#[ignore = "genera PNG en target/ui-preview (necesita GPU)"]
fn ui_preview_overviews() {
    use overview::OverviewKind::*;
    render(
        "overview-workspaces",
        false,
        open_overview(Workspaces, "", 1.0),
    );
    render(
        "overview-workspaces-search",
        false,
        open_overview(Workspaces, "code", 1.0),
    );
    render("overview-activities", false, open_activities("", 1.0));
    render(
        "overview-activities-search",
        false,
        open_activities("term", 1.0),
    );
    render(
        "strip-h-top",
        true,
        strip_state(StripH, Side::Top, Side::Right, 1.0),
    );
    render(
        "strip-h-bottom",
        true,
        strip_state(StripH, Side::Bottom, Side::Right, 1.0),
    );
    render(
        "strip-v-left",
        true,
        strip_state(StripV, Side::Top, Side::Left, 1.0),
    );
    render(
        "strip-v-right",
        true,
        strip_state(StripV, Side::Top, Side::Right, 1.0),
    );

    // Pantalla de 1080p: las dimensiones derivadas deben crecer con ella.
    let big = (1920.0, 1080.0);
    render_sized(
        "overview-workspaces-1080p",
        big,
        false,
        open_overview(Workspaces, "", 1.5),
    );
    render_sized(
        "overview-activities-1080p",
        big,
        false,
        open_activities("", 1.5),
    );
    render_sized(
        "strip-h-top-1080p",
        big,
        true,
        strip_state(StripH, Side::Top, Side::Right, 1.5),
    );
    render_sized(
        "strip-v-right-1080p",
        big,
        true,
        strip_state(StripV, Side::Top, Side::Right, 1.5),
    );

    // La pantalla del R3: 1366×768. Aquí las franjas miden 154 px (1/5) y 228 px (1/6).
    let r3 = (1366.0, 768.0);
    let k = 1366.0 / 1280.0;
    render_sized(
        "strip-h-top-1366",
        r3,
        true,
        strip_state(StripH, Side::Top, Side::Right, k),
    );
    render_sized(
        "strip-v-right-1366",
        r3,
        true,
        strip_state(StripV, Side::Top, Side::Right, k),
    );

    // Topbar (la pantalla del R3: 1366×768).
    render_sized("topbar", r3, true, topbar_state(false, false, k));
    render_sized("topbar-quick", r3, true, topbar_state(true, false, k));
    render_sized("topbar-calendar", r3, true, topbar_state(false, true, k));
    #[cfg(feature = "notifications")]
    {
        let mut many = topbar_state(false, true, k);
        many.notifications.notification_list = sample_notifications(5);
        many.notifications.dnd = true;
        render_sized("topbar-calendar-many", r3, true, many);
        let mut empty = topbar_state(false, true, k);
        empty.notifications.notification_list.clear();
        render_sized("topbar-calendar-empty", r3, true, empty);
    }
}

fn topbar_state(quick: bool, calendar: bool, k: f32) -> UiState {
    let mut state = sample_state(k);
    state.topbar.enabled = true;
    state.topbar.quick_open = quick;
    state.topbar.calendar_open = calendar;
    #[cfg(feature = "notifications")]
    {
        state.notifications.notification_list = sample_notifications(2);
    }
    state.shared.snapshot.window_title = "Rust — Mozilla Firefox".into();
    // Datos del sistema de ejemplo (los que leería `sysinfo`).
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
    state
}

#[cfg(feature = "notifications")]
fn sample_notifications(n: usize) -> Vec<crate::notifications::Notification> {
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
