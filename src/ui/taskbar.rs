//! Taskbar estilo Windows, acoplable a los cuatro lados (feature `taskbar`).
//!
//! Reutiliza el modelo del dock (pinned + clientes de `Shared`) y presenta una
//! vista horizontal o vertical con iconos centrados, bandeja anclada al borde
//! final y autohide al acercar el cursor al lado elegido.
//!
//! Toda la geometría sale de las funciones de este archivo (`taskbar_rect`,
//! `hot_rect`, `near_edge`, `cell_size`): el dibujo y `input_rects` no pueden
//! desincronizarse. El estado de la app activa es el de `Shared`, no uno propio.

use super::*;
use crate::{
    config::{Side, TaskbarConfig},
    shared::DockAction,
};

/// Franja (px) que despierta la taskbar oculta. La usan `hot_rect` y `near_edge`.
const HOT: i32 = 24;
const GAP: f32 = 3.0;
const SEP: f32 = 9.0;

/// Estado de la taskbar.
#[derive(Default)]
pub struct TaskbarState {
    pub enabled: bool,
    pub progress: f32,
    pub visible: bool,
}

impl TaskbarState {
    pub fn new() -> Self {
        let cfg = &config::get().taskbar;
        Self {
            enabled: cfg.enabled,
            progress: if cfg.enabled { 1.0 } else { 0.0 },
            visible: cfg.enabled,
        }
    }
}

/// Enciende o apaga la taskbar. Lo usan el comando `taskbar` (alternar) y `taskbar-off`
/// (el topbar la apaga al activarse).
pub fn set_enabled(state: &mut UiState, on: bool) {
    let tb = &mut state.taskbar;
    tb.enabled = on;
    tb.visible = on;
    tb.progress = if on { 1.0 } else { 0.0 };
}

/// Rect de la taskbar en coordenadas del surface.
pub fn taskbar_rect(state: &UiState, screen_w: i32, screen_h: i32) -> (i32, i32, i32, i32) {
    let cfg = &state.shared.config.taskbar;
    let size = cfg.size as i32;
    match cfg.side {
        Side::Bottom => (0, screen_h - size, screen_w, size),
        Side::Top => (0, 0, screen_w, size),
        Side::Left => (0, 0, size, screen_h),
        Side::Right => (screen_w - size, 0, size, screen_h),
    }
}

/// Zona sensible aunque esté oculta: una franja fina en el borde.
pub fn hot_rect(state: &UiState, screen_w: i32, screen_h: i32) -> (i32, i32, i32, i32) {
    let cfg = &state.shared.config.taskbar;
    match cfg.side {
        Side::Bottom => (0, screen_h - HOT, screen_w, HOT),
        Side::Top => (0, 0, screen_w, HOT),
        Side::Left => (0, 0, HOT, screen_h),
        Side::Right => (screen_w - HOT, 0, HOT, screen_h),
    }
}

/// `true` si el cursor está en la franja de activación del lado elegido.
pub fn near_edge(cfg: &TaskbarConfig, cursor: (i32, i32), screen_w: i32, screen_h: i32) -> bool {
    let (x, y) = cursor;
    match cfg.side {
        Side::Bottom => y >= screen_h - HOT,
        Side::Top => y <= HOT,
        Side::Left => x <= HOT,
        Side::Right => x >= screen_w - HOT,
    }
}

/// Lado de cada celda: sigue al grosor de la barra (48 px dan celdas de 40, como en el mockup).
fn cell_size(cfg: &TaskbarConfig) -> f32 {
    (cfg.size as f32 - 8.0).max(32.0)
}

/// Celda cuadrada de `size` px en la posición `along` del eje de la barra.
fn cell(bar: egui::Rect, horizontal: bool, along: f32, size: f32) -> egui::Rect {
    if horizontal {
        egui::Rect::from_min_size(
            egui::pos2(along, bar.center().y - size / 2.0),
            egui::vec2(size, size),
        )
    } else {
        egui::Rect::from_min_size(
            egui::pos2(bar.center().x - size / 2.0, along),
            egui::vec2(size, size),
        )
    }
}

