//! Modelo de Actividades (lanzador de apps y ventanas): tiles, búsqueda y activación, sin
//! dibujo y con tests. El dibujo vive en `activities.rs`.

use crate::{desktop::DesktopApp, ipc::ClientInfo};

/// Puntuación de una app para la búsqueda; `None` si alguna palabra no coincide.
/// Prefijo del nombre > inicio de palabra > contiene > `id` > comentario.
pub fn app_score(query: &str, name: &str, id: &str, comment: &str) -> Option<i32> {
    let (name, id, comment) = (
        name.to_lowercase(),
        id.to_lowercase(),
        comment.to_lowercase(),
    );
    let mut total = 0;
    let mut any = false;
    for word in query.split_whitespace() {
        any = true;
        let word = word.to_lowercase();
        let score = if name.starts_with(&word) {
            100
        } else if name
            .split([' ', '-', '_', '.'])
            .any(|w| w.starts_with(&word))
        {
            80
        } else if name.contains(&word) {
            60
        } else if id.contains(&word) {
            40
        } else if comment.contains(&word) {
            20
        } else {
            return None;
        };
        total += score;
    }
    any.then_some(total)
}

/// Una app de la cuadrícula de Actividades.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    /// Clave para resolver el icono (`id` del `.desktop`, o la clase si no hay entrada).
    pub id: String,
    pub name: String,
    /// Comando de lanzamiento; `None` si solo conocemos la ventana (sin `.desktop`).
    pub exec: Option<String>,
    /// Direcciones de las ventanas abiertas de esta app.
    pub windows: Vec<String>,
}

struct Candidate {
    tile: Tile,
    comment: String,
    /// Orden de la primera ventana en la lista de clientes (para ordenar las abiertas).
    first_window: Option<usize>,
    pinned: Option<usize>,
}

/// Tiles de Actividades.
///
/// Sin búsqueda: primero las abiertas, luego las ancladas y después el resto por nombre.
/// Con búsqueda: por puntuación. Las ventanas sin `.desktop` aparecen igualmente (se pueden
/// enfocar, no lanzar).
pub fn build_tiles(
    apps: &[DesktopApp],
    clients: &[ClientInfo],
    pinned: &[String],
    query: &str,
    cap: usize,
) -> Vec<Tile> {
    let clients: Vec<(usize, &ClientInfo)> = clients
        .iter()
        .enumerate()
        .filter(|(_, c)| c.mapped)
        .collect();
    let mut claimed = vec![false; clients.len()];
    let mut candidates: Vec<Candidate> = Vec::new();

    for app in apps.iter().filter(|a| a.launchable()) {
        let mut windows = Vec::new();
        let mut first = None;
        for (slot, (index, client)) in clients.iter().enumerate() {
            if app.matches_class(&client.class) {
                claimed[slot] = true;
                first.get_or_insert(*index);
                windows.push(client.address.clone());
            }
        }
        candidates.push(Candidate {
            tile: Tile {
                id: app.id.clone(),
                name: app.name.clone(),
                exec: Some(app.exec.clone()),
                windows,
            },
            comment: app.comment.clone(),
            first_window: first,
            pinned: pinned.iter().position(|p| app.matches_name(p)),
        });
    }

    // Ventanas de apps que no tienen `.desktop`: una tile por clase.
    for (slot, (index, client)) in clients.iter().enumerate() {
        if claimed[slot] || client.class.is_empty() {
            continue;
        }
        let key = client.class.to_lowercase();
        if let Some(existing) = candidates
            .iter_mut()
            .find(|c| c.tile.exec.is_none() && c.tile.id == key)
        {
            existing.tile.windows.push(client.address.clone());
            continue;
        }
        candidates.push(Candidate {
            tile: Tile {
                id: key.clone(),
                name: client.class.clone(),
                exec: None,
                windows: vec![client.address.clone()],
            },
            comment: String::new(),
            first_window: Some(*index),
            pinned: pinned.iter().position(|p| p.to_lowercase() == key),
        });
    }

    let query = query.trim();
    let mut ranked: Vec<(i32, Candidate)> = if query.is_empty() {
        candidates.into_iter().map(|c| (0, c)).collect()
    } else {
        candidates
            .into_iter()
            .filter_map(|c| app_score(query, &c.tile.name, &c.tile.id, &c.comment).map(|s| (s, c)))
            .collect()
    };

    ranked.sort_by(|(sa, a), (sb, b)| {
        let by_name = || a.tile.name.to_lowercase().cmp(&b.tile.name.to_lowercase());
        if !query.is_empty() {
            // Mejor puntuación; a igualdad, las abiertas y luego por nombre.
            return sb
                .cmp(sa)
                .then(b.first_window.is_some().cmp(&a.first_window.is_some()))
                .then_with(by_name);
        }
        match (a.first_window, b.first_window) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => match (a.pinned, b.pinned) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => by_name(),
            },
        }
    });
    ranked.into_iter().take(cap).map(|(_, c)| c.tile).collect()
}

/// Qué hacer al activar una tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Activation {
    Focus(String),
    Launch(Vec<String>),
    Nothing,
}

