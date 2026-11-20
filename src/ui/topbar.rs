//! Topbar estilo GNOME (feature `topbar`): Actividades, los workspaces y la app enfocada a la
//! izquierda, reloj en el centro y estado a la derecha. El reloj abre el calendario (con las
//! notificaciones debajo) y el estado abre un resumen del sistema, cada uno anclado bajo su botón.
//!
//! Los datos son los reales de `Shared` (`sysinfo` y batería). El resumen del sistema es de
//! **solo lectura**: actuar sobre wifi, volumen o brillo exige lanzar comandos del sistema
//! (`nmcli`, `wpctl`, `brightnessctl`…) y se añadirá cuando pueda comprobarse en la máquina.

use chrono::{Datelike, Local, NaiveDate};

use super::panel::{days_in_month, format_minutes, MONTHS_LONG};
use super::*;

/// Estado de la topbar y sus desplegables.
#[derive(Default)]
pub struct TopbarState {
    pub enabled: bool,
    pub quick_open: bool,
    pub calendar_open: bool,
    /// Meses respecto al actual que muestra el calendario (0 = este mes).
    pub month_offset: i32,
    pub selected: Option<NaiveDate>,
}

impl TopbarState {
    pub fn new() -> Self {
        Self {
            enabled: config::get().topbar.enabled,
            ..Self::default()
        }
    }
}

// ── Geometría: una sola fuente para el dibujo y para el `input_region` ─────

const POPOVER_W: f32 = 300.0;
const POPOVER_GAP: f32 = 6.0;
const EDGE: f32 = 12.0;

fn bar_height(state: &UiState) -> f32 {
    state.shared.config.topbar.height as f32
}

/// Rect de la franja superior.
pub fn topbar_rect(state: &UiState, screen_w: i32) -> (i32, i32, i32, i32) {
    (0, 0, screen_w, bar_height(state) as i32)
}

fn as_ints(r: egui::Rect) -> (i32, i32, i32, i32) {
    (
        r.min.x.round() as i32,
        r.min.y.round() as i32,
        r.width().round() as i32,
        r.height().round() as i32,
    )
}

/// Líneas de pie del resumen del sistema (salida de audio y batería).
fn quick_footer_lines(state: &UiState) -> usize {
    usize::from(state.shared.system.audio_output.is_some())
        + usize::from(state.shared.battery.present)
}

fn quick_card(state: &UiState, screen_w: f32) -> egui::Rect {
    let lines = quick_footer_lines(state);
    let footer = if lines > 0 {
        4.0 + 18.0 * lines as f32
    } else {
        0.0
    };
    egui::Rect::from_min_size(
        egui::pos2(screen_w - POPOVER_W - EDGE, bar_height(state) + POPOVER_GAP),
        egui::vec2(POPOVER_W, 168.0 + footer),
    )
}

/// Rect del resumen del sistema (anclado a la derecha).
pub fn quick_rect(state: &UiState, screen_w: i32) -> (i32, i32, i32, i32) {
    as_ints(quick_card(state, screen_w as f32))
}

/// Mes visible del calendario: `(año, mes)` desplazado `delta` meses.
pub fn shift_month(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let index = year * 12 + month as i32 - 1 + delta;
    (index.div_euclid(12), index.rem_euclid(12) as u32 + 1)
}

/// `(hueco inicial, días del mes, filas)` de la cuadrícula de un mes (semana desde el lunes).
pub fn calendar_grid(year: i32, month: u32) -> (usize, u32, usize) {
    let offset = NaiveDate::from_ymd_opt(year, month, 1)
        .map(|d| d.weekday().num_days_from_monday() as usize)
        .unwrap_or(0);
    let days = days_in_month(year, month);
    (offset, days, (offset + days as usize).div_ceil(7))
}

/// Workspaces que muestra la barra: los ocupados y el actual, ordenados, hasta 8.
pub fn visible_workspaces(occupied: &[u32], current: u32) -> Vec<u32> {
    let mut ids = occupied.to_vec();
    if !ids.contains(&current) {
        ids.push(current);
    }
    ids.sort_unstable();
    ids.dedup();
    ids.truncate(8);
    ids
}