struct Item {
    name: String,
    glyph: &'static str,
    command: String,
    windows: Vec<crate::ipc::ClientInfo>,
}

fn make_item(state: &UiState, name: &str) -> Item {
    let key = name.to_lowercase();
    let windows: Vec<_> = state
        .shared
        .clients
        .iter()
        .filter(|c| c.class.to_lowercase() == key)
        .cloned()
        .collect();
    Item {
        name: name.to_string(),
        // Mismo icono de respaldo que el dock: un solo mapeo clase → glifo.
        glyph: dock_icon(name),
        command: name.to_string(),
        windows,
    }
}

fn items(state: &UiState) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    for pinned in &state.shared.config.dock.pinned {
        out.push(make_item(state, pinned));
    }
    for client in &state.shared.clients {
        let key = client.class.to_lowercase();
        if !out.iter().any(|it| it.name.to_lowercase() == key) {
            out.push(make_item(state, &client.class));
        }
    }
    out
}

/// Color de fondo de un botón: pulsado, al pasar el cursor o activo.
fn hover_fill(response: &egui::Response, palette: Palette, active: bool) -> Color32 {
    if active {
        palette.accent_soft
    } else if response.is_pointer_button_down_on() {
        palette.badge
    } else if response.hovered() {
        palette.hover
    } else {
        Color32::TRANSPARENT
    }
}

/// Punto de «ventana abierta» sobre el borde de pantalla donde se acopla la barra.
fn edge_point(rect: egui::Rect, side: Side) -> egui::Pos2 {
    let c = rect.center();
    match side {
        Side::Bottom => egui::pos2(c.x, rect.max.y - 1.0),
        Side::Top => egui::pos2(c.x, rect.min.y + 1.0),
        Side::Left => egui::pos2(rect.min.x + 1.0, c.y),
        Side::Right => egui::pos2(rect.max.x - 1.0, c.y),
    }
}

/// Marca de la app activa, en el borde de pantalla (como en el mockup).
fn active_mark(rect: egui::Rect, side: Side) -> egui::Rect {
    let c = rect.center();
    match side {
        Side::Bottom => egui::Rect::from_min_size(
            egui::pos2(c.x - 8.0, rect.max.y - 2.0),
            egui::vec2(16.0, 2.0),
        ),
        Side::Top => {
            egui::Rect::from_min_size(egui::pos2(c.x - 8.0, rect.min.y), egui::vec2(16.0, 2.0))
        }
        Side::Left => {
            egui::Rect::from_min_size(egui::pos2(rect.min.x, c.y - 8.0), egui::vec2(2.0, 16.0))
        }
        Side::Right => egui::Rect::from_min_size(
            egui::pos2(rect.max.x - 2.0, c.y - 8.0),
            egui::vec2(2.0, 16.0),
        ),
    }
}

/// Botón de sistema (inicio, buscar, tareas). Devuelve `true` si se pulsó.
fn tool_slot(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    id: &'static str,
    glyph: &'static str,
    color: Color32,
    tip: &str,
    palette: Palette,
) -> bool {
    let response = ui.interact(rect, ui.scope_id().with(id), egui::Sense::click());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(9),
        hover_fill(&response, palette, false),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        icon_font(rect.height() * 0.5),
        color,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let clicked = response.clicked();
    let _ = response.on_hover_text(tip.to_string());
    clicked
}

/// Un icono de la bandeja con su descripción real al pasar el cursor.
struct TrayIcon {
    glyph: &'static str,
    color: Color32,
    tip: String,
    /// Punto de aviso (notificaciones sin leer).
    pending: bool,
}

