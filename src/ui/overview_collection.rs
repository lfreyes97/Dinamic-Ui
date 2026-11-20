//! Vistas del overview que no son la rejilla: Actividades y las franjas acoplables.

use super::overview::{
    self, activity_tiles, current_ws, drop_shadow, hint_bar, mini_window, pointer_moved,
    search_pill, strip_ids, OverviewKind, ACTIVITY_COLS,
};
use super::overview_model as model;
use super::*;
use crate::config::Side;

/// Rect de la franja horizontal o vertical. El grosor es una fracción de la pantalla (1/5 del
/// alto, 1/6 del ancho) o los píxeles que fije la config. Con `progress < 1` se desliza fuera de pantalla hacia el borde en el que está
/// acoplada.
pub fn strip_rect(state: &UiState, screen_w: i32, screen_h: i32) -> (i32, i32, i32, i32) {
    let hidden = 1.0 - crate::smootherstep(state.overview.progress);
    let slide = |size: i32| (hidden * size as f32).round() as i32;
    let cfg = &state.shared.config.overview;
    match state.overview.kind {
        OverviewKind::StripH => {
            let h = cfg.strip_h_height.resolve(screen_h as f32);
            match cfg.strip_h_side {
                Side::Bottom => (0, screen_h - h + slide(h), screen_w, h),
                _ => (0, -slide(h), screen_w, h),
            }
        }
        OverviewKind::StripV => {
            let w = cfg.strip_v_width.resolve(screen_w as f32);
            match cfg.strip_v_side {
                Side::Left => (-slide(w), 0, w, screen_h),
                _ => (screen_w - w + slide(w), 0, w, screen_h),
            }
        }
        _ => (0, 0, screen_w, screen_h),
    }
}

/// Imagen del monitor (mismo aspecto que la pantalla) centrada dentro de `area`.
fn monitor_rect(area: egui::Rect, aspect: f32) -> egui::Rect {
    let (w, h) = if area.width() / area.height().max(1.0) > aspect {
        (area.height() * aspect, area.height())
    } else {
        (area.width(), area.width() / aspect)
    };
    egui::Rect::from_center_size(area.center(), egui::vec2(w.max(1.0), h.max(1.0)))
}

