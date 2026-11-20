//! Command menu: lanzador, archivos, workspaces y comandos.

use super::*;

/// Estado del command menu.
#[derive(Default)]
pub struct MenuState {
    pub menu_open: bool,
    pub menu_mode: crate::launcher::MenuMode,
    pub menu_query: String,
    pub menu_selected: usize,
    pub menu_entries: Vec<crate::launcher::Entry>,
}

/// Recarga las entradas según el modo actual.
pub(super) fn refresh_entries(state: &mut UiState) {
    use crate::launcher::{self, MenuMode};
    state.menu.menu_entries = match state.menu.menu_mode {
        MenuMode::Apps => launcher::apps(),
        MenuMode::Files => launcher::files(&config::get().menu.file_dirs),
        MenuMode::Workspaces => launcher::workspaces(&state.shared.snapshot.occupied),
        MenuMode::Commands => launcher::commands(),
    };
    state.menu.menu_selected = 0;
}

/// Teclado del menú abierto: navegación, modos, búsqueda y ejecución.
pub(super) fn handle_key(state: &mut UiState, key: smithay_client_toolkit::seat::keyboard::Keysym) {
    use crate::launcher::{filter, MenuMode};
    use smithay_client_toolkit::seat::keyboard::Keysym;

    let count = filter(&state.menu.menu_entries, &state.menu.menu_query).len();
    match key {
        Keysym::Escape => state.menu.menu_open = false,
        Keysym::Up => state.menu.menu_selected = state.menu.menu_selected.saturating_sub(1),
        Keysym::Down => {
            if count > 0 {
                state.menu.menu_selected = (state.menu.menu_selected + 1).min(count - 1);
            }
        }
        Keysym::Tab => {
            state.menu.menu_mode = state.menu.menu_mode.next();
            state.menu.menu_query.clear();
            refresh_entries(state);
        }
        Keysym::BackSpace => {
            state.menu.menu_query.pop();
            state.menu.menu_selected = 0;
        }
        Keysym::Return => {
            if let Some(entry) = filter(&state.menu.menu_entries, &state.menu.menu_query)
                .into_iter()
                .nth(state.menu.menu_selected)
            {
                let action = entry.action;
                state.shared.menu_action = Some(action);
                state.menu.menu_open = false;
            }
        }
        Keysym::_1 | Keysym::_2 | Keysym::_3 | Keysym::_4 => {
            let index = match key {
                Keysym::_1 => 0,
                Keysym::_2 => 1,
                Keysym::_3 => 2,
                _ => 3,
            };
            state.menu.menu_mode = MenuMode::ALL[index];
            state.menu.menu_query.clear();
            refresh_entries(state);
        }
        _ => {
            let text = xkbcommon::xkb::keysym_to_utf8(key);
            if text.chars().count() == 1 && !text.chars().next().unwrap().is_control() {
                state.menu.menu_query.push_str(&text);
                state.menu.menu_selected = 0;
            }
        }
    }
}

