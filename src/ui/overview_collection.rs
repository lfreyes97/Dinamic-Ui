//! Vistas adicionales del overview: Actividades y franjas acoplables.
//! Se incluyen desde `overview.rs` para mantener el módulo original intacto.

use super::overview::OverviewKind;
use super::*;
use crate::config::Side;

/// Workspaces con ventanas, más el actual (hasta `max`).
pub(super) fn workspace_ids(state: &UiState, max: usize) -> Vec<u32> {
    let mut ids = state.shared.snapshot.occupied.clone();
    let current = state.shared.snapshot.workspace.parse::<u32>().unwrap_or(1);
    if !ids.contains(&current) {
        ids.push(current);
    }
    ids.sort_unstable();
    ids.dedup();
    ids.truncate(max);
    if ids.is_empty() {
        ids.push(1);
    }
    ids
}

/// Rect de la franja horizontal o vertical.
pub fn strip_rect(state: &UiState, screen_w: i32, screen_h: i32) -> (i32, i32, i32, i32) {
    match state.overview.kind {
        OverviewKind::StripH => match state.shared.config.overview.strip_h_side {
            Side::Bottom => (0, screen_h - 68, screen_w, 68),
            _ => (0, 0, screen_w, 68),
        },
        OverviewKind::StripV => match state.shared.config.overview.strip_v_side {
            Side::Left => (0, 0, 78, screen_h),
            _ => (screen_w - 78, 0, 78, screen_h),
        },
        _ => (0, 0, screen_w, screen_h),
    }
}

fn search_pill(ui: &egui::Ui, screen: egui::Rect, placeholder: &str) {
    let r = egui::Rect::from_center_size(
        egui::pos2(screen.center().x, screen.min.y + 30.0),
        egui::vec2(380.0, 34.0),
    );
    ui.painter().rect(
        r,
        CornerRadius::same(17),
        Palette::with_alpha(pal().solid(), 250),
        Stroke::new(1.0, pal().border),
        StrokeKind::Middle,
    );
    icon_at(
        ui,
        egui::pos2(r.min.x + 16.0, r.center().y),
        ic::MAGNIFYING_GLASS,
        ICON_MD,
        pal().muted,
    );
    text_at(
        ui,
        egui::pos2(r.min.x + 30.0, r.center().y),
        egui::Align2::LEFT_CENTER,
        placeholder,
        TEXT_BODY,
        pal().dim,
    );
    text_at(
        ui,
        egui::pos2(r.max.x - 14.0, r.center().y),
        egui::Align2::RIGHT_CENTER,
        "ESC",
        TEXT_CAPTION,
        pal().muted,
    );
}

fn app_names(state: &UiState) -> Vec<String> {
    let mut out: Vec<String> = state.shared.config.dock.pinned.clone();
    for c in &state.shared.clients {
        let key = c.class.clone();
        if !out.iter().any(|n| n.eq_ignore_ascii_case(&key)) {
            out.push(key);
        }
    }
    for fixed in ["firefox", "code", "kitty", "files", "spotify", "settings"] {
        if out.len() >= 12 {
            break;
        }
        if !out.iter().any(|n| n.eq_ignore_ascii_case(fixed)) {
            out.push(fixed.to_string());
        }
    }
    out.truncate(12);
    out
}

