//! Barra: dibujo de la isla/píldora, workspaces, reloj y estado (feature `bar`).

use super::*;

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
                let active = state.shared.snapshot.workspace == n.to_string();
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
                if state.shared.snapshot.occupied.contains(&n) {
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
                    state.shared.pending_commands.push(switch_workspace_cmd(n));
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
    let battery_color = battery_tone(state.shared.battery.percent, state.shared.battery.charging);
    let (wifi_icon, wifi_color, wifi_tip) = match &state.shared.system.wifi {
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
    let (audio_icon, audio_color, audio_tip) = match &state.shared.system.volume {
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
            if state.shared.battery.present {
                ui.label(
                    RichText::new(format!("{}%", state.shared.battery.percent))
                        .size(TEXT_BODY)
                        .color(battery_color),
                );
            }
            battery_glyph(ui, state.shared.battery.percent, battery_color);
        },
    );
}

/// Isla/píldora/barra: dibujo y crossfade del estado `bar`.
pub(super) fn draw_bar_shell(
    ui: &mut egui::Ui,
    state: &mut UiState,
    palette: Palette,
    t: f32,
    pill_w: f32,
) {
    // El borde se enciende con decaimiento suave tras un evento de Hyprland.
    let glow = state.shared.pulse * state.shared.pulse;
    let accent = palette.accent;
    let border = if glow > 0.01 {
        let a = (70.0 + 150.0 * glow).min(255.0) as u8;
        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), a)
    } else {
        palette.border
    };

    // El surface es ancho completo y fijo; los elementos se dibujan centrados.
    let screen = ui.max_rect();
    // Caída con aceleración (gravedad), no lineal: la gota cae y se posa.
    let p = state.bar.island_progress;
    let island_t = p * p;
    let in_island_mode = t < 0.5;
    let pill = if in_island_mode {
        island_rect(screen.width(), island_t)
    } else {
        egui::Rect::from_min_size(
            egui::Pos2::new(((screen.width() - pill_w) / 2.0).round(), 0.0),
            egui::Vec2::new(pill_w.round(), metrics().bar_h as f32),
        )
    };
    let radius = (pill.height() / 2.0).round().min(16.0) as u8;
    // En modo isla las esquinas superiores son rectas (tocan el borde de la
    // pantalla); solo se redondean las inferiores.
    let corners = if in_island_mode {
        CornerRadius {
            nw: 0,
            ne: 0,
            sw: radius,
            se: radius,
        }
    } else {
        CornerRadius::same(radius)
    };
    ui.painter().rect(
        pill,
        corners,
        palette.surface,
        Stroke::new(1.0 + glow * 1.5, border),
        StrokeKind::Middle,
    );

    let inner = egui::Rect::from_min_max(
        pill.min + egui::Vec2::new(BAR_INSET_X, 5.0),
        pill.max - egui::Vec2::new(BAR_INSET_X, 5.0),
    );
    let header_h = metrics().bar_h as f32 - 8.0;
    let header = egui::Rect::from_min_size(
        inner.min,
        egui::Vec2::new(inner.width(), header_h.min(inner.height())),
    );

    // Crossfade isla <-> barra sobre el mismo rectángulo.
    let island_alpha = 1.0 - smoothstep(0.15, 0.75, t);
    let bar_alpha = smoothstep(0.25, 0.85, t);

    if island_alpha > 0.01 && in_island_mode {
        if island_t > 0.1 {
            // Icono + hora con más presencia, agrupados y centrados.
            let now = Local::now().format("%H:%M").to_string();
            let icon_pos = pill.center() - egui::vec2(30.0, 0.0);
            let time_pos = pill.center() + egui::vec2(8.0, 0.0);
            ui.painter().text(
                icon_pos,
                egui::Align2::CENTER_CENTER,
                ic::CLOCK,
                icon_font(ICON_MD),
                palette.accent,
            );
            text_at(
                ui,
                time_pos,
                egui::Align2::CENTER_CENTER,
                now,
                TEXT_DISPLAY_SM,
                palette.text,
            );
        }
        // Un punto de acento en el notch en reposo.
        if island_t < 0.5 {
            ui.painter().circle_filled(
                pill.center(),
                1.4,
                palette.accent.gamma_multiply(0.5 + island_t),
            );
        }
        // Clic: notch/compacto -> compacto enganchado -> barra.
        let response = ui.interact(
            pill,
            ui.scope_id().with(egui::IdSalt::new("island-click")),
            egui::Sense::click(),
        );
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if response.clicked() {
            if state.bar.island_compact {
                state.bar.expanded = true;
                state.bar.island_compact = false;
            } else {
                state.bar.island_compact = true;
            }
        }
    }
    if bar_alpha > 0.01 {
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header)
                .layout(egui::Layout::left_to_right(egui::Align::Center))
                .id_salt("bar-layer"),
        );
        child.set_opacity(bar_alpha);
        draw_bar_row(&mut child, state, palette);
    }
}
