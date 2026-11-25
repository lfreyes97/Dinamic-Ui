//! Franjas acopladas del overview de workspaces (horizontal y vertical).

use super::nav::window_rect;
use super::overlay::{mini_window, monitor_rect, pointer_moved};
use super::overview::{current_ws, strip_ids, OverviewKind};
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
    // La franja vive dentro del área que no reservan topbar/taskbar.
    let [top, right, bottom, left] = super::reservations(state);
    let avail_w = (screen_w - left - right).max(0);
    let avail_h = (screen_h - top - bottom).max(0);
    match state.overview.kind {
        OverviewKind::StripH => {
            let h = cfg.strip_h_height.resolve(avail_h as f32).min(avail_h);
            match cfg.strip_h_side {
                Side::Bottom => (left, screen_h - bottom - h + slide(h), avail_w, h),
                _ => (left, top - slide(h), avail_w, h),
            }
        }
        OverviewKind::StripV => {
            let w = cfg.strip_v_width.resolve(avail_w as f32).min(avail_w);
            match cfg.strip_v_side {
                Side::Left => (left - slide(w), top, w, avail_h),
                _ => (screen_w - right - w + slide(w), top, w, avail_h),
            }
        }
        _ => (0, 0, screen_w, screen_h),
    }
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
        CornerRadius::same(Radius::of(Radius::SM)),
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
        CornerRadius::same(Radius::of(Radius::SM)),
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
            CornerRadius::same(Radius::of(Radius::SM)),
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
        let r = window_rect(win, monitor, mon, 3.0);
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

/// Fondo de una franja: panel casi opaco con un filete en el borde que da al escritorio.
fn strip_background(ui: &egui::Ui, strip: egui::Rect, edge: (egui::Pos2, egui::Pos2)) {
    ui.painter().rect_filled(
        strip,
        CornerRadius::same(Radius::of(Radius::NONE)),
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
    let (gap, nav) = (8.0, 20.0 * sc);
    // Controles fijos: ‹ ›. (La franja es un overview en sí: no enlaza con otra vista.)
    let controls = nav + gap + gap + nav;
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
    let (gap, nav) = (8.0, 20.0 * sc);
    let controls = nav + gap + gap + nav;
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
}