/// Overview de actividades: apps + tira de ventanas.
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
    search_pill(ui, screen, "Busca aplicaciones…");

    let apps = app_names(state);
    let cols = 6usize.min(apps.len().max(1));
    let cw = 86.0;
    let total_w = cols as f32 * cw;
    let origin = egui::pos2(screen.center().x - total_w / 2.0, screen.min.y + 80.0);
    for (i, name) in apps.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let c = egui::pos2(
            origin.x + col as f32 * cw + cw / 2.0,
            origin.y + row as f32 * 78.0 + 23.0,
        );
        let box_rect = egui::Rect::from_center_size(c, egui::vec2(46.0, 46.0));
        let resp = ui.interact(
            box_rect,
            ui.scope_id().with(("ov-app", i)),
            egui::Sense::click(),
        );
        ui.painter()
            .rect_filled(box_rect, CornerRadius::same(12), pal().hover);
        if let Some(texture) = state.shared.icon(ui.ctx(), name) {
            egui::Image::new(egui::load::SizedTexture::new(
                texture.id(),
                egui::vec2(22.0, 22.0),
            ))
            .paint_at(ui, egui::Rect::from_center_size(c, egui::vec2(22.0, 22.0)));
        } else {
            let glyph = if name.to_lowercase().contains("code") {
                ic::CODE
            } else if name.to_lowercase().contains("kitty") || name.to_lowercase().contains("term")
            {
                ic::TERMINAL
            } else if name.to_lowercase().contains("spotify") {
                ic::MUSIC_NOTES
            } else if name.to_lowercase().contains("file") {
                ic::FOLDER
            } else if name.to_lowercase().contains("setting")
                || name.to_lowercase().contains("gear")
            {
                ic::GEAR
            } else {
                ic::APP_WINDOW
            };
            icon_at(ui, c, glyph, ICON_LG, palette.text);
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        text_at(
            ui,
            egui::pos2(c.x, c.y + 34.0),
            egui::Align2::CENTER_CENTER,
            truncate(name, 11),
            TEXT_CAPTION,
            palette.muted,
        );
        if resp.clicked() {
            if let Some(win) = state
                .shared
                .clients
                .iter()
                .find(|w| w.class.eq_ignore_ascii_case(name))
            {
                state.shared.overview_focus = Some(win.address.clone());
            } else {
                state.shared.pending_spawns.push(vec![name.clone()]);
            }
        }
    }

    // Tira de ventanas abiertas.
    let clients: Vec<_> = state.shared.clients.iter().take(5).collect();
    let fw = 92.0;
    let film_w = clients.len() as f32 * (fw + 8.0) - 8.0;
    let film_y = screen.max.y - 74.0;
    for (i, win) in clients.iter().enumerate() {
        let r = egui::Rect::from_min_size(
            egui::pos2(
                screen.center().x - film_w / 2.0 + i as f32 * (fw + 8.0),
                film_y,
            ),
            egui::vec2(fw, 48.0),
        );
        let resp = ui.interact(r, ui.scope_id().with(("ov-film", i)), egui::Sense::click());
        ui.painter().rect(
            r,
            CornerRadius::same(7),
            Palette::with_alpha(pal().hover, 250),
            Stroke::new(
                1.0,
                if win.address == state.shared.snapshot.window_address {
                    palette.accent
                } else {
                    palette.border
                },
            ),
            StrokeKind::Middle,
        );
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                r.min + egui::vec2(8.0, 7.0),
                egui::vec2(r.width() - 16.0, 22.0),
            ),
            CornerRadius::same(3),
            palette.badge,
        );
        text_at(
            ui,
            r.min + egui::vec2(7.0, 38.0),
            egui::Align2::LEFT_CENTER,
            truncate(&win.title, 14),
            TEXT_CAPTION,
            palette.muted,
        );
        if resp.clicked() {
            state.shared.overview_focus = Some(win.address.clone());
        }
    }
    text_at(
        ui,
        egui::pos2(screen.center().x, screen.max.y - 14.0),
        egui::Align2::CENTER_CENTER,
        "↵ lanza   ·   Esc cierra",
        TEXT_CAPTION,
        palette.muted,
    );
}

fn nav_button(ui: &mut egui::Ui, rect: egui::Rect, glyph: &str, id: &'static str) -> bool {
    let resp = ui.interact(rect, ui.scope_id().with(id), egui::Sense::click());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(6), pal().hover);
    icon_at(ui, rect.center(), glyph, ICON_SM, pal().muted);
    resp.clicked()
}