/// Alto de la sección de notificaciones del calendario (0 si no está compilada).
#[cfg(feature = "notifications")]
fn notifications_height(state: &UiState) -> f32 {
    let n = state.notifications.notification_list.len();
    let body = if n == 0 {
        40.0
    } else {
        n.min(MAX_NOTIFICATIONS) as f32 * 46.0 + if n > MAX_NOTIFICATIONS { 18.0 } else { 0.0 }
    };
    38.0 + body + 10.0
}

#[cfg(not(feature = "notifications"))]
fn notifications_height(_state: &UiState) -> f32 {
    0.0
}

/// Notificaciones que caben en el desplegable (el resto se resume en «+N más»).
#[cfg(feature = "notifications")]
const MAX_NOTIFICATIONS: usize = 3;

fn viewed_month(state: &UiState) -> (i32, u32) {
    let now = Local::now();
    shift_month(now.year(), now.month(), state.topbar.month_offset)
}

fn calendar_card(state: &UiState, screen_w: f32) -> egui::Rect {
    let (year, month) = viewed_month(state);
    let (_, _, rows) = calendar_grid(year, month);
    egui::Rect::from_min_size(
        egui::pos2(
            (screen_w - POPOVER_W) / 2.0,
            bar_height(state) + POPOVER_GAP,
        ),
        egui::vec2(
            POPOVER_W,
            74.0 + rows as f32 * 24.0 + notifications_height(state),
        ),
    )
}

/// Rect del calendario con las notificaciones (anclado al centro). Su alto depende de las
/// semanas del mes y de cuántas notificaciones hay.
pub fn calendar_rect(state: &UiState, screen_w: i32) -> (i32, i32, i32, i32) {
    as_ints(calendar_card(state, screen_w as f32))
}

// ── Dibujo ─────────────────────────────────────────────────────────────────

fn dropdown_card(ui: &egui::Ui, rect: egui::Rect) {
    drop_shadow(ui, rect, 14, 10, 28, 90);
    ui.painter().rect(
        rect,
        CornerRadius::same(14),
        Palette::with_alpha(pal().raised, 252),
        Stroke::new(1.0, pal().border),
        StrokeKind::Middle,
    );
}

/// Fondo de un botón de la barra: resalta al pasar el cursor o mientras su desplegable está abierto.
fn hover_pill(ui: &egui::Ui, rect: egui::Rect, hovered: bool, open: bool) {
    if hovered || open {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(7),
            if open { pal().pressed } else { pal().hover },
        );
    }
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
}

