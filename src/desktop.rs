//! Entradas `.desktop`: una sola lectura para los iconos, el command menu y el overview.
//!
//! Solo se lee el grupo `[Desktop Entry]` (las acciones `[Desktop Action …]` repiten claves
//! como `Name` o `NoDisplay` y no deben contaminar la entrada principal).

use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesktopApp {
    /// Nombre del archivo sin extensión, en minúsculas (`org.mozilla.firefox`).
    pub id: String,
    pub name: String,
    pub comment: String,
    /// `Exec=` sin códigos de campo (`%u`, `%F`…).
    pub exec: String,
    pub icon: String,
    /// `StartupWMClass=` en minúsculas, si existe.
    pub wm_class: Option<String>,
    /// `NoDisplay`, `Hidden` o un tipo que no es `Application`.
    pub hidden: bool,
}

impl DesktopApp {
    /// ¿Se puede mostrar en un lanzador y ejecutar?
    pub fn launchable(&self) -> bool {
        // `sh -c '…'` lleva comillas anidadas que no sabemos reconstruir de forma fiable.
        !self.hidden
            && !self.name.is_empty()
            && !self.exec.is_empty()
            && !self.exec.starts_with("sh -c")
    }

    /// ¿Una ventana con esta `class` de Hyprland pertenece a esta app?
    pub fn matches_class(&self, class: &str) -> bool {
        let class = class.to_lowercase();
        !class.is_empty()
            && (self.id == class
                || self.wm_class.as_deref() == Some(class.as_str())
                || self.name.to_lowercase() == class
                || self.binary().is_some_and(|b| b == class))
    }

    /// ¿`pin` (un nombre de `[dock] pinned`) designa a esta app?
    pub fn matches_name(&self, pin: &str) -> bool {
        self.matches_class(pin)
    }

    /// Nombre del ejecutable (primer token de `Exec`, sin ruta), en minúsculas.
    fn binary(&self) -> Option<String> {
        let first = split_exec(&self.exec).into_iter().next()?;
        let name = first.rsplit('/').next()?.to_lowercase();
        (!name.is_empty()).then_some(name)
    }
}

/// Lee una entrada. `stem` es el nombre del archivo sin extensión.
/// `None` si el texto no tiene grupo `[Desktop Entry]`.
pub fn parse(stem: &str, text: &str) -> Option<DesktopApp> {
    let mut in_entry = false;
    let mut seen = false;
    let mut app = DesktopApp {
        id: stem.to_lowercase(),
        ..Default::default()
    };
    let mut kind_ok = true;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            if in_entry {
                break; // acabó el grupo principal
            }
            in_entry = line == "[Desktop Entry]";
            seen |= in_entry;
            continue;
        }
        if !in_entry || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Name" => app.name = value.to_string(),
            "Comment" => app.comment = value.to_string(),
            "Exec" => app.exec = clean_exec(value),
            "Icon" => app.icon = value.to_string(),
            "StartupWMClass" => app.wm_class = Some(value.to_lowercase()),
            "NoDisplay" | "Hidden" => app.hidden |= value == "true",
            "Type" => kind_ok = value == "Application",
            _ => {}
        }
    }
    app.hidden |= !kind_ok;
    seen.then_some(app)
}

