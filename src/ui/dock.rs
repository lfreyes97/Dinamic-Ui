//! Dock inferior: modelo (pinned + clientes) y vista autohide.

use super::*;

/// Estado del dock: animación, visibilidad y menú de ventanas.
#[derive(Default)]
pub struct DockState {
    pub dock_progress: f32,
    pub dock_visible: bool,
    pub dock_action: Option<DockAction>,
    pub dock_menu: Option<usize>,
}

impl DockState {
    pub fn new() -> Self {
        let shown = std::env::var_os("DINAMIC_UI_DOCK").is_some();
        Self {
            dock_progress: if shown { 1.0 } else { 0.0 },
            dock_visible: shown,
            ..Self::default()
        }
    }
}

struct DockItem {
    name: String,
    command: String,
    windows: Vec<crate::ipc::ClientInfo>,
}
/// Icono Phosphor según la clase de la ventana (o el comando fijado).
pub(super) fn dock_icon(name: &str) -> &'static str {
    let n = name.to_lowercase();
    if n.contains("firefox") || n.contains("chrome") || n.contains("chromium") {
        ic::GLOBE
    } else if n.contains("code") || n.contains("codium") {
        ic::CODE
    } else if n.contains("kitty") || n.contains("term") || n.contains("alacritty") {
        ic::TERMINAL
    } else if n.contains("mail") || n.contains("thunderbird") {
        ic::ENVELOPE
    } else if n.contains("spotify") || n.contains("music") {
        ic::MUSIC_NOTES
    } else if n.contains("nautilus") || n.contains("thunar") || n.contains("files") {
        ic::FOLDER
    } else if n.contains("obs") || n.contains("camera") {
        ic::CAMERA
    } else {
        ic::APP_WINDOW
    }
}

