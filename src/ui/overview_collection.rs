//! Vistas del overview que no son la rejilla: Actividades y las franjas acoplables.

use super::overview::{
    self, activity_tiles, current_ws, pointer_moved, strip_ids, OverviewKind, ACTIVITY_COLS,
};
use super::overview_model as model;
use super::*;
use crate::config::Side;

/// Rect de la franja horizontal o vertical. Con `progress < 1` se desliza fuera de pantalla
/// hacia el borde en el que está acoplada.
pub fn strip_rect(state: &UiState, screen_w: i32, screen_h: i32) -> (i32, i32, i32, i32) {
    let hidden = 1.0 - crate::smootherstep(state.overview.progress);
    let slide = |size: i32| (hidden * size as f32).round() as i32;
    match state.overview.kind {
        OverviewKind::StripH => match state.shared.config.overview.strip_h_side {
            Side::Bottom => (0, screen_h - 68 + slide(68), screen_w, 68),
            _ => (0, -slide(68), screen_w, 68),
        },
        OverviewKind::StripV => match state.shared.config.overview.strip_v_side {
            Side::Left => (-slide(78), 0, 78, screen_h),
            _ => (screen_w - 78 + slide(78), 0, 78, screen_h),
        },
        _ => (0, 0, screen_w, screen_h),
    }
}

