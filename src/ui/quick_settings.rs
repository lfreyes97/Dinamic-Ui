//! Controles interactivos de QuickSettings, compartidos por el overlay del
//! topbar y la pestaña System del panel. Solo encolan acciones y pintan el
//! semáforo; la ejecución vive en `main.rs` y en `actions.rs`.
//!
//! Layout inspirado en GNOME 50 (`QuickSettingsLayout`): rejilla de dos columnas,
//! items con `column-span` y filas que crecen con su hijo más alto. No hay altura
//! fija: ambos hosts piden `required_height(state)`.

use super::*;
use crate::actions::{ActionPhase, SettingAction, SettingId};

/// Columnas del grid, como `N_QUICK_SETTINGS_COLUMNS` en gnome-shell 50.
const COLUMNS: usize = 2;
const GAP: f32 = 8.0;
const TOGGLE_H: f32 = 56.0;
const SLIDER_H: f32 = 34.0;

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
    } else if active {
        palette.accent
    } else if hovered {
        Palette::sheen(22)
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

/// Un control del grid. Los toggles ocupan una columna; sliders y acciones,
/// las dos, igual que los items de ancho completo de GNOME.
enum Item {
    Toggle {
        id: SettingId,
        action: SettingAction,
        icon: &'static str,
        title: &'static str,
        subtitle: String,
        on: bool,
    },
    Slider {
        id: SettingId,
        icon: &'static str,
        value: Option<u8>,
    },
    /// Perfil de energía: alterna entre ahorro y equilibrado.
    PowerMode { current: Option<String> },
}

impl Item {
    fn span(&self) -> usize {
        match self {
            Self::Toggle { .. } | Self::PowerMode { .. } => 1,
            Self::Slider { .. } => COLUMNS,
        }
    }

    fn height(&self) -> f32 {
        match self {
            Self::Toggle { .. } | Self::PowerMode { .. } => TOGGLE_H,
            Self::Slider { .. } => SLIDER_H,
        }
    }
}

fn items(state: &UiState) -> Vec<Item> {
    let mut out = Vec::new();
    // Los sliders van primero, como en el panel de GNOME 50; el bloqueo vive en
    // la cabecera, no se repite al final de la rejilla.
    if enabled(state, "volume") {
        out.push(Item::Slider {
            id: SettingId::Volume,
            icon: ic::SPEAKER_HIGH,
            value: state.shared.system.volume.as_ref().map(|v| v.percent),
        });
    }
    if enabled(state, "brightness") {
        out.push(Item::Slider {
            id: SettingId::Brightness,
            icon: ic::SUN,
            value: state.shared.system.brightness,
        });
    }
    if enabled(state, "wifi") {
        let wifi = state.shared.system.wifi.as_ref();
        let on = wifi.is_some_and(|w| w.enabled);
        let subtitle = match wifi.and_then(|w| w.ssid.as_deref()) {
            Some(ssid) if on => ssid.to_string(),
            Some(_) => "Sin conexión".into(),
            None => "Sin datos".into(),
        };
        out.push(Item::Toggle {
            id: SettingId::Wifi,
            action: SettingAction::Wifi(!on),
            icon: if on { ic::WIFI_HIGH } else { ic::WIFI_SLASH },
            title: "Wi-Fi",
            subtitle,
            on,
        });
    }
    if enabled(state, "bluetooth") {
        let bt = state.shared.system.bluetooth.as_ref();
        let on = bt.is_some_and(|b| b.powered);
        let subtitle = match bt {
            None => "Sin datos".into(),
            Some(b) if b.connected > 0 => format!(
                "{} conectado{}",
                b.connected,
                if b.connected == 1 { "" } else { "s" }
            ),
            Some(_) if on => "Activado".into(),
            Some(_) => "Apagado".into(),
        };
        out.push(Item::Toggle {
            id: SettingId::Bluetooth,
            action: SettingAction::Bluetooth(!on),
            icon: if on {
                ic::BLUETOOTH
            } else {
                ic::BLUETOOTH_SLASH
            },
            title: "Bluetooth",
            subtitle,
            on,
        });
    }
    #[cfg(feature = "notifications")]
    if enabled(state, "dnd") {
        let dnd = state.notifications.dnd;
        out.push(Item::Toggle {
            id: SettingId::Dnd,
            action: SettingAction::Dnd(!dnd),
            icon: if dnd { ic::MOON } else { ic::BELL },
            title: "No molestar",
            subtitle: if dnd { "Activado" } else { "Desactivado" }.into(),
            on: dnd,
        });
    }
    if enabled(state, "power-mode") {
        out.push(Item::PowerMode {
            current: state.shared.system.power_profile.clone(),
        });
    }
    out
}

struct Placed {
    rect: egui::Rect,
    item: Item,
}

/// Empaqueta los items en filas de `COLUMNS` columnas. Un item con span mayor
/// que el hueco restante empieza una fila nueva; el alto de la fila es el máximo
/// de sus items. Devuelve también el alto total usado.
fn layout(items: Vec<Item>, area: egui::Rect) -> (Vec<Placed>, f32) {
    let col_w = (area.width() - GAP * (COLUMNS as f32 - 1.0)) / COLUMNS as f32;
    let mut placed = Vec::new();
    let mut col = 0usize;
    let mut y = area.min.y;
    let mut row_h = 0.0f32;
    for item in items {
        let span = item.span().min(COLUMNS);
        if col + span > COLUMNS {
            y += row_h + GAP;
            col = 0;
            row_h = 0.0;
        }
        let x = area.min.x + col as f32 * (col_w + GAP);
        let w = span as f32 * col_w + (span.saturating_sub(1)) as f32 * GAP;
        let h = item.height();
        row_h = row_h.max(h);
        placed.push(Placed {
            rect: egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h)),
            item,
        });
        col += span;
        if col >= COLUMNS {
            y += row_h + GAP;
            col = 0;
            row_h = 0.0;
        }
    }
    let used = if col == 0 {
        y - GAP - area.min.y
    } else {
        y + row_h - area.min.y
    };
    (placed, used.max(0.0))
}

