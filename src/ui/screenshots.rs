//! Panel de capturas: región, pantalla, ventana activa y retardo.
//!
//! Como todos los componentes, **no ejecuta nada**: encola un `sh -c` en
//! `Shared.pending_spawns` y `main.rs` lo lanza fuera del render. `notify-send`
//! vuelve a nuestro propio servidor de notificaciones, así que la confirmación
//! sale como tarjeta del shell.

use super::*;
use smithay_client_toolkit::seat::keyboard::Keysym;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScreenshotMode {
    #[default]
    Region,
    Screen,
    Window,
    Delay,
}

impl ScreenshotMode {
    const ALL: [Self; 4] = [Self::Region, Self::Screen, Self::Window, Self::Delay];

    fn label(self) -> &'static str {
        match self {
            Self::Region => "Región",
            Self::Screen => "Pantalla completa",
            Self::Window => "Ventana activa",
            Self::Delay => "Pantalla en 5 s",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Region => "Arrastra el área con el ratón",
            Self::Screen => "Captura todos los monitores",
            Self::Window => "Captura la ventana enfocada",
            Self::Delay => "Tiempo para preparar la escena",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Region => ic::CROP,
            Self::Screen => ic::MONITOR,
            Self::Window => ic::APP_WINDOW,
            Self::Delay => ic::TIMER,
        }
    }

    /// Script que ejecutará el núcleo. Se guarda como texto para no depender de
    /// que `sh` esté en el PATH del compositor (lo está, pero el argv es explícito).
    fn script(self) -> &'static str {
        match self {
            Self::Region => {
                r#"grim -g "$(slurp)" - | wl-copy && notify-send -u low 'Captura' 'Región copiada al portapapeles'"#
            }
            Self::Screen => {
                r#"grim - | wl-copy && notify-send -u low 'Captura' 'Pantalla copiada al portapapeles'"#
            }
            Self::Window => {
                r#"geo=$(hyprctl activewindow -j | jq -r '"\(.at[0]),\(.at[1]) \(.size[0]x\(.size[1])"'); grim -g "$geo" - | wl-copy && notify-send -u low 'Captura' 'Ventana copiada al portapapeles'"#
            }
            Self::Delay => {
                r#"sleep 5; grim - | wl-copy && notify-send -u low 'Captura' 'Pantalla copiada al portapapeles'"#
            }
        }
    }
}

#[derive(Default)]
pub struct ScreenshotState {
    /// Objetivo: panel abierto o cerrado. La animación lo sigue con `progress`.
    pub open: bool,
    pub progress: f32,
    pub selected: usize,
}

impl ScreenshotState {
    /// Hay algo que dibujar, también mientras se desvanece al cerrar.
    pub fn visible(&self) -> bool {
        self.open || self.progress > 0.005
    }
}

/// Abre el panel; cierra primero cualquier otro overlay a pantalla completa.
pub(super) fn open(state: &mut UiState) {
    super::component::close_overlays(state);
    state.screenshots.open = true;
    state.screenshots.selected = 0;
}

/// Avanza el fundido. Devuelve si hay que repintar.
pub(super) fn update(state: &mut UiState, dt: f32) -> bool {
    let s = &mut state.screenshots;
    let before = s.progress;
    s.progress = crate::approach(s.progress, if s.open { 1.0 } else { 0.0 }, dt * 7.0);
    (s.progress - before).abs() > 0.0005
}

/// Encola la captura del modo elegido y cierra el panel.
fn capture(state: &mut UiState) {
    let mode = ScreenshotMode::ALL[state.screenshots.selected.min(3)];
    state
        .shared
        .pending_spawns
        .push(vec!["sh".into(), "-c".into(), mode.script().into()]);
    state.screenshots.open = false;
}

pub(super) fn handle_key(state: &mut UiState, key: Keysym) -> bool {
    if !state.screenshots.open {
        return false;
    }
    match key {
        Keysym::Escape => state.screenshots.open = false,
        Keysym::Up | Keysym::Left => {
            state.screenshots.selected = state.screenshots.selected.saturating_sub(1)
        }
        Keysym::Down | Keysym::Right | Keysym::Tab => {
            state.screenshots.selected = (state.screenshots.selected + 1).min(3)
        }
        Keysym::Return | Keysym::KP_Enter => capture(state),
        _ => return false,
    }
    true
}

