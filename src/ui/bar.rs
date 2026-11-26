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
                    (palette.text, palette.solid())
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
    let battery_color = battery_tone(state.shared.battery.percent, state.shared.battery.charging);
    let (wifi_icon, wifi_color, wifi_tip) = wifi_status(&state.shared.system, palette);
    let (audio_icon, audio_color, audio_tip) = audio_status(&state.shared.system, palette);
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

/// Zonas de histéresis del notch. La de entrada es la franja de siempre
/// (`y ≤ 18`, `|x − centro| ≤ 100`): su sensibilidad ya estaba bien y no se toca.
/// La de salida la amplía con el rect animado más 12 px, así que el cursor puede
/// bajar por dentro de la píldora sin que se retraiga antes de tiempo; el chip y
/// la píldora comparten ese mismo recorrido de salida.
pub(super) fn notch_hover_zones(screen_w: f32, progress: f32) -> (egui::Rect, egui::Rect) {
    let cx = screen_w / 2.0;
    let enter = egui::Rect::from_min_max(egui::pos2(cx - 100.0, 0.0), egui::pos2(cx + 100.0, 18.0));
    let exit = enter.union(island_rect(screen_w, progress * progress).expand(12.0));
    (enter, exit)
}

/// El contenido va por delante del contenedor: se apaga con fundido cuando la
/// píldora todavía conserva ~30 % de su recorrido, no con un umbral seco.
fn island_content_alpha(progress: f32) -> f32 {
    smoothstep(0.30, 0.80, progress)
}

/// Escala del contenido (icono, hora y chip): `p²(2 − p) < p` para todo `p < 1`,
/// así que el contenido siempre va por delante del contenedor. A mitad de viaje
/// está al 38 % de tamaño mientras la píldora todavía va al 50 % de recorrido.
fn island_content_scale(progress: f32) -> f32 {
    progress * progress * (2.0 - progress)
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
        Palette::with_alpha(accent, a)
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
        let content_alpha = island_content_alpha(p);
        let s = island_content_scale(p);
        // El punto de reposo se apaga con la misma curva con la que entra el
        // contenido: ni solape ni corte seco al cambiar de una cosa a otra.
        let dot_alpha = (1.0 - content_alpha) * (0.5 + island_t).min(1.0);
        if dot_alpha > 0.01 {
            ui.painter().circle_filled(
                pill.center(),
                1.4,
                palette.accent.gamma_multiply(dot_alpha),
            );
        }
        // Icono, hora y chip comparten opacidad y escala propias: se encogen y
        // desaparecen antes de que la píldora termine de retraerse.
        if content_alpha > 0.01 {
            let mut content = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("island-content")
                    .max_rect(pill),
            );
            content.set_opacity(island_alpha * content_alpha);
            let now = Local::now().format("%H:%M").to_string();
            let icon_pos = pill.center() - egui::vec2(30.0 * s, 0.0);
            let time_pos = pill.center() + egui::vec2(8.0 * s, 0.0);
            content.painter().text(
                icon_pos,
                egui::Align2::CENTER_CENTER,
                ic::CLOCK,
                icon_font(ICON_MD * s),
                palette.accent,
            );
            text_at(
                &content,
                time_pos,
                egui::Align2::CENTER_CENTER,
                now,
                TEXT_DISPLAY_SM * s,
                palette.text,
            );
            if config::get().bar.notch_control
                && config::get().bar.max_state >= config::BarMaxState::Compact
            {
                // Anclado al borde derecho del rect animado y escalado con el
                // contenido: nunca queda fuera de la píldora al entrar o salir.
                let rect = egui::Rect::from_center_size(
                    egui::pos2(pill.max.x - 22.0 * s, pill.center().y),
                    egui::Vec2::splat(16.0 * s),
                );
                // El clic lo atiende el rect grande (notch → compacto → barra);
                // este interact solo dibuja hover/cursor.
                let _ = mode_chip(&content, rect, ic::CARET_UP, palette, "notch-mode", s);
            }
        }
        // Clic: notch/compacto -> compacto enganchado -> barra, siempre a través
        // de los comandos para que `bar.max_state` también valga con el ratón.
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
                dispatch_command("expand", state);
            } else {
                dispatch_command("compact", state);
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

        // Hueco entre el reloj y el estado: atajo mouse para subir a topbar o
        // bajar a la isla (equivalente a los comandos `topbar` / `collapse`).
        if config::get().bar.hueco_control && t >= 0.5 {
            let cx = header.center().x;
            let cy = header.center().y - 8.0;
            let size = egui::vec2(16.0, 16.0);
            if mode_chip(
                ui,
                egui::Rect::from_min_size(egui::pos2(cx + 80.0, cy), size),
                ic::CARET_UP,
                palette,
                "bar-mode-up",
                1.0,
            ) {
                dispatch_command("topbar", state);
            }
            if mode_chip(
                ui,
                egui::Rect::from_min_size(egui::pos2(cx + 98.0, cy), size),
                ic::CARET_DOWN,
                palette,
                "bar-mode-down",
                1.0,
            ) {
                dispatch_command("collapse", state);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f32 = 1280.0;

    #[test]
    fn entrada_conserva_la_franja_original() {
        let (enter, _) = notch_hover_zones(W, 0.0);
        assert!(enter.contains(egui::pos2(W / 2.0, 10.0)));
        assert!(enter.contains(egui::pos2(W / 2.0 + 90.0, 10.0)));
        assert!(!enter.contains(egui::pos2(W / 2.0 + 101.0, 10.0)));
        assert!(!enter.contains(egui::pos2(W / 2.0, 19.0)));
    }

    #[test]
    fn salida_sigue_a_la_pildora_abierta() {
        let (_, exit) = notch_hover_zones(W, 1.0);
        // Dentro de la píldora, por debajo de los 18 px de la franja de entrada.
        assert!(exit.contains(egui::pos2(W / 2.0 + 55.0, 25.0)));
        assert!(!exit.contains(egui::pos2(W / 2.0 + 110.0, 25.0)));
    }

    #[test]
    fn contenido_se_encoge_y_apaga_antes_que_el_contenedor() {
        assert!(island_content_alpha(0.0) <= 0.01);
        assert!(island_content_alpha(1.0) > 0.99);
        // La píldora todavía está al 30 % de su recorrido y el contenido ya se fue.
        assert!(island_content_alpha(0.30) <= 0.01);
        let mid = island_content_alpha(0.5);
        assert!(mid > 0.0 && mid < 1.0);

        assert!((island_content_scale(1.0) - 1.0).abs() < 1e-6);
        // La escala del contenido siempre queda POR DEBAJO del progreso del rect.
        assert!(island_content_scale(0.5) < 0.5);
        assert!(island_content_scale(0.3) < 0.3);
        assert!(island_content_scale(0.3) < island_content_scale(0.5));
    }
}