/// Miniatura de workspace compartida por las franjas.
fn thumb(
    ui: &mut egui::Ui,
    state: &mut UiState,
    rect: egui::Rect,
    id: u32,
    palette: Palette,
    vertical: bool,
) {
    let current = state.shared.snapshot.workspace.parse::<u32>().unwrap_or(1);
    let active = id == current;
    ui.painter().rect(
        rect,
        CornerRadius::same(6),
        pal().hover,
        Stroke::new(
            if active { 2.0 } else { 1.0 },
            if active {
                palette.accent
            } else {
                palette.border
            },
        ),
        StrokeKind::Middle,
    );
    let content = rect.shrink2(egui::vec2(6.0, 12.0));
    let screen_w = ui.max_rect().width().max(1.0);
    let screen_h = ui.max_rect().height().max(1.0);
    for win in state
        .shared
        .clients
        .iter()
        .filter(|c| c.workspace == id as i32)
    {
        let x = content.min.x + win.x as f32 / screen_w * content.width();
        let y = content.min.y + win.y as f32 / screen_h * content.height();
        let w = (win.w as f32 / screen_w * content.width()).max(4.0);
        let h = (win.h as f32 / screen_h * content.height()).max(4.0);
        let r = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h)).intersect(content);
        ui.painter()
            .rect_filled(r, CornerRadius::same(2), palette.badge);
    }
    let label = format!("{id}");
    text_at(
        ui,
        rect.min + egui::vec2(5.0, rect.height() - 5.0),
        egui::Align2::LEFT_CENTER,
        label,
        TEXT_CAPTION,
        if active {
            palette.accent
        } else {
            palette.muted
        },
    );
    let resp = ui.interact(
        rect,
        ui.scope_id().with(("ov-strip-ws", id)),
        egui::Sense::click(),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.clicked() {
        state.shared.overview_workspace = Some(id);
        state.overview.overview = false;
    }
    let _ = vertical;
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
    ui.painter().line_segment(
        [
            egui::pos2(strip.min.x, if y == 0 { strip.max.y } else { strip.min.y }),
            egui::pos2(strip.max.x, if y == 0 { strip.max.y } else { strip.min.y }),
        ],
        Stroke::new(1.0, pal().border),
    );

    let all = workspace_ids(state, 20);
    let offset = state.overview.strip_offset.min(all.len().saturating_sub(1));
    let ids: Vec<u32> = all.iter().skip(offset).take(5).copied().collect();
    let tw = 86.0;
    let total = ids.len() as f32 * (tw + 6.0) - 6.0;
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
    for (i, id) in ids.iter().enumerate() {
        thumb(
            ui,
            state,
            egui::Rect::from_min_size(
                egui::pos2(start + i as f32 * (tw + 6.0), strip.center().y - 23.0),
                egui::vec2(tw, 46.0),
            ),
            *id,
            palette,
            false,
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
        state.overview.strip_offset += 1;
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
        palette.muted,
    );
    if resp.clicked() {
        state.overview.kind = OverviewKind::Workspaces;
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
    let edge = if x == 0 { strip.max.x } else { strip.min.x };
    ui.painter().line_segment(
        [egui::pos2(edge, strip.min.y), egui::pos2(edge, strip.max.y)],
        Stroke::new(1.0, pal().border),
    );

    let all = workspace_ids(state, 20);
    let offset = state.overview.strip_offset.min(all.len().saturating_sub(1));
    let ids: Vec<u32> = all.iter().skip(offset).take(6).copied().collect();
    let th = 34.0;
    let total = ids.len() as f32 * (th + 6.0) - 6.0;
    let start = strip.center().y - total / 2.0;
    if nav_button(
        ui,
        egui::Rect::from_center_size(
            egui::pos2(strip.center().x, start - 18.0),
            egui::vec2(20.0, 20.0),
        ),
        ic::CARET_LEFT,
        "ov-v-prev",
    ) {
        state.overview.strip_offset = state.overview.strip_offset.saturating_sub(1);
    }
    for (i, id) in ids.iter().enumerate() {
        thumb(
            ui,
            state,
            egui::Rect::from_min_size(
                egui::pos2(strip.center().x - 28.0, start + i as f32 * (th + 6.0)),
                egui::vec2(56.0, th),
            ),
            *id,
            palette,
            true,
        );
    }
    if nav_button(
        ui,
        egui::Rect::from_center_size(
            egui::pos2(strip.center().x, start + total + 18.0),
            egui::vec2(20.0, 20.0),
        ),
        ic::CARET_RIGHT,
        "ov-v-next",
    ) {
        state.overview.strip_offset += 1;
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
        palette.muted,
    );
    if resp.clicked() {
        state.overview.kind = OverviewKind::Workspaces;
    }
}
