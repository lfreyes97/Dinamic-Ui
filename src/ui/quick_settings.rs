//! Controles interactivos de QuickSettings, compartidos por el overlay del
//! topbar y la pestaña System del panel. Solo encolan acciones y pintan el
//! semáforo; la ejecución vive en `main.rs` y en `actions.rs`.

use super::*;
use crate::actions::{ActionPhase, SettingAction, SettingId};

fn enabled(state: &UiState, key: &str) -> bool {
    state
        .shared
        .config
        .quick_settings
        .items
        .iter()
        .any(|i| i == key)
}

fn amber(state: &UiState, id: SettingId) -> bool {
    matches!(
        state.shared.settings.phase(id),
        Some(ActionPhase::AwaitingConfirmation | ActionPhase::InFlight)
    )
}

fn surface(
    ui: &egui::Ui,
    rect: egui::Rect,
    hovered: bool,
    active: bool,
    pending: bool,
    palette: Palette,
) {
    let fill = if pending {
        palette.accent_soft
    } else if hovered {
        Palette::sheen(22)
    } else if active {
        Palette::sheen(16)
    } else {
        Palette::sheen(10)
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(10),
        fill,
        egui::Stroke::new(1.0, palette.border),
        egui::StrokeKind::Middle,
    );
}

fn section(ui: &egui::Ui, rect: egui::Rect, y: f32, label: &str, palette: Palette) -> f32 {
    text_at(
        ui,
        egui::pos2(rect.min.x, y + 5.0),
        egui::Align2::LEFT_CENTER,
        label,
        TEXT_CAPTION,
        palette.dim,
    );
    y + 16.0
}