/// Quita los códigos de campo de `Exec` (`%f`, `%U`, `%i`…); `%%` pasa a `%`.
pub fn clean_exec(exec: &str) -> String {
    let mut out = String::with_capacity(exec.len());
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        if chars.next() == Some('%') {
            out.push('%');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Separa una línea de comando en argumentos respetando comillas (`"a b"`, `'a b'`) y `\`.
pub fn split_exec(exec: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut has_token = false;
    let mut quote: Option<char> = None;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') | (None, '\\') => {
                if let Some(next) = chars.next() {
                    current.push(next);
                    has_token = true;
                }
            }
            (Some(_), c) => current.push(c),
            (None, '"') | (None, '\'') => {
                quote = Some(c);
                has_token = true;
            }
            (None, c) if c.is_whitespace() => {
                if has_token {
                    args.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            (None, c) => {
                current.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        args.push(current);
    }
    args
}

/// Directorios de aplicaciones: primero el del usuario, que gana a los del sistema.
fn app_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    match std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        Some(home) => dirs.push(PathBuf::from(home).join("applications")),
        None => {
            if let Some(home) = std::env::var_os("HOME") {
                dirs.push(PathBuf::from(home).join(".local/share/applications"));
            }
        }
    }
    let system = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());
    dirs.extend(
        system
            .split(':')
            .map(|d| PathBuf::from(d).join("applications")),
    );
    dirs
}

/// Todas las entradas (también las ocultas: sirven para resolver iconos de ventanas abiertas).
/// Si un `id` aparece en varios directorios, vale el primero (el del usuario).
pub fn scan() -> Vec<DesktopApp> {
    let mut out: Vec<DesktopApp> = Vec::new();
    for dir in app_dirs() {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("desktop") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(app) = parse(stem, &text) {
                if !out.iter().any(|a| a.id == app.id) {
                    out.push(app);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIREFOX: &str = "\
[Desktop Entry]
Version=1.0
Name=Firefox
Name[es]=Firefox web
Comment=Navega
Exec=/usr/lib/firefox/firefox --new-window %u
Icon=firefox
Type=Application
StartupWMClass=firefox-esr

[Desktop Action new-private-window]
Name=Ventana privada
Exec=firefox --private-window %u
NoDisplay=true
";

    #[test]
    fn parses_only_the_main_group() {
        let app = parse("Org.Mozilla.Firefox", FIREFOX).unwrap();
        assert_eq!(app.id, "org.mozilla.firefox");
        assert_eq!(app.name, "Firefox");
        assert_eq!(app.exec, "/usr/lib/firefox/firefox --new-window");
        assert_eq!(app.icon, "firefox");
        assert_eq!(app.wm_class.as_deref(), Some("firefox-esr"));
        // `NoDisplay=true` pertenece a la acción, no a la entrada.
        assert!(!app.hidden);
        assert!(app.launchable());
    }

    #[test]
    fn hidden_and_non_application_entries_are_not_launchable() {
        let nodisplay = "[Desktop Entry]\nName=X\nExec=x\nNoDisplay=true\n";
        assert!(!parse("x", nodisplay).unwrap().launchable());
        let link = "[Desktop Entry]\nName=X\nExec=x\nType=Link\n";
        assert!(!parse("x", link).unwrap().launchable());
        let shell = "[Desktop Entry]\nName=X\nExec=sh -c 'a b'\n";
        assert!(!parse("x", shell).unwrap().launchable());
        assert!(parse("x", "Name=sin grupo\n").is_none());
    }

    #[test]
    fn exec_field_codes_are_removed() {
        assert_eq!(clean_exec("app %U"), "app");
        assert_eq!(
            clean_exec("app --icon %i --name %c %f"),
            "app --icon --name"
        );
        assert_eq!(clean_exec("app 100%% %k"), "app 100%");
    }

    #[test]
    fn split_exec_respects_quotes_and_escapes() {
        assert_eq!(split_exec("a b  c"), ["a", "b", "c"]);
        assert_eq!(split_exec(r#"env "A B"=1 app"#), ["env", "A B=1", "app"]);
        assert_eq!(split_exec("a 'b c' d"), ["a", "b c", "d"]);
        assert_eq!(split_exec(r"a b\ c"), ["a", "b c"]);
        assert_eq!(split_exec(r#"a "" b"#), ["a", "", "b"]);
        assert!(split_exec("   ").is_empty());
    }

    #[test]
    fn matches_windows_by_id_class_name_or_binary() {
        let app = parse("org.mozilla.firefox", FIREFOX).unwrap();
        assert!(app.matches_class("org.mozilla.firefox"));
        assert!(app.matches_class("Firefox-ESR"));
        assert!(app.matches_class("firefox")); // nombre y binario
        assert!(!app.matches_class("kitty"));
        assert!(!app.matches_class(""));
    }
}
