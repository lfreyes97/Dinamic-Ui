//! Cliente mínimo del IPC de Hyprland.
//!
//! Consulta el socket de comandos (`.socket.sock`) para obtener el workspace
//! activo y la ventana activa. Las consultas se limitan a una cada 400 ms para
//! no saturar al compositor.

use std::{
    env,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    time::{Duration, Instant},
};

use serde::de::DeserializeOwned;
use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HyprlandSnapshot {
    pub workspace: String,
    pub window_title: String,
    pub window_class: String,
    pub window_address: String,
    /// Ids de los workspaces con al menos una ventana (sin los especiales).
    pub occupied: Vec<u32>,
}

#[derive(Deserialize)]
struct WorkspaceListItem {
    id: i64,
    #[serde(default)]
    windows: u32,
}

#[derive(Deserialize)]
struct WorkspaceResponse {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct WindowResponse {
    #[serde(default)]
    title: String,
    #[serde(default)]
    class: String,
    #[serde(default)]
    address: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientInfo {
    pub class: String,
    pub title: String,
    pub address: String,
    pub mapped: bool,
    /// Workspace al que pertenece la ventana.
    pub workspace: i32,
    /// Posición y tamaño en coordenadas lógicas del monitor.
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub struct HyprlandIpc {
    socket: Option<PathBuf>,
    event_stream: Option<UnixStream>,
    event_buffer: String,
    last_query: Instant,
    snapshot: HyprlandSnapshot,
}

impl HyprlandIpc {
    pub fn new() -> Self {
        let socket = socket_path();
        if socket.is_none() {
            eprintln!(
                "[dinamic-ui] No se encontró el socket de Hyprland; se mostrarán datos de ejemplo."
            );
        }
        let event_stream = socket.as_ref().and_then(|path| {
            let mut event_path = path.clone();
            event_path.set_file_name(".socket2.sock");
            let stream = UnixStream::connect(event_path).ok()?;
            stream.set_nonblocking(true).ok()?;
            Some(stream)
        });

        Self {
            socket,
            event_stream,
            event_buffer: String::new(),
            last_query: Instant::now() - Duration::from_secs(1),
            snapshot: HyprlandSnapshot::default(),
        }
    }

    pub fn snapshot(&self) -> &HyprlandSnapshot {
        &self.snapshot
    }

    /// Posición del cursor (`cursorpos` devuelve "x, y" en texto).
    pub fn cursor_pos(&self) -> Option<(i32, i32)> {
        let path = self.socket.as_ref()?;
        let mut stream = UnixStream::connect(path).ok()?;
        stream.set_read_timeout(Some(Duration::from_millis(60))).ok()?;
        stream.write_all(b"cursorpos").ok()?;
        let mut buf = [0u8; 128];
        let n = stream.read(&mut buf).ok()?;
        let text = String::from_utf8_lossy(&buf[..n]);
        let mut parts = text.split(',');
        let x = parts.next()?.trim().parse().ok()?;
        let y = parts.next()?.trim().parse().ok()?;
        Some((x, y))
    }

    /// Clientes mapeados, para el dock.
    pub fn clients(&self) -> Vec<ClientInfo> {
        let Some(path) = self.socket.as_ref() else {
            return Vec::new();
        };
        let Some(items) = query_array::<Vec<serde_json::Value>>(path, "j/clients") else {
            return Vec::new();
        };
        items
            .into_iter()
            .filter_map(|v| {
                let get = |key: &str| v.get(key)?.as_str().map(str::to_string);
                let at = v.get("at")?.as_array()?;
                let size = v.get("size")?.as_array()?;
                Some(ClientInfo {
                    class: get("class").unwrap_or_default(),
                    title: get("title").unwrap_or_default(),
                    address: get("address").unwrap_or_default(),
                    mapped: v.get("mapped")?.as_bool().unwrap_or(false),
                    workspace: v
                        .get("workspace")?
                        .get("id")?
                        .as_i64()
                        .unwrap_or(0) as i32,
                    x: at.first()?.as_i64().unwrap_or(0) as i32,
                    y: at.get(1)?.as_i64().unwrap_or(0) as i32,
                    w: size.first()?.as_i64().unwrap_or(0) as i32,
                    h: size.get(1)?.as_i64().unwrap_or(0) as i32,
                })
            })
            .filter(|c| c.mapped && !c.class.is_empty())
            .collect()
    }

    /// Fuerza la siguiente consulta de estado (tras un evento de Hyprland).
    pub fn force_refresh(&mut self) {
        self.last_query = Instant::now() - Duration::from_secs(1);
    }

    /// Envía un comando `dispatch` a Hyprland (con config en Lua, una expresión
    /// como `hl.dsp.focus({ workspace = 2 })`).
    ///
    /// Hyprland 0.57 descarta la petición si el cliente cierra el socket sin
    /// leer la respuesta (comprobado: escribir y cerrar no ejecuta nada), así
    /// que hay que esperar el "ok". Se hace en un hilo aparte para no bloquear
    /// el render; si Hyprland responde con un error, se registra.
    pub fn dispatch(&self, command: &str) {
        let Some(path) = self.socket.clone() else {
            return;
        };
        // Hyprland con config Lua no acepta la sintaxis clásica `dispatch ...`.
        // Traducimos las dos órdenes que usa la UI a `eval` con la API Lua.
        let request = if let Some(id) = command.strip_prefix("workspace ") {
            format!("dispatch hl.dsp.focus({{ workspace = {id} }})")
        } else if let Some(class) = command.strip_prefix("focuswindow class:") {
            format!(
                "dispatch local ws=hl.get_windows() for _,w in ipairs(ws) do if w.class=='{class}' then hl.dsp.focus({{ window = 'address:'..w.address }}) break end end"
            )
        } else if let Some(address) = command.strip_prefix("focuswindow address:") {
            format!("dispatch hl.dsp.focus({{ window = 'address:{address}' }})")
        } else {
            format!("dispatch {command}")
        };
        std::thread::spawn(move || {
            let result = (|| -> std::io::Result<String> {
                let mut stream = UnixStream::connect(&path)?;
                stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                stream.write_all(request.as_bytes())?;
                let mut reply = String::new();
                stream.read_to_string(&mut reply)?;
                Ok(reply)
            })();
            match result {
                Ok(reply) if reply.trim() == "ok" => {}
                Ok(reply) => eprintln!("[dinamic-ui] hyprland `{request}`: {}", reply.trim()),
                Err(e) => eprintln!("[dinamic-ui] hyprland `{request}`: {e}"),
            }
        });
    }

    /// Lee el socket de eventos sin bloquear. Devuelve `true` si cambió el
    /// workspace o la ventana activa (para disparar una reacción visual).
    pub fn poll_events(&mut self) -> bool {
        let Some(stream) = self.event_stream.as_mut() else {
            return false;
        };

        let mut buf = [0u8; 8192];
        loop {
            match stream.read(&mut buf) {
                Ok(0) => {
                    self.event_stream.take();
                    break;
                }
                Ok(n) => self.event_buffer.push_str(&String::from_utf8_lossy(&buf[..n])),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.event_stream.take();
                    break;
                }
            }
        }

        let mut changed = false;
        while let Some(pos) = self.event_buffer.find('\n') {
            let line: String = self.event_buffer.drain(..=pos).collect();
            let line = line.trim_end();
            if line.starts_with("activewindow>>")
                || line.starts_with("workspace>>")
                || line.starts_with("focusedmon>>")
                || line.starts_with("closewindow>>")
                || line.starts_with("openwindow>>")
                || line.starts_with("movewindow")
                || line.starts_with("moveworkspace")
                || line.starts_with("createworkspace")
                || line.starts_with("destroyworkspace")
            {
                changed = true;
            }
        }
        changed
    }

    /// Actualiza el snapshot si ha pasado suficiente tiempo.
    pub fn poll(&mut self) {
        if self.last_query.elapsed() < Duration::from_millis(400) {
            return;
        }
        self.last_query = Instant::now();

        let Some(path) = &self.socket else {
            self.snapshot = demo_snapshot();
            return;
        };

        // Hyprland cierra el socket de comandos tras cada petición: hace falta
        // una conexión nueva por consulta.
        let ws: Option<WorkspaceResponse> = query(path, "j/activeworkspace");
        let win: Option<WindowResponse> = query(path, "j/activewindow");
        let all: Option<Vec<WorkspaceListItem>> = query_array(path, "j/workspaces");

        if let Some(ws) = ws {
            self.snapshot.workspace = ws.name;
        }
        if let Some(win) = win {
            self.snapshot.window_title = win.title;
            self.snapshot.window_class = win.class;
            self.snapshot.window_address = win.address;
        }
        if let Some(all) = all {
            let mut occupied: Vec<u32> = all
                .iter()
                .filter(|w| w.windows > 0 && w.id > 0)
                .map(|w| w.id as u32)
                .collect();
            occupied.sort_unstable();
            self.snapshot.occupied = occupied;
        }
    }
}

fn query<T: DeserializeOwned>(path: &std::path::Path, command: &str) -> Option<T> {
    let mut stream = UnixStream::connect(path).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(60)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(60)));
    stream.write_all(command.as_bytes()).ok()?;