/// Command menu: lanzador y buscador, con pestañas de modo.
pub(super) fn draw_command_menu(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    use crate::launcher::{filter, MenuMode};

    let screen = ui.max_rect();
    ui.painter()
        .rect_filled(screen, CornerRadius::same(0), Palette::scrim(205));

    let w = 560.0f32.min(screen.width() - 60.0);
    let h = 520.0f32.min(screen.height() - 60.0);
    let card = egui::Rect::from_center_size(screen.center(), egui::vec2(w, h));
    ui.painter().rect(
        card,
        CornerRadius::same(18),
        pal().card_fill,
        Stroke::new(1.0, pal().border),
        StrokeKind::Middle,
    );

    let x = card.min.x + 18.0;
    let mut y = card.min.y;

    // ── Buscador ──
    y += 17.0;
    ui.painter().text(
        egui::pos2(x + 9.0, y + 9.0),
        egui::Align2::CENTER_CENTER,
        ic::MAGNIFYING_GLASS,
        icon_font(ICON_MD),
        palette.muted,
    );
    let query = if state.menu.menu_query.is_empty() {
        "Busca apps, archivos, comandos…".to_string()
    } else {
        state.menu.menu_query.clone()
    };
    ui.painter().text(
        egui::pos2(x + 28.0, y + 9.0),
        egui::Align2::LEFT_CENTER,
        query,
        egui::FontId::new(15.0, egui::FontFamily::Proportional),
        if state.menu.menu_query.is_empty() {
            palette.border
        } else {
            palette.text
        },
    );
    // Cursor parpadeante simple.
    if !state.menu.menu_query.is_empty() {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(x + 30.0 + state.menu.menu_query.len() as f32 * 7.6, y + 2.0),
                egui::vec2(1.5, 14.0),
            ),
            CornerRadius::same(0),
            palette.accent,
        );
    }
    ui.painter().text(
        egui::pos2(card.max.x - 26.0, y + 9.0),
        egui::Align2::RIGHT_CENTER,
        "ESC",
        egui::FontId::new(9.0, egui::FontFamily::Proportional),
        palette.muted,
    );
    y += 34.0;
    ui.painter().line_segment(
        [
            egui::pos2(card.min.x + 1.0, y),
            egui::pos2(card.max.x - 1.0, y),
        ],
        Stroke::new(1.0, pal().border),
    );

    // ── Modos ──
    y += 11.0;
    let chip_w = (w - 36.0 - 15.0) / 4.0;
    for (i, mode) in MenuMode::ALL.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            egui::pos2(x + i as f32 * (chip_w + 5.0), y),
            egui::vec2(chip_w, 28.0),
        );
        let active = *mode == state.menu.menu_mode;
        if active {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(8), pal().hover);
        }
        let label = format!("{}  {}", mode.label(), mode.shortcut());
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::new(10.0, egui::FontFamily::Proportional),
            if active { palette.text } else { palette.muted },
        );
        let r = ui.interact(
            rect,
            ui.scope_id().with(("menu-mode", i)),
            egui::Sense::click(),
        );
        if r.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if r.clicked() {
            state.menu.menu_mode = *mode;
            state.menu.menu_query.clear();
            state.menu.menu_entries.clear();
        }
    }
    y += 40.0;
    ui.painter().line_segment(
        [
            egui::pos2(card.min.x + 1.0, y),
            egui::pos2(card.max.x - 1.0, y),
        ],
        Stroke::new(1.0, pal().border),
    );

    // ── Resultados ──
    y += 10.0;
    if state.menu.menu_entries.is_empty() {
        state.menu.menu_entries = match state.menu.menu_mode {
            MenuMode::Apps => crate::launcher::apps(),
            MenuMode::Files => crate::launcher::files(&config::get().menu.file_dirs),
            MenuMode::Workspaces => crate::launcher::workspaces(&state.shared.snapshot.occupied),
            MenuMode::Commands => crate::launcher::commands(),
        };
    }
    let results = filter(&state.menu.menu_entries, &state.menu.menu_query);
    let row_h = 48.0;
    let max_rows = ((card.max.y - 52.0 - y) / row_h).floor() as usize;
    for (i, entry) in results.iter().take(max_rows).enumerate() {
        let rect = egui::Rect::from_min_size(
            egui::pos2(card.min.x + 9.0, y + i as f32 * row_h),
            egui::vec2(w - 18.0, row_h - 4.0),
        );
        let selected = i == state.menu.menu_selected;
        if selected {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(10), palette.accent_soft);
        }
        let icon_rect = egui::Rect::from_center_size(
            rect.min + egui::vec2(26.0, rect.height() / 2.0),
            egui::vec2(30.0, 30.0),
        );
        if let Some(texture) = state.shared.icon(ui.ctx(), &entry.icon_key) {
            egui::Image::new(egui::load::SizedTexture::new(
                texture.id(),
                icon_rect.size(),
            ))
            .corner_radius(8.0)
            .paint_at(ui, icon_rect);
        } else {
            ui.painter()
                .rect_filled(icon_rect, CornerRadius::same(8), palette.badge);
            let glyph = match entry.icon_key.as_str() {
                "folder" => ic::FOLDER,
                "file" => ic::FILE,
                "workspace" => ic::SQUARES_FOUR,
                _ => ic::APP_WINDOW,
            };
            ui.painter().text(
                icon_rect.center(),
                egui::Align2::CENTER_CENTER,
                glyph,
                icon_font(14.0),
                palette.text,
            );
        }
        ui.painter().text(
            egui::pos2(rect.min.x + 48.0, rect.center().y - 7.0),
            egui::Align2::LEFT_CENTER,
            truncate(&entry.title, 40),
            egui::FontId::new(12.0, egui::FontFamily::Proportional),
            palette.text,
        );
        ui.painter().text(
            egui::pos2(rect.min.x + 48.0, rect.center().y + 7.0),
            egui::Align2::LEFT_CENTER,
            truncate(&entry.subtitle, 52),
            egui::FontId::new(9.5, egui::FontFamily::Proportional),
            palette.muted,
        );
        if selected {
            ui.painter().text(
                egui::pos2(rect.max.x - 14.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                "↵",
                egui::FontId::new(11.0, egui::FontFamily::Proportional),
                palette.accent,
            );
        }
        let r = ui.interact(
            rect,
            ui.scope_id().with(("menu-result", i)),
            egui::Sense::click(),
        );
        if r.hovered() {
            state.menu.menu_selected = i;
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if r.clicked() {
            state.shared.menu_action = Some(entry.action.clone());
            state.menu.menu_open = false;
        }
    }

    // ── Pie ──
    let footer_y = card.max.y - 26.0;
    ui.painter().line_segment(
        [
            egui::pos2(card.min.x + 1.0, footer_y - 8.0),
            egui::pos2(card.max.x - 1.0, footer_y - 8.0),
        ],
        Stroke::new(1.0, pal().border),
    );
    ui.painter().text(
        egui::pos2(x, footer_y),
        egui::Align2::LEFT_CENTER,
        "↑↓ navegar   ↵ abrir   Tab modo   Esc cerrar",
        egui::FontId::new(9.5, egui::FontFamily::Proportional),
        palette.muted,
    );
    ui.painter().text(
        egui::pos2(card.max.x - 18.0, footer_y),
        egui::Align2::RIGHT_CENTER,
        format!("{} resultados", results.len()),
        egui::FontId::new(9.5, egui::FontFamily::Proportional),
        palette.muted,
    );
}
