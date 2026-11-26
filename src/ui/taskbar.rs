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
    /// Desplegable con las apps que no caben en la barra.
    pub overflow_open: bool,
}

impl TaskbarState {
    pub fn new() -> Self {
        let cfg = &config::get().taskbar;
        Self {
            enabled: cfg.enabled,
            progress: if cfg.enabled { 1.0 } else { 0.0 },
            visible: cfg.enabled,
            overflow_open: false,
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
    tb.overflow_open = false;
}

/// Rect de la taskbar en coordenadas del surface.
pub fn taskbar_rect(state: &UiState, screen_w: i32, screen_h: i32) -> (i32, i32, i32, i32) {
    let cfg = &state.shared.config.taskbar;
    let size = cfg.size as i32;
    // Solo se ceden las reservas perpendiculares: la reserva de este lado es
    // la de la propia barra y no debe desplazarla.
    let [top, _, bottom, _] = super::reservations(state);
    match cfg.side {
        Side::Bottom => (0, screen_h - size, screen_w, size),
        Side::Top => (0, top, screen_w, size),
        Side::Left => (0, top, size, (screen_h - top - bottom).max(0)),
        Side::Right => (screen_w - size, top, size, (screen_h - top - bottom).max(0)),
    }
}

/// Zona sensible aunque esté oculta: una franja fina en el borde.
pub fn hot_rect(state: &UiState, screen_w: i32, screen_h: i32) -> (i32, i32, i32, i32) {
    let cfg = &state.shared.config.taskbar;
    let [top, _, bottom, _] = super::reservations(state);
    match cfg.side {
        Side::Bottom => (0, screen_h - HOT, screen_w, HOT),
        Side::Top => (0, top, screen_w, HOT),
        Side::Left => (0, top, HOT, (screen_h - top - bottom).max(0)),
        Side::Right => (screen_w - HOT, top, HOT, (screen_h - top - bottom).max(0)),
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
        CornerRadius::same(Radius::of(Radius::MD)),
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
    // Orden de lectura como en el mockup: Wi-Fi, volumen, batería y campana.
    let (glyph, color, tip) = wifi_status(&state.shared.system, palette);
    out.push(TrayIcon {
        glyph,
        color,
        tip,
        pending: false,
    });
    let (glyph, color, tip) = audio_status(&state.shared.system, palette);
    out.push(TrayIcon {
        glyph,
        color,
        tip,
        pending: false,
    });
    let battery = &state.shared.battery;
    if battery.present {
        // Sin batería (sobremesa) no se dibuja nada: no se inventa un nivel.
        out.push(TrayIcon {
            glyph: battery_icon(battery.percent, battery.charging),
            color: battery_tone(battery.percent, battery.charging),
            tip: format!(
                "Batería {}%{}",
                battery.percent,
                if battery.charging { ", cargando" } else { "" }
            ),
            pending: false,
        });
    }
    #[cfg(feature = "notifications")]
    out.push(bell_icon(state, palette));
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
        CornerRadius::same(Radius::of(Radius::SM)),
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

/// Un clic en una app: la lanza si no tiene ventanas; con varias, vuelve a la
/// más recientemente enfocada; si ya tenía el foco, pasa a la siguiente.
fn activate(state: &mut UiState, app: &Item, active: bool) {
    if app.windows.is_empty() {
        state.shared.dock_action = Some(DockAction::Launch(app.command.clone()));
        return;
    }
    let current = &state.shared.snapshot.window_address;
    let address = if active {
        let index = app
            .windows
            .iter()
            .position(|w| &w.address == current)
            .map_or(0, |i| (i + 1) % app.windows.len());
        app.windows[index].address.clone()
    } else {
        state
            .shared
            .last_focused_of(&app.name, &app.windows)
            .unwrap_or_else(|| app.windows[0].address.clone())
    };
    state.shared.dock_action = Some(DockAction::Focus(address));
}

// ── Colocación: una sola fuente para el dibujo y para la zona de entrada ─────

/// Apps como máximo en el desplegable (si hay más, una última fila lo indica).
const MAX_OVERFLOW_ROWS: usize = 10;
/// Ancho del desplegable de apps.
const OVERFLOW_W: f32 = 240.0;

/// Colocación de los elementos de la barra a lo largo de su eje.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// Botones de sistema: Inicio, Buscar y Ver tareas.
    pub tools: [egui::Rect; 3],
    /// Centro del separador entre los botones de sistema y las apps.
    pub separator: egui::Pos2,
    /// Celdas de las apps que caben, en orden.
    pub apps: Vec<egui::Rect>,
    /// Botón «más» si hay apps que no caben (ocupa la celda siguiente a la última app).
    pub more: Option<egui::Rect>,
    /// Lado de cada celda.
    pub item: f32,
}

/// Coloca los botones de sistema, el separador y las apps de `app_count` en `bar`, centrados en
/// su eje y sin pasar del 62 % de su largo. Si no caben todas, la última celda pasa a ser el
/// botón «más».
pub fn plan(cfg: &TaskbarConfig, bar: egui::Rect, app_count: usize) -> Plan {
    let horizontal = cfg.side.is_horizontal();
    let item = cell_size(cfg);
    let slot = item + GAP;
    let axis = if horizontal {
        bar.width()
    } else {
        bar.height()
    };
    let tools_len = 3.0 * slot - GAP;
    let avail = axis * 0.62 - tools_len - SEP;
    // Celdas que caben tras los botones de sistema (cada una ocupa `slot`, la última sin hueco).
    let room = (((avail + GAP) / slot).floor().max(0.0)) as usize;
    let (visible, needs_more) = if app_count <= room {
        (app_count, false)
    } else {
        (room.saturating_sub(1), true)
    };
    let slots = visible + usize::from(needs_more);
    let total = tools_len
        + if slots > 0 {
            SEP + slots as f32 * slot - GAP
        } else {
            0.0
        };
    let center = if horizontal {
        bar.center().x
    } else {
        bar.center().y
    };
    let start = center - total / 2.0;
    let at = |along: f32| cell(bar, horizontal, along, item);
    let first_app = start + tools_len + SEP;
    let sep_along = start + tools_len + SEP / 2.0;
    Plan {
        tools: [at(start), at(start + slot), at(start + 2.0 * slot)],
        separator: if horizontal {
            egui::pos2(sep_along, bar.center().y)
        } else {
            egui::pos2(bar.center().x, sep_along)
        },
        apps: (0..visible)
            .map(|i| at(first_app + i as f32 * slot))
            .collect(),
        more: needs_more.then(|| at(first_app + visible as f32 * slot)),
        item,
    }
}

fn row_height(cfg: &TaskbarConfig) -> f32 {
    (cell_size(cfg) * 0.85).max(34.0)
}

/// Rect del desplegable de las `hidden` apps que no caben, anclado al botón «más» y siempre
/// dentro de la pantalla.
pub fn overflow_card(
    cfg: &TaskbarConfig,
    bar: egui::Rect,
    button: egui::Rect,
    hidden: usize,
    screen: egui::Rect,
) -> egui::Rect {
    let rows = hidden.min(MAX_OVERFLOW_ROWS) + usize::from(hidden > MAX_OVERFLOW_ROWS);
    let (w, h) = (OVERFLOW_W, rows as f32 * row_height(cfg) + 12.0);
    let (x, y) = match cfg.side {
        Side::Bottom => (button.center().x - w / 2.0, bar.min.y - h - 6.0),
        Side::Top => (button.center().x - w / 2.0, bar.max.y + 6.0),
        Side::Left => (bar.max.x + 6.0, button.center().y - h / 2.0),
        Side::Right => (bar.min.x - w - 6.0, button.center().y - h / 2.0),
    };
    let max_x = (screen.max.x - w - 8.0).max(screen.min.x + 8.0);
    let max_y = (screen.max.y - h - 8.0).max(screen.min.y + 8.0);
    egui::Rect::from_min_size(
        egui::pos2(
            x.clamp(screen.min.x + 8.0, max_x),
            y.clamp(screen.min.y + 8.0, max_y),
        ),
        egui::vec2(w, h),
    )
}

/// Rect de la barra como `egui::Rect`.
fn bar_rect(state: &UiState, screen_w: i32, screen_h: i32) -> egui::Rect {
    let (x, y, w, h) = taskbar_rect(state, screen_w, screen_h);
    egui::Rect::from_min_size(
        egui::pos2(x as f32, y as f32),
        egui::vec2(w as f32, h as f32),
    )
}

/// ¿Está abierto el desplegable de apps? Entonces toda la pantalla recibe el clic (un clic fuera
/// lo cierra), igual que los desplegables del topbar.
pub fn overflow_active(state: &UiState) -> bool {
    state.taskbar.enabled && state.taskbar.progress > 0.5 && state.taskbar.overflow_open
}

/// Descripción al pasar el cursor por una app: nombre y, si hay varias, cuántas ventanas.
fn app_tip(app: &Item) -> String {
    match app.windows.len() {
        0 | 1 => app.name.clone(),
        n => format!("{} · {n} ventanas", app.name),
    }
}

/// Dibuja la taskbar. El orden de dibujo es: fondo, botones, apps, bandeja y desplegable.
pub(super) fn draw_taskbar(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    if !state.taskbar.enabled || state.taskbar.progress <= 0.01 {
        return;
    }
    let cfg = state.shared.config.taskbar;
    let horizontal = cfg.side.is_horizontal();
    let screen = ui.max_rect();
    let bar = bar_rect(state, screen.width() as i32, screen.height() as i32);

    // Con el desplegable abierto, un clic fuera lo cierra. Se registra antes que todo lo demás:
    // los botones y la tarjeta, registrados después, ganan sobre él.
    if state.taskbar.overflow_open {
        let outside = ui.interact(
            screen,
            ui.scope_id().with("tb-outside"),
            egui::Sense::click(),
        );
        if outside.clicked() {
            state.taskbar.overflow_open = false;
        }
    }

    ui.painter().rect(
        bar,
        CornerRadius::same(Radius::of(Radius::NONE)),
        palette.surface,
        Stroke::new(1.0, palette.border),
        StrokeKind::Middle,
    );

    let apps = items(state);
    let layout = plan(&cfg, bar, apps.len());
    let item = layout.item;
    let font = item * 0.5;

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
    for ((glyph, id, tip, commands), rect) in tools.into_iter().zip(layout.tools) {
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

    // Separador fino entre los botones de sistema y las apps (como en el mockup).
    let half = item * 0.3;
    let (a, b) = if horizontal {
        (
            egui::pos2(layout.separator.x, layout.separator.y - half),
            egui::pos2(layout.separator.x, layout.separator.y + half),
        )
    } else {
        (
            egui::pos2(layout.separator.x - half, layout.separator.y),
            egui::pos2(layout.separator.x + half, layout.separator.y),
        )
    };
    ui.painter()
        .line_segment([a, b], Stroke::new(1.0, palette.border));

    for (i, (app, rect)) in apps.iter().zip(layout.apps.iter().copied()).enumerate() {
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
            CornerRadius::same(Radius::of(Radius::MD)),
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
                CornerRadius::same(Radius::of(Radius::NONE)),
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
        // Clic central: otra instancia (como en el dock).
        let middle = response.middle_clicked();
        let _ = response.on_hover_text(app_tip(app));
        if clicked {
            activate(state, app, active);
        } else if middle {
            state.shared.dock_action = Some(DockAction::Launch(app.command.clone()));
        }
    }

    // Botón «más»: las apps que no caben salen en un desplegable.
    let hidden = &apps[layout.apps.len()..];
    if let Some(more) = layout.more {
        let tip = match hidden.len() {
            1 => "1 app más".to_string(),
            n => format!("{n} apps más"),
        };
        if tool_slot(
            ui,
            more,
            "tb-more",
            ic::DOTS_THREE,
            palette.text,
            &tip,
            palette,
        ) {
            state.taskbar.overflow_open = !state.taskbar.overflow_open;
        }
    } else {
        state.taskbar.overflow_open = false;
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
        // De arriba abajo en el mismo orden que en horizontal, con la hora encima.
        let mut y = bar.max.y - 8.0 - tray.len() as f32 * step;
        for (i, icon) in tray.iter().enumerate() {
            let center = egui::pos2(bar.center().x, y + step / 2.0);
            tray_button(ui, center, step, ("tb-tray", i), icon, palette);
            y += step;
        }
        text_at(
            ui,
            egui::pos2(
                bar.center().x,
                bar.max.y - 8.0 - tray.len() as f32 * step - 8.0,
            ),
            egui::Align2::CENTER_CENTER,
            time,
            TEXT_CAPTION,
            palette.text,
        );
    }

    if state.taskbar.overflow_open {
        if let Some(more) = layout.more {
            draw_overflow(ui, state, palette, bar, more, hidden, screen);
        }
    }
}

/// Desplegable con las apps que no caben: icono, nombre y, si están abiertas, cuántas ventanas.
fn draw_overflow(
    ui: &mut egui::Ui,
    state: &mut UiState,
    palette: Palette,
    bar: egui::Rect,
    button: egui::Rect,
    hidden: &[Item],
    screen: egui::Rect,
) {
    let cfg = state.shared.config.taskbar;
    let card = overflow_card(&cfg, bar, button, hidden.len(), screen);
    drop_shadow(ui, card, 12, 8, 24, 90);
    ui.painter().rect(
        card,
        CornerRadius::same(Radius::of(Radius::CARD)),
        Palette::with_alpha(pal().solid(), 250),
        Stroke::new(1.0, palette.border),
        StrokeKind::Middle,
    );
    // Absorbe el clic en las zonas sin widget: no cuenta como «clic fuera».
    ui.interact(
        card,
        ui.scope_id().with("tb-overflow-card"),
        egui::Sense::click(),
    );
    let row_h = row_height(&cfg);
    for (i, app) in hidden.iter().take(MAX_OVERFLOW_ROWS).enumerate() {
        let row = egui::Rect::from_min_size(
            card.min + egui::vec2(6.0, 6.0 + i as f32 * row_h),
            egui::vec2(card.width() - 12.0, row_h),
        );
        let active = state
            .shared
            .snapshot
            .window_class
            .eq_ignore_ascii_case(&app.name);
        let resp = ui.interact(
            row,
            ui.scope_id().with(("tb-overflow-app", i)),
            egui::Sense::click(),
        );
        ui.painter().rect_filled(
            row,
            CornerRadius::same(Radius::of(Radius::MD)),
            hover_fill(&resp, palette, active),
        );
        let icon_px = row_h * 0.5;
        let icon_c = egui::pos2(row.min.x + row_h * 0.5, row.center().y);
        if let Some(texture) = state.shared.icon(ui.ctx(), &app.name) {
            egui::Image::new(egui::load::SizedTexture::new(
                texture.id(),
                egui::vec2(icon_px, icon_px),
            ))
            .paint_at(
                ui,
                egui::Rect::from_center_size(icon_c, egui::vec2(icon_px, icon_px)),
            );
        } else {
            icon_at(ui, icon_c, app.glyph, icon_px * 0.85, palette.text);
        }
        text_at(
            ui,
            egui::pos2(row.min.x + row_h, row.center().y),
            egui::Align2::LEFT_CENTER,
            truncate(&app.name, 22),
            TEXT_BODY,
            if active { palette.accent } else { palette.text },
        );
        if !app.windows.is_empty() {
            let label = if app.windows.len() > 1 {
                format!("{}", app.windows.len())
            } else {
                String::new()
            };
            ui.painter().circle_filled(
                egui::pos2(row.max.x - 14.0, row.center().y),
                2.2,
                palette.accent,
            );
            if !label.is_empty() {
                text_at(
                    ui,
                    egui::pos2(row.max.x - 24.0, row.center().y),
                    egui::Align2::RIGHT_CENTER,
                    label,
                    TEXT_CAPTION,
                    palette.muted,
                );
            }
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() {
            activate(state, app, active);
            state.taskbar.overflow_open = false;
        }
    }
    if hidden.len() > MAX_OVERFLOW_ROWS {
        text_at(
            ui,
            egui::pos2(
                card.center().x,
                card.min.y + 6.0 + MAX_OVERFLOW_ROWS as f32 * row_h + row_h / 2.0,
            ),
            egui::Align2::CENTER_CENTER,
            format!("+{} más", hidden.len() - MAX_OVERFLOW_ROWS),
            TEXT_CAPTION,
            palette.dim,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(side: Side, size: u32) -> TaskbarConfig {
        TaskbarConfig {
            side,
            size,
            ..TaskbarConfig::default()
        }
    }

    fn bar(side: Side, size: f32) -> egui::Rect {
        let (w, h) = (1366.0, 768.0);
        match side {
            Side::Bottom => {
                egui::Rect::from_min_size(egui::pos2(0.0, h - size), egui::vec2(w, size))
            }
            Side::Top => egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, size)),
            Side::Left => egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(size, h)),
            Side::Right => {
                egui::Rect::from_min_size(egui::pos2(w - size, 0.0), egui::vec2(size, h))
            }
        }
    }

    #[test]
    fn un_clic_vuelve_a_la_ventana_mas_reciente_de_la_app() {
        let mut state = UiState::new_preview();
        let win = |address: &str| crate::ipc::ClientInfo {
            class: "kitty".into(),
            title: address.into(),
            address: address.into(),
            mapped: true,
            workspace: 1,
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        };
        let app = Item {
            name: "kitty".into(),
            glyph: "",
            command: "kitty".into(),
            windows: vec![win("a"), win("b")],
        };
        state.shared.touch_window("a");
        state.shared.touch_window("b");
        activate(&mut state, &app, false);
        match state.shared.dock_action.take() {
            Some(DockAction::Focus(address)) => assert_eq!(address, "b"),
            other => panic!("esperaba Focus(b), got {other:?}"),
        }
    }

    #[test]
    fn few_apps_fit_without_a_more_button_and_are_centered() {
        let c = cfg(Side::Bottom, 48);
        let b = bar(Side::Bottom, 48.0);
        let p = plan(&c, b, 4);
        assert_eq!(p.apps.len(), 4);
        assert!(p.more.is_none());
        // Centrado: el bloque (primer botón a última app) es simétrico respecto al centro.
        let left = p.tools[0].min.x;
        let right = p.apps.last().unwrap().max.x;
        assert!(((left + right) / 2.0 - b.center().x).abs() < 0.5);
        // Las celdas están dentro de la barra y no se solapan.
        for pair in p.apps.windows(2) {
            assert!(pair[0].max.x <= pair[1].min.x);
        }
        assert!(p.apps.iter().all(|r| b.contains_rect(*r)));
    }

    #[test]
    fn without_apps_only_the_tool_buttons_are_laid_out() {
        let p = plan(&cfg(Side::Bottom, 48), bar(Side::Bottom, 48.0), 0);
        assert!(p.apps.is_empty() && p.more.is_none());
        assert!(p.tools[0].max.x <= p.tools[1].min.x && p.tools[1].max.x <= p.tools[2].min.x);
    }

    #[test]
    fn many_apps_overflow_into_a_more_button_that_fits() {
        let c = cfg(Side::Bottom, 48);
        let b = bar(Side::Bottom, 48.0);
        let p = plan(&c, b, 60);
        let more = p.more.expect("sobran apps: debe haber botón «más»");
        assert!(p.apps.len() < 60);
        // El botón «más» sigue a la última app, dentro de la barra y del 62 % del ancho.
        assert!(more.min.x >= p.apps.last().unwrap().max.x);
        assert!(b.contains_rect(more));
        assert!(more.max.x - p.tools[0].min.x <= b.width() * 0.62 + 0.5);
        // Y siguen centradas.
        assert!(((p.tools[0].min.x + more.max.x) / 2.0 - b.center().x).abs() < 0.5);
    }

    #[test]
    fn layout_follows_the_axis_on_every_side() {
        for side in [Side::Bottom, Side::Top, Side::Left, Side::Right] {
            let c = cfg(side, 48);
            let b = bar(side, 48.0);
            let p = plan(&c, b, 6);
            let horizontal = side.is_horizontal();
            for r in p.tools.iter().chain(p.apps.iter()) {
                assert!(b.contains_rect(*r), "{side:?}: {r:?} fuera de {b:?}");
                // Las celdas están centradas en el eje corto.
                let off = if horizontal {
                    r.center().y - b.center().y
                } else {
                    r.center().x - b.center().x
                };
                assert!(off.abs() < 0.5);
            }
            // El separador queda entre los botones de sistema y la primera app.
            let (sep, last_tool, first_app) = if horizontal {
                (p.separator.x, p.tools[2].max.x, p.apps[0].min.x)
            } else {
                (p.separator.y, p.tools[2].max.y, p.apps[0].min.y)
            };
            assert!(last_tool < sep && sep < first_app, "{side:?}");
        }
    }

    #[test]
    fn overflow_card_stays_on_screen_and_next_to_the_bar() {
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1366.0, 768.0));
        for side in [Side::Bottom, Side::Top, Side::Left, Side::Right] {
            let c = cfg(side, 48);
            let b = bar(side, 48.0);
            let p = plan(&c, b, 60);
            let more = p.more.unwrap();
            for hidden in [1, 5, 10, 40] {
                let card = overflow_card(&c, b, more, hidden, screen);
                assert!(
                    screen.contains_rect(card),
                    "{side:?} con {hidden}: {card:?}"
                );
                // No tapa la barra.
                assert!(!card.intersects(b), "{side:?}: {card:?} solapa {b:?}");
            }
        }
        // Con 40 ocultas se limita a 10 filas + la de «+N más».
        let c = cfg(Side::Bottom, 48);
        let b = bar(Side::Bottom, 48.0);
        let more = plan(&c, b, 60).more.unwrap();
        let card = overflow_card(&c, b, more, 40, screen);
        assert!((card.height() - (11.0 * row_height(&c) + 12.0)).abs() < 0.5);
    }

    #[test]
    fn app_tip_mentions_window_count_only_when_there_are_several() {
        let item = |windows: usize| Item {
            name: "firefox".into(),
            glyph: "",
            command: "firefox".into(),
            windows: (0..windows)
                .map(|i| crate::ipc::ClientInfo {
                    class: "firefox".into(),
                    title: String::new(),
                    address: format!("0x{i}"),
                    mapped: true,
                    workspace: 1,
                    x: 0,
                    y: 0,
                    w: 1,
                    h: 1,
                })
                .collect(),
        };
        assert_eq!(app_tip(&item(0)), "firefox");
        assert_eq!(app_tip(&item(1)), "firefox");
        assert_eq!(app_tip(&item(3)), "firefox · 3 ventanas");
    }
}