    let mut buf = [0u8; 16384];
    let n = stream.read(&mut buf).ok()?;
    if n == 0 {
        return None;
    }

    // Hyprland responde con JSON pero a veces añade bytes de eventos detrás.
    let text = String::from_utf8_lossy(&buf[..n]);
    let start = text.find('{')?;
    let end = text.rfind('}')? + 1;
    serde_json::from_str(&text[start..end]).ok()
}

/// Como `query`, pero para respuestas JSON que son un array (`j/workspaces`).
fn query_array<T: DeserializeOwned>(path: &std::path::Path, command: &str) -> Option<T> {
    let mut stream = UnixStream::connect(path).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(60)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(60)));
    stream.write_all(command.as_bytes()).ok()?;

    let mut buf = [0u8; 16384];
    let n = stream.read(&mut buf).ok()?;
    if n == 0 {
        return None;
    }
    let text = String::from_utf8_lossy(&buf[..n]);
    let start = text.find('[')?;
    let end = text.rfind(']')? + 1;
    serde_json::from_str(&text[start..end]).ok()
}

fn socket_path() -> Option<PathBuf> {
    let runtime = env::var("XDG_RUNTIME_DIR").ok()?;
    let signature = env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    let path = PathBuf::from(runtime)
        .join("hypr")
        .join(signature)
        .join(".socket.sock");
    path.exists().then_some(path)
}

fn demo_snapshot() -> HyprlandSnapshot {
    HyprlandSnapshot {
        workspace: "1".into(),
        window_title: "Dinamic UI (demo)".into(),
        window_class: "dinamic-ui".into(),
        window_address: String::new(),
        occupied: vec![1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occupied_workspaces_skip_empty_and_special() {
        let json = r#"[{"id":2,"name":"2","windows":1},{"id":4,"name":"4","windows":0},
            {"id":-98,"name":"special:magic","windows":3},{"id":1,"name":"1","windows":2}]"#;
        let all: Vec<WorkspaceListItem> = serde_json::from_str(json).unwrap();
        let mut occupied: Vec<u32> = all
            .iter()
            .filter(|w| w.windows > 0 && w.id > 0)
            .map(|w| w.id as u32)
            .collect();
        occupied.sort_unstable();
        assert_eq!(occupied, vec![1, 2]);
    }
}
