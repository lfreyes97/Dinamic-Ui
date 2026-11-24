//! Acciones de configuración del sistema y su ciclo de vida (QuickSettings).
//!
//! Los componentes solo encolan `SettingAction`. `main.rs` ejecuta las acciones
//! y este módulo mantiene el semáforo: ámbar mientras se espera una confirmación
//! o la lectura real, y cierre cuando `sysinfo` confirma o expira el timeout.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::sysinfo::SystemInfo;

const TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingId {
    Wifi,
    Bluetooth,
    Dnd,
    Volume,
    Brightness,
    Lock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingValue {
    Bool(bool),
    Percent(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingAction {
    Wifi(bool),
    Bluetooth(bool),
    Dnd(bool),
    Volume(u8),
    Brightness(u8),
    Lock,
}

impl SettingAction {
    pub fn id(self) -> SettingId {
        match self {
            Self::Wifi(_) => SettingId::Wifi,
            Self::Bluetooth(_) => SettingId::Bluetooth,
            Self::Dnd(_) => SettingId::Dnd,
            Self::Volume(_) => SettingId::Volume,
            Self::Brightness(_) => SettingId::Brightness,
            Self::Lock => SettingId::Lock,
        }
    }

    pub fn desired(self) -> SettingValue {
        match self {
            Self::Wifi(on) | Self::Bluetooth(on) | Self::Dnd(on) => SettingValue::Bool(on),
            Self::Volume(pct) | Self::Brightness(pct) => SettingValue::Percent(pct),
            Self::Lock => SettingValue::Bool(true),
        }
    }

    pub fn needs_confirmation(self) -> bool {
        matches!(self, Self::Lock)
    }

    /// Comando externo a ejecutar. DND se resuelve dentro del proceso, sin argv.
    pub fn command(self) -> Option<Vec<String>> {
        let s = |v: &str| v.to_string();
        Some(match self {
            Self::Wifi(on) => vec![
                s("nmcli"),
                s("radio"),
                s("wifi"),
                s(if on { "on" } else { "off" }),
            ],
            Self::Bluetooth(on) => vec![
                s("bluetoothctl"),
                s("power"),
                s(if on { "on" } else { "off" }),
            ],
            Self::Volume(pct) => vec![
                s("wpctl"),
                s("set-volume"),
                s("@DEFAULT_AUDIO_SINK@"),
                s(&format!("{pct}%")),
            ],
            Self::Brightness(pct) => vec![s("brightnessctl"), s("set"), s(&format!("{pct}%"))],
            Self::Lock => vec![s("hyprlock")],
            Self::Dnd(_) => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionPhase {
    AwaitingConfirmation,
    InFlight,
}

#[derive(Debug, Clone)]
pub struct Pending {
    pub action: SettingAction,
    pub phase: ActionPhase,
    pub since: Instant,
    pub since_revision: u64,
}

#[derive(Debug, Default)]
pub struct SettingsState {
    /// Peticiones en vuelo: la UI las pinta en ámbar.
    pub pending: HashMap<SettingId, Pending>,
    /// Acciones listas para ejecutar (o pendientes de confirmación).
    pub queue: Vec<SettingAction>,
    /// Historial acotado para debug.
    pub log: Vec<String>,
}

impl SettingsState {
    fn push_log(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > 64 {
            let n = self.log.len() - 64;
            self.log.drain(..n);
        }
    }

    pub fn enqueue(&mut self, action: SettingAction) {
        let id = action.id();
        self.push_log(format!("ENQUEUE {id:?} desired={:?}", action.desired()));
        let phase = if action.needs_confirmation() {
            ActionPhase::AwaitingConfirmation
        } else {
            ActionPhase::InFlight
        };
        self.pending.insert(
            id,
            Pending {
                action,
                phase,
                since: Instant::now(),
                since_revision: 0,
            },
        );
        if phase == ActionPhase::InFlight {
            self.queue.push(action);
        }
    }

    pub fn confirm(&mut self, id: SettingId) {
        if let Some(p) = self.pending.get_mut(&id) {
            if p.phase == ActionPhase::AwaitingConfirmation {
                p.phase = ActionPhase::InFlight;
                p.since = Instant::now();
                let action = p.action;
                self.queue.push(action);
                self.push_log(format!("CONFIRM {id:?} → EXEC"));
            }
        }
    }

    pub fn cancel(&mut self, id: SettingId) {
        if self
            .pending
            .get(&id)
            .is_some_and(|p| p.phase == ActionPhase::AwaitingConfirmation)
        {
            self.pending.remove(&id);
            self.push_log(format!("CANCEL {id:?}"));
        }
    }

    pub fn phase(&self, id: SettingId) -> Option<ActionPhase> {
        self.pending.get(&id).map(|p| p.phase)
    }

    /// Cierra las peticiones cuando la lectura confirma o cuando expiran.
    /// `sys_revision` y `dnd_revision` permiten esperar a datos nuevos.
    pub fn reconcile(
        &mut self,
        now: Instant,
        sys_revision: u64,
        dnd_revision: u64,
        system: &SystemInfo,
        dnd: bool,
    ) {
        let mut finished = Vec::new();
        for (id, p) in &self.pending {
            if p.phase != ActionPhase::InFlight {
                continue;
            }
            let elapsed = now.duration_since(p.since);
            let fresh = match id {
                SettingId::Dnd => {
                    dnd_revision > p.since_revision || elapsed >= Duration::from_millis(200)
                }
                _ => sys_revision > p.since_revision || p.since_revision == 0,
            };
            let actual = actual_value(*id, system, dnd);
            if fresh && actual == Some(p.action.desired()) {
                finished.push((*id, format!("DONE {id:?}")));
            } else if elapsed >= TIMEOUT {
                finished.push((*id, format!("TIMEOUT {id:?} — se revierte")));
            }
        }
        for (id, line) in finished {
            self.push_log(line);
            self.pending.remove(&id);
        }
    }

    /// Para el previewer: no hay lectura real, solo se limpia el semáforo.
    pub fn clear_pending(&mut self) {
        self.pending.clear();
    }
}

fn actual_value(id: SettingId, system: &SystemInfo, dnd: bool) -> Option<SettingValue> {
    Some(match id {
        SettingId::Wifi => SettingValue::Bool(system.wifi.as_ref()?.enabled),
        SettingId::Bluetooth => SettingValue::Bool(system.bluetooth.as_ref()?.powered),
        SettingId::Dnd => SettingValue::Bool(dnd),
        SettingId::Volume => SettingValue::Percent(system.volume.as_ref()?.percent),
        SettingId::Brightness => SettingValue::Percent(system.brightness?),
        SettingId::Lock => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_enqueues_immediately() {
        let mut s = SettingsState::default();
        s.enqueue(SettingAction::Wifi(true));
        assert_eq!(s.queue, vec![SettingAction::Wifi(true)]);
        assert_eq!(s.phase(SettingId::Wifi), Some(ActionPhase::InFlight));
    }

    #[test]
    fn lock_waits_for_confirmation() {
        let mut s = SettingsState::default();
        s.enqueue(SettingAction::Lock);
        assert!(s.queue.is_empty());
        assert_eq!(
            s.phase(SettingId::Lock),
            Some(ActionPhase::AwaitingConfirmation)
        );
        s.confirm(SettingId::Lock);
        assert_eq!(s.queue, vec![SettingAction::Lock]);
        assert_eq!(s.phase(SettingId::Lock), Some(ActionPhase::InFlight));
    }

    #[test]
    fn wifi_command_is_the_expected_argv() {
        assert_eq!(
            SettingAction::Wifi(true).command(),
            Some(vec![
                "nmcli".into(),
                "radio".into(),
                "wifi".into(),
                "on".into()
            ])
        );
    }
}