/// Iconos de la bandeja de izquierda a derecha, con el estado real de `Shared`.
fn tray_icons(state: &UiState, palette: Palette) -> Vec<TrayIcon> {
    let mut out = Vec::new();
    #[cfg(feature = "notifications")]
    out.push(bell_icon(state, palette));
    let battery = &state.shared.battery;
    if battery.present {
        // Sin batería (sobremesa) no se dibuja nada: no se inventa un nivel.
        let glyph = battery_icon(battery.percent, battery.charging);
        out.push(TrayIcon {
            glyph,
            color: battery_tone(battery.percent, battery.charging),
            tip: format!(
                "Batería {}%{}",
                battery.percent,
                if battery.charging { ", cargando" } else { "" }
            ),
            pending: false,
        });
    }
    let (glyph, color, tip) = audio_status(&state.shared.system, palette);
    out.push(TrayIcon {
        glyph,
        color,
        tip,
        pending: false,
    });
    let (glyph, color, tip) = wifi_status(&state.shared.system, palette);
    out.push(TrayIcon {
        glyph,
        color,
        tip,
        pending: false,
    });
    out
}

#[cfg(feature = "notifications")]
fn bell_icon(state: &UiState, palette: Palette) -> TrayIcon {
    let n = &state.notifications;
    let (glyph, tip) = if n.dnd {
        (ic::BELL_SLASH, "No molestar: activado".to_string())
    } else {
        (ic::BELL, "Notificaciones".to_string())
    };
    TrayIcon {
        glyph,
        color: palette.text,
        tip,
        pending: !n.notification_list.is_empty(),
    }
}

