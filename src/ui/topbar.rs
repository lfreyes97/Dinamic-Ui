//! Topbar estilo GNOME (feature `topbar`): Actividades, reloj, estado y
//! desplegables de ajustes rápidos y calendario.

use super::*;
use chrono::{Datelike, Local, NaiveDate};

/// Estado de la topbar y sus desplegables.
#[derive(Default)]
pub struct TopbarState {
    pub enabled: bool,
    pub quick_open: bool,
    pub calendar_open: bool,
    pub toggles: [bool; 4],
    pub volume: u8,
    pub brightness: u8,
    pub selected_day: Option<u32>,
}

impl TopbarState {
    pub fn new() -> Self {
        Self {
            enabled: config::get().topbar.enabled,
            toggles: [true, true, false, false],
            volume: 64,
            brightness: 80,
            ..Self::default()
        }
    }
}

/// Rect de la franja superior.
pub fn topbar_rect(state: &UiState, screen_w: i32) -> (i32, i32, i32, i32) {
    let h = state.shared.config.topbar.height as i32;
    (0, 0, screen_w, h)
}

fn dropdown_card(ui: &egui::Ui, rect: egui::Rect) {
    ui.painter().rect(
        rect,
        CornerRadius::same(14),
        Palette::with_alpha(pal().raised, 252),
        Stroke::new(1.0, pal().border),
        StrokeKind::Middle,
    );
}

