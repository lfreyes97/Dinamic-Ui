//! Modelo de Actividades (lanzador de apps y ventanas): tiles, búsqueda y activación, sin
//! dibujo y con tests. El dibujo vive en `activities.rs`.

use crate::{desktop::DesktopApp, ipc::ClientInfo, search};

/// Medidas de la tira de ventanas abiertas, derivadas de su alto y de la proporción del monitor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Film {
    /// Tarjeta completa `(ancho, alto)`.
    pub card: (f32, f32),
    /// Vista esquemática `(ancho, alto)`: tiene la proporción del monitor.
    pub pic: (f32, f32),
    pub pad: f32,
    /// Alto reservado al pie (icono y título).
    pub caption: f32,
    pub gap: f32,
    /// Escala de textos y relleno (1× a ~70 px de alto de tira).
    pub scale: f32,
    /// Tarjetas que caben en el ancho de la pantalla.
    pub capacity: usize,
}

/// Calcula la tira para una pantalla de `screen_w` px, un alto de tira `film_h` y la proporción
/// `aspect` (ancho/alto) del monitor. La vista esquemática ocupa todo lo que deja el pie.
pub fn film_layout(screen_w: f32, film_h: f32, aspect: f32) -> Film {
    let scale = (film_h / 70.0).sqrt().clamp(1.0, 1.8);
    let (pad, caption, gap) = (8.0 * scale, 24.0 * scale, 10.0 * scale);
    let pic_h = (film_h - caption - 2.0 * pad).max(16.0);
    let pic_w = pic_h * aspect.max(0.5);
    let card_w = pic_w + 2.0 * pad;
    // Margen de 60 px a cada lado para que la tira no toque los bordes.
    let avail = (screen_w - 120.0).max(card_w);
    let capacity = (((avail + gap) / (card_w + gap)).floor() as usize).clamp(1, 12);
    Film {
        card: (card_w, film_h),
        pic: (pic_w, pic_h),
        pad,
        caption,
        gap,
        scale,
        capacity,
    }
}

/// Ventanas de la tira: solo las mapeadas, con la activa primero y el resto en su orden.
pub fn order_windows(clients: &[ClientInfo], active: &str) -> Vec<ClientInfo> {
    let (mut first, rest): (Vec<_>, Vec<_>) = clients
        .iter()
        .filter(|c| c.mapped)
        .cloned()
        .partition(|c| c.address == active);
    first.extend(rest);
    first
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
            .filter_map(|c| {
                search::score(query, &c.tile.name, &c.tile.id, &c.comment).map(|s| (s, c))
            })
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
    fn film_cards_follow_the_strip_height_and_the_monitor_aspect() {
        // 1366×768 con 1/5 de alto (154 px): una tira de verdad, no de ~70 px.
        let f = film_layout(1366.0, 154.0, 1366.0 / 768.0);
        assert!(f.card.1 > 140.0 && f.pic.1 > 80.0, "{f:?}");
        // La vista esquemática conserva la proporción del monitor.
        assert!((f.pic.0 / f.pic.1 - 1366.0 / 768.0).abs() < 1e-3);
        // Todo cabe dentro de la tarjeta: vista + pie + relleno.
        assert!(f.pic.1 + f.caption + 2.0 * f.pad <= f.card.1 + 1e-3);
        assert!(f.pic.0 + 2.0 * f.pad <= f.card.0 + 1e-3);
        // Caben varias tarjetas, y nunca desbordan el ancho.
        assert!(f.capacity >= 4 && f.capacity <= 12, "{f:?}");
        let used = f.capacity as f32 * (f.card.0 + f.gap) - f.gap;
        assert!(used <= 1366.0 - 120.0 + 1e-3);
    }

    #[test]
    fn film_grows_with_the_strip_and_never_degenerates() {
        let small = film_layout(1366.0, 70.0, 16.0 / 9.0);
        let big = film_layout(1366.0, 216.0, 16.0 / 9.0);
        assert!(big.card.0 > small.card.0 && big.pic.1 > small.pic.1);
        assert!(
            big.capacity <= small.capacity,
            "tarjetas mayores, menos caben"
        );
        // Pantallas o tiras diminutas: siempre al menos una tarjeta con vista legible.
        let tiny = film_layout(200.0, 40.0, 16.0 / 9.0);
        assert!(tiny.capacity >= 1 && tiny.pic.1 >= 16.0);
        // Una pantalla enorme no pasa de 12 tarjetas.
        assert_eq!(film_layout(10_000.0, 100.0, 16.0 / 9.0).capacity, 12);
    }

    #[test]
    fn open_windows_are_ordered_active_first_and_unmapped_are_dropped() {
        let (_, mut clients) = sample();
        clients[3].mapped = false;
        let ordered = order_windows(&clients, "0xb"); // firefox
        let addresses: Vec<&str> = ordered.iter().map(|c| c.address.as_str()).collect();
        assert_eq!(addresses, ["0xb", "0xa", "0xc"]);
        // Sin ventana activa conocida: orden original.
        let none = order_windows(&clients, "0xzzz");
        assert_eq!(none.len(), 3);
        assert_eq!(none[0].address, "0xa");
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
