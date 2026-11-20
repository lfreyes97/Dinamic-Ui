//! Datos del command menu: apps (.desktop), archivos y comandos.
//!
//! Una sola fuente de verdad; la UI solo filtra y ejecuta acciones. El criterio de búsqueda
//! es el de `search.rs`, compartido con Actividades.

use std::{fs, path::PathBuf};

use crate::search;

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
    /// Identificador para la búsqueda: el `id` del `.desktop` en apps, la clave en comandos;
    /// vacío si no aplica (archivos, workspaces).
    pub id: String,
    /// Clase/nombre para resolver el icono en `app_icon_textures`.
    pub icon_key: String,
    pub action: EntryAction,
}

/// Filtra y ordena por puntuación (criterio de `search::score` sobre título, id y subtítulo).
/// Consulta vacía: todas las entradas, en su orden.
pub fn filter(entries: &[Entry], query: &str) -> Vec<Entry> {
    let query = query.trim();
    if query.is_empty() {
        return entries.to_vec();
    }
    let mut scored: Vec<(i32, &Entry)> = entries
        .iter()
        .filter_map(|e| search::score(query, &e.title, &e.id, &e.subtitle).map(|s| (s, e)))
        .collect();
    // Estable: a igualdad de puntuación se mantiene el orden original.
    scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
    scored.into_iter().map(|(_, e)| e.clone()).collect()
}

/// Apps de los `.desktop` del sistema y del usuario.
pub fn apps() -> Vec<Entry> {
    let mut out: Vec<Entry> = crate::desktop::scan()
        .into_iter()
        .filter(|app| app.launchable())
        .map(|app| Entry {
            subtitle: if app.comment.is_empty() {
                "Aplicación".into()
            } else {
                app.comment
            },
            title: app.name,
            id: app.id.clone(),
            // El `id` siempre está en el mapa de iconos (el nombre del icono solo a veces).
            icon_key: app.id,
            action: EntryAction::Launch(app.exec),
        })
        .collect();
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
        id: String::new(),
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
            id: String::new(),
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
        id: key.into(),
        icon_key: key.into(),
        action: EntryAction::Command(key.into()),
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, subtitle: &str, id: &str) -> Entry {
        Entry {
            title: title.into(),
            subtitle: subtitle.into(),
            id: id.into(),
            icon_key: String::new(),
            action: EntryAction::Command(String::new()),
        }
    }

    fn titles(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|e| e.title.as_str()).collect()
    }

    #[test]
    fn empty_query_keeps_everything_in_order() {
        let entries = vec![
            entry("Zeta", "", ""),
            entry("Alfa", "", ""),
            entry("Beta", "", ""),
        ];
        assert_eq!(titles(&filter(&entries, "")), ["Zeta", "Alfa", "Beta"]);
        assert_eq!(titles(&filter(&entries, "   ")), ["Zeta", "Alfa", "Beta"]);
    }

    #[test]
    fn query_ranks_by_score_and_drops_non_matches() {
        let entries = vec![
            entry("Visor de imágenes", "Ver fotos", ""),
            entry("Firefox", "Navegador", ""),
            entry("Fotos", "Galería", ""),
        ];
        // «fo»: «Fotos» (prefijo del nombre, 100) > «Firefox» (contiene, 60) > «Visor» (solo
        // aparece en el subtítulo «Ver fotos», 20). Es la única coincidencia de «Visor».
        let found = filter(&entries, "fo");
        assert_eq!(titles(&found), ["Fotos", "Firefox", "Visor de imágenes"]);
        assert!(filter(&entries, "zzz").is_empty());
    }

    #[test]
    fn matching_is_strict_no_subsequence() {
        let entries = vec![entry("Firefox", "Navegador", "firefox")];
        // Antes, la subsecuencia encontraba «frfx»; el criterio compartido no.
        assert!(filter(&entries, "frfx").is_empty());
        assert_eq!(titles(&filter(&entries, "fox")), ["Firefox"]);
    }

    #[test]
    fn every_word_must_match_some_field() {
        let entries = vec![
            entry("Panel de control", "Abrir/cerrar el panel", "panel"),
            entry("Overview", "Vista de workspaces", "overview"),
        ];
        assert_eq!(
            titles(&filter(&entries, "panel control")),
            ["Panel de control"]
        );
        assert!(filter(&entries, "panel workspaces zzz").is_empty());
    }

    #[test]
    fn id_and_subtitle_count_as_fallback_fields() {
        let entries = vec![
            entry("Firefox", "Navegador", "org.mozilla.firefox"),
            entry("Salir del shell", "Cerrar Dinamic UI", "quit"),
            entry("Documento", "/home/usuario/Documentos", ""),
        ];
        // Por id (apps y comandos).
        assert_eq!(titles(&filter(&entries, "mozilla")), ["Firefox"]);
        assert_eq!(titles(&filter(&entries, "quit")), ["Salir del shell"]);
        // Por subtítulo (ruta de un archivo).
        assert_eq!(titles(&filter(&entries, "documentos")), ["Documento"]);
    }
}