pub(super) fn draw(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    if !state.screenshots.visible() {
        return;
    }
    let (progress, selected) = (state.screenshots.progress, state.screenshots.selected);
    let screen = ui.max_rect();
    let previous = ui.opacity();
    ui.set_opacity(previous * crate::smootherstep(progress));
    ui.painter()
        .rect_filled(screen, CornerRadius::same(0), Palette::scrim(205));

    let w = 520.0f32.min(screen.width() - 60.0);
    let h = 330.0f32.min(screen.height() - 60.0);
    let card = egui::Rect::from_center_size(screen.center(), egui::vec2(w, h));
    ui.painter().rect(
        card,
        CornerRadius::same(16),
        pal().card_fill,
        Stroke::new(1.0, palette.border),
        StrokeKind::Middle,
    );
    text_at(
        ui,
        card.min + egui::vec2(20.0, 26.0),
        egui::Align2::LEFT_CENTER,
        "Captura de pantalla",
        16.0,
        palette.text_hi,
    );

    let row_h = 52.0;
    for (i, mode) in ScreenshotMode::ALL.iter().enumerate() {
        let row = egui::Rect::from_min_size(
            card.min + egui::vec2(20.0, 52.0 + i as f32 * row_h),
            egui::vec2(card.width() - 40.0, row_h - 6.0),
        );
        let response = ui.interact(
            row,
            ui.scope_id().with(("shot-mode", i)),
            egui::Sense::click(),
        );
        if response.clicked() {
            state.screenshots.selected = i;
            capture(state);
            break;
        }
        let active = i == selected || response.hovered();
        ui.painter().rect(
            row,
            CornerRadius::same(9),
            if i == selected {
                Palette::with_alpha(palette.accent_soft, 70)
            } else {
                Palette::sheen(8)
            },
            Stroke::new(
                1.0,
                if active {
                    palette.accent
                } else {
                    palette.border
                },
            ),
            StrokeKind::Middle,
        );
        icon_at(
            ui,
            row.min + egui::vec2(20.0, row.height() / 2.0),
            mode.icon(),
            ICON_MD,
            if active {
                palette.accent
            } else {
                palette.muted
            },
        );
        text_at(
            ui,
            row.min + egui::vec2(46.0, row.height() / 2.0 - 8.0),
            egui::Align2::LEFT_CENTER,
            mode.label(),
            12.0,
            palette.text,
        );
        text_at(
            ui,
            row.min + egui::vec2(46.0, row.height() / 2.0 + 9.0),
            egui::Align2::LEFT_CENTER,
            mode.hint(),
            9.0,
            palette.muted,
        );
        if active {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
    }

    text_at(
        ui,
        egui::pos2(card.center().x, card.max.y - 18.0),
        egui::Align2::CENTER_CENTER,
        "↑ ↓ elige · ↵ captura · Esc cierra",
        10.0,
        palette.muted,
    );
    ui.set_opacity(previous);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todos_los_modos_usan_grim_y_wl_copy() {
        for mode in ScreenshotMode::ALL {
            let script = mode.script();
            assert!(script.contains("grim"), "{}", mode.label());
            assert!(script.contains("wl-copy"), "{}", mode.label());
        }
    }

    #[test]
    fn el_retardo_es_anterior_a_la_captura() {
        let script = ScreenshotMode::Delay.script();
        assert!(script.find("sleep 5").unwrap() < script.find("grim").unwrap());
    }

    #[test]
    fn teclado_navega_captura_y_cierra() {
        let mut state = UiState::new_preview();
        state.screenshots.open = true;
        assert!(handle_key(&mut state, Keysym::Down));
        assert_eq!(state.screenshots.selected, 1);
        assert!(handle_key(&mut state, Keysym::Return));
        assert!(!state.screenshots.open);
        assert_eq!(state.shared.pending_spawns.len(), 1);
        assert_eq!(state.shared.pending_spawns[0][0], "sh");
        assert!(state.shared.pending_spawns[0][2].contains("grim -"));

        state.screenshots.open = true;
        assert!(handle_key(&mut state, Keysym::Escape));
        assert!(!state.screenshots.open);
    }
}