/// Dibuja la topbar y los desplegables abiertos.
pub(super) fn draw_topbar(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    if !state.topbar.enabled {
        return;
    }
    let screen = ui.max_rect();
    let h = bar_height(state);
    let bar = egui::Rect::from_min_size(screen.min, egui::vec2(screen.width(), h));
    ui.painter()
        .rect_filled(bar, CornerRadius::same(0), palette.surface);
    ui.painter().line_segment(
        [
            egui::pos2(bar.min.x, bar.max.y),
            egui::pos2(bar.max.x, bar.max.y),
        ],
        Stroke::new(1.0, palette.border),
    );
    let cy = bar.center().y;

    // ── Izquierda: Actividades + app enfocada ───────────────────────────
    let act = egui::Rect::from_min_size(
        egui::pos2(bar.min.x + 8.0, cy - 12.0),
        egui::vec2(104.0, 24.0),
    );
    let act_response = ui.interact(
        act,
        ui.scope_id().with("tb-activities"),
        egui::Sense::click(),
    );
    hover_pill(ui, act, act_response.hovered(), false);
    icon_at(
        ui,
        egui::pos2(act.min.x + 14.0, cy),
        ic::SQUARES_FOUR,
        13.0,
        palette.text,
    );
    text_at(
        ui,
        egui::pos2(act.min.x + 28.0, cy),
        egui::Align2::LEFT_CENTER,
        "Actividades",
        TEXT_BODY,
        palette.text,
    );
    if act_response.clicked() {
        state.topbar.quick_open = false;
        state.topbar.calendar_open = false;
        // Abre Actividades si está compilada; si no, el overview de workspaces. Va por el
        // registro de comandos: el topbar no conoce el estado de ninguno de los dos.
        if !super::dispatch_command("activities", state) {
            super::dispatch_command("overview", state);
        }
    }

    // Workspaces: puntos, con el actual alargado. Un clic cambia de workspace.
    let current = state.shared.snapshot.workspace.parse::<u32>().unwrap_or(1);
    let mut wx = act.max.x + 12.0;
    for id in visible_workspaces(&state.shared.snapshot.occupied, current) {
        let is_current = id == current;
        let w = if is_current { 22.0 } else { 8.0 };
        let hit =
            egui::Rect::from_min_size(egui::pos2(wx - 4.0, cy - 12.0), egui::vec2(w + 8.0, 24.0));
        let resp = ui
            .interact(hit, ui.scope_id().with(("tb-ws", id)), egui::Sense::click())
            .on_hover_text(format!("Workspace {id}"));
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        ui.painter().rect_filled(
            egui::Rect::from_min_size(egui::pos2(wx, cy - 4.0), egui::vec2(w, 8.0)),
            CornerRadius::same(4),
            if is_current {
                palette.accent
            } else if resp.hovered() {
                palette.text
            } else {
                palette.muted
            },
        );
        if resp.clicked() && !is_current {
            state
                .shared
                .pending_commands
                .push(format!("workspace {id}"));
        }
        wx += w + 6.0;
    }

    let focus = state.shared.snapshot.window_title.clone();
    if !focus.is_empty() {
        text_at(
            ui,
            egui::pos2(wx + 10.0, cy),
            egui::Align2::LEFT_CENTER,
            truncate(&focus, 28),
            TEXT_BODY,
            palette.muted,
        );
    }

    // ── Centro: fecha y hora (abre el calendario) ───────────────────────
    let clock =
        egui::Rect::from_center_size(egui::pos2(bar.center().x, cy), egui::vec2(150.0, 24.0));
    let clock_response = ui.interact(clock, ui.scope_id().with("tb-clock"), egui::Sense::click());
    hover_pill(
        ui,
        clock,
        clock_response.hovered(),
        state.topbar.calendar_open,
    );
    text_at(
        ui,
        egui::pos2(clock.center().x - 4.0, cy),
        egui::Align2::RIGHT_CENTER,
        short_date(),
        TEXT_CAPTION,
        palette.muted,
    );
    text_at(
        ui,
        egui::pos2(clock.center().x + 4.0, cy),
        egui::Align2::LEFT_CENTER,
        Local::now().format("%H:%M").to_string(),
        TEXT_BODY,
        palette.text,
    );
    if clock_response.clicked() {
        state.topbar.calendar_open = !state.topbar.calendar_open;
        state.topbar.quick_open = false;
        state.topbar.month_offset = 0;
    }

    // ── Derecha: wifi · audio · batería (abre el resumen del sistema) ───
    let (wifi_icon, wifi_color, _) = wifi_status(&state.shared.system, palette);
    let (audio_icon, audio_color, _) = audio_status(&state.shared.system, palette);
    let battery = state.shared.battery.clone();
    let battery_color = battery_tone(battery.percent, battery.charging);
    let status_w = 22.0 + 22.0 + if battery.present { 66.0 } else { 0.0 } + 20.0;
    let status = egui::Rect::from_min_size(
        egui::pos2(bar.max.x - status_w - 8.0, cy - 12.0),
        egui::vec2(status_w, 24.0),
    );
    let status_response = ui.interact(
        status,
        ui.scope_id().with("tb-status"),
        egui::Sense::click(),
    );
    hover_pill(
        ui,
        status,
        status_response.hovered(),
        state.topbar.quick_open,
    );
    let mut x = status.min.x + 11.0;
    icon_at(ui, egui::pos2(x, cy), wifi_icon, ICON_SM, wifi_color);
    x += 22.0;
    icon_at(ui, egui::pos2(x, cy), audio_icon, ICON_SM, audio_color);
    x += 11.0;
    if battery.present {
        // Icono por nivel (sin el halo del glifo de la barra, que aquí pesa demasiado).
        let glyph = match battery.percent {
            _ if battery.charging => ic::BATTERY_CHARGING,
            85..=100 => ic::BATTERY_FULL,
            60..=84 => ic::BATTERY_HIGH,
            35..=59 => ic::BATTERY_MEDIUM,
            10..=34 => ic::BATTERY_LOW,
            _ => ic::BATTERY_EMPTY,
        };
        icon_at(ui, egui::pos2(x + 15.0, cy), glyph, ICON_SM, battery_color);
        text_at(
            ui,
            egui::pos2(x + 32.0, cy),
            egui::Align2::LEFT_CENTER,
            format!("{}%", battery.percent),
            TEXT_CAPTION,
            palette.text,
        );
    }
    icon_at(
        ui,
        egui::pos2(status.max.x - 10.0, cy),
        ic::CARET_DOWN,
        ICON_XS,
        palette.muted,
    );
    if status_response.clicked() {
        state.topbar.quick_open = !state.topbar.quick_open;
        state.topbar.calendar_open = false;
    }

    if state.topbar.quick_open {
        draw_quick(ui, state, palette);
    }
    if state.topbar.calendar_open {
        draw_calendar(ui, state, palette);
    }
}