/// Dock inferior: se desliza desde abajo cuando el cursor llega al borde.
pub(super) fn draw_dock(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let cfg = &config::get().dock;
    if !cfg.enabled || state.dock.dock_progress <= 0.01 {
        return;
    }
    let screen = ui.max_rect();

    // Un elemento por app fijada + uno por clase en ejecución, con TODAS sus
    // ventanas para poder moverse entre ellas.
    let mut items: Vec<DockItem> = Vec::new();
    for pinned in &cfg.pinned {
        let key = pinned.to_lowercase();
        let windows: Vec<_> = state
            .dock_clients
            .iter()
            .filter(|c| c.class.to_lowercase() == key)
            .cloned()
            .collect();
        items.push(DockItem {
            name: pinned.clone(),
            command: pinned.clone(),
            windows,
        });
    }
    for client in &state.dock_clients {
        let key = client.class.to_lowercase();
        if !items.iter().any(|it| it.name.to_lowercase() == key) {
            let windows: Vec<_> = state
                .dock_clients
                .iter()
                .filter(|c| c.class.to_lowercase() == key)
                .cloned()
                .collect();
            items.push(DockItem {
                name: client.class.clone(),
                command: client.class.clone(),
                windows,
            });
        }
    }
    if items.is_empty() {
        return;
    }

    // Pedir iconos reales y subir los que ya vinieron del hilo.
    for item in &items {
        state.app_icons.request(&item.name);
    }
    for (class, rgba) in state.app_icons.drain() {
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [
                crate::appicons::SIZE as usize,
                crate::appicons::SIZE as usize,
            ],
            &rgba,
        );
        let handle = ui.ctx().load_texture(
            format!("appicon-{class}"),
            image,
            egui::TextureOptions::LINEAR,
        );
        state.app_icon_textures.insert(class, handle);
    }

    const ITEM: f32 = 42.0;
    const GAP: f32 = 6.0;
    const H: f32 = 56.0;
    const BOTTOM: f32 = 10.0;
    let w = 14.0 + items.len() as f32 * (ITEM + GAP) - GAP;
    let hidden = (1.0 - state.dock.dock_progress) * (H + BOTTOM + 12.0);
    let dock = egui::Rect::from_min_size(
        egui::pos2(
            ((screen.width() - w) / 2.0).round(),
            screen.max.y - BOTTOM - H + hidden,
        ),
        egui::vec2(w, H),
    );

    ui.painter().rect(
        dock,
        CornerRadius::same(16),
        Color32::from_rgba_unmultiplied(10, 10, 10, 245),
        Stroke::new(1.0, Color32::from_gray(0x33)),
        StrokeKind::Middle,
    );

    // Rectángulos absolutos: el centro vertical es exactamente dock.h / 2.
    ui.set_opacity(state.dock.dock_progress);
    let icon_y = dock.center().y - ITEM / 2.0;
    for (i, item) in items.iter().enumerate() {
        let name = &item.name;
        let running = !item.windows.is_empty();
        let rect = egui::Rect::from_min_size(
            egui::pos2(dock.min.x + 7.0 + i as f32 * (ITEM + GAP), icon_y),
            egui::vec2(ITEM, ITEM),
        );
        let active = state.snapshot.window_class.eq_ignore_ascii_case(name);
        let color = if active {
            Color32::from_gray(17)
        } else {
            palette.text
        };
        let mut response = ui.interact(
            rect,
            ui.scope_id().with(("dock-item", i)),
            egui::Sense::click(),
        );

        let pressed = response.is_pointer_button_down_on();
        let fill = if pressed {
            palette.border
        } else if active {
            palette.accent
        } else if running {
            palette.badge
        } else {
            Color32::from_gray(0x18)
        };
        ui.painter().rect_filled(rect, CornerRadius::same(12), fill);

        let icon_size = if pressed { 22.0 } else { 26.0 };
        let icon_rect =
            egui::Rect::from_center_size(rect.center(), egui::vec2(icon_size, icon_size));
        if let Some(texture) = state.app_icon_textures.get(&name.to_lowercase()) {
            egui::Image::new(egui::load::SizedTexture::new(
                texture.id(),
                icon_rect.size(),
            ))
            .paint_at(ui, icon_rect);
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                dock_icon(name),
                icon_font(if pressed { 18.0 } else { 20.0 }),
                color,
            );
        }

        // Puntos: una ventana abierta por punto (máximo 3).
        let dots = item.windows.len().min(3);
        for d in 0..dots {
            let x = rect.min.x + ITEM / 2.0 - (dots as f32 - 1.0) * 3.0 + d as f32 * 6.0;
            ui.painter().circle_filled(
                egui::pos2(x, rect.max.y - 2.0),
                1.6,
                if active {
                    Color32::from_gray(17)
                } else {
                    palette.muted
                },
            );
        }

        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            let tip = item
                .windows
                .first()
                .map(|c| c.title.as_str())
                .unwrap_or(name.as_str())
                .to_string();
            response = response.on_hover_text(tip);
        }

        if response.clicked() {
            if item.windows.is_empty() {
                state.dock.dock_action = Some(DockAction::Launch(item.command.clone()));
            } else if item.windows.len() == 1 {
                state.dock.dock_action = Some(DockAction::Focus(item.windows[0].address.clone()));
            } else if active {
                // Ya está activa: ciclar a la siguiente ventana.
                let current = item
                    .windows
                    .iter()
                    .position(|w| w.address == state.snapshot.window_address)
                    .unwrap_or(0);
                let next = (current + 1) % item.windows.len();
                state.dock.dock_action =
                    Some(DockAction::Focus(item.windows[next].address.clone()));
            } else {
                state.dock.dock_menu = if state.dock.dock_menu == Some(i) {
                    None
                } else {
                    Some(i)
                };
            }
        }
        if response.secondary_clicked() && running {
            state.dock.dock_menu = Some(i);
        }
        if response.middle_clicked() {
            state.dock.dock_action = Some(DockAction::Launch(item.command.clone()));
        }
    }

    // Menú de ventanas de la app (se dibuja sobre el dock).
    if let Some(index) = state.dock.dock_menu {
        if let Some(item) = items.get(index) {
            let row_h = 24.0;
            let menu_w = 200.0;
            let menu_h = 26.0 + item.windows.len() as f32 * row_h + 26.0;
            let menu = egui::Rect::from_min_size(
                egui::pos2(
                    dock.min.x + 7.0 + index as f32 * (ITEM + GAP) - 40.0,
                    dock.min.y - menu_h - 6.0,
                ),
                egui::vec2(menu_w, menu_h),
            );
            ui.painter().rect(
                menu,
                CornerRadius::same(12),
                Color32::from_rgba_unmultiplied(12, 12, 12, 250),
                Stroke::new(1.0, Color32::from_gray(0x3a)),
                StrokeKind::Middle,
            );
            text_at(
                ui,
                menu.min + egui::vec2(12.0, 14.0),
                egui::Align2::LEFT_CENTER,
                truncate(&item.name, 22),
                TEXT_CAPTION,
                palette.muted,
            );
            for (j, win) in item.windows.iter().enumerate() {
                let row = egui::Rect::from_min_size(
                    menu.min + egui::vec2(6.0, 22.0 + j as f32 * row_h),
                    egui::vec2(menu_w - 12.0, row_h),
                );
                let r = ui.interact(
                    row,
                    ui.scope_id().with(("dock-menu-win", j)),
                    egui::Sense::click(),
                );
                if r.hovered() {
                    ui.painter()
                        .rect_filled(row, CornerRadius::same(6), palette.badge);
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                text_at(
                    ui,
                    row.min + egui::vec2(8.0, row.height() / 2.0),
                    egui::Align2::LEFT_CENTER,
                    truncate(&win.title, 24),
                    TEXT_BODY,
                    palette.text,
                );
                if r.clicked() {
                    state.dock.dock_action = Some(DockAction::Focus(win.address.clone()));
                    state.dock.dock_menu = None;
                }
            }
            let new_row = egui::Rect::from_min_size(
                menu.min + egui::vec2(6.0, menu_h - 28.0),
                egui::vec2(menu_w - 12.0, 22.0),
            );
            let r = ui.interact(
                new_row,
                ui.scope_id().with("dock-menu-new"),
                egui::Sense::click(),
            );
            if r.hovered() {
                ui.painter()
                    .rect_filled(new_row, CornerRadius::same(6), palette.badge);
            }
            text_at(
                ui,
                new_row.min + egui::vec2(8.0, new_row.height() / 2.0),
                egui::Align2::LEFT_CENTER,
                "Nueva ventana",
                TEXT_BODY,
                palette.muted,
            );
            if r.clicked() {
                state.dock.dock_action = Some(DockAction::Launch(item.command.clone()));
                state.dock.dock_menu = None;
            }
        }
    }
    ui.set_opacity(1.0);
}
