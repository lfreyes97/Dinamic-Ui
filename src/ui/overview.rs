//! Overview de workspaces con ventanas en posición relativa.

use super::*;

/// Estado del overview (modo pantalla completa).
#[derive(Default)]
pub struct OverviewState {
    pub overview: bool,
    pub overview_focus: Option<String>,
    pub overview_workspace: Option<u32>,
}

/// Overview: rejilla de workspaces con las ventanas en su posición relativa.
pub(super) fn draw_overview(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let screen = ui.max_rect();
    let painter = ui.painter();
    painter.rect_filled(
        screen,
        CornerRadius::same(0),
        Color32::from_black_alpha(200),
    );
    // Registrado antes que las tarjetas: un clic en ventana gana sobre el fondo.
    let backdrop = ui.interact(
        screen,
        ui.scope_id().with(egui::IdSalt::new("overview-backdrop")),
        egui::Sense::click(),
    );
    if backdrop.clicked() {
        state.overview.overview = false;
    }

    let mut ids: Vec<u32> = state.snapshot.occupied.clone();
    let current = state.snapshot.workspace.parse::<u32>().unwrap_or(1);
    if !ids.contains(&current) {
        ids.push(current);
    }
    ids.sort_unstable();
    ids.dedup();
    ids.truncate(6);
    if ids.is_empty() {
        ids.push(1);
    }

    let cols = ids.len().clamp(1, 3);
    let rows = ids.len().div_ceil(cols);
    let gap = 14.0;
    let area = screen.shrink2(egui::vec2(40.0, 56.0));
    let card_h = ((area.height() - gap * (rows as f32 - 1.0)) / rows as f32).min(240.0);
    let card_w =
        (card_h * 16.0 / 9.0).min((area.width() - gap * (cols as f32 - 1.0)) / cols as f32);
    let card_h = card_w * 9.0 / 16.0;
    let total_w = cols as f32 * card_w + gap * (cols as f32 - 1.0);
    let total_h = rows as f32 * card_h + gap * (rows as f32 - 1.0);
    let origin = egui::pos2(
        screen.center().x - total_w / 2.0,
        screen.center().y - total_h / 2.0 + 10.0,
    );

    text_at(
        ui,
        egui::pos2(screen.center().x, origin.y - 22.0),
        egui::Align2::CENTER_CENTER,
        "Overview · Esc para cerrar",
        TEXT_BODY,
        palette.muted,
    );

    let monitor_w = screen.width().max(1.0);
    let monitor_h = screen.height().max(1.0);

    for (i, id) in ids.iter().enumerate() {
        let x = origin.x + (i % cols) as f32 * (card_w + gap);
        let y = origin.y + (i / cols) as f32 * (card_h + gap);
        let card = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(card_w, card_h));
        painter.rect(
            card,
            CornerRadius::same(12),
            Color32::from_rgba_unmultiplied(14, 14, 14, 240),
            Stroke::new(
                if *id == current { 2.0 } else { 1.0 },
                if *id == current {
                    palette.accent
                } else {
                    palette.border
                },
            ),
            StrokeKind::Middle,
        );
        let card_response = ui.interact(
            card,
            ui.scope_id().with(("overview-card", *id)),
            egui::Sense::click(),
        );
        if card_response.clicked() {
            state.overview.overview_workspace = Some(*id);
        }

        // Cabecera reservada: las ventanas nunca tapan el título del workspace.
        let content = egui::Rect::from_min_max(
            card.min + egui::vec2(6.0, 22.0),
            card.max - egui::vec2(6.0, 6.0),
        );
        text_at(
            ui,
            card.min + egui::vec2(10.0, 11.0),
            egui::Align2::LEFT_CENTER,
            format!("Workspace {id}"),
            TEXT_CAPTION,
            palette.muted,
        );
        let windows: Vec<_> = state
            .dock_clients
            .iter()
            .filter(|c| c.workspace == *id as i32)
            .collect();
        for (j, win) in windows.iter().enumerate() {
            let wx = content.min.x + win.x as f32 / monitor_w * content.width();
            let wy = content.min.y + win.y as f32 / monitor_h * content.height();
            let ww = (win.w as f32 / monitor_w * content.width()).max(8.0);
            let wh = (win.h as f32 / monitor_h * content.height()).max(8.0);
            let rect = egui::Rect::from_min_size(egui::pos2(wx, wy), egui::vec2(ww, wh))
                .intersect(content);
            let active_window = win.address == state.snapshot.window_address;
            let response = ui.interact(
                rect,
                ui.scope_id().with(("overview-win", j, *id)),
                egui::Sense::click(),
            );
            painter.rect_filled(
                rect,
                CornerRadius::same(4),
                if active_window {
                    palette.badge.gamma_multiply(1.4)
                } else {
                    palette.badge
                },
            );
            painter.rect_stroke(
                rect,
                CornerRadius::same(4),
                Stroke::new(
                    1.0,
                    if active_window {
                        palette.accent
                    } else if response.hovered() {
                        palette.text
                    } else {
                        palette.border
                    },
                ),
                StrokeKind::Middle,
            );
            let icon_center = rect.min + egui::vec2(9.0, 9.0);
            if let Some(texture) = state.app_icon_textures.get(&win.class.to_lowercase()) {
                egui::Image::new(egui::load::SizedTexture::new(
                    texture.id(),
                    egui::vec2(12.0, 12.0),
                ))
                .paint_at(
                    ui,
                    egui::Rect::from_center_size(icon_center, egui::vec2(12.0, 12.0)),
                );
            } else {
                painter.text(
                    icon_center,
                    egui::Align2::CENTER_CENTER,
                    dock_icon(&win.class),
                    icon_font(10.0),
                    palette.text,
                );
            }
            if rect.width() > 70.0 {
                painter.text(
                    rect.min + egui::vec2(18.0, 9.0),
                    egui::Align2::LEFT_CENTER,
                    truncate(&win.title, 18),
                    egui::FontId::new(9.0, egui::FontFamily::Proportional),
                    palette.text,
                );
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if response.clicked() {
                state.overview.overview_focus = Some(win.address.clone());
            }
        }
    }
}
