//! Datos del command menu: apps (.desktop), archivos y comandos.
//!
//! Una sola fuente de verdad; la UI solo filtra y ejecuta acciones.

use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuMode {
    #[default]
    Apps,
    Files,
    Workspaces,
    Commands,
}

impl MenuMode {
    pub const ALL: [Self; 4] = [Self::Apps, Self::Files, Self::Workspaces, Self::Commands];
    pub fn label(self) -> &'static str {
        match self {
            Self::Apps => "Apps",
            Self::Files => "Archivos",
            Self::Workspaces => "Workspaces",
            Self::Commands => "Comandos",
        }
    }
    pub fn shortcut(self) -> &'static str {
        match self {
            Self::Apps => "1",
            Self::Files => "2",
            Self::Workspaces => "3",
            Self::Commands => "4",
        }
    }
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone)]
pub enum EntryAction {
    Launch(String),
    OpenFile(PathBuf),
    Workspace(u32),
    Command(String),
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub title: String,
    pub subtitle: String,
    /// Clase/nombre para resolver el icono en `app_icon_textures`.
    pub icon_key: String,
    pub action: EntryAction,
}

/// Puntuación simple: substring puntúa alto; subsecuencia, bajo.
fn score(query: &str, text: &str) -> i32 {
    if query.is_empty() {
        return 1;
    }
    let q = query.to_lowercase();
    let t = text.to_lowercase();
    if let Some(pos) = t.find(&q) {
        return 100 - pos.min(50) as i32;
    }
    let mut qi = q.chars();
    let Some(mut current) = qi.next() else {
        return 0;
    };
    let mut points = 0;
    for c in t.chars() {
        if c == current {
            points += 1;
            if let Some(next) = qi.next() {
                current = next;
            } else {
                return points;
            }
        }
    }
    0
}

pub fn filter(entries: &[Entry], query: &str) -> Vec<Entry> {
    let mut scored: Vec<(i32, Entry)> = entries
        .iter()
        .filter_map(|e| {
            let s = score(query, &format!("{} {}", e.title, e.subtitle));
            (s > 0).then(|| (s, e.clone()))
        })
        .collect();
    scored.sort_by_key(|(s, _)| -s);
    scored.into_iter().map(|(_, e)| e).collect()
}

/// Apps de los `.desktop` del sistema y del usuario.
pub fn apps() -> Vec<Entry> {
    let mut dirs = vec![PathBuf::from("/usr/share/applications")];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }
    let mut out = Vec::new();
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let field = |key: &str| -> Option<String> {
                text.lines()
                    .find_map(|l| l.strip_prefix(key)?.strip_prefix('=')?.trim().into())
                    .map(str::to_string)
            };
            if field("NoDisplay").as_deref() == Some("true")
                || field("Hidden").as_deref() == Some("true")
            {
                continue;
            }
            let Some(name) = field("Name") else { continue };
            let Some(exec) = field("Exec") else { continue };
            let exec = exec
                .replace("%f", "")
                .replace("%F", "")
                .replace("%u", "")
                .replace("%U", "")
                .trim()
                .to_string();
            if exec.is_empty() || exec.starts_with("sh -c") {
                continue;
            }
            let icon = field("Icon").unwrap_or_default();
            let icon_key = if Path::new(&icon).is_absolute() {
                name.to_lowercase()
            } else {
                icon.to_lowercase()
            };
            out.push(Entry {
                subtitle: field("Comment").unwrap_or_else(|| "Aplicación".into()),
                title: name,
                icon_key,
                action: EntryAction::Launch(exec),
            });
        }
    }
    out.sort_by(|a, b| a.title.cmp(&b.title));
    out.dedup_by(|a, b| a.title == b.title);
    out
}

/// Archivos recientes de los directorios configurados (limitado).
pub fn files(dirs: &[String]) -> Vec<Entry> {
    let mut out = Vec::new();
    for raw in dirs {
        let dir = expand_home(raw);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten().take(120) {
            let path = entry.path();
            if path.is_dir() && out.len() < 200 {
                // Un nivel de profundidad, sin entrar en ocultos.
                let Ok(children) = fs::read_dir(&path) else {
                    continue;
                };
                for child in children.flatten().take(40) {
                    push_file(&mut out, child.path());
                }
            } else {
                push_file(&mut out, path);
            }
            if out.len() >= 300 {
                break;
            }
        }
    }
    out
}

fn push_file(out: &mut Vec<Entry>, path: PathBuf) {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return;
    };
    if name.starts_with('.') {
        return;
    }
    let subtitle = path
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("~")
        .to_string();
    out.push(Entry {
        title: name.to_string(),
        subtitle,
        icon_key: if path.is_dir() {
            "folder".into()
        } else {
            "file".into()
        },
        action: EntryAction::OpenFile(path),
    });
}

pub fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

pub fn workspaces(ids: &[u32]) -> Vec<Entry> {
    ids.iter()
        .map(|id| Entry {
            title: format!("Workspace {id}"),
            subtitle: "Cambiar de workspace".into(),
            icon_key: "workspace".into(),
            action: EntryAction::Workspace(*id),
        })
        .collect()
}

pub fn commands() -> Vec<Entry> {
    [
        ("Panel de control", "Abrir/cerrar el panel", "panel"),
        ("Overview", "Vista de workspaces", "overview"),
        ("Recargar configuración", "config.toml", "reload"),
        ("Modo compacto", "Isla con reloj", "compact"),
        ("Salir del shell", "Cerrar Dinamic UI", "quit"),
    ]
    .into_iter()
    .map(|(title, subtitle, key)| Entry {
        title: title.into(),
        subtitle: subtitle.into(),
        icon_key: key.into(),
        action: EntryAction::Command(key.into()),
    })
    .collect()
}
