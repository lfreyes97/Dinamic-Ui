//! Vista previa de la UI real, renderizada a PNG sin compositor (solo desarrollo).
//!
//! `cargo test ui_preview -- --ignored` deja las imágenes en `target/ui-preview/`.
//! Usa datos de ejemplo y la config por defecto, así que el resultado es reproducible.

use std::sync::Arc;

use egui_kittest::Harness;

#[cfg(feature = "notifications")]
use super::sample::sample_notifications;
use super::sample::{apps, fake_desktop, sample_state, sample_system};
use super::*;
use crate::config::Side;

const W: f32 = 1280.0;
const H: f32 = 720.0;

fn render(name: &str, windows_behind: bool, state: UiState) {
    render_sized(name, (W, H), windows_behind, state);
}

pub(super) fn render_sized(name: &str, size: (f32, f32), windows_behind: bool, mut state: UiState) {
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
                #[cfg(feature = "dashboard")]
                {
                    state.dashboard.progress = if state.dashboard.open { 1.0 } else { 0.0 };
                }
                #[cfg(feature = "config-panel")]
                {
                    state.config_panel.progress = if state.config_panel.open { 1.0 } else { 0.0 };
                }
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
    render_sized(
        "overview-activities-1366",
        r3,
        false,
        open_activities("", k),
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
    sample_system(&mut state);
    state
}

/// Estado del taskbar: `topbar_state` pone los datos del sistema de ejemplo; aquí se
/// desactiva el topbar y se acopla el taskbar al lado pedido.
fn taskbar_state(side: Side, pinned: &[&str], k: f32) -> UiState {
    let mut state = topbar_state(false, false, k);
    state.topbar.enabled = false;
    state.shared.snapshot.window_class = "firefox".into();
    let mut cfg = crate::config::Config::default();
    cfg.taskbar.enabled = true;
    cfg.taskbar.side = side;
    cfg.dock.pinned = pinned.iter().map(|s| s.to_string()).collect();
    state.shared.config = Arc::new(cfg);
    state.taskbar.enabled = true;
    state.taskbar.progress = 1.0;
    state.taskbar.visible = true;
    state
}

#[test]
#[ignore = "genera PNG en target/ui-preview (necesita GPU)"]
fn ui_preview_taskbar() {
    let pinned = ["firefox", "code", "kitty", "spotify"];
    let r3 = (1366.0, 768.0);
    let k = 1366.0 / 1280.0;
    render_sized(
        "taskbar-bottom",
        r3,
        true,
        taskbar_state(Side::Bottom, &pinned, k),
    );
    render_sized(
        "taskbar-top",
        r3,
        true,
        taskbar_state(Side::Top, &pinned, k),
    );
    render_sized(
        "taskbar-left",
        r3,
        true,
        taskbar_state(Side::Left, &pinned, k),
    );
    render_sized(
        "taskbar-right",
        r3,
        true,
        taskbar_state(Side::Right, &pinned, k),
    );
    // Muchas apps: comprueba el recorte de iconos y el centrado de la barra.
    let many = [
        "firefox",
        "code",
        "kitty",
        "spotify",
        "obsidian",
        "gimp",
        "obs",
        "vlc",
        "alacritty",
        "gnome-control-center",
        "org.gnome.Nautilus",
        "google-chrome",
        "thunderbird",
        "slack",
        "discord",
        "steam",
        "blender",
        "krita",
    ];
    render_sized(
        "taskbar-bottom-many",
        r3,
        true,
        taskbar_state(Side::Bottom, &many, k),
    );
    // Con el desplegable de las apps que no caben abierto, abajo y en el lado derecho.
    for (name, side) in [
        ("taskbar-overflow-bottom", Side::Bottom),
        ("taskbar-overflow-right", Side::Right),
    ] {
        let mut state = taskbar_state(side, &many, k);
        state.taskbar.overflow_open = true;
        render_sized(name, r3, true, state);
    }
}
