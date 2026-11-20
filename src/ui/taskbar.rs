//! Taskbar estilo Windows, acoplable a los cuatro lados (feature `taskbar`).
//!
//! Reutiliza el modelo del dock (pinned + clientes de `Shared`) y presenta una
//! vista horizontal o vertical con iconos centrados, bandeja anclada al borde
//! final y autohide al acercar el cursor al lado elegido.

use super::*;
use crate::{config::Side, shared::DockAction};

/// Estado de la taskbar.
#[derive(Default)]
pub struct TaskbarState {
    pub enabled: bool,
    pub progress: f32,
    pub visible: bool,
    pub active: Option<String>,
    pub menu: Option<usize>,
}

impl TaskbarState {
    pub fn new() -> Self {
        let cfg = &config::get().taskbar;
        Self {
            enabled: cfg.enabled,
            progress: if cfg.enabled { 1.0 } else { 0.0 },
            visible: cfg.enabled,
            ..Self::default()
        }
    }
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
    const HOT: i32 = 24;
    match cfg.side {
        Side::Bottom => (0, screen_h - HOT, screen_w, HOT),
        Side::Top => (0, 0, screen_w, HOT),
        Side::Left => (0, 0, HOT, screen_h),
        Side::Right => (screen_w - HOT, 0, HOT, screen_h),
    }
}

struct Item {
    name: String,
    glyph: &'static str,
    command: String,
    windows: Vec<crate::ipc::ClientInfo>,
}

fn make_item(state: &UiState, name: &str, glyph: &'static str) -> Item {
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
        glyph,
        command: name.to_string(),
        windows,
    }
}

fn glyph_for(name: &str) -> &'static str {
    let n = name.to_lowercase();
    if n.contains("code") {
        ic::CODE
    } else if n.contains("kitty") || n.contains("term") {
        ic::TERMINAL
    } else if n.contains("firefox") || n.contains("chrome") {
        ic::GLOBE
    } else if n.contains("spotify") || n.contains("music") {
        ic::MUSIC_NOTES
    } else if n.contains("file") || n.contains("nautilus") {
        ic::FOLDER
    } else {
        ic::APP_WINDOW
    }
}

fn items(state: &UiState) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    for pinned in &state.shared.config.dock.pinned {
        out.push(make_item(state, pinned, glyph_for(pinned)));
    }
    for client in &state.shared.clients {
        let key = client.class.to_lowercase();
        if !out.iter().any(|it| it.name.to_lowercase() == key) {
            out.push(make_item(state, &client.class, glyph_for(&client.class)));
        }
    }
    out
}

