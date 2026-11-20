//! Tarjeta de notificación bajo la barra.

use super::*;

/// La isla solo muestra workspace, reloj y batería, con `space-between`.
pub(super) fn draw_notification_card(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    state: &mut UiState,
    palette: Palette,
) {
    ui.painter().rect(
        rect,
        CornerRadius::same(14),
        Color32::from_rgba_unmultiplied(10, 10, 10, 250),
        Stroke::new(1.0, Color32::from_gray(0x33)),
        StrokeKind::Middle,
    );
    let inner = rect.shrink(10.0);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::LEFT))
            .id_salt("notification-card"),
    );
    child.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.horizontal(|ui| {
            ui.label(icon_bold(ic::BELL, ICON_SM, palette.accent));
            ui.add_space(SP_S);
            if let Some(notification) = &state.notification {
                ui.label(
                    RichText::new(truncate(&notification.app, 20))
                        .size(TEXT_CAPTION)
                        .family(egui::FontFamily::Name(FONT_SEMIBOLD.into()))
                        .color(palette.muted),
                );
                ui.label(
                    RichText::new(&notification.time)
                        .size(TEXT_CAPTION)
                        .color(palette.muted),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if state.notification_count > 0 {
                    ui.label(
                        RichText::new(format!("×{}", state.notification_count))
                            .size(TEXT_CAPTION)
                            .family(egui::FontFamily::Name(FONT_SEMIBOLD.into()))
                            .color(palette.accent),
                    )
                    .on_hover_text("Notificaciones esta sesión");
                }
                let close = egui::Button::new(icon_bold(ic::X, ICON_XS, palette.muted))
                    .frame(false)
                    .corner_radius(5.0);
                if ui.add(close).clicked() {
                    state.dismiss_notification = true;
                }
            });
        });
        if let Some(notification) = &state.notification {
            ui.label(
                RichText::new(truncate(&notification.summary, 38))
                    .size(TEXT_BODY)
                    .family(egui::FontFamily::Name(FONT_SEMIBOLD.into()))
                    .color(palette.text),
            );
            if !notification.body.is_empty() {
                ui.label(
                    RichText::new(truncate(&notification.body, 48))
                        .size(TEXT_CAPTION)
                        .color(palette.muted),
                );
            }
        }
    });
}