/// Con ventanas abiertas enfoca la siguiente a la activa (cicla entre varias); si no, lanza.
pub fn activate(tile: &Tile, active_address: &str) -> Activation {
    if !tile.windows.is_empty() {
        let next = tile
            .windows
            .iter()
            .position(|a| a == active_address)
            .map(|i| (i + 1) % tile.windows.len())
            .unwrap_or(0);
        return Activation::Focus(tile.windows[next].clone());
    }
    match &tile.exec {
        Some(exec) => {
            let argv = crate::desktop::split_exec(exec);
            if argv.is_empty() {
                Activation::Nothing
            } else {
                Activation::Launch(argv)
            }
        }
        None => Activation::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(class: &str, title: &str, address: &str, ws: i32) -> ClientInfo {
        ClientInfo {
            class: class.into(),
            title: title.into(),
            address: address.into(),
            mapped: true,
            workspace: ws,
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        }
    }

    fn app(id: &str, name: &str, exec: &str, comment: &str) -> DesktopApp {
        DesktopApp {
            id: id.into(),
            name: name.into(),
            comment: comment.into(),
            exec: exec.into(),
            ..Default::default()
        }
    }

    #[test]
    fn app_score_prefers_prefix_then_word_start_then_substring() {
        let prefix = app_score("fire", "Firefox", "org.mozilla.firefox", "").unwrap();
        let word = app_score("web", "Epiphany Web", "epiphany", "").unwrap();
        let contains = app_score("fox", "Firefox", "firefox", "").unwrap();
        let comment = app_score("navega", "Otra", "otra", "Navega por la web").unwrap();
        assert!(prefix > word && word > contains && contains > comment);
        assert_eq!(app_score("zzz", "Firefox", "firefox", "web"), None);
        assert_eq!(app_score("   ", "Firefox", "firefox", ""), None);
        // Con varias palabras deben coincidir todas.
        assert!(app_score("fire fox", "Firefox", "firefox", "").is_some());
        assert_eq!(app_score("fire zzz", "Firefox", "firefox", ""), None);
    }

    fn sample() -> (Vec<DesktopApp>, Vec<ClientInfo>) {
        let apps = vec![
            app("firefox", "Firefox", "firefox %u", "Navegador"),
            app("kitty", "Kitty", "kitty", "Terminal"),
            app("code", "Code", "code", "Editor"),
            app("alacritty", "Alacritty", "alacritty", "Terminal"),
        ];
        let clients = vec![
            client("kitty", "zsh", "0xa", 1),
            client("firefox", "Rust", "0xb", 2),
            client("kitty", "htop", "0xc", 1),
            client("OBS", "obs", "0xd", 3), // sin .desktop
        ];
        (apps, clients)
    }

    #[test]
    fn tiles_without_query_put_running_first_then_pinned_then_alphabetical() {
        let (apps, clients) = sample();
        let pinned = vec!["code".to_string()];
        let tiles = build_tiles(&apps, &clients, &pinned, "", 24);
        let names: Vec<&str> = tiles.iter().map(|t| t.name.as_str()).collect();
        // Abiertas por orden de aparición (Kitty, Firefox, OBS), anclada (Code), resto (Alacritty).
        assert_eq!(names, ["Kitty", "Firefox", "OBS", "Code", "Alacritty"]);
        assert_eq!(tiles[0].windows, ["0xa", "0xc"]);
        assert_eq!(tiles[2].exec, None); // OBS: se puede enfocar, no lanzar
    }

    #[test]
    fn tiles_with_query_are_ranked_and_filtered() {
        let (apps, clients) = sample();
        let tiles = build_tiles(&apps, &clients, &[], "term", 24);
        // Coinciden por comentario: Kitty (abierta) antes que Alacritty.
        let names: Vec<&str> = tiles.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Kitty", "Alacritty"]);
        assert!(build_tiles(&apps, &clients, &[], "zzz", 24).is_empty());
        // El tope se respeta.
        assert_eq!(build_tiles(&apps, &clients, &[], "", 2).len(), 2);
    }

    #[test]
    fn unmapped_windows_and_hidden_apps_are_ignored() {
        let (mut apps, mut clients) = sample();
        clients[3].mapped = false;
        apps[3].hidden = true;
        let tiles = build_tiles(&apps, &clients, &[], "", 24);
        assert!(tiles
            .iter()
            .all(|t| t.name != "OBS" && t.name != "Alacritty"));
    }

    #[test]
    fn activation_focuses_cycles_or_launches() {
        let (apps, clients) = sample();
        let tiles = build_tiles(&apps, &clients, &[], "", 24);
        let kitty = &tiles[0];
        // Sin ventana activa de la app: la primera. Con una activa: la siguiente, y cicla.
        assert_eq!(activate(kitty, "0xzzz"), Activation::Focus("0xa".into()));
        assert_eq!(activate(kitty, "0xa"), Activation::Focus("0xc".into()));
        assert_eq!(activate(kitty, "0xc"), Activation::Focus("0xa".into()));
        let code = build_tiles(&apps, &clients, &[], "code", 24).remove(0);
        assert_eq!(activate(&code, ""), Activation::Launch(vec!["code".into()]));
        let obs = &tiles[2];
        assert_eq!(activate(obs, ""), Activation::Focus("0xd".into()));
        let no_exec = Tile {
            id: "x".into(),
            name: "x".into(),
            exec: None,
            windows: vec![],
        };
        assert_eq!(activate(&no_exec, ""), Activation::Nothing);
    }
}