/// Píldora de búsqueda: muestra lo que se escribe (el teclado lo recoge `overview::handle_key`).
fn search_pill(ui: &egui::Ui, screen: egui::Rect, query: &str, placeholder: &str) {
    let r = egui::Rect::from_center_size(
        egui::pos2(screen.center().x, screen.min.y + 30.0),
        egui::vec2(380.0, 34.0),
    );
    ui.painter().rect(
        r,
        CornerRadius::same(17),
        Palette::with_alpha(pal().solid(), 250),
        Stroke::new(
            1.0,
            if query.is_empty() {
                pal().border
            } else {
                pal().accent
            },
        ),
        StrokeKind::Middle,
    );
    icon_at(
        ui,
        egui::pos2(r.min.x + 16.0, r.center().y),
        ic::MAGNIFYING_GLASS,
        ICON_MD,
        pal().muted,
    );
    let (text, color) = if query.is_empty() {
        (placeholder.to_string(), pal().dim)
    } else {
        (format!("{query}|"), pal().text)
    };
    text_at(
        ui,
        egui::pos2(r.min.x + 30.0, r.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        TEXT_BODY,
        color,
    );
}

/// Overview de actividades: buscador, apps instaladas y tira de ventanas abiertas.
pub(super) fn draw_activities(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let screen = ui.max_rect();
    ui.painter()
        .rect_filled(screen, CornerRadius::same(0), Palette::scrim(205));
    let backdrop = ui.interact(
        screen,
        ui.scope_id().with("ov-act-backdrop"),
        egui::Sense::click(),
    );
    if backdrop.clicked() {
        state.overview.overview = false;
    }

    let query = state.overview.query.clone();
    search_pill(ui, screen, &query, "Busca aplicaciones…");

    let tiles = activity_tiles(state);
    state.overview.selected = state.overview.selected.min(tiles.len().saturating_sub(1));
    let moved = pointer_moved(ui);

    if tiles.is_empty() {
        let message = if query.is_empty() {
            "No hay aplicaciones".to_string()
        } else {
            format!("Sin resultados para «{query}»")
        };
        text_at(
            ui,
            egui::pos2(screen.center().x, screen.min.y + 120.0),
            egui::Align2::CENTER_CENTER,
            message,
            TEXT_BODY,
            palette.muted,
        );
    }

    let cols = ACTIVITY_COLS.min(tiles.len().max(1));
    let cw = 96.0;
    let row_h = 84.0;
    let total_w = cols as f32 * cw;
    let origin = egui::pos2(screen.center().x - total_w / 2.0, screen.min.y + 84.0);
    for (i, tile) in tiles.iter().enumerate() {
        let c = egui::pos2(
            origin.x + (i % cols) as f32 * cw + cw / 2.0,
            origin.y + (i / cols) as f32 * row_h + 25.0,
        );
        let cell = egui::Rect::from_center_size(
            egui::pos2(c.x, c.y + 8.0),
            egui::vec2(cw - 6.0, row_h - 4.0),
        );
        let resp = ui.interact(
            cell,
            ui.scope_id().with(("ov-app", &tile.id)),
            egui::Sense::click(),
        );
        if resp.hovered() && moved {
            state.overview.selected = i;
        }
        let selected = state.overview.selected == i;
        let box_rect = egui::Rect::from_center_size(c, egui::vec2(48.0, 48.0));
        ui.painter().rect(
            box_rect,
            CornerRadius::same(12),
            if selected {
                palette.accent_soft
            } else {
                palette.hover
            },
            Stroke::new(
                if selected { 1.5 } else { 0.0 },
                if selected {
                    palette.accent
                } else {
                    egui::Color32::TRANSPARENT
                },
            ),
            StrokeKind::Middle,
        );
        if let Some(texture) = state.shared.icon(ui.ctx(), &tile.id) {
            egui::Image::new(egui::load::SizedTexture::new(
                texture.id(),
                egui::vec2(26.0, 26.0),
            ))
            .paint_at(ui, egui::Rect::from_center_size(c, egui::vec2(26.0, 26.0)));
        } else {
            icon_at(ui, c, dock_icon(&tile.name), ICON_LG, palette.text);
        }
        // Un punto por ventana abierta (hasta tres).
        let dots = tile.windows.len().min(3);
        for k in 0..dots {
            let dx = (k as f32 - (dots as f32 - 1.0) / 2.0) * 6.0;
            ui.painter().circle_filled(
                egui::pos2(c.x + dx, box_rect.max.y - 5.0),
                1.8,
                palette.accent,
            );
        }
        text_at(
            ui,
            egui::pos2(c.x, c.y + 38.0),
            egui::Align2::CENTER_CENTER,
            truncate(&tile.name, 12),
            TEXT_CAPTION,
            if selected {
                palette.text
            } else {
                palette.muted
            },
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() {
            overview::run_tile(state, tile);
        }
    }

    // Tira de ventanas abiertas: tantas como caben.
    let active = state.shared.snapshot.window_address.clone();
    let fw = 104.0;
    let capacity = (((screen.width() - 120.0) / (fw + 8.0)).floor() as usize).clamp(1, 12);
    let windows: Vec<crate::ipc::ClientInfo> = state
        .shared
        .clients
        .iter()
        .filter(|c| c.mapped)
        .take(capacity)
        .cloned()
        .collect();
    let film_w = windows.len() as f32 * (fw + 8.0) - 8.0;
    let film_y = screen.max.y - 78.0;
    for (i, win) in windows.iter().enumerate() {
        let r = egui::Rect::from_min_size(
            egui::pos2(
                screen.center().x - film_w / 2.0 + i as f32 * (fw + 8.0),
                film_y,
            ),
            egui::vec2(fw, 52.0),
        );
        let resp = ui.interact(
            r,
            ui.scope_id().with(("ov-film", &win.address)),
            egui::Sense::click(),
        );
        let is_active = win.address == active;
        ui.painter().rect(
            r,
            CornerRadius::same(7),
            if resp.hovered() {
                palette.pressed
            } else {
                Palette::with_alpha(palette.hover, 250)
            },
            Stroke::new(
                if is_active { 1.5 } else { 1.0 },
                if is_active {
                    palette.accent
                } else if resp.hovered() {
                    palette.muted
                } else {
                    palette.border
                },
            ),
            StrokeKind::Middle,
        );
        let icon_center = egui::pos2(r.min.x + 17.0, r.min.y + 18.0);
        if let Some(texture) = state.shared.icon(ui.ctx(), &win.class) {
            egui::Image::new(egui::load::SizedTexture::new(
                texture.id(),
                egui::vec2(18.0, 18.0),
            ))
            .paint_at(
                ui,
                egui::Rect::from_center_size(icon_center, egui::vec2(18.0, 18.0)),
            );
        } else {
            icon_at(
                ui,
                icon_center,
                dock_icon(&win.class),
                ICON_SM,
                palette.text,
            );
        }
        text_at(
            ui,
            egui::pos2(r.min.x + 32.0, r.min.y + 18.0),
            egui::Align2::LEFT_CENTER,
            truncate(&win.class, 11),
            TEXT_CAPTION,
            palette.text,
        );
        text_at(
            ui,
            r.min + egui::vec2(8.0, 38.0),
            egui::Align2::LEFT_CENTER,
            truncate(&win.title, 16),
            TEXT_CAPTION,
            palette.muted,
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() {
            state.shared.overview_focus = Some(win.address.clone());
        }
    }
    text_at(
        ui,
        egui::pos2(screen.center().x, screen.max.y - 16.0),
        egui::Align2::CENTER_CENTER,
        "Escribe para buscar  ·  ←→↑↓ navega  ·  ↵ abre o enfoca  ·  Esc limpia / cierra",
        TEXT_CAPTION,
        palette.muted,
    );
}

fn nav_button(ui: &mut egui::Ui, rect: egui::Rect, glyph: &str, id: &'static str) -> bool {
    let resp = ui.interact(rect, ui.scope_id().with(id), egui::Sense::click());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(6),
        if resp.hovered() {
            pal().pressed
        } else {
            pal().hover
        },
    );
    icon_at(ui, rect.center(), glyph, ICON_SM, pal().muted);
    resp.clicked()
}

/// Miniatura de workspace compartida por las franjas. `index` es su posición en la lista
/// completa de workspaces (para la selección del teclado).
fn thumb(
    ui: &mut egui::Ui,
    state: &mut UiState,
    rect: egui::Rect,
    id: u32,
    index: usize,
    palette: Palette,
) {
    let active_ws = id == current_ws(state);
    let resp = ui.interact(
        rect,
        ui.scope_id().with(("ov-strip-ws", id)),
        egui::Sense::click(),
    );
    if resp.hovered() && pointer_moved(ui) {
        state.overview.selected = index;
    }
    let selected = state.overview.selected == index;
    let (width, color) = if active_ws {
        (2.0, palette.accent)
    } else if selected {
        (2.0, palette.text)
    } else if resp.hovered() {
        (1.0, palette.muted)
    } else {
        (1.0, palette.border)
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(6),
        if selected || resp.hovered() {
            palette.pressed
        } else {
            palette.hover
        },
        Stroke::new(width, color),
        StrokeKind::Middle,
    );
    let content = rect.shrink2(egui::vec2(6.0, 12.0));
    let monitor = ui.max_rect().size();
    let active_address = state.shared.snapshot.window_address.clone();
    let windows: Vec<crate::ipc::ClientInfo> = state
        .shared
        .clients
        .iter()
        .filter(|c| c.mapped && c.workspace == id as i32)
        .cloned()
        .collect();
    for j in model::paint_order(&windows, &active_address) {
        let win = &windows[j];
        let r = model::window_rect(win, monitor, content, 4.0);
        if r.width() <= 0.0 || r.height() <= 0.0 {
            continue;
        }
        let is_active = win.address == active_address;
        ui.painter().rect(
            r,
            CornerRadius::same(2),
            palette.badge,
            Stroke::new(
                if is_active { 1.0 } else { 0.0 },
                if is_active {
                    palette.accent
                } else {
                    egui::Color32::TRANSPARENT
                },
            ),
            StrokeKind::Middle,
        );
    }
    text_at(
        ui,
        rect.min + egui::vec2(5.0, rect.height() - 5.0),
        egui::Align2::LEFT_CENTER,
        format!("{id}"),
        TEXT_CAPTION,
        if active_ws {
            palette.accent
        } else {
            palette.muted
        },
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.clicked() {
        state.shared.overview_workspace = Some(id);
        state.overview.overview = false;
    }
}

/// Franja horizontal de workspaces (arriba o abajo).
pub(super) fn draw_strip_h(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let screen = ui.max_rect();
    let (x, y, w, h) = strip_rect(state, screen.width() as i32, screen.height() as i32);
    let strip = egui::Rect::from_min_size(
        egui::pos2(x as f32, y as f32),
        egui::vec2(w as f32, h as f32),
    );
    ui.painter()
        .rect_filled(strip, CornerRadius::same(0), pal().surface);
    let at_top = state.shared.config.overview.strip_h_side != Side::Bottom;
    let edge = if at_top { strip.max.y } else { strip.min.y };
    ui.painter().line_segment(
        [egui::pos2(strip.min.x, edge), egui::pos2(strip.max.x, edge)],
        Stroke::new(1.0, pal().border),
    );

    let all = strip_ids(state);
    let offset = state.overview.strip_offset.min(all.len().saturating_sub(1));
    let visible = all
        .iter()
        .skip(offset)
        .take(5)
        .copied()
        .collect::<Vec<u32>>();
    let tw = 86.0;
    let total = visible.len() as f32 * (tw + 6.0) - 6.0;
    let start = strip.center().x - total / 2.0;
    if nav_button(
        ui,
        egui::Rect::from_center_size(
            egui::pos2(start - 18.0, strip.center().y),
            egui::vec2(20.0, 20.0),
        ),
        ic::CARET_LEFT,
        "ov-h-prev",
    ) {
        state.overview.strip_offset = state.overview.strip_offset.saturating_sub(1);
    }
    for (i, id) in visible.iter().enumerate() {
        thumb(
            ui,
            state,
            egui::Rect::from_min_size(
                egui::pos2(start + i as f32 * (tw + 6.0), strip.center().y - 23.0),
                egui::vec2(tw, 46.0),
            ),
            *id,
            offset + i,
            palette,
        );
    }
    if nav_button(
        ui,
        egui::Rect::from_center_size(
            egui::pos2(start + total + 18.0, strip.center().y),
            egui::vec2(20.0, 20.0),
        ),
        ic::CARET_RIGHT,
        "ov-h-next",
    ) {
        // No avanzar más allá del último grupo visible.
        let max_offset = all.len().saturating_sub(5);
        state.overview.strip_offset = (state.overview.strip_offset + 1).min(max_offset);
    }
    let expand = egui::Rect::from_center_size(
        egui::pos2(strip.max.x - 34.0, strip.center().y),
        egui::vec2(40.0, 30.0),
    );
    let resp = ui.interact(
        expand,
        ui.scope_id().with("ov-h-expand"),
        egui::Sense::click(),
    );
    icon_at(
        ui,
        expand.center(),
        ic::SQUARES_FOUR,
        ICON_MD,
        if resp.hovered() {
            palette.text
        } else {
            palette.muted
        },
    );
    if resp.clicked() {
        overview::open(state, OverviewKind::Workspaces);
    }
}

/// Franja vertical de workspaces (izquierda o derecha).
pub(super) fn draw_strip_v(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let screen = ui.max_rect();
    let (x, y, w, h) = strip_rect(state, screen.width() as i32, screen.height() as i32);
    let strip = egui::Rect::from_min_size(
        egui::pos2(x as f32, y as f32),
        egui::vec2(w as f32, h as f32),
    );
    ui.painter()
        .rect_filled(strip, CornerRadius::same(0), pal().surface);
    let at_left = state.shared.config.overview.strip_v_side == Side::Left;
    let edge = if at_left { strip.max.x } else { strip.min.x };
    ui.painter().line_segment(
        [egui::pos2(edge, strip.min.y), egui::pos2(edge, strip.max.y)],
        Stroke::new(1.0, pal().border),
    );

    let all = strip_ids(state);
    let offset = state.overview.strip_offset.min(all.len().saturating_sub(1));
    let visible = all
        .iter()
        .skip(offset)
        .take(6)
        .copied()
        .collect::<Vec<u32>>();
    let th = 34.0;
    let total = visible.len() as f32 * (th + 6.0) - 6.0;
    let start = strip.center().y - total / 2.0;
    if nav_button(
        ui,
        egui::Rect::from_center_size(
            egui::pos2(strip.center().x, start - 18.0),
            egui::vec2(20.0, 20.0),
        ),
        ic::CARET_UP,
        "ov-v-prev",
    ) {
        state.overview.strip_offset = state.overview.strip_offset.saturating_sub(1);
    }
    for (i, id) in visible.iter().enumerate() {
        thumb(
            ui,
            state,
            egui::Rect::from_min_size(
                egui::pos2(strip.center().x - 28.0, start + i as f32 * (th + 6.0)),
                egui::vec2(56.0, th),
            ),
            *id,
            offset + i,
            palette,
        );
    }
    if nav_button(
        ui,
        egui::Rect::from_center_size(
            egui::pos2(strip.center().x, start + total + 18.0),
            egui::vec2(20.0, 20.0),
        ),
        ic::CARET_DOWN,
        "ov-v-next",
    ) {
        let max_offset = all.len().saturating_sub(6);
        state.overview.strip_offset = (state.overview.strip_offset + 1).min(max_offset);
    }
    let expand = egui::Rect::from_center_size(
        egui::pos2(strip.center().x, strip.max.y - 22.0),
        egui::vec2(40.0, 26.0),
    );
    let resp = ui.interact(
        expand,
        ui.scope_id().with("ov-v-expand"),
        egui::Sense::click(),
    );
    icon_at(
        ui,
        expand.center(),
        ic::SQUARES_FOUR,
        ICON_MD,
        if resp.hovered() {
            palette.text
        } else {
            palette.muted
        },
    );
    if resp.clicked() {
        overview::open(state, OverviewKind::Workspaces);
    }
}