/// Dibuja la taskbar. El orden de dibujo es: fondo, iconos, bandeja.
pub(super) fn draw_taskbar(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let enabled = state.taskbar.enabled;
    let progress = state.taskbar.progress;
    let active_override = state.taskbar.active.clone();
    if !enabled || progress <= 0.01 {
        return;
    }
    let cfg = &state.shared.config.taskbar;
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
        Color32::from_rgba_unmultiplied(18, 18, 21, 232),
        Stroke::new(1.0, Color32::from_gray(0x2a)),
        StrokeKind::Middle,
    );

    const ITEM: f32 = 38.0;
    const GAP: f32 = 3.0;
    const SEP: f32 = 9.0;
    let apps = items(state);
    let total = 3.0 * ITEM + SEP + apps.len() as f32 * (ITEM + GAP) - GAP;
    let max_span = if horizontal {
        bar.width() * 0.62
    } else {
        bar.height() * 0.62
    };
    let visible_apps = apps
        .len()
        .min(((max_span - 3.0 * ITEM - SEP) / (ITEM + GAP)).max(0.0) as usize);

    let mut cursor = if horizontal {
        bar.center().x - total / 2.0
    } else {
        bar.center().y - total / 2.0
    };

    let mut slot = |glyph: &'static str, id: (&'static str, usize), tip: &str| {
        let rect = if horizontal {
            egui::Rect::from_min_size(
                egui::pos2(cursor, bar.center().y - ITEM / 2.0),
                egui::vec2(ITEM, ITEM),
            )
        } else {
            egui::Rect::from_min_size(
                egui::pos2(bar.center().x - ITEM / 2.0, cursor),
                egui::vec2(ITEM, ITEM),
            )
        };
        cursor += ITEM + GAP;
        let response = ui.interact(rect, ui.scope_id().with(id), egui::Sense::click());
        let color = if glyph == ic::SQUARES_FOUR {
            Color32::from_rgb(0x7f, 0xc8, 0xff)
        } else {
            palette.text
        };
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let _ = response.on_hover_text(tip.to_string());
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            icon_font(20.0),
            color,
        );
        rect
    };

    slot(ic::SQUARES_FOUR, ("tb-start", 0), "Inicio");
    slot(ic::MAGNIFYING_GLASS, ("tb-search", 0), "Buscar");
    slot(ic::APP_WINDOW, ("tb-tasks", 0), "Ver tareas");
    cursor += SEP - GAP;

    for (i, item) in apps.iter().take(visible_apps).enumerate() {
        let rect = if horizontal {
            egui::Rect::from_min_size(
                egui::pos2(cursor, bar.center().y - ITEM / 2.0),
                egui::vec2(ITEM, ITEM),
            )
        } else {
            egui::Rect::from_min_size(
                egui::pos2(bar.center().x - ITEM / 2.0, cursor),
                egui::vec2(ITEM, ITEM),
            )
        };
        cursor += ITEM + GAP;
        let active = active_override
            .as_deref()
            .map(|a| a.eq_ignore_ascii_case(&item.name))
            .unwrap_or_else(|| {
                state
                    .shared
                    .snapshot
                    .window_class
                    .eq_ignore_ascii_case(&item.name)
            });
        let running = !item.windows.is_empty();
        let response = ui.interact(
            rect,
            ui.scope_id().with(("tb-app", i)),
            egui::Sense::click(),
        );
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(9),
            if active {
                Color32::from_rgba_unmultiplied(
                    palette.accent.r(),
                    palette.accent.g(),
                    palette.accent.b(),
                    42,
                )
            } else if response.is_pointer_button_down_on() {
                palette.badge
            } else {
                Color32::TRANSPARENT
            },
        );
        let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(22.0, 22.0));
        if let Some(texture) = state.shared.icon(ui.ctx(), &item.name) {
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
                item.glyph,
                icon_font(20.0),
                color,
            );
        }
        // Indicador: barra en el borde de acople, punto si solo está abierta.
        if active || running {
            let dot = if horizontal {
                egui::pos2(rect.center().x, rect.max.y - 1.0)
            } else {
                egui::pos2(rect.max.x - 1.0, rect.center().y)
            };
            ui.painter().circle_filled(
                dot,
                1.8,
                if active {
                    palette.accent
                } else {
                    palette.muted
                },
            );
            if active {
                let mark = if horizontal {
                    egui::Rect::from_min_size(
                        egui::pos2(rect.center().x - 8.0, rect.max.y - 2.0),
                        egui::vec2(16.0, 2.0),
                    )
                } else {
                    egui::Rect::from_min_size(
                        egui::pos2(rect.max.x - 2.0, rect.center().y - 8.0),
                        egui::vec2(2.0, 16.0),
                    )
                };
                ui.painter()
                    .rect_filled(mark, CornerRadius::same(1), palette.accent);
            }
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if response.clicked() {
            if item.windows.is_empty() {
                state.shared.dock_action = Some(DockAction::Launch(item.command.clone()));
                state.taskbar.active = Some(item.name.clone());
            } else {
                state.shared.dock_action = Some(DockAction::Focus(item.windows[0].address.clone()));
                state.taskbar.active = Some(item.name.clone());
            }
        }
        let _ = response.on_hover_text(item.name.clone());
    }

    // ── Bandeja ─────────────────────────────────────────────────────────
    let now = Local::now();
    let time = now.format("%H:%M").to_string();
    let date = now.format("%d %b").to_string();
    if horizontal {
        let mut x = bar.max.x - 8.0;
        let icons = [ic::BELL, ic::BATTERY_HIGH, ic::SPEAKER_HIGH, ic::WIFI_HIGH];
        for glyph in icons.into_iter().rev() {
            let r = egui::Rect::from_center_size(
                egui::pos2(x - 14.0, bar.center().y),
                egui::vec2(18.0, 18.0),
            );
            ui.painter().text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                glyph,
                icon_font(15.0),
                if glyph == ic::BATTERY_HIGH {
                    Color32::from_rgb(0x8b, 0xe0, 0x9b)
                } else {
                    palette.text
                },
            );
            x -= 26.0;
        }
        text_at(
            ui,
            egui::pos2(x - 14.0, bar.center().y - 6.0),
            egui::Align2::RIGHT_CENTER,
            time,
            TEXT_BODY,
            palette.text,
        );
        text_at(
            ui,
            egui::pos2(x - 14.0, bar.center().y + 7.0),
            egui::Align2::RIGHT_CENTER,
            date,
            TEXT_CAPTION,
            palette.muted,
        );
    } else {
        let mut y = bar.max.y - 8.0;
        for glyph in [ic::BELL, ic::BATTERY_HIGH, ic::SPEAKER_HIGH, ic::WIFI_HIGH] {
            let r = egui::Rect::from_center_size(
                egui::pos2(bar.center().x, y - 12.0),
                egui::vec2(18.0, 18.0),
            );
            ui.painter().text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                glyph,
                icon_font(14.0),
                palette.text,
            );
            y -= 24.0;
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
