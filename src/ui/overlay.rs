//! Piezas visuales compartidas por los overlays a pantalla completa (overview y actividades):
//! buscador con chip `ESC`, barra de teclas, sombra y ventana esquemática.

use super::*;

/// ¿El cursor se movió en este frame? Evita que una tarjeta quieta bajo el cursor robe la
/// selección que se acaba de mover con el teclado.
pub(super) fn pointer_moved(ui: &egui::Ui) -> bool {
    ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO)
}

/// Ancho en píxeles de un texto (para colocar chips y etiquetas).
fn text_width(ui: &egui::Ui, text: &str, size: f32) -> f32 {
    ui.painter()
        .layout_no_wrap(
            text.to_string(),
            egui::FontId::new(size, egui::FontFamily::Proportional),
            egui::Color32::WHITE,
        )
        .size()
        .x
}

/// Píldora de búsqueda con el chip `ESC`. Muestra lo que se escribe (las teclas las recoge
/// `handle_key`).
pub(super) fn search_pill(ui: &egui::Ui, screen: egui::Rect, query: &str, placeholder: &str) {
    let p = pal();
    let w = (screen.width() * 0.78).min(400.0);
    let r = egui::Rect::from_center_size(
        egui::pos2(screen.center().x, screen.min.y + 33.0),
        egui::vec2(w, 34.0),
    );
    drop_shadow(ui, r, 17, 10, 30, 100);
    ui.painter().rect(
        r,
        CornerRadius::same(17),
        Palette::with_alpha(p.solid(), 232),
        Stroke::new(1.0, if query.is_empty() { p.border } else { p.accent }),
        StrokeKind::Middle,
    );
    icon_at(
        ui,
        egui::pos2(r.min.x + 17.0, r.center().y),
        ic::MAGNIFYING_GLASS,
        ICON_MD,
        p.muted,
    );
    let (text, color) = if query.is_empty() {
        (placeholder.to_string(), p.dim)
    } else {
        (format!("{query}|"), p.text)
    };
    text_at(
        ui,
        egui::pos2(r.min.x + 32.0, r.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        TEXT_BODY,
        color,
    );
    let esc_w = text_width(ui, "ESC", 8.5) + 12.0;
    let chip = egui::Rect::from_center_size(
        egui::pos2(r.max.x - 14.0 - esc_w / 2.0, r.center().y),
        egui::vec2(esc_w, 16.0),
    );
    ui.painter().rect(
        chip,
        CornerRadius::same(5),
        Palette::sheen(8),
        Stroke::new(1.0, p.border),
        StrokeKind::Middle,
    );
    text_at(
        ui,
        chip.center(),
        egui::Align2::CENTER_CENTER,
        "ESC",
        8.5,
        p.muted,
    );
}

/// Barra de ayuda inferior con las teclas dibujadas como chips.
pub(super) fn hint_bar(ui: &egui::Ui, screen: egui::Rect, items: &[(&str, &str)]) {
    let p = pal();
    let (size, pad, gap_in, gap_out) = (9.5, 6.0, 5.0, 16.0);
    let widths: Vec<(f32, f32)> = items
        .iter()
        .map(|(key, label)| {
            (
                text_width(ui, key, size) + pad * 2.0,
                text_width(ui, label, size),
            )
        })
        .collect();
    let total: f32 = widths.iter().map(|(k, l)| k + gap_in + l).sum::<f32>()
        + gap_out * (items.len().saturating_sub(1)) as f32;
    let y = screen.max.y - 18.0;
    let mut x = screen.center().x - total / 2.0;
    for ((key, label), (kw, lw)) in items.iter().zip(widths) {
        let chip = egui::Rect::from_min_size(egui::pos2(x, y - 8.0), egui::vec2(kw, 16.0));
        ui.painter().rect(
            chip,
            CornerRadius::same(4),
            Palette::sheen(8),
            Stroke::new(1.0, p.border),
            StrokeKind::Middle,
        );
        text_at(
            ui,
            chip.center(),
            egui::Align2::CENTER_CENTER,
            *key,
            size,
            p.muted,
        );
        x += kw + gap_in;
        text_at(
            ui,
            egui::pos2(x, y),
            egui::Align2::LEFT_CENTER,
            *label,
            size,
            p.dim,
        );
        x += lw + gap_out;
    }
}

/// Ventana esquemática dentro de una miniatura: gris translúcido; la activa, teñida de acento.
pub(super) fn mini_window(
    ui: &egui::Ui,
    rect: egui::Rect,
    active: bool,
    hovered: bool,
    radius: u8,
) {
    let p = pal();
    let (fill, stroke) = if hovered {
        (Palette::with_alpha(p.muted, 80), p.text)
    } else if active {
        (
            Palette::with_alpha(p.accent, 77),
            Palette::with_alpha(p.accent, 115),
        )
    } else {
        (Palette::with_alpha(p.muted, 56), Palette::sheen(23))
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(radius),
        fill,
        Stroke::new(1.0, stroke),
        StrokeKind::Middle,
    );
}

/// Imagen del monitor (mismo aspecto que la pantalla) centrada dentro de `area`.
pub(super) fn monitor_rect(area: egui::Rect, aspect: f32) -> egui::Rect {
    let (w, h) = if area.width() / area.height().max(1.0) > aspect {
        (area.height() * aspect, area.height())
    } else {
        (area.width(), area.width() / aspect)
    };
    egui::Rect::from_center_size(area.center(), egui::vec2(w.max(1.0), h.max(1.0)))
}