/// Casilla de estado (icono, título y subtítulo) del resumen del sistema.
fn status_tile(
    ui: &egui::Ui,
    rect: egui::Rect,
    glyph: &str,
    glyph_color: Color32,
    title: &str,
    subtitle: String,
    on: bool,
) {
    let p = pal();
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(10),
        if on { p.accent_soft } else { p.badge },
    );
    icon_at(
        ui,
        egui::pos2(rect.min.x + 20.0, rect.center().y),
        glyph,
        ICON_MD,
        glyph_color,
    );
    text_at(
        ui,
        egui::pos2(rect.min.x + 38.0, rect.center().y - 7.0),
        egui::Align2::LEFT_CENTER,
        title,
        TEXT_BODY,
        if on { p.text } else { p.muted },
    );
    text_at(
        ui,
        egui::pos2(rect.min.x + 38.0, rect.center().y + 8.0),
        egui::Align2::LEFT_CENTER,
        subtitle,
        TEXT_CAPTION,
        p.muted,
    );
}

/// Resumen del sistema (solo lectura) con los datos reales.
fn draw_quick(ui: &mut egui::Ui, state: &UiState, palette: Palette) {
    let card = quick_card(state, ui.max_rect().width());
    dropdown_card(ui, card);
    text_at(
        ui,
        egui::pos2(card.min.x + 16.0, card.min.y + 17.0),
        egui::Align2::LEFT_CENTER,
        "Sistema",
        TEXT_CAPTION,
        palette.dim,
    );

    let system = &state.shared.system;
    let tile_w = (card.width() - 32.0 - 8.0) / 2.0;
    let tile = |col: f32| {
        egui::Rect::from_min_size(
            egui::pos2(card.min.x + 16.0 + col * (tile_w + 8.0), card.min.y + 30.0),
            egui::vec2(tile_w, 44.0),
        )
    };
    let (wifi_icon, wifi_color, _) = wifi_status(system, palette);
    let (wifi_sub, wifi_on) = match &system.wifi {
        None => ("Sin datos".to_string(), false),
        Some(w) if !w.enabled => ("Desactivado".to_string(), false),
        Some(w) => match &w.ssid {
            Some(ssid) => (truncate(ssid, 14), true),
            None => ("Sin conexión".to_string(), false),
        },
    };
    status_tile(
        ui,
        tile(0.0),
        wifi_icon,
        wifi_color,
        "Wi-Fi",
        wifi_sub,
        wifi_on,
    );

    let (bt_icon, bt_sub, bt_on) = match &system.bluetooth {
        None => (ic::BLUETOOTH, "Sin datos".to_string(), false),
        Some(b) if !b.powered => (ic::BLUETOOTH_SLASH, "Apagado".to_string(), false),
        Some(b) if b.connected == 0 => (ic::BLUETOOTH, "Activado".to_string(), true),
        Some(b) => (
            ic::BLUETOOTH_CONNECTED,
            format!(
                "{} conectado{}",
                b.connected,
                if b.connected == 1 { "" } else { "s" }
            ),
            true,
        ),
    };
    status_tile(
        ui,
        tile(1.0),
        bt_icon,
        if bt_on { palette.accent } else { palette.muted },
        "Bluetooth",
        bt_sub,
        bt_on,
    );

    // Volumen y brillo: barras con el valor real (o «—» si no hay dato).
    let volume = system.volume.as_ref();
    let rows = [
        (
            "Volumen",
            match volume {
                Some(v) if v.muted => "Silenciado".to_string(),
                Some(v) => format!("{}%", v.percent),
                None => "—".to_string(),
            },
            volume.map_or(0.0, |v| {
                if v.muted {
                    0.0
                } else {
                    v.percent as f32 / 100.0
                }
            }),
            palette.accent,
        ),
        (
            "Brillo",
            system
                .brightness
                .map_or("—".to_string(), |b| format!("{b}%")),
            system.brightness.map_or(0.0, |b| b as f32 / 100.0),
            palette.text,
        ),
    ];
    let mut y = card.min.y + 88.0;
    for (label, value, fraction, fill) in rows {
        text_at(
            ui,
            egui::pos2(card.min.x + 16.0, y + 5.0),
            egui::Align2::LEFT_CENTER,
            label,
            TEXT_CAPTION,
            palette.muted,
        );
        text_at(
            ui,
            egui::pos2(card.max.x - 16.0, y + 5.0),
            egui::Align2::RIGHT_CENTER,
            value,
            TEXT_CAPTION,
            palette.text,
        );
        bar_at(
            ui,
            egui::Rect::from_min_size(
                egui::pos2(card.min.x + 16.0, y + 14.0),
                egui::vec2(card.width() - 32.0, 5.0),
            ),
            fraction,
            palette.track,
            fill,
        );
        y += 34.0;
    }

    // Pie: salida de audio y batería.
    y += 4.0;
    if let Some(output) = &system.audio_output {
        text_at(
            ui,
            egui::pos2(card.min.x + 16.0, y + 7.0),
            egui::Align2::LEFT_CENTER,
            format!("Salida: {}", truncate(output, 34)),
            TEXT_CAPTION,
            palette.muted,
        );
        y += 18.0;
    }
    let battery = &state.shared.battery;
    if battery.present {
        let mut text = format!("Batería: {}%", battery.percent);
        if battery.charging {
            text.push_str(" · cargando");
        }
        if let Some(minutes) = battery.minutes_left {
            text.push_str(&format!(" · {}", format_minutes(minutes)));
        }
        text_at(
            ui,
            egui::pos2(card.min.x + 16.0, y + 7.0),
            egui::Align2::LEFT_CENTER,
            text,
            TEXT_CAPTION,
            palette.muted,
        );
    }
}