/// Alto que necesita el contenido con los items activos. Lo usan los dos hosts
/// para dimensionar su rect, en vez de números fijos.
pub(super) fn required_height(state: &UiState) -> f32 {
    let (_, height) = layout(
        items(state),
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 0.0)),
    );
    height
}

/// Toggle estilo GNOME QuickToggle: icono a la izquierda, título y subtítulo
/// apilados a la derecha (gnome-shell 50, `quickSettings.js:56-127`).
#[allow(clippy::too_many_arguments)]
fn draw_toggle(
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
        ui.scope_id()
            .with(egui::IdSalt::new(("qs-toggle", id as u8))),
        egui::Sense::click(),
    );
    surface(ui, rect, resp.hovered(), on, pending, palette);

    let icon_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(10.0, (rect.height() - 24.0) / 2.0),
        egui::vec2(24.0, 24.0),
    );
    let active = on && !pending;
    let color = if pending {
        palette.accent
    } else if active {
        palette.solid()
    } else {
        palette.muted
    };
    icon_at(ui, icon_rect.center(), icon, ICON_MD, color);
    let text_x = rect.min.x + 42.0;
    let title_color = if pending {
        palette.accent
    } else if active {
        palette.solid()
    } else {
        palette.text
    };
    text_at(
        ui,
        egui::pos2(text_x, rect.center().y - 7.0),
        egui::Align2::LEFT_CENTER,
        title,
        TEXT_BODY,
        title_color,
    );
    text_at(
        ui,
        egui::pos2(text_x, rect.center().y + 9.0),
        egui::Align2::LEFT_CENTER,
        if pending { "…" } else { subtitle },
        TEXT_CAPTION,
        if active { palette.solid() } else { title_color },
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.clicked() && !pending {
        state.shared.settings.enqueue(action);
    }
}

/// Slider de ancho completo: icono + porcentaje arriba, pista abajo.
fn draw_slider(
    ui: &mut egui::Ui,
    state: &mut UiState,
    rect: egui::Rect,
    id: SettingId,
    icon: &str,
    value: Option<u8>,
    palette: Palette,
) {
    let pending = amber(state, id);
    let pct = value.unwrap_or(0) as f32 / 100.0;
    // Icono, pista y porcentaje comparten la misma línea.
    let bar = egui::Rect::from_min_size(
        egui::pos2(rect.min.x + 32.0, rect.center().y - 2.5),
        egui::vec2(rect.width() - 66.0, 5.0),
    );
    icon_at(
        ui,
        egui::pos2(rect.min.x + 10.0, rect.center().y),
        icon,
        ICON_MD,
        if pending {
            palette.accent
        } else {
            palette.text
        },
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
            .with(egui::IdSalt::new(("qs-slider", id as u8))),
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
        let pct = ((x - bar.min.x) / bar.width()).clamp(0.0, 1.0);
        let value = (pct * 100.0).round().clamp(0.0, 100.0) as u8;
        let action = match id {
            SettingId::Volume => SettingAction::Volume(value),
            SettingId::Brightness => SettingAction::Brightness(value),
            _ => return,
        };
        state.shared.settings.enqueue(action);
    }
}