/// Tile cuadrado de conectividad: icono grande, título y estado.
#[allow(clippy::too_many_arguments)]
fn toggle_tile(
    ui: &mut egui::Ui,
    state: &mut UiState,
    rect: egui::Rect,
    id: SettingId,
    action: SettingAction,
    icon: &str,
    title: &str,
    subtitle: &str,
    on: bool,
    palette: Palette,
) {
    let pending = amber(state, id);
    let resp = ui.interact(
        rect,
        ui.scope_id().with(egui::IdSalt::new(("qs-tile", id as u8))),
        egui::Sense::click(),
    );
    surface(ui, rect, resp.hovered(), on, pending, palette);
    let cx = rect.center().x;
    icon_at(
        ui,
        egui::pos2(cx, rect.min.y + 17.0),
        icon,
        ICON_LG,
        if pending {
            palette.accent
        } else if on {
            palette.text
        } else {
            palette.muted
        },
    );
    text_at(
        ui,
        egui::pos2(cx, rect.min.y + 32.0),
        egui::Align2::CENTER_CENTER,
        title,
        TEXT_BODY,
        palette.text,
    );
    text_at(
        ui,
        egui::pos2(cx, rect.min.y + 45.0),
        egui::Align2::CENTER_CENTER,
        if pending { "…" } else { subtitle },
        TEXT_CAPTION,
        if pending {
            palette.accent
        } else {
            palette.muted
        },
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.clicked() && !pending {
        state.shared.settings.enqueue(action);
    }
}

/// Fila de interruptor para DND y acciones simples.
#[allow(clippy::too_many_arguments)]
fn toggle_row(
    ui: &mut egui::Ui,
    state: &mut UiState,
    rect: egui::Rect,
    id: SettingId,
    action: SettingAction,
    icon: &str,
    title: &str,
    subtitle: &str,
    on: bool,
    palette: Palette,
) {
    let pending = amber(state, id);
    let chip = egui::Rect::from_min_size(
        rect.min + egui::vec2(0.0, (rect.height() - 26.0) / 2.0),
        egui::vec2(26.0, 26.0),
    );
    surface(ui, chip, false, on, pending, palette);
    icon_at(
        ui,
        chip.center(),
        icon,
        ICON_MD,
        if pending {
            palette.accent
        } else if on {
            palette.text
        } else {
            palette.muted
        },
    );
    text_at(
        ui,
        egui::pos2(rect.min.x + 36.0, rect.center().y - 6.0),
        egui::Align2::LEFT_CENTER,
        title,
        TEXT_BODY,
        palette.text,
    );
    text_at(
        ui,
        egui::pos2(rect.min.x + 36.0, rect.center().y + 8.0),
        egui::Align2::LEFT_CENTER,
        if pending { "… esperando" } else { subtitle },
        TEXT_CAPTION,
        if pending {
            palette.accent
        } else {
            palette.muted
        },
    );
    // Interruptor dibujado a la derecha: pista visual del estado, no un widget extra.
    let sw = egui::Rect::from_min_size(
        egui::pos2(rect.max.x - 32.0, rect.center().y - 8.0),
        egui::vec2(32.0, 16.0),
    );
    ui.painter().rect(
        sw,
        egui::CornerRadius::same(8),
        if on { palette.accent } else { palette.track },
        egui::Stroke::new(1.0, palette.border),
        egui::StrokeKind::Middle,
    );
    ui.painter().circle_filled(
        egui::pos2(
            if on { sw.max.x - 8.0 } else { sw.min.x + 8.0 },
            sw.center().y,
        ),
        5.0,
        if on { palette.solid() } else { palette.muted },
    );

    let resp = ui.interact(
        rect,
        ui.scope_id().with(egui::IdSalt::new(("qs-row", id as u8))),
        egui::Sense::click(),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.clicked() && !pending {
        #[cfg(feature = "notifications")]
        if let SettingAction::Dnd(_) = action {
            state.notifications.toggle_dnd = true;
        }
        state.shared.settings.enqueue(action);
    }
}

fn range_row(
    ui: &mut egui::Ui,
    state: &mut UiState,
    rect: egui::Rect,
    id: SettingId,
    label: &str,
    value: Option<u8>,
    palette: Palette,
) {
    let pending = amber(state, id);
    let pct = value.unwrap_or(0) as f32 / 100.0;
    text_at(
        ui,
        egui::pos2(rect.min.x, rect.center().y - 6.0),
        egui::Align2::LEFT_CENTER,
        label,
        TEXT_BODY,
        palette.text,
    );
    text_at(
        ui,
        egui::pos2(rect.max.x, rect.center().y - 6.0),
        egui::Align2::RIGHT_CENTER,
        if pending {
            "…".to_string()
        } else {
            value.map_or("—".to_string(), |v| format!("{v}%"))
        },
        TEXT_CAPTION,
        if pending {
            palette.accent
        } else {
            palette.muted
        },
    );
    let bar = egui::Rect::from_min_size(
        egui::pos2(rect.min.x, rect.max.y - 8.0),
        egui::vec2(rect.width(), 5.0),
    );
    bar_at(
        ui,
        bar,
        pct,
        palette.track,
        if pending {
            palette.accent_soft
        } else {
            palette.accent
        },
    );
    if value.is_none() {
        return;
    }
    let resp = ui.interact(
        rect,
        ui.scope_id()
            .with(egui::IdSalt::new(("qs-range", id as u8))),
        egui::Sense::click(),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.clicked() && !pending {
        let x = ui
            .ctx()
            .input(|i| i.pointer.hover_pos().map(|p| p.x))
            .unwrap_or(rect.min.x);
        let pct = ((x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
        let value = (pct * 100.0).round().clamp(0.0, 100.0) as u8;
        let action = match id {
            SettingId::Volume => SettingAction::Volume(value),
            SettingId::Brightness => SettingAction::Brightness(value),
            _ => return,
        };
        state.shared.settings.enqueue(action);
    }
}

fn lock_row(ui: &mut egui::Ui, state: &mut UiState, rect: egui::Rect, palette: Palette) {
    let pending = amber(state, SettingId::Lock);
    let chip = egui::Rect::from_min_size(
        rect.min + egui::vec2(0.0, (rect.height() - 26.0) / 2.0),
        egui::vec2(26.0, 26.0),
    );
    surface(ui, chip, false, false, pending, palette);
    icon_at(
        ui,
        chip.center(),
        ic::LOCK_KEY,
        ICON_MD,
        if pending {
            palette.accent
        } else {
            palette.text
        },
    );
    text_at(
        ui,
        egui::pos2(rect.min.x + 36.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        if pending {
            "¿Bloquear sesión?"
        } else {
            "Bloquear sesión"
        },
        TEXT_BODY,
        if pending {
            palette.accent
        } else {
            palette.text
        },
    );
    if pending {
        let confirm = egui::Rect::from_min_size(
            egui::pos2(rect.max.x - 100.0, rect.center().y - 10.0),
            egui::vec2(46.0, 20.0),
        );
        let cancel = confirm.translate(egui::vec2(50.0, 0.0));
        if ui
            .put(confirm, egui::Button::new("Sí").corner_radius(6.0))
            .clicked()
        {
            state.shared.settings.confirm(SettingId::Lock);
        }
        if ui
            .put(cancel, egui::Button::new("No").corner_radius(6.0))
            .clicked()
        {
            state.shared.settings.cancel(SettingId::Lock);
        }
    } else {
        icon_at(
            ui,
            egui::pos2(rect.max.x - 12.0, rect.center().y),
            ic::CARET_RIGHT,
            ICON_XS,
            palette.muted,
        );
        let resp = ui.interact(
            rect,
            ui.scope_id().with(egui::IdSalt::new("qs-lock")),
            egui::Sense::click(),
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() {
            state.shared.settings.enqueue(SettingAction::Lock);
        }
    }
}

/// Dibuja los controles en el rect que reciba el host (topbar o panel).
pub(super) fn draw(ui: &mut egui::Ui, rect: egui::Rect, state: &mut UiState, palette: Palette) {
    let mut y = rect.min.y + 2.0;
    let row = |y: f32| {
        egui::Rect::from_min_size(egui::pos2(rect.min.x, y), egui::vec2(rect.width(), 30.0))
    };

    // Conectividad: dos tiles que se leen como interruptores de un vistazo.
    if enabled(state, "wifi") || enabled(state, "bluetooth") {
        y = section(ui, rect, y, "CONECTIVIDAD", palette);
        let w = (rect.width() - 8.0) / 2.0;
        let tiles_y = y;
        let tile = |col: f32| {
            egui::Rect::from_min_size(
                egui::pos2(rect.min.x + col * (w + 8.0), tiles_y),
                egui::vec2(w, 52.0),
            )
        };
        if enabled(state, "wifi") {
            let wifi = state.shared.system.wifi.as_ref();
            let on = wifi.is_some_and(|w| w.enabled);
            let sub = match wifi.and_then(|w| w.ssid.as_deref()) {
                Some(ssid) if on => ssid.to_string(),
                Some(_) => "Sin conexión".into(),
                None => "Sin datos".into(),
            };
            toggle_tile(
                ui,
                state,
                tile(0.0),
                SettingId::Wifi,
                SettingAction::Wifi(!on),
                if on { ic::WIFI_HIGH } else { ic::WIFI_SLASH },
                "Wi-Fi",
                &sub,
                on,
                palette,
            );
        }
        if enabled(state, "bluetooth") {
            let bt = state.shared.system.bluetooth.as_ref();
            let on = bt.is_some_and(|b| b.powered);
            let sub = match bt {
                None => "Sin datos".into(),
                Some(b) if b.connected > 0 => format!(
                    "{} conectado{}",
                    b.connected,
                    if b.connected == 1 { "" } else { "s" }
                ),
                Some(_) if on => "Activado".into(),
                Some(_) => "Apagado".into(),
            };
            toggle_tile(
                ui,
                state,
                tile(1.0),
                SettingId::Bluetooth,
                SettingAction::Bluetooth(!on),
                if on {
                    ic::BLUETOOTH
                } else {
                    ic::BLUETOOTH_SLASH
                },
                "Bluetooth",
                &sub,
                on,
                palette,
            );
        }
        y += 52.0 + 12.0;
    }

    if enabled(state, "volume") || enabled(state, "brightness") {
        y = section(ui, rect, y, "NIVELES", palette);
        if enabled(state, "volume") {
            range_row(
                ui,
                state,
                row(y),
                SettingId::Volume,
                "Volumen",
                state.shared.system.volume.as_ref().map(|v| v.percent),
                palette,
            );
            y += 34.0;
        }
        if enabled(state, "brightness") {
            range_row(
                ui,
                state,
                row(y),
                SettingId::Brightness,
                "Brillo",
                state.shared.system.brightness,
                palette,
            );
            y += 34.0;
        }
        y += 8.0;
    }

    if enabled(state, "dnd") || enabled(state, "lock") {
        y = section(ui, rect, y, "SISTEMA", palette);
        #[cfg(feature = "notifications")]
        if enabled(state, "dnd") {
            let dnd = state.notifications.dnd;
            toggle_row(
                ui,
                state,
                row(y),
                SettingId::Dnd,
                SettingAction::Dnd(!dnd),
                if dnd { ic::MOON } else { ic::BELL },
                "No molestar",
                if dnd { "Activado" } else { "Desactivado" },
                dnd,
                palette,
            );
            y += 34.0;
        }
        if enabled(state, "lock") {
            lock_row(ui, state, row(y), palette);
        }
    }
}