/// Botón cuadrado con borde de la cabecera del calendario.
fn month_button(ui: &mut egui::Ui, rect: egui::Rect, glyph: &str, id: &'static str) -> bool {
    let p = pal();
    let resp = ui.interact(rect, ui.scope_id().with(id), egui::Sense::click());
    ui.painter().rect(
        rect,
        CornerRadius::same(6),
        if resp.hovered() {
            Palette::sheen(22)
        } else {
            Palette::sheen(10)
        },
        Stroke::new(1.0, p.border),
        StrokeKind::Middle,
    );
    icon_at(
        ui,
        rect.center(),
        glyph,
        11.0,
        if resp.hovered() { p.text } else { p.muted },
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// Calendario navegable por meses.
fn draw_calendar(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let card = calendar_card(state, ui.max_rect().width());
    dropdown_card(ui, card);
    let (year, month) = viewed_month(state);
    let (offset, days, _) = calendar_grid(year, month);
    let today = Local::now().date_naive();

    // Cabecera: «Noviembre 2026» (un clic vuelve a hoy) y los botones de mes.
    let name = MONTHS_LONG[(month - 1) as usize];
    let mut chars = name.chars();
    let title = match chars.next() {
        Some(first) => format!("{}{} {year}", first.to_uppercase(), chars.as_str()),
        None => year.to_string(),
    };
    let title_rect = egui::Rect::from_min_size(
        egui::pos2(card.min.x + 10.0, card.min.y + 10.0),
        egui::vec2(170.0, 24.0),
    );
    let title_resp = ui.interact(
        title_rect,
        ui.scope_id().with("tb-month-title"),
        egui::Sense::click(),
    );
    hover_pill(ui, title_rect, title_resp.hovered(), false);
    text_at(
        ui,
        egui::pos2(title_rect.min.x + 6.0, title_rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        TEXT_BODY,
        palette.text,
    );
    if title_resp.clicked() {
        state.topbar.month_offset = 0;
    }
    let next = egui::Rect::from_min_size(
        egui::pos2(card.max.x - 14.0 - 22.0, card.min.y + 11.0),
        egui::vec2(22.0, 22.0),
    );
    let prev = next.translate(egui::vec2(-26.0, 0.0));
    if month_button(ui, prev, ic::CARET_LEFT, "tb-month-prev") {
        state.topbar.month_offset -= 1;
    }
    if month_button(ui, next, ic::CARET_RIGHT, "tb-month-next") {
        state.topbar.month_offset += 1;
    }

    let cw = (card.width() - 28.0) / 7.0;
    for (i, wd) in ["L", "M", "X", "J", "V", "S", "D"].iter().enumerate() {
        text_at(
            ui,
            egui::pos2(card.min.x + 14.0 + cw * (i as f32 + 0.5), card.min.y + 50.0),
            egui::Align2::CENTER_CENTER,
            *wd,
            TEXT_CAPTION,
            palette.dim,
        );
    }
    for day in 1..=days {
        let idx = offset + day as usize - 1;
        let center = egui::pos2(
            card.min.x + 14.0 + cw * ((idx % 7) as f32 + 0.5),
            card.min.y + 74.0 + (idx / 7) as f32 * 24.0,
        );
        let date = NaiveDate::from_ymd_opt(year, month, day);
        let is_today = date == Some(today);
        let selected = date.is_some() && state.topbar.selected == date;
        let resp = ui.interact(
            egui::Rect::from_center_size(center, egui::vec2(24.0, 24.0)),
            ui.scope_id().with(("topbar-day", day)),
            egui::Sense::click(),
        );
        if selected {
            ui.painter().circle_filled(center, 11.0, palette.accent);
        } else if is_today {
            ui.painter()
                .circle_filled(center, 11.0, palette.accent_soft);
        } else if resp.hovered() {
            ui.painter().circle_filled(center, 11.0, palette.hover);
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() {
            state.topbar.selected = date;
        }
        text_at(
            ui,
            center,
            egui::Align2::CENTER_CENTER,
            day.to_string(),
            TEXT_CAPTION,
            if selected {
                palette.solid()
            } else if is_today {
                palette.accent
            } else {
                palette.text
            },
        );
    }

    #[cfg(feature = "notifications")]
    {
        let (_, _, rows) = calendar_grid(year, month);
        let top = card.min.y + 74.0 + rows as f32 * 24.0;
        draw_notifications(ui, state, palette, card, top);
    }
}

/// Notificaciones bajo el calendario: cabecera con «No molestar» y vaciar, y las más recientes.
#[cfg(feature = "notifications")]
fn draw_notifications(
    ui: &mut egui::Ui,
    state: &mut UiState,
    palette: Palette,
    card: egui::Rect,
    top: f32,
) {
    ui.painter().line_segment(
        [
            egui::pos2(card.min.x + 14.0, top + 2.0),
            egui::pos2(card.max.x - 14.0, top + 2.0),
        ],
        Stroke::new(1.0, palette.border),
    );
    let hy = top + 20.0;
    text_at(
        ui,
        egui::pos2(card.min.x + 16.0, hy),
        egui::Align2::LEFT_CENTER,
        "Notificaciones",
        TEXT_BODY,
        palette.text,
    );

    // No molestar: interruptor a la derecha, con su etiqueta.
    let switch = egui::Rect::from_center_size(
        egui::pos2(card.max.x - 16.0 - 15.0, hy),
        egui::vec2(30.0, 17.0),
    );
    let switch_resp = ui
        .interact(switch, ui.scope_id().with("tb-dnd"), egui::Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let dnd = state.notifications.dnd;
    ui.painter().rect_filled(
        switch,
        CornerRadius::same(9),
        if dnd { palette.accent } else { palette.faint },
    );
    ui.painter().circle_filled(
        egui::pos2(
            if dnd {
                switch.max.x - 8.5
            } else {
                switch.min.x + 8.5
            },
            switch.center().y,
        ),
        6.5,
        if dnd {
            palette.solid()
        } else {
            palette.text_hi
        },
    );
    if switch_resp.clicked() {
        state.notifications.toggle_dnd = true;
    }
    text_at(
        ui,
        egui::pos2(switch.min.x - 8.0, hy),
        egui::Align2::RIGHT_CENTER,
        "No molestar",
        TEXT_CAPTION,
        if dnd { palette.text } else { palette.muted },
    );

    let list: Vec<crate::notifications::Notification> =
        state.notifications.notification_list.clone();
    let mut y = top + 38.0;
    if list.is_empty() {
        icon_at(
            ui,
            egui::pos2(card.center().x - 52.0, y + 16.0),
            ic::BELL_SLASH,
            ICON_SM,
            palette.dim,
        );
        text_at(
            ui,
            egui::pos2(card.center().x - 38.0, y + 16.0),
            egui::Align2::LEFT_CENTER,
            "Sin notificaciones",
            TEXT_BODY,
            palette.muted,
        );
        return;
    }
    for notification in list.iter().take(MAX_NOTIFICATIONS) {
        let row = egui::Rect::from_min_size(
            egui::pos2(card.min.x + 12.0, y),
            egui::vec2(card.width() - 24.0, 42.0),
        );
        ui.painter()
            .rect_filled(row, CornerRadius::same(9), palette.badge);
        icon_at(
            ui,
            egui::pos2(row.min.x + 18.0, row.center().y),
            ic::BELL,
            ICON_SM,
            palette.accent,
        );
        text_at(
            ui,
            egui::pos2(row.min.x + 36.0, row.min.y + 13.0),
            egui::Align2::LEFT_CENTER,
            format!(
                "{} · {}",
                truncate(&notification.app, 18),
                notification.time
            ),
            TEXT_CAPTION,
            palette.muted,
        );
        text_at(
            ui,
            egui::pos2(row.min.x + 36.0, row.min.y + 29.0),
            egui::Align2::LEFT_CENTER,
            truncate(&notification.summary, 34),
            TEXT_BODY,
            palette.text,
        );
        // Cerrar esta notificación.
        let close = egui::Rect::from_center_size(
            egui::pos2(row.max.x - 16.0, row.center().y),
            egui::vec2(20.0, 20.0),
        );
        let close_resp = ui
            .interact(
                close,
                ui.scope_id().with(("tb-notif-close", notification.id)),
                egui::Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        icon_at(
            ui,
            close.center(),
            ic::X,
            ICON_XS,
            if close_resp.hovered() {
                palette.text
            } else {
                palette.muted
            },
        );
        if close_resp.clicked() {
            state.notifications.dismiss_notification_id = Some(notification.id);
        }
        y += 46.0;
    }
    if list.len() > MAX_NOTIFICATIONS {
        let more = list.len() - MAX_NOTIFICATIONS;
        let clear = egui::Rect::from_center_size(
            egui::pos2(card.max.x - 56.0, y + 9.0),
            egui::vec2(96.0, 16.0),
        );
        let clear_resp = ui
            .interact(
                clear,
                ui.scope_id().with("tb-notif-clear"),
                egui::Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        text_at(
            ui,
            egui::pos2(card.min.x + 16.0, y + 9.0),
            egui::Align2::LEFT_CENTER,
            format!("+{more} más"),
            TEXT_CAPTION,
            palette.dim,
        );
        text_at(
            ui,
            clear.center(),
            egui::Align2::CENTER_CENTER,
            "Borrar todo",
            TEXT_CAPTION,
            if clear_resp.hovered() {
                palette.text
            } else {
                palette.muted
            },
        );
        if clear_resp.clicked() {
            state.notifications.clear_all_notifications = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_month_wraps_years_in_both_directions() {
        assert_eq!(shift_month(2026, 11, 0), (2026, 11));
        assert_eq!(shift_month(2026, 11, 1), (2026, 12));
        assert_eq!(shift_month(2026, 11, 2), (2027, 1));
        assert_eq!(shift_month(2026, 1, -1), (2025, 12));
        assert_eq!(shift_month(2026, 3, -15), (2024, 12));
        assert_eq!(shift_month(2026, 12, 25), (2029, 1));
    }

    #[test]
    fn workspaces_are_sorted_unique_and_include_the_current_one() {
        assert_eq!(visible_workspaces(&[3, 1, 3], 2), [1, 2, 3]);
        assert_eq!(visible_workspaces(&[], 4), [4]);
        assert_eq!(visible_workspaces(&[1, 2], 2), [1, 2]);
        // Tope de 8 puntos.
        let many: Vec<u32> = (1..=12).collect();
        assert_eq!(visible_workspaces(&many, 1).len(), 8);
    }

    #[test]
    fn calendar_grid_counts_the_weeks_a_month_spans() {
        // Noviembre de 2026 empieza en domingo y tiene 30 días: 6 filas.
        assert_eq!(calendar_grid(2026, 11), (6, 30, 6));
        // Febrero de 2027 empieza en lunes y tiene 28 días: 4 filas exactas.
        assert_eq!(calendar_grid(2027, 2), (0, 28, 4));
        // Febrero de 2026 empieza en domingo: 28 días en 5 filas.
        assert_eq!(calendar_grid(2026, 2), (6, 28, 5));
        // Año bisiesto.
        assert_eq!(calendar_grid(2028, 2).1, 29);
    }
}
