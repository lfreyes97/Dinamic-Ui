//! Estado de la batería leyendo sysfs (sin dependencias externas).

use std::{fs, path::Path};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BatteryState {
    pub present: bool,
    pub percent: u8,
    pub charging: bool,
    /// Minutos hasta vaciarse (descargando) o hasta llenarse (cargando).
    /// `None` si el kernel no lo expone ni se puede calcular.
    pub minutes_left: Option<u32>,
}

pub fn read() -> BatteryState {
    let Ok(entries) = fs::read_dir("/sys/class/power_supply") else {
        return BatteryState::default();
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("BAT") && !name.starts_with("CMB") {
            continue;
        }
        let path = entry.path();
        let Some(percent) = read_u8(&path.join("capacity")) else {
            continue;
        };
        let status = fs::read_to_string(path.join("status"))
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        let charging = status == "charging";
        return BatteryState {
            present: true,
            percent: percent.min(100),
            charging,
            minutes_left: minutes_left(&path, charging),
        };
    }

    BatteryState::default()
}

/// Tiempo restante: `time_to_*_now` (segundos) si existe; si no, se calcula
/// con energía/potencia (`energy_*`/`power_now`) o carga/corriente
/// (`charge_*`/`current_now`).
fn minutes_left(path: &Path, charging: bool) -> Option<u32> {
    let direct = if charging { "time_to_full_now" } else { "time_to_empty_now" };
    if let Some(secs) = read_u64(&path.join(direct)) {
        return (secs > 0).then_some((secs / 60) as u32);
    }
    for (now, full, rate) in [
        ("energy_now", "energy_full", "power_now"),
        ("charge_now", "charge_full", "current_now"),
    ] {
        let (Some(now), Some(rate)) = (read_u64(&path.join(now)), read_u64(&path.join(rate)))
        else {
            continue;
        };
        if rate == 0 {
            return None;
        }
        let amount = if charging {
            read_u64(&path.join(full))?.saturating_sub(now)
        } else {
            now
        };
        return Some((amount * 60 / rate) as u32);
    }
    None
}

fn read_u64(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn read_u8(path: &Path) -> Option<u8> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}
