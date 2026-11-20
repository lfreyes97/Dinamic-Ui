//! Servidor de notificaciones **freedesktop** (`org.freedesktop.Notifications`).
//!
//! Dinamic UI registra el nombre y atiende `Notify`, `CloseNotification`,
//! `GetCapabilities` y `GetServerInformation`. No depende de Mako ni de ningún
//! daemon concreto (los clientes estándar, p. ej. `notify-send`, hablan este
//! protocolo). Si otro servidor ya tiene el nombre, se limita a avisar por log.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use zbus::{blocking::Connection, interface, zvariant::OwnedValue};

#[derive(Debug, Clone)]
pub struct Notification {
    pub id: u32,
    pub app: String,
    pub summary: String,
    pub body: String,
    pub time: String,
    pub received: Instant,
}

#[derive(Default)]
struct Shared {
    queue: VecDeque<Notification>,
    next_id: u32,
    revision: u64,
    /// Notificaciones recibidas desde el inicio (o desde "Limpiar todo").
    session_count: u64,
    dnd: bool,
}

/// Cola compartida entre el hilo D-Bus y el loop principal.
pub struct Notifications {
    shared: Arc<Mutex<Shared>>,
}

struct Server {
    shared: Arc<Mutex<Shared>>,
}

#[interface(name = "org.freedesktop.Notifications")]
impl Server {
    /// Estándar: `susssasa{sv}i` → devuelve el id asignado.
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &mut self,
        app_name: &str,
        replaces_id: u32,
        _app_icon: &str,
        summary: &str,
        body: &str,
        _actions: Vec<String>,
        _hints: HashMap<String, OwnedValue>,
        _expire_timeout: i32,
    ) -> u32 {
        let Ok(mut state) = self.shared.lock() else {
            return 0;
        };
        if replaces_id != 0 {
            state.queue.retain(|n| n.id != replaces_id);
        }
        state.next_id = state.next_id.wrapping_add(1).max(1);
        let id = state.next_id;
        state.session_count += 1;
        state.queue.push_back(Notification {
            id,
            app: if app_name.is_empty() {
                "Notificación".to_string()
            } else {
                app_name.to_string()
            },
            summary: summary.to_string(),
            body: body.to_string(),
            time: chrono::Local::now().format("%H:%M").to_string(),
            received: Instant::now(),
        });
        if state.queue.len() > 16 {
            state.queue.pop_front();
        }
        state.revision += 1;
        id
    }

    fn close_notification(&mut self, id: u32) {
        if let Ok(mut state) = self.shared.lock() {
            state.queue.retain(|n| n.id != id);
            state.revision += 1;
        }
    }

    fn get_capabilities(&self) -> Vec<&'static str> {
        vec!["body", "actions"]
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "Dinamic UI".to_string(),
            "Dinamic UI".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
            "1.2".to_string(),
        )
    }
}

impl Notifications {
    pub fn new() -> Self {
        let mut state = Shared::default();
        if std::env::var_os("DINAMIC_UI_DND").is_some() {
            state.dnd = true;
        }
        let shared = Arc::new(Mutex::new(state));
        let listener = Arc::clone(&shared);
        thread::Builder::new()
            .name("dinamic-ui-notifications".into())
            .spawn(move || serve(listener))
            .ok();
        Self { shared }
    }

    /// Devuelve (activa, número en cola, revisión). Descarta las caducadas.
    /// Con DND activo la isla no muestra nada, pero la cola sigue disponible.
    pub fn poll(&self, ttl: Duration) -> (Option<Notification>, usize, u64) {
        let Ok(mut state) = self.shared.lock() else {
            return (None, 0, 0);
        };
        let now = Instant::now();
        state.queue.retain(|n| now.duration_since(n.received) < ttl);
        let active = if state.dnd {
            None
        } else {
            state.queue.front().cloned()
        };
        (active, state.queue.len(), state.revision)
    }

    /// Copia de la cola para el panel.
    pub fn list(&self) -> Vec<Notification> {
        self.shared
            .lock()
            .map(|s| s.queue.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn dnd(&self) -> bool {
        self.shared.lock().map(|s| s.dnd).unwrap_or(false)
    }

    pub fn set_dnd(&self, on: bool) {
        if let Ok(mut state) = self.shared.lock() {
            state.dnd = on;
            state.revision += 1;
        }
    }

    pub fn dismiss(&self) {
        if let Ok(mut state) = self.shared.lock() {
            state.queue.pop_front();
            state.revision += 1;
        }
    }

    pub fn dismiss_id(&self, id: u32) {
        if let Ok(mut state) = self.shared.lock() {
            state.queue.retain(|n| n.id != id);
            state.revision += 1;
        }
    }

    pub fn clear_all(&self) {
        if let Ok(mut state) = self.shared.lock() {
            state.queue.clear();
            state.session_count = 0;
            state.revision += 1;
        }
    }

    /// Total acumulado en la sesión (se reinicia con `clear_all`).
    pub fn session_count(&self) -> u64 {
        self.shared.lock().map(|s| s.session_count).unwrap_or(0)
    }
}

fn serve(shared: Arc<Mutex<Shared>>) {
    let Ok(conn) = Connection::session() else {
        eprintln!("[dinamic-ui] DBus: no se pudo conectar a la sesión");
        return;
    };
    if let Err(e) = conn
        .object_server()
        .at("/org/freedesktop/Notifications", Server { shared })
    {
        eprintln!("[dinamic-ui] DBus: no se pudo registrar el objeto: {e}");
        return;
    }
    match conn.request_name("org.freedesktop.Notifications") {
        Ok(()) => eprintln!("[dinamic-ui] servidor de notificaciones registrado"),
        Err(e) => eprintln!("[dinamic-ui] otro daemon tiene el nombre ({e}); sin notificaciones"),
    }
    loop {
        thread::park();
    }
}