/// Overview de actividades: buscador, apps instaladas y tira de ventanas abiertas.
pub(super) fn draw_activities(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let screen = ui.max_rect();
    ui.painter()
        .rect_filled(screen, CornerRadius::same(0), Palette::scrim(176));
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

    // Zona de la cuadrícula: entre el buscador y la tira de ventanas.
    // Las medidas del mockup son para 720p: se escalan con la pantalla.
    let k = model::ui_scale(screen.height());
    let film_h = 70.0 * k;
    let film_y = screen.max.y - 40.0 - film_h;
    let top = screen.min.y + 92.0 * k.sqrt();
    let cols = ACTIVITY_COLS.min(tiles.len().max(1));
    let rows = tiles.len().div_ceil(cols.max(1)).max(1);
    let (pitch_x, pitch_y, tile) = (104.0 * k, 96.0 * k, 56.0 * k);
    let block_h = rows as f32 * pitch_y;
    let free = (film_y - 24.0 - top - block_h).max(0.0);
    let origin = egui::pos2(
        screen.center().x - cols as f32 * pitch_x / 2.0,
        top + free * 0.4,
    );

    if tiles.is_empty() {
        let message = if query.is_empty() {
            "No hay aplicaciones".to_string()
        } else {
            format!("Sin resultados para «{query}»")
        };
        text_at(
            ui,
            egui::pos2(screen.center().x, top + 60.0),
            egui::Align2::CENTER_CENTER,
            message,
            TEXT_BODY,
            palette.muted,
        );
    }

    for (i, t) in tiles.iter().enumerate() {
        let c = egui::pos2(
            origin.x + (i % cols) as f32 * pitch_x + pitch_x / 2.0,
            origin.y + (i / cols) as f32 * pitch_y + tile / 2.0,
        );
        let cell = egui::Rect::from_center_size(
            egui::pos2(c.x, c.y + 10.0),
            egui::vec2(pitch_x - 8.0, pitch_y - 6.0),
        );
        let resp = ui.interact(
            cell,
            ui.scope_id().with(("ov-app", &t.id)),
            egui::Sense::click(),
        );
        if resp.hovered() && moved {
            state.overview.selected = i;
        }
        let selected = state.overview.selected == i;
        let box_rect = egui::Rect::from_center_size(c, egui::vec2(tile, tile));
        let lift = ui.ctx().animate_bool_with_time(
            ui.scope_id().with(("ov-app-lift", &t.id)),
            resp.hovered() || selected,
            0.12,
        );
        let box_rect = box_rect.translate(egui::vec2(0.0, -2.0 * lift));
        ui.painter().rect(
            box_rect,
            CornerRadius::same(14),
            if selected {
                palette.accent_soft
            } else {
                Palette::with_alpha(palette.raised, 235)
            },
            Stroke::new(
                if selected { 1.5 } else { 1.0 },
                if selected {
                    palette.accent
                } else {
                    palette.border
                },
            ),
            StrokeKind::Middle,
        );
        if let Some(texture) = state.shared.icon(ui.ctx(), &t.id) {
            egui::Image::new(egui::load::SizedTexture::new(
                texture.id(),
                egui::vec2(30.0 * k, 30.0 * k),
            ))
            .paint_at(
                ui,
                egui::Rect::from_center_size(box_rect.center(), egui::vec2(30.0 * k, 30.0 * k)),
            );
        } else {
            icon_at(
                ui,
                box_rect.center(),
                dock_icon(&t.name),
                ICON_LG,
                palette.text,
            );
        }
        // Un punto por ventana abierta (hasta tres).
        let dots = t.windows.len().min(3);
        for k in 0..dots {
            let dx = (k as f32 - (dots as f32 - 1.0) / 2.0) * 6.0;
            ui.painter().circle_filled(
                egui::pos2(box_rect.center().x + dx, box_rect.max.y - 6.0),
                1.9,
                palette.accent,
            );
        }
        text_at(
            ui,
            egui::pos2(c.x, c.y + tile / 2.0 + 14.0),
            egui::Align2::CENTER_CENTER,
            truncate(&t.name, 13),
            9.5 * k.sqrt(),
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
            overview::run_tile(state, t);
        }
    }

    // Tira de ventanas abiertas: tantas como caben, con una vista esquemática de cada una.
    let active = state.shared.snapshot.window_address.clone();
    let (fw, gap) = (124.0 * k, 10.0 * k);
    let capacity = (((screen.width() - 120.0) / (fw + gap)).floor() as usize).clamp(1, 12);
    let windows: Vec<crate::ipc::ClientInfo> = state
        .shared
        .clients
        .iter()
        .filter(|c| c.mapped)
        .take(capacity)
        .cloned()
        .collect();
    let film_w = windows.len() as f32 * (fw + gap) - gap;
    let monitor = screen.size();
    let aspect = monitor.x.max(1.0) / monitor.y.max(1.0);
    for (i, win) in windows.iter().enumerate() {
        let r = egui::Rect::from_min_size(
            egui::pos2(
                screen.center().x - film_w / 2.0 + i as f32 * (fw + gap),
                film_y,
            ),
            egui::vec2(fw, film_h),
        );
        let resp = ui.interact(
            r,
            ui.scope_id().with(("ov-film", &win.address)),
            egui::Sense::click(),
        );
        let is_active = win.address == active;
        drop_shadow(ui, r, 8, 6, 16, 80);
        ui.painter().rect(
            r,
            CornerRadius::same(8),
            Palette::with_alpha(palette.raised, 240),
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
        // Vista esquemática de la ventana dentro del monitor.
        let pic =
            egui::Rect::from_min_max(r.min + egui::vec2(8.0, 7.0), r.max - egui::vec2(8.0, 26.0));
        let mon = monitor_rect(pic, aspect);
        ui.painter()
            .rect_filled(mon, CornerRadius::same(3), Palette::sheen(8));
        let wr = model::window_rect(win, monitor, mon, 6.0);
        if wr.width() > 0.0 && wr.height() > 0.0 {
            mini_window(ui, wr, is_active, resp.hovered(), 2);
        }
        // Pie: icono y título.
        let icon_c = egui::pos2(r.min.x + 15.0, r.max.y - 12.0);
        if let Some(texture) = state.shared.icon(ui.ctx(), &win.class) {
            egui::Image::new(egui::load::SizedTexture::new(
                texture.id(),
                egui::vec2(13.0, 13.0),
            ))
            .paint_at(
                ui,
                egui::Rect::from_center_size(icon_c, egui::vec2(13.0, 13.0)),
            );
        } else {
            icon_at(ui, icon_c, dock_icon(&win.class), 11.0, palette.text);
        }
        text_at(
            ui,
            egui::pos2(r.min.x + 27.0, r.max.y - 12.0),
            egui::Align2::LEFT_CENTER,
            truncate(&win.title, 15),
            8.5 * k.sqrt(),
            palette.muted,
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() {
            state.shared.overview_focus = Some(win.address.clone());
        }
    }

    hint_bar(
        ui,
        screen,
        &[
            ("← → ↑ ↓", "navega"),
            ("↵", "abre o enfoca"),
            ("Esc", if query.is_empty() { "cierra" } else { "limpia" }),
        ],
    );
}

/// Botón cuadrado con borde de las franjas (anterior / siguiente).
fn nav_button(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    glyph: &str,
    id: &'static str,
    scale: f32,
) -> bool {
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
        11.0 * scale,
        if resp.hovered() { p.text } else { p.muted },
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// Miniatura de workspace de las franjas: número arriba a la derecha, nombre abajo a la
/// izquierda y las ventanas dentro. `index` es su posición en la lista completa.
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
        (1.0, Palette::with_alpha(palette.accent, 150))
    } else if selected {
        (1.5, palette.text)
    } else if resp.hovered() {
        (1.0, palette.muted)
    } else {
        (1.0, palette.border)
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(6),
        if active_ws {
            Palette::with_alpha(crate::theme::mix(palette.raised, palette.accent, 0.12), 240)
        } else if selected || resp.hovered() {
            Palette::with_alpha(palette.pressed, 245)
        } else {
            Palette::with_alpha(palette.raised, 235)
        },
        Stroke::new(width, color),
        StrokeKind::Middle,
    );
    if active_ws {
        ui.painter().rect_stroke(
            rect.expand(1.5),
            CornerRadius::same(7),
            Stroke::new(1.0, Palette::with_alpha(palette.accent, 50)),
            StrokeKind::Middle,
        );
    }

    let monitor = ui.max_rect().size();
    let aspect = monitor.x.max(1.0) / monitor.y.max(1.0);
    // Los textos crecen con la miniatura (que crece con el grosor de la franja).
    let fk = (rect.height() / 46.0).sqrt().clamp(1.0, 1.9);
    let mon = monitor_rect(rect.shrink2(egui::vec2(5.0, 7.0)), aspect);
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
        let r = model::window_rect(win, monitor, mon, 3.0);
        if r.width() > 0.0 && r.height() > 0.0 {
            mini_window(ui, r, win.address == active_address, false, 2);
        }
    }
    text_at(
        ui,
        egui::pos2(rect.max.x - 5.0, rect.min.y + 7.0),
        egui::Align2::RIGHT_CENTER,
        format!("{id}"),
        7.0 * fk,
        palette.dim,
    );
    if let Some(name) = state.shared.config.workspaces.name_of(id) {
        text_at(
            ui,
            egui::pos2(rect.min.x + 5.0, rect.max.y - 6.0),
            egui::Align2::LEFT_CENTER,
            truncate(name, 12),
            7.5 * fk,
            if active_ws {
                palette.accent
            } else {
                palette.muted
            },
        );
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.clicked() {
        state.shared.overview_workspace = Some(id);
        state.overview.overview = false;
    }
}

/// Bloque "Overview": icono y etiqueta; abre la vista de pantalla completa.
fn overview_button(ui: &mut egui::Ui, rect: egui::Rect, id: &'static str, scale: f32) -> bool {
    let p = pal();
    let resp = ui.interact(rect, ui.scope_id().with(id), egui::Sense::click());
    let color = if resp.hovered() { p.text } else { p.muted };
    icon_at(
        ui,
        egui::pos2(rect.center().x, rect.center().y - 7.0 * scale),
        ic::FRAME_CORNERS,
        ICON_MD * scale,
        color,
    );
    text_at(
        ui,
        egui::pos2(rect.center().x, rect.center().y + 12.0 * scale),
        egui::Align2::CENTER_CENTER,
        "Overview",
        8.5 * scale,
        color,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// Fondo de una franja: panel casi opaco con un filete en el borde que da al escritorio.
fn strip_background(ui: &egui::Ui, strip: egui::Rect, edge: (egui::Pos2, egui::Pos2)) {
    ui.painter().rect_filled(
        strip,
        CornerRadius::same(0),
        Palette::with_alpha(pal().solid(), 236),
    );
    ui.painter()
        .line_segment([edge.0, edge.1], Stroke::new(1.0, pal().border));
}

/// Medidas de una miniatura de franja: la imagen del monitor mantiene su proporción y la
/// miniatura la envuelve con un margen, así las ventanas ocupan todo el espacio disponible.
fn thumb_size_h(strip_h: f32, aspect: f32) -> egui::Vec2 {
    let th = (strip_h - 22.0).max(30.0);
    let mon_h = th - 14.0;
    egui::vec2((mon_h * aspect + 10.0).max(72.0), th)
}

fn thumb_size_v(strip_w: f32, aspect: f32) -> egui::Vec2 {
    let tw = (strip_w - 22.0).max(40.0);
    let mon_w = tw - 10.0;
    egui::vec2(tw, (mon_w / aspect + 14.0).max(30.0))
}

/// Franja horizontal de workspaces (arriba o abajo).
pub(super) fn draw_strip_h(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let screen = ui.max_rect();
    let (x, y, w, h) = strip_rect(state, screen.width() as i32, screen.height() as i32);
    let strip = egui::Rect::from_min_size(
        egui::pos2(x as f32, y as f32),
        egui::vec2(w as f32, h as f32),
    );
    let at_top = state.shared.config.overview.strip_h_side != Side::Bottom;
    let edge_y = if at_top { strip.max.y } else { strip.min.y };
    strip_background(
        ui,
        strip,
        (
            egui::pos2(strip.min.x, edge_y),
            egui::pos2(strip.max.x, edge_y),
        ),
    );

    let aspect = screen.width().max(1.0) / screen.height().max(1.0);
    let thumb_sz = thumb_size_h(strip.height(), aspect);
    // Los controles crecen con el grosor de la franja (1× a 68 px; ~1,5× a 154 px).
    let sc = (strip.height() / 68.0).sqrt().clamp(1.0, 1.6);
    let (gap, nav, ovw) = (8.0, 20.0 * sc, 56.0 * sc);
    // Controles fijos: ‹ › | Overview (y el margen de los extremos).
    let controls = nav + gap + gap + nav + 14.0 + 1.0 + 14.0 + ovw;
    let capacity = model::fit_count(strip.width() - 48.0 - controls, thumb_sz.x, gap, 3, 8);
    state.overview.strip_visible = capacity;

    let all = strip_ids(state);
    let offset = state
        .overview
        .strip_offset
        .min(all.len().saturating_sub(capacity));
    state.overview.strip_offset = offset;
    let visible: Vec<u32> = all.iter().skip(offset).take(capacity).copied().collect();
    let thumbs_w = visible.len() as f32 * (thumb_sz.x + gap) - gap;
    let group_w = controls + thumbs_w;
    let mut cursor = strip.center().x - group_w / 2.0;
    let cy = strip.center().y;

    if nav_button(
        ui,
        egui::Rect::from_min_size(egui::pos2(cursor, cy - nav / 2.0), egui::vec2(nav, nav)),
        ic::CARET_LEFT,
        "ov-h-prev",
        sc,
    ) {
        state.overview.strip_offset = state.overview.strip_offset.saturating_sub(1);
    }
    cursor += nav + gap;
    for (i, id) in visible.iter().enumerate() {
        thumb(
            ui,
            state,
            egui::Rect::from_min_size(
                egui::pos2(
                    cursor + i as f32 * (thumb_sz.x + gap),
                    cy - thumb_sz.y / 2.0,
                ),
                thumb_sz,
            ),
            *id,
            offset + i,
            palette,
        );
    }
    cursor += thumbs_w + gap;
    if nav_button(
        ui,
        egui::Rect::from_min_size(egui::pos2(cursor, cy - nav / 2.0), egui::vec2(nav, nav)),
        ic::CARET_RIGHT,
        "ov-h-next",
        sc,
    ) {
        let max_offset = all.len().saturating_sub(capacity);
        state.overview.strip_offset = (state.overview.strip_offset + 1).min(max_offset);
    }
    cursor += nav + 14.0;
    ui.painter().line_segment(
        [
            egui::pos2(cursor, cy - thumb_sz.y / 2.0 + 4.0),
            egui::pos2(cursor, cy + thumb_sz.y / 2.0 - 4.0),
        ],
        Stroke::new(1.0, palette.border),
    );
    cursor += 1.0 + 14.0;
    if overview_button(
        ui,
        egui::Rect::from_min_size(
            egui::pos2(cursor, cy - thumb_sz.y / 2.0),
            egui::vec2(ovw, thumb_sz.y),
        ),
        "ov-h-expand",
        sc,
    ) {
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
    let at_left = state.shared.config.overview.strip_v_side == Side::Left;
    let edge_x = if at_left { strip.max.x } else { strip.min.x };
    strip_background(
        ui,
        strip,
        (
            egui::pos2(edge_x, strip.min.y),
            egui::pos2(edge_x, strip.max.y),
        ),
    );

    let aspect = screen.width().max(1.0) / screen.height().max(1.0);
    let thumb_sz = thumb_size_v(strip.width(), aspect);
    let sc = (strip.width() / 78.0).sqrt().clamp(1.0, 1.6);
    let (gap, nav, ovh) = (8.0, 20.0 * sc, 40.0 * sc);
    let controls = nav + gap + gap + nav + 14.0 + 1.0 + 14.0 + ovh;
    let capacity = model::fit_count(strip.height() - 24.0 - controls, thumb_sz.y, gap, 3, 8);
    state.overview.strip_visible = capacity;

    let all = strip_ids(state);
    let offset = state
        .overview
        .strip_offset
        .min(all.len().saturating_sub(capacity));
    state.overview.strip_offset = offset;
    let visible: Vec<u32> = all.iter().skip(offset).take(capacity).copied().collect();
    let thumbs_h = visible.len() as f32 * (thumb_sz.y + gap) - gap;
    let group_h = controls + thumbs_h;
    let cx = strip.center().x;
    let mut cursor = (strip.center().y - group_h / 2.0).max(strip.min.y + 6.0);

    if nav_button(
        ui,
        egui::Rect::from_min_size(egui::pos2(cx - nav / 2.0, cursor), egui::vec2(nav, nav)),
        ic::CARET_UP,
        "ov-v-prev",
        sc,
    ) {
        state.overview.strip_offset = state.overview.strip_offset.saturating_sub(1);
    }
    cursor += nav + gap;
    for (i, id) in visible.iter().enumerate() {
        thumb(
            ui,
            state,
            egui::Rect::from_min_size(
                egui::pos2(
                    cx - thumb_sz.x / 2.0,
                    cursor + i as f32 * (thumb_sz.y + gap),
                ),
                thumb_sz,
            ),
            *id,
            offset + i,
            palette,
        );
    }
    cursor += thumbs_h + gap;
    if nav_button(
        ui,
        egui::Rect::from_min_size(egui::pos2(cx - nav / 2.0, cursor), egui::vec2(nav, nav)),
        ic::CARET_DOWN,
        "ov-v-next",
        sc,
    ) {
        let max_offset = all.len().saturating_sub(capacity);
        state.overview.strip_offset = (state.overview.strip_offset + 1).min(max_offset);
    }
    cursor += nav + 14.0;
    ui.painter().line_segment(
        [
            egui::pos2(cx - thumb_sz.x / 2.0, cursor),
            egui::pos2(cx + thumb_sz.x / 2.0, cursor),
        ],
        Stroke::new(1.0, palette.border),
    );
    cursor += 1.0 + 14.0;
    if overview_button(
        ui,
        egui::Rect::from_min_size(
            egui::pos2(cx - 28.0 * sc, cursor),
            egui::vec2(56.0 * sc, ovh),
        ),
        "ov-v-expand",
        sc,
    ) {
        overview::open(state, OverviewKind::Workspaces);
    }
}