/// Cabecera del overlay: píldora de batería y botón redondo de bloqueo, como en
/// el panel de GNOME 50. El bloqueo ya existe como acción, no es un botón falso.
pub(super) fn draw_header(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    state: &mut UiState,
    palette: Palette,
) {
    if state.shared.battery.present {
        let pill = egui::Rect::from_min_size(rect.min, egui::vec2(76.0, rect.height()));
        surface(ui, pill, false, false, false, palette);
        icon_at(
            ui,
            egui::pos2(pill.min.x + 13.0, pill.center().y),
            ic::BATTERY_FULL,
            ICON_XS,
            palette.text,
        );
        text_at(
            ui,
            egui::pos2(pill.min.x + 26.0, pill.center().y),
            egui::Align2::LEFT_CENTER,
            format!("{}%", state.shared.battery.percent),
            TEXT_CAPTION,
            palette.text,
        );
    }

    if !enabled(state, "lock") {
        return;
    }
    let button = egui::Rect::from_min_size(
        egui::pos2(rect.max.x - rect.height(), rect.min.y),
        egui::vec2(rect.height(), rect.height()),
    );
    let pending = amber(state, SettingId::Lock);
    let resp = ui.interact(
        button,
        ui.scope_id().with(egui::IdSalt::new("qs-lock-header")),
        egui::Sense::click(),
    );
    surface(ui, button, resp.hovered(), false, pending, palette);
    icon_at(
        ui,
        button.center(),
        ic::LOCK_KEY,
        ICON_MD,
        if pending {
            palette.accent
        } else {
            palette.text
        },
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.clicked() && !pending {
        state.shared.settings.enqueue(SettingAction::Lock);
    }
}

/// Dibuja los controles en el rect que reciba el host (topbar o panel).
pub(super) fn draw(ui: &mut egui::Ui, area: egui::Rect, state: &mut UiState, palette: Palette) {
    let (placed, _) = layout(items(state), area);
    for placed in placed {
        match placed.item {
            Item::Toggle {
                id,
                action,
                icon,
                title,
                subtitle,
                on,
            } => draw_toggle(
                ui,
                state,
                placed.rect,
                id,
                action,
                icon,
                title,
                &subtitle,
                on,
                palette,
            ),
            Item::Slider { id, icon, value } => {
                draw_slider(ui, state, placed.rect, id, icon, value, palette)
            }
            Item::PowerMode { current } => {
                let target = if current.as_deref() == Some("power-saver") {
                    "balanced"
                } else {
                    "power-saver"
                };
                let subtitle = current.unwrap_or_else(|| "Sin datos".into());
                let on = subtitle != "balanced" && subtitle != "Sin datos";
                draw_toggle(
                    ui,
                    state,
                    placed.rect,
                    SettingId::PowerMode,
                    SettingAction::PowerMode(target),
                    ic::LIGHTNING,
                    "Energía",
                    &subtitle,
                    on,
                    palette,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toggle(id: SettingId) -> Item {
        Item::Toggle {
            id,
            action: SettingAction::Wifi(true),
            icon: "",
            title: "",
            subtitle: String::new(),
            on: false,
        }
    }

    fn slider(id: SettingId) -> Item {
        Item::Slider {
            id,
            icon: "",
            value: Some(50),
        }
    }

    #[test]
    fn grid_packs_spans_into_two_columns() {
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(268.0, 0.0));
        let (placed, height) = layout(
            vec![
                slider(SettingId::Volume),
                slider(SettingId::Brightness),
                toggle(SettingId::Wifi),
                toggle(SettingId::Bluetooth),
                toggle(SettingId::Dnd),
            ],
            area,
        );
        let ys: Vec<f32> = placed.iter().map(|p| p.rect.min.y).collect();
        assert_eq!(
            ys,
            vec![
                0.0,
                SLIDER_H + GAP,
                SLIDER_H + GAP + SLIDER_H + GAP,
                SLIDER_H + GAP + SLIDER_H + GAP,
                SLIDER_H + GAP + SLIDER_H + GAP + TOGGLE_H + GAP
            ]
        );
        assert_eq!(
            height,
            SLIDER_H + GAP + SLIDER_H + GAP + TOGGLE_H + GAP + TOGGLE_H
        );
        // Los toggles comparten fila y los items de ancho completo ocupan las dos columnas.
        assert_eq!(placed[0].rect.width(), area.width());
        assert_eq!(placed[1].rect.width(), area.width());
        assert_eq!(
            placed[2].rect.width() + GAP + placed[3].rect.width(),
            area.width()
        );
    }
}