/// Un icono de la bandeja. `size` es la celda (el área de respuesta al cursor).
fn tray_button(
    ui: &mut egui::Ui,
    center: egui::Pos2,
    size: f32,
    id: (&'static str, usize),
    icon: &TrayIcon,
    palette: Palette,
) {
    let rect = egui::Rect::from_center_size(center, egui::vec2(size, size));
    let response = ui.interact(rect, ui.scope_id().with(id), egui::Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(6),
        hover_fill(&response, palette, false),
    );
    ui.painter().text(
        center,
        egui::Align2::CENTER_CENTER,
        icon.glyph,
        icon_font(size * 0.625),
        icon.color,
    );
    if icon.pending {
        ui.painter().circle_filled(
            rect.right_top() + egui::vec2(-4.0, 4.0),
            2.5,
            palette.accent,
        );
    }
    let _ = response.on_hover_text(icon.tip.clone());
}

/// Un clic en una app: la lanza si no tiene ventanas; si ya tiene el foco, pasa a la
/// siguiente de sus ventanas (igual que el dock).
fn activate(state: &mut UiState, app: &Item, active: bool) {
    if app.windows.is_empty() {
        state.shared.dock_action = Some(DockAction::Launch(app.command.clone()));
        return;
    }
    let current = &state.shared.snapshot.window_address;
    let index = if active {
        app.windows
            .iter()
            .position(|w| &w.address == current)
            .map_or(0, |i| (i + 1) % app.windows.len())
    } else {
        0
    };
    state.shared.dock_action = Some(DockAction::Focus(app.windows[index].address.clone()));
}

/// Dibuja la taskbar. El orden de dibujo es: fondo, botones, apps y bandeja.
pub(super) fn draw_taskbar(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    if !state.taskbar.enabled || state.taskbar.progress <= 0.01 {
        return;
    }
    let cfg = state.shared.config.taskbar;
    let horizontal = cfg.side.is_horizontal();
    let screen = ui.max_rect();
    let (x, y, w, h) = taskbar_rect(state, screen.width() as i32, screen.height() as i32);
    let bar = egui::Rect::from_min_size(
        egui::pos2(x as f32, y as f32),
        egui::vec2(w as f32, h as f32),
    );

    ui.painter().rect(
        bar,
        CornerRadius::same(0),
        palette.surface,
        Stroke::new(1.0, palette.border),
        StrokeKind::Middle,
    );

    let item = cell_size(&cfg);
    let font = item * 0.5;
    let apps = items(state);
    let max_span = if horizontal {
        bar.width() * 0.62
    } else {
        bar.height() * 0.62
    };
    let fixed = 3.0 * item + SEP;
    // Solo cuentan las apps que se dibujan: así el bloque queda centrado aunque se recorte.
    let visible_apps = apps
        .len()
        .min(((max_span - fixed) / (item + GAP)).max(0.0) as usize);
    let total = fixed + visible_apps as f32 * (item + GAP) - GAP;

    let mut along = if horizontal {
        bar.center().x - total / 2.0
    } else {
        bar.center().y - total / 2.0
    };

    // Botones de sistema: abren las vistas internas (no lanzan comandos del sistema).
    // Cada uno prueba sus comandos en orden hasta que alguno esté compilado.
    let tools: [(
        &'static str,
        &'static str,
        &'static str,
        &'static [&'static str],
    ); 3] = [
        (
            ic::SQUARES_FOUR,
            "tb-start",
            "Inicio",
            &["activities", "overview"],
        ),
        (ic::MAGNIFYING_GLASS, "tb-search", "Buscar", &["menu"]),
        (ic::APP_WINDOW, "tb-tasks", "Ver tareas", &["overview"]),
    ];
    for (glyph, id, tip, commands) in tools {
        let rect = cell(bar, horizontal, along, item);
        along += item + GAP;
        let color = if glyph == ic::SQUARES_FOUR {
            palette.info
        } else {
            palette.text
        };
        if tool_slot(ui, rect, id, glyph, color, tip, palette) {
            for command in commands {
                if super::dispatch_command(command, state) {
                    break;
                }
            }
        }
    }
    along += SEP - GAP;

    for (i, app) in apps.iter().take(visible_apps).enumerate() {
        let rect = cell(bar, horizontal, along, item);
        along += item + GAP;
        let active = state
            .shared
            .snapshot
            .window_class
            .eq_ignore_ascii_case(&app.name);
        let running = !app.windows.is_empty();
        let response = ui.interact(
            rect,
            ui.scope_id().with(("tb-app", i)),
            egui::Sense::click(),
        );
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(9),
            hover_fill(&response, palette, active),
        );
        let icon_px = item * 0.55;
        let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(icon_px, icon_px));
        if let Some(texture) = state.shared.icon(ui.ctx(), &app.name) {
            egui::Image::new(egui::load::SizedTexture::new(
                texture.id(),
                icon_rect.size(),
            ))
            .paint_at(ui, icon_rect);
        } else {
            let color = if active { palette.accent } else { palette.text };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                app.glyph,
                icon_font(font),
                color,
            );
        }
        // Indicador en el borde de pantalla: marca si está activa, punto si solo tiene ventanas.
        if active {
            ui.painter().rect_filled(
                active_mark(rect, cfg.side),
                CornerRadius::same(1),
                palette.accent,
            );
        } else if running {
            ui.painter()
                .circle_filled(edge_point(rect, cfg.side), 1.8, palette.muted);
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let clicked = response.clicked();
        let _ = response.on_hover_text(app.name.clone());
        if clicked {
            activate(state, app, active);
        }
    }

    // ── Bandeja: anclada al borde final, con el estado real ─────────────────
    let time = Local::now().format("%H:%M").to_string();
    let date = short_date();
    let tray = tray_icons(state, palette);
    let step = item * 0.6;
    if horizontal {
        let mut x = bar.max.x - 8.0;
        for (i, icon) in tray.iter().enumerate().rev() {
            let center = egui::pos2(x - step / 2.0, bar.center().y);
            tray_button(ui, center, step, ("tb-tray", i), icon, palette);
            x -= step;
        }
        text_at(
            ui,
            egui::pos2(x - 6.0, bar.center().y - 6.0),
            egui::Align2::RIGHT_CENTER,
            time,
            TEXT_BODY,
            palette.text,
        );
        text_at(
            ui,
            egui::pos2(x - 6.0, bar.center().y + 7.0),
            egui::Align2::RIGHT_CENTER,
            date,
            TEXT_CAPTION,
            palette.muted,
        );
    } else {
        let mut y = bar.max.y - 8.0;
        for (i, icon) in tray.iter().enumerate() {
            let center = egui::pos2(bar.center().x, y - step / 2.0);
            tray_button(ui, center, step, ("tb-tray", i), icon, palette);
            y -= step;
        }
        text_at(
            ui,
            egui::pos2(bar.center().x, y - 8.0),
            egui::Align2::CENTER_CENTER,
            time,
            TEXT_CAPTION,
            palette.text,
        );
    }
}
