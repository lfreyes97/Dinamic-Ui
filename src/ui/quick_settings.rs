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

fn chip(ui: &egui::Ui, rect: egui::Rect, icon: &str, palette: Palette, on: bool, pending: bool) {
    let fill = if pending {
        palette.accent_soft
    } else if on {
        palette.badge
    } else {
        Palette::sheen(10)
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(7),
        fill,
        egui::Stroke::new(1.0, palette.border),
        egui::StrokeKind::Middle,
    );
    icon_at(
        ui,
        rect.center(),
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
}

#[allow(clippy::too_many_arguments)]
fn toggle_row(
    ui: &mut egui::Ui,
    state: &mut UiState,
    rect: egui::Rect,
    id: SettingId,
    action: SettingAction,
    icon: &str,
    label: &str,
    subtitle: &str,
    on: bool,
    palette: Palette,
) {
    let pending = amber(state, id);
    let chip_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(0.0, (rect.height() - 26.0) / 2.0),
        egui::vec2(26.0, 26.0),
    );
    chip(ui, chip_rect, icon, palette, on, pending);
    text_at(
        ui,
        egui::pos2(rect.min.x + 36.0, rect.center().y - 7.0),
        egui::Align2::LEFT_CENTER,
        label,
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

    let resp = ui.interact(
        rect,
        ui.scope_id().with(egui::IdSalt::new(("qs", id as u8))),
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
        egui::pos2(rect.min.x, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        TEXT_BODY,
        palette.text,
    );
    text_at(
        ui,
        egui::pos2(rect.max.x, rect.center().y),
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
        egui::vec2(rect.width(), 4.0),
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

/// Dibuja los controles en el rect que reciba el host (topbar o panel).
pub(super) fn draw(ui: &mut egui::Ui, rect: egui::Rect, state: &mut UiState, palette: Palette) {
    let row_h = 32.0;
    let mut y = rect.min.y + 4.0;
    let row = |y: f32| {
        egui::Rect::from_min_size(
            egui::pos2(rect.min.x, y),
            egui::vec2(rect.width(), row_h - 2.0),
        )
    };

    if enabled(state, "wifi") {
        let wifi = state.shared.system.wifi.as_ref();
        let wifi_on = wifi.is_some_and(|w| w.enabled);
        let wifi_sub = match wifi.and_then(|w| w.ssid.as_deref()) {
            Some(ssid) if wifi_on => ssid.to_string(),
            Some(_) => "Sin conexión".to_string(),
            None => "Sin datos".to_string(),
        };
        toggle_row(
            ui,
            state,
            row(y),
            SettingId::Wifi,
            SettingAction::Wifi(!wifi_on),
            if wifi_on {
                ic::WIFI_HIGH
            } else {
                ic::WIFI_SLASH
            },
            "Wi-Fi",
            &wifi_sub,
            wifi_on,
            palette,
        );
        y += row_h;
    }

    if enabled(state, "bluetooth") {
        let bt = state.shared.system.bluetooth.as_ref();
        let bt_on = bt.is_some_and(|b| b.powered);
        let bt_sub = match bt {
            None => "Sin datos".to_string(),
            Some(b) if b.connected > 0 => format!(
                "{} conectado{}",
                b.connected,
                if b.connected == 1 { "" } else { "s" }
            ),
            Some(_) if bt_on => "Activado".to_string(),
            Some(_) => "Apagado".to_string(),
        };
        toggle_row(
            ui,
            state,
            row(y),
            SettingId::Bluetooth,
            SettingAction::Bluetooth(!bt_on),
            if bt_on {
                ic::BLUETOOTH
            } else {
                ic::BLUETOOTH_SLASH
            },
            "Bluetooth",
            &bt_sub,
            bt_on,
            palette,
        );
        y += row_h;
    }

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
        y += row_h;
    }

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
        y += row_h;
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
        y += row_h;
    }

    // Lock: botón de acción con confirmación (ámbar mientras espera).
    if enabled(state, "lock") {
        let lock_rect = row(y);
        let pending = amber(state, SettingId::Lock);
        let icon_rect = egui::Rect::from_min_size(
            lock_rect.min + egui::vec2(0.0, (lock_rect.height() - 26.0) / 2.0),
            egui::vec2(26.0, 26.0),
        );
        chip(ui, icon_rect, ic::LOCK_KEY, palette, false, pending);
        text_at(
            ui,
            egui::pos2(lock_rect.min.x + 36.0, lock_rect.center().y),
            egui::Align2::LEFT_CENTER,
            if pending {
                "¿Bloquear la sesión?"
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
                egui::pos2(lock_rect.max.x - 96.0, lock_rect.center().y - 10.0),
                egui::vec2(48.0, 20.0),
            );
            let cancel = confirm.translate(egui::vec2(52.0, 0.0));
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
            let resp = ui.interact(
                lock_rect,
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
}
