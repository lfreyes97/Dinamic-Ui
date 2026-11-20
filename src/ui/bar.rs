//! Barra: isla/píldora y fila de workspaces, reloj y estado.

use super::*;

/// Estado de la barra: isla → píldora → barra, con panel acoplado.
#[derive(Default)]
pub struct BarState {
    pub expansion: f32,
    pub expanded: bool,
    pub show_panel: bool,
    pub panel_progress: f32,
    pub panel_tab: usize,
    pub island_progress: f32,
    pub island_active: bool,
    pub island_compact: bool,
    pub layout_preset: usize,
}

impl BarState {
    pub fn new() -> Self {
        let panel = std::env::var_os("DINAMIC_UI_PANEL").is_some();
        Self {
            expanded: std::env::var_os("DINAMIC_UI_EXPANDED").is_some(),
            show_panel: panel,
            panel_progress: if panel { 1.0 } else { 0.0 },
            panel_tab: std::env::var("DINAMIC_UI_PANEL_TAB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0)
                .min(crate::ui::panel::PANEL_TABS.len() - 1),
            island_active: std::env::var_os("DINAMIC_UI_ISLAND").is_some(),
            ..Self::default()
        }
    }
}

/// Icono de estado de la barra con tooltip.
pub(super) fn bar_icon(ui: &mut egui::Ui, icon: &str, color: Color32, tip: String) {
    ui.add(egui::Label::new(icon_bold(icon, ICON_SYSTEM, color)).sense(egui::Sense::hover()))
        .on_hover_text(tip);
}

/// Modo largo: workspaces · opciones | reloj+fecha | sistema.
///
/// El reloj va siempre en el centro geométrico de la píldora (w/2), no en el
/// espacio que sobra entre los bloques. A su izquierda y derecha quedan dos
/// zonas de ancho fijo respecto a ese centro; si los workspaces no caben en la
/// izquierda, sus botones se encogen (mínimo `WS_MIN`) en vez de empujar el reloj.
pub(super) fn draw_bar_row(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let rect = ui.max_rect();
    let h = rect.height();
    let cx = rect.center().x;

    // Separadores a ambos lados del reloj.
    let separator = Stroke::new(1.0, palette.border);
    for x in [cx - BAR_SEP_OFFSET, cx + BAR_SEP_OFFSET] {
        ui.painter()
            .vline(x, (rect.min.y + 5.0)..=(rect.max.y - 5.0), separator);
    }

    // Zona izquierda: workspaces (1..N) como círculos, con el activo invertido,
    // y el botón del panel.
    let left = egui::Rect::from_min_max(
        rect.min,
        egui::Pos2::new(cx - BAR_SEP_OFFSET - SP_M, rect.max.y),
    );
    island_slot(
        ui,
        "bar-left",
        left,
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_clip_rect(left.intersect(ui.clip_rect()));
            ui.spacing_mut().item_spacing.x = WS_GAP;
            let count = config::get().workspaces.count.max(1);
            let free = left.width() - OPTIONS_W - SP_M;
            let size =
                ((free - WS_GAP * (count as f32 - 1.0)) / count as f32).clamp(WS_MIN, WS_MAX);
            for n in 1..=count {
                let active = state.snapshot.workspace == n.to_string();
                // Círculo de tamaño exacto (un `Button` de egui impone su mínimo
                // y desbordaría la zona izquierda).
                let (rect, response) =
                    ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::click());
                let response = response.on_hover_cursor(CursorIcon::PointingHand);
                let (bg, fg) = if active {
                    (palette.text, Color32::from_gray(17))
                } else if response.hovered() {
                    (palette.border, palette.text)
                } else {
                    (palette.badge, palette.muted)
                };
                ui.painter().circle_filled(rect.center(), size / 2.0, bg);
                if state.snapshot.occupied.contains(&n) {
                    ui.painter().circle_filled(
                        egui::Pos2::new(rect.center().x, rect.max.y - 3.5),
                        1.75,
                        occupied_color(palette, active),
                    );
                }
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    n.to_string(),
                    egui::FontId::proportional(TEXT_BODY),
                    fg,
                );
                if response.clicked() {
                    state.pending_commands.push(switch_workspace_cmd(n));
                }
            }
            ui.add_space(SP_M);
            panel_toggle_button(ui, state, palette);
        },
    );

    // Zona central: reloj + fecha centrados en w/2.
    island_slot(
        ui,
        "bar-clock",
        egui::Rect::from_center_size(
            egui::Pos2::new(cx, rect.center().y),
            egui::Vec2::new(BAR_CLOCK_HALF * 2.0, h),
        ),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(
                RichText::new(Local::now().format("%H:%M").to_string())
                    .size(TEXT_CLOCK)
                    .family(egui::FontFamily::Name(FONT_SEMIBOLD.into()))
                    .color(palette.text),
            );
            ui.label(
                RichText::new(short_date())
                    .size(TEXT_CAPTION)
                    .color(palette.muted),
            );
        },
    );

    // Zona derecha: batería · % · wifi · audio, con el estado real de `sysinfo`.
    let system_color = Color32::from_rgb(0xf0, 0xf0, 0xf0);
    let battery_color = battery_tone(state.battery.percent, state.battery.charging);
    let (wifi_icon, wifi_color, wifi_tip) = match &state.system.wifi {
        None => (ic::WIFI_HIGH, palette.muted, "Wi-Fi: sin datos".to_string()),
        Some(w) if !w.enabled => (
            ic::WIFI_SLASH,
            palette.muted,
            "Wi-Fi desactivado".to_string(),
        ),
        Some(w) => match &w.ssid {
            Some(ssid) => (ic::WIFI_HIGH, system_color, format!("Wi-Fi: {ssid}")),
            None => (ic::WIFI_X, palette.muted, "Wi-Fi sin conexión".to_string()),
        },
    };
    let (audio_icon, audio_color, audio_tip) = match &state.system.volume {
        None => (
            ic::SPEAKER_HIGH,
            palette.muted,
            "Audio: sin datos".to_string(),
        ),
        Some(v) if v.muted => (
            ic::SPEAKER_SLASH,
            palette.muted,
            "Audio silenciado".to_string(),
        ),
        Some(v) => {
            let icon = match v.percent {
                0 => ic::SPEAKER_NONE,
                1..=49 => ic::SPEAKER_LOW,
                _ => ic::SPEAKER_HIGH,
            };
            (icon, system_color, format!("Volumen {}%", v.percent))
        }
    };
    island_slot(
        ui,
        "bar-system",
        egui::Rect::from_min_max(
            egui::Pos2::new(rect.max.x - BAR_RIGHT_W, rect.min.y),
            rect.max,
        ),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = SP_L;
            // En right_to_left el primer widget queda a la derecha.
            bar_icon(ui, audio_icon, audio_color, audio_tip);
            bar_icon(ui, wifi_icon, wifi_color, wifi_tip);
            if state.battery.present {
                ui.label(
                    RichText::new(format!("{}%", state.battery.percent))
                        .size(TEXT_BODY)
                        .color(battery_color),
                );
            }
            battery_glyph(ui, state.battery.percent, battery_color);
        },
    );
}
