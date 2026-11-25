//! OSD de volumen y brillo: una barra efímera bajo el borde superior.
//!
//! El valor sale de `Shared.system`, que el hilo `sysinfo` refresca al instante cuando el OSD
//! pide su grupo de datos (ver `SystemMonitor::force_refresh`). El componente no ejecuta nada:
//! el disparador llega como comando de control o como acción de QuickSettings.

use super::*;
use crate::needs::{Need, Needs};

/// Segundos que la barra se mantiene visible antes de empezar a desvanecerse.
const HOLD: f32 = 1.2;
/// Duración del fundido de salida.
const FADE: f32 = 0.18;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OsdKind {
    #[default]
    Volume,
    Brightness,
}

#[derive(Debug, Clone, Default)]
pub struct OsdState {
    /// Último valor mostrado; se conserva mientras se desvanece.
    pub kind: OsdKind,
    /// `true` mientras debe permanecer visible.
    pub active: bool,
    /// Segundos desde el último disparo.
    pub age: f32,
    /// Opacidad 0..1 (1 mientras está activo).
    pub progress: f32,
}

impl OsdState {
    /// Muestra (o reinicia) el OSD para el tipo dado.
    pub fn show(&mut self, kind: OsdKind) {
        self.kind = kind;
        self.active = true;
        self.age = 0.0;
        self.progress = 1.0;
    }

    pub fn visible(&self) -> bool {
        self.progress > 0.005
    }
}

/// Avanza el temporizador: mantiene, suelta y desvanece. Devuelve si hay que repintar.
pub(super) fn update(osd: &mut OsdState, dt: f32) -> bool {
    if !osd.active && osd.progress <= 0.0 {
        return false;
    }
    if osd.active {
        osd.age += dt;
        if osd.age >= HOLD {
            osd.active = false;
        }
    }
    if osd.active {
        return false;
    }
    let before = osd.progress;
    osd.progress = crate::approach(osd.progress, 0.0, dt / FADE);
    (before - osd.progress).abs() > 0.0005
}

/// Solo pide el dato que muestra, y solo mientras está activo.
pub(super) fn needs(osd: &OsdState) -> Needs {
    if !osd.active {
        return Needs::NONE;
    }
    match osd.kind {
        OsdKind::Volume => Needs::NONE.with(Need::Audio, true),
        OsdKind::Brightness => Needs::NONE.with(Need::Display, true),
    }
}

pub(super) fn draw(ui: &mut egui::Ui, state: &UiState, palette: Palette) {
    let osd = &state.osd;
    if !osd.visible() {
        return;
    }
    let system = &state.shared.system;
    let (icon, label, pct) = match osd.kind {
        OsdKind::Volume => match &system.volume {
            Some(v) => {
                let icon = match (v.muted, v.percent) {
                    (true, _) => ic::SPEAKER_SLASH,
                    (false, 0) => ic::SPEAKER_NONE,
                    (false, 1..=49) => ic::SPEAKER_LOW,
                    _ => ic::SPEAKER_HIGH,
                };
                (icon, format!("{}%", v.percent), v.percent as f32 / 100.0)
            }
            None => (ic::SPEAKER_HIGH, "sin datos".to_string(), 0.0),
        },
        OsdKind::Brightness => {
            let pct = system.brightness.unwrap_or(0);
            (ic::SUN, format!("{pct}%"), pct as f32 / 100.0)
        }
    };

    const W: f32 = 210.0;
    const H: f32 = 44.0;
    #[cfg(feature = "topbar")]
    let top = if state.shared.config.topbar.enabled {
        state.shared.config.topbar.height as f32
    } else {
        metrics().bar_h as f32
    };
    #[cfg(not(feature = "topbar"))]
    let top = metrics().bar_h as f32;
    let screen = ui.max_rect();
    let r = egui::Rect::from_center_size(
        egui::pos2(screen.center().x, top + 10.0 + H / 2.0),
        egui::vec2(W, H),
    );

    let previous = ui.opacity();
    ui.set_opacity(previous * crate::smootherstep(osd.progress));
    drop_shadow(ui, r, 12, 6, 20, 90);
    ui.painter().rect(
        r,
        CornerRadius::same(Radius::of(Radius::MD)),
        Palette::with_alpha(palette.solid(), 232),
        Stroke::new(1.0, palette.border),
        StrokeKind::Middle,
    );
    icon_at(
        ui,
        r.min + egui::vec2(18.0, H / 2.0),
        icon,
        ICON_MD,
        palette.text_hi,
    );
    text_at(
        ui,
        egui::pos2(r.min.x + 36.0, r.min.y + 14.0),
        egui::Align2::LEFT_CENTER,
        label,
        11.0,
        palette.text,
    );
    let bar = egui::Rect::from_min_size(
        egui::pos2(r.min.x + 36.0, r.min.y + 26.0),
        egui::vec2(W - 52.0, 6.0),
    );
    bar_at(ui, bar, pct, palette.track, palette.accent);
    ui.set_opacity(previous);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_holds_then_fades() {
        let mut s = OsdState::default();
        assert!(!s.visible());
        s.show(OsdKind::Volume);
        assert!(s.visible());
        assert!(needs(&s).get(Need::Audio));

        for _ in 0..12 {
            update(&mut s, 0.1);
        }
        assert!(!s.active, "a los 1,2 s suelta el dato");
        assert_eq!(needs(&s), Needs::NONE);
        assert!(s.visible(), "todavía se está desvaneciendo");

        for _ in 0..10 {
            update(&mut s, 0.1);
        }
        assert_eq!(s.progress, 0.0);
        assert!(!s.visible());
    }

    #[test]
    fn un_nuevo_disparo_reinicia_el_temporizador() {
        let mut s = OsdState::default();
        s.show(OsdKind::Volume);
        for _ in 0..10 {
            update(&mut s, 0.1);
        }
        s.show(OsdKind::Brightness);
        assert!(s.active);
        assert_eq!(s.age, 0.0);
        assert_eq!(s.progress, 1.0);
        assert!(needs(&s).get(Need::Display));
    }
}
