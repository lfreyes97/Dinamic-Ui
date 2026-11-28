//! Datos que los componentes piden al núcleo en un frame, y su histéresis.
//!
//! Fase 6.0 del plan de widgets: `wants_system()` era un booleño grueso que
//! hacía que cualquier vista leyera **todos** los datos del sistema. Aquí cada
//! componente declara solo lo que muestra y el núcleo toma la unión. Un flag
//! sigue vivo unos segundos después de que nadie lo pida, para que el notch no
//! arranque y pare el servicio cada vez que entra y sale el cursor.

use std::time::{Duration, Instant};

/// Dato del sistema que un componente puede necesitar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    Time,
    Battery,
    Network,
    Audio,
    Display,
    Layout,
    /// Perfil de energía (`powerprofilesctl`), para QuickSettings.
    Power,
}

pub const ALL_NEEDS: [Need; 7] = [
    Need::Time,
    Need::Battery,
    Need::Network,
    Need::Audio,
    Need::Display,
    Need::Layout,
    Need::Power,
];

/// Conjunto de datos solicitados. Se propaga por valor; `Copy` es deliberado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Needs(u8);

impl Needs {
    pub const NONE: Self = Self(0);
    /// Todo lo que antes se leía en bloque (sin la hora, que no consulta el hilo).
    pub const SYSTEM: Self = Self(0b1111_1110);
    pub const ALL: Self = Self(0b1111_1111);

    pub fn from_bits(bits: u8) -> Self {
        Self(bits & Self::ALL.0)
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn get(self, need: Need) -> bool {
        self.0 & Self::bit(need) != 0
    }

    pub fn set(&mut self, need: Need, value: bool) {
        let bit = Self::bit(need);
        if value {
            self.0 |= bit;
        } else {
            self.0 &= !bit;
        }
    }

    pub fn with(mut self, need: Need, value: bool) -> Self {
        self.set(need, value);
        self
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    fn bit(need: Need) -> u8 {
        match need {
            Need::Time => 1 << 0,
            Need::Battery => 1 << 1,
            Need::Network => 1 << 2,
            Need::Audio => 1 << 3,
            Need::Display => 1 << 4,
            Need::Layout => 1 << 5,
            Need::Power => 1 << 6,
        }
    }
}

/// Mantiene cada flag vivo durante `hold` tras su última petición.
#[derive(Debug)]
pub struct NeedTracker {
    last: [Option<Instant>; 7],
    hold: Duration,
}

impl NeedTracker {
    pub fn new(hold: Duration) -> Self {
        Self {
            last: [None; 7],
            hold,
        }
    }

    /// Cambia la histéresis en caliente (lo usa `--tune needs.hold=`).
    pub fn set_hold(&mut self, hold: Duration) {
        self.hold = hold;
    }

    /// Actualiza el estado con lo pedido ahora y devuelve lo que debe seguir vivo.
    pub fn update(&mut self, now: Instant, requested: Needs) -> Needs {
        for (i, need) in ALL_NEEDS.iter().enumerate() {
            if requested.get(*need) {
                self.last[i] = Some(now);
            } else if self.last[i].is_some_and(|t| now.saturating_duration_since(t) > self.hold) {
                self.last[i] = None;
            }
        }
        self.effective(now)
    }

    /// Lo que sigue vivo ahora, sin cambiar el estado.
    pub fn effective(&self, now: Instant) -> Needs {
        let mut out = Needs::NONE;
        for (i, need) in ALL_NEEDS.iter().enumerate() {
            if self.last[i].is_some_and(|t| now.saturating_duration_since(t) <= self.hold) {
                out.set(*need, true);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn union_and_bits_roundtrip() {
        let a = Needs::NONE.with(Need::Time, true).with(Need::Audio, true);
        let b = Needs::NONE.with(Need::Battery, true);
        let u = a.union(b);
        assert!(u.get(Need::Time) && u.get(Need::Audio) && u.get(Need::Battery));
        assert!(!u.get(Need::Network));
        assert_eq!(Needs::from_bits(u.bits()), u);
    }

    #[test]
    fn hysteresis_keeps_flags_alive_then_drops_them() {
        let hold = Duration::from_millis(40);
        let mut tracker = NeedTracker::new(hold);
        let t0 = Instant::now();
        let wanted = Needs::NONE.with(Need::Battery, true);
        assert_eq!(tracker.update(t0, wanted), wanted);
        let t1 = t0 + Duration::from_millis(20);
        assert!(tracker.effective(t1).get(Need::Battery));
        let t2 = t0 + Duration::from_millis(60);
        assert!(!tracker.update(t2, Needs::NONE).get(Need::Battery));
    }

    #[test]
    fn a_new_request_resets_the_hold() {
        let hold = Duration::from_millis(40);
        let mut tracker = NeedTracker::new(hold);
        let t0 = Instant::now();
        let req = Needs::NONE.with(Need::Network, true);
        tracker.update(t0, req);
        sleep(Duration::from_millis(25));
        let t1 = Instant::now();
        tracker.update(t1, req);
        sleep(Duration::from_millis(25));
        assert!(tracker.effective(Instant::now()).get(Need::Network));
    }
}