/// Dibuja la topbar y los desplegables abiertos.
pub(super) fn draw_topbar(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    if !state.topbar.enabled {
        return;
    }
    let screen = ui.max_rect();
    let h = state.shared.config.topbar.height as f32;
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
    // ── Izquierda: Activities + app enfocada ───────────────────────────
    let act = egui::Rect::from_min_size(
        egui::pos2(bar.min.x + 10.0, cy - 12.0),
        egui::vec2(104.0, 24.0),
    );
    let act_response = ui.interact(
        act,
        ui.scope_id().with("tb-activities"),
        egui::Sense::click(),
    );
    if act_response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    ui.painter().text(
        egui::pos2(act.min.x + 10.0, cy),
        egui::Align2::CENTER_CENTER,
        ic::SQUARES_FOUR,
        icon_font(13.0),
        palette.text,
    );
    text_at(
        ui,
        egui::pos2(act.min.x + 22.0, cy),
        egui::Align2::LEFT_CENTER,
        "Actividades",
        TEXT_BODY,
        palette.text,
    );
    if act_response.clicked() {
        state.topbar.quick_open = false;
        state.topbar.calendar_open = false;
        #[cfg(feature = "overview")]
        {
            state.overview.overview = true;
        }
    }
    let focus = state.shared.snapshot.window_title.clone();
    if !focus.is_empty() {
        let focus = truncate(&focus, 24);
        text_at(
            ui,
            egui::pos2(act.max.x + 16.0, cy),
            egui::Align2::LEFT_CENTER,
            focus,
            TEXT_BODY,
            palette.muted,
        );
    }

    // ── Centro: reloj ───────────────────────────────────────────────────
    let now = Local::now();
    let date = now.format("%a %d %b").to_string();
    let time = now.format("%H:%M").to_string();
    let clock =
        egui::Rect::from_center_size(egui::pos2(bar.center().x, cy), egui::vec2(120.0, 24.0));
    let clock_response = ui.interact(clock, ui.scope_id().with("tb-clock"), egui::Sense::click());
    if clock_response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    text_at(
        ui,
        egui::pos2(clock.center().x - 4.0, cy),
        egui::Align2::RIGHT_CENTER,
        date,
        TEXT_CAPTION,
        palette.muted,
    );
    text_at(
        ui,
        egui::pos2(clock.center().x + 4.0, cy),
        egui::Align2::LEFT_CENTER,
        time,
        TEXT_BODY,
        palette.text,
    );
    if clock_response.clicked() {
        state.topbar.calendar_open = !state.topbar.calendar_open;
        state.topbar.quick_open = false;
    }

    // ── Derecha: estado ─────────────────────────────────────────────────
    let status = egui::Rect::from_min_size(
        egui::pos2(bar.max.x - 110.0, cy - 12.0),
        egui::vec2(100.0, 24.0),
    );
    let status_response = ui.interact(
        status,
        ui.scope_id().with("tb-status"),
        egui::Sense::click(),
    );
    if status_response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    icon_at(
        ui,
        egui::pos2(status.min.x + 10.0, cy),
        ic::WIFI_HIGH,
        ICON_SM,
        palette.text,
    );
    icon_at(
        ui,
        egui::pos2(status.min.x + 30.0, cy),
        ic::SPEAKER_HIGH,
        ICON_SM,
        palette.text,
    );
    icon_at(
        ui,
        egui::pos2(status.min.x + 50.0, cy),
        ic::BATTERY_HIGH,
        ICON_SM,
        battery_tone(state.shared.battery.percent, state.shared.battery.charging),
    );
    text_at(
        ui,
        egui::pos2(status.min.x + 66.0, cy),
        egui::Align2::LEFT_CENTER,
        format!("{}%", state.topbar.volume),
        TEXT_CAPTION,
        palette.muted,
    );
    icon_at(
        ui,
        egui::pos2(status.max.x - 8.0, cy),
        if state.topbar.quick_open {
            ic::CARET_LEFT
        } else {
            ic::CARET_RIGHT
        },
        ICON_XS,
        palette.muted,
    );
    if status_response.clicked() {
        state.topbar.quick_open = !state.topbar.quick_open;
        state.topbar.calendar_open = false;
    }

    // ── Desplegable de ajustes rápidos ──────────────────────────────────
    if state.topbar.quick_open {
        let card = egui::Rect::from_min_size(
            egui::pos2(bar.max.x - 292.0, bar.max.y + 6.0),
            egui::vec2(280.0, 218.0),
        );
        dropdown_card(ui, card);
        let t = &mut state.topbar;
        let labels = ["Wi-Fi", "Bluetooth", "Avión", "Nocturno"];
        let glyphs = [ic::WIFI_HIGH, ic::BLUETOOTH, ic::APP_WINDOW, ic::MOON];
        for i in 0..4 {
            let r = egui::Rect::from_min_size(
                egui::pos2(
                    card.min.x + 10.0 + (i % 2) as f32 * 134.0,
                    card.min.y + 34.0 + (i / 2) as f32 * 36.0,
                ),
                egui::vec2(126.0, 30.0),
            );
            let on = t.toggles[i];
            ui.painter().rect_filled(
                r,
                CornerRadius::same(8),
                if on {
                    palette.accent.gamma_multiply(0.16)
                } else {
                    palette.badge
                },
            );
            let resp = ui.interact(
                r,
                ui.scope_id().with(("topbar-toggle", i)),
                egui::Sense::click(),
            );
            if resp.clicked() {
                t.toggles[i] = !t.toggles[i];
            }
            icon_at(
                ui,
                egui::pos2(r.min.x + 16.0, r.center().y),
                glyphs[i],
                ICON_SM,
                if on { palette.accent } else { palette.muted },
            );
            text_at(
                ui,
                egui::pos2(r.min.x + 30.0, r.center().y),
                egui::Align2::LEFT_CENTER,
                labels[i],
                TEXT_CAPTION,
                if on { palette.text } else { palette.muted },
            );
        }
        let mut y = card.min.y + 112.0;
        for (label, value, fill) in [
            ("Volumen", t.volume, palette.accent),
            ("Brillo", t.brightness, palette.text),
        ] {
            text_at(
                ui,
                egui::pos2(card.min.x + 12.0, y + 5.0),
                egui::Align2::LEFT_CENTER,
                label,
                TEXT_CAPTION,
                palette.muted,
            );
            text_at(
                ui,
                egui::pos2(card.max.x - 12.0, y + 5.0),
                egui::Align2::RIGHT_CENTER,
                format!("{value}%"),
                TEXT_CAPTION,
                palette.text,
            );
            bar_at(
                ui,
                egui::Rect::from_min_size(
                    egui::pos2(card.min.x + 12.0, y + 14.0),
                    egui::vec2(card.width() - 24.0, 5.0),
                ),
                value as f32 / 100.0,
                palette.track,
                fill,
            );
            y += 36.0;
        }
    }

    // ── Desplegable del calendario ──────────────────────────────────────
    if state.topbar.calendar_open {
        let card = egui::Rect::from_min_size(
            egui::pos2(bar.center().x - 150.0, bar.max.y + 6.0),
            egui::vec2(300.0, 224.0),
        );
        dropdown_card(ui, card);
        let year = now.year();
        let month = now.month();
        let month_name = now.format("%B %Y").to_string();
        text_at(
            ui,
            egui::pos2(card.min.x + 12.0, card.min.y + 16.0),
            egui::Align2::LEFT_CENTER,
            month_name,
            TEXT_BODY,
            palette.text,
        );
        let first =
            NaiveDate::from_ymd_opt(year, month, 1).unwrap_or_else(|| Local::now().date_naive());
        let offset = first.weekday().num_days_from_monday() as usize;
        let days = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            _ => {
                if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                    29
                } else {
                    28
                }
            }
        };
        let cw = (card.width() - 24.0) / 7.0;
        for (i, wd) in ["L", "M", "X", "J", "V", "S", "D"].iter().enumerate() {
            text_at(
                ui,
                egui::pos2(card.min.x + 12.0 + cw * (i as f32 + 0.5), card.min.y + 34.0),
                egui::Align2::CENTER_CENTER,
                *wd,
                TEXT_CAPTION,
                palette.dim,
            );
        }
        let today = now.day();
        for day in 1..=days {
            let idx = offset + day as usize - 1;
            let col = idx % 7;
            let row = idx / 7;
            let center = egui::pos2(
                card.min.x + 12.0 + cw * (col as f32 + 0.5),
                card.min.y + 52.0 + row as f32 * 24.0 + 10.0,
            );
            let selected = state.topbar.selected_day == Some(day);
            if selected || day == today {
                ui.painter().circle_filled(
                    center,
                    10.0,
                    if selected {
                        palette.accent
                    } else {
                        palette.accent.gamma_multiply(0.25)
                    },
                );
            }
            let resp = ui.interact(
                egui::Rect::from_center_size(center, egui::vec2(22.0, 22.0)),
                ui.scope_id().with(("topbar-day", day)),
                egui::Sense::click(),
            );
            if resp.clicked() {
                state.topbar.selected_day = Some(day);
            }
            text_at(
                ui,
                center,
                egui::Align2::CENTER_CENTER,
                day.to_string(),
                TEXT_CAPTION,
                if selected || day == today {
                    palette.solid()
                } else {
                    palette.text
                },
            );
        }
    }
}
