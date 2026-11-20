//! Modelo del overview: qué se muestra y cómo se navega, sin dibujo y con tests.
//!
//! El dibujo (`overview.rs`, `overview_collection.rs`) solo consume estas funciones.

use crate::{desktop::DesktopApp, ipc::ClientInfo};

/// Workspaces a mostrar: los ocupados más el actual, ordenados y sin repetir, hasta `max`.
pub fn workspace_ids(occupied: &[u32], current: u32, max: usize) -> Vec<u32> {
    let mut ids: Vec<u32> = occupied.to_vec();
    if !ids.contains(&current) {
        ids.push(current);
    }
    ids.sort_unstable();
    ids.dedup();
    ids.truncate(max);
    if ids.is_empty() {
        ids.push(1);
    }
    ids
}

/// Rellena `ids` con los workspaces libres de menor número (hasta `max_id`) para que haya al
/// menos `min_total`. Así las rejillas muestran también workspaces vacíos, como el mockup.
pub fn pad_workspace_ids(mut ids: Vec<u32>, min_total: usize, max_id: u32) -> Vec<u32> {
    let mut candidate = 1;
    while ids.len() < min_total && candidate <= max_id {
        if !ids.contains(&candidate) {
            ids.push(candidate);
        }
        candidate += 1;
    }
    ids.sort_unstable();
    ids
}

/// Posición de `current` en `ids` (0 si no está).
pub fn initial_selection(ids: &[u32], current: u32) -> usize {
    ids.iter().position(|id| *id == current).unwrap_or(0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// Mueve la selección en una rejilla de `cols` columnas con `count` elementos.
/// Izquierda y derecha recorren de forma lineal (saltan de fila); arriba y abajo no envuelven.
pub fn step(selected: usize, dir: Dir, cols: usize, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    let cols = cols.max(1);
    let last = count - 1;
    let selected = selected.min(last);
    match dir {
        Dir::Left => selected.saturating_sub(1),
        Dir::Right => (selected + 1).min(last),
        Dir::Up => {
            if selected >= cols {
                selected - cols
            } else {
                selected
            }
        }
        Dir::Down => {
            // Si la fila de abajo es más corta, cae en su último elemento.
            if selected / cols < last / cols {
                (selected + cols).min(last)
            } else {
                selected
            }
        }
    }
}

// ── Dimensiones: se derivan de la pantalla (el mockup es una guía, no una medida) ──

/// Escala de la UI de los overviews respecto a 720p, entre 1× y 1,6×.
pub fn ui_scale(screen_h: f32) -> f32 {
    (screen_h / 720.0).clamp(1.0, 1.6)
}

/// Alto de la franja horizontal: el configurado, o ~8,5 % del alto de pantalla (68–120 px).
pub fn strip_h_height(configured: u32, screen_h: f32) -> i32 {
    if configured > 0 {
        configured.clamp(40, 240) as i32
    } else {
        (screen_h * 0.085).round().clamp(68.0, 120.0) as i32
    }
}

/// Ancho de la franja vertical: el configurado, o ~5 % del ancho de pantalla (78–130 px).
pub fn strip_v_width(configured: u32, screen_w: f32) -> i32 {
    if configured > 0 {
        configured.clamp(48, 240) as i32
    } else {
        (screen_w * 0.05).round().clamp(78.0, 130.0) as i32
    }
}

/// Cuántos elementos de `item` px (con `gap` entre ellos) caben en `avail` px.
pub fn fit_count(avail: f32, item: f32, gap: f32, min: usize, max: usize) -> usize {
    (((avail + gap) / (item + gap)).floor().max(0.0) as usize).clamp(min, max)
}

/// Desplazamiento mínimo para que `selected` quede dentro de una ventana de `visible` elementos.
pub fn scroll_to(offset: usize, selected: usize, visible: usize) -> usize {
    let visible = visible.max(1);
    if selected < offset {
        selected
    } else if selected >= offset + visible {
        selected + 1 - visible
    } else {
        offset
    }
}

/// Rect de una ventana dentro de la miniatura `content` de un workspace.
pub fn window_rect(
    win: &ClientInfo,
    monitor: egui::Vec2,
    content: egui::Rect,
    min_size: f32,
) -> egui::Rect {
    let (mw, mh) = (monitor.x.max(1.0), monitor.y.max(1.0));
    let x = content.min.x + win.x as f32 / mw * content.width();
    let y = content.min.y + win.y as f32 / mh * content.height();
    let w = (win.w as f32 / mw * content.width()).max(min_size);
    let h = (win.h as f32 / mh * content.height()).max(min_size);
    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h)).intersect(content)
}

/// Orden de pintado: la ventana activa va al final (encima). El resto conserva su orden.
pub fn paint_order(windows: &[ClientInfo], active: &str) -> Vec<usize> {
    let (mut below, above): (Vec<usize>, Vec<usize>) =
        (0..windows.len()).partition(|i| windows[*i].address != active);
    below.extend(above);
    below
}

/// ¿La ventana coincide con la búsqueda? Cada palabra debe aparecer en la clase o el título.
pub fn window_matches(query: &str, class: &str, title: &str) -> bool {
    let haystack = format!("{} {}", class.to_lowercase(), title.to_lowercase());
    query
        .split_whitespace()
        .all(|word| haystack.contains(&word.to_lowercase()))
}

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
    fn workspace_ids_are_sorted_unique_and_include_current() {
        assert_eq!(workspace_ids(&[3, 1, 3], 2, 6), [1, 2, 3]);
        assert_eq!(workspace_ids(&[], 4, 6), [4]);
        assert_eq!(workspace_ids(&[1, 2, 3, 4, 5, 6, 7], 1, 3), [1, 2, 3]);
        assert_eq!(initial_selection(&[1, 3, 5], 3), 1);
        assert_eq!(initial_selection(&[1, 3, 5], 9), 0);
    }

    #[test]
    fn padding_fills_the_lowest_free_workspaces() {
        // Ocupados 1, 3 y 5: se completan 2, 4 y 6 hasta llegar a 6.
        assert_eq!(pad_workspace_ids(vec![1, 3, 5], 6, 10), [1, 2, 3, 4, 5, 6]);
        // Ya hay suficientes: no se toca.
        assert_eq!(pad_workspace_ids(vec![2, 4, 9], 3, 10), [2, 4, 9]);
        // Sin ids libres hasta `max_id`: se queda corto.
        assert_eq!(pad_workspace_ids(vec![1, 2], 6, 3), [1, 2, 3]);
        assert_eq!(pad_workspace_ids(vec![], 0, 10), Vec::<u32>::new());
    }

    #[test]
    fn step_moves_across_rows_and_clamps() {
        // 8 elementos en 6 columnas: filas [0..5] y [6,7].
        assert_eq!(step(0, Dir::Left, 6, 8), 0);
        assert_eq!(step(5, Dir::Right, 6, 8), 6); // salta de fila
        assert_eq!(step(7, Dir::Right, 6, 8), 7);
        assert_eq!(step(2, Dir::Down, 6, 8), 7); // fila corta: cae en el último
        assert_eq!(step(1, Dir::Down, 6, 8), 7);
        assert_eq!(step(6, Dir::Up, 6, 8), 0);
        assert_eq!(step(3, Dir::Up, 6, 8), 3); // primera fila
        assert_eq!(step(7, Dir::Down, 6, 8), 7); // última fila
        assert_eq!(step(5, Dir::Right, 6, 0), 0); // vacío
        assert_eq!(step(99, Dir::Left, 6, 3), 1); // selección fuera de rango
    }

    #[test]
    fn dimensions_follow_the_screen_and_respect_overrides() {
        // Automático: crece con la pantalla dentro de unos límites.
        assert_eq!(strip_h_height(0, 720.0), 68);
        assert_eq!(strip_h_height(0, 1080.0), 92);
        assert_eq!(strip_h_height(0, 2160.0), 120);
        assert_eq!(strip_v_width(0, 1280.0), 78);
        assert_eq!(strip_v_width(0, 1920.0), 96);
        assert_eq!(strip_v_width(0, 3840.0), 130);
        // Configurado: manda, con límites razonables.
        assert_eq!(strip_h_height(100, 720.0), 100);
        assert_eq!(strip_h_height(5, 720.0), 40);
        assert_eq!(strip_v_width(999, 1920.0), 240);
        assert_eq!(ui_scale(720.0), 1.0);
        assert_eq!(ui_scale(360.0), 1.0);
        assert!((ui_scale(1080.0) - 1.5).abs() < 1e-6);
        assert_eq!(ui_scale(4000.0), 1.6);
    }

    #[test]
    fn fit_count_is_clamped() {
        assert_eq!(fit_count(500.0, 80.0, 8.0, 3, 8), 5);
        assert_eq!(fit_count(10.0, 80.0, 8.0, 3, 8), 3); // mínimo
        assert_eq!(fit_count(5000.0, 80.0, 8.0, 3, 8), 8); // máximo
        assert_eq!(fit_count(-20.0, 80.0, 8.0, 1, 8), 1);
    }

    #[test]
    fn scroll_keeps_the_selection_visible() {
        assert_eq!(scroll_to(0, 2, 5), 0);
        assert_eq!(scroll_to(0, 7, 5), 3); // 3..=7
        assert_eq!(scroll_to(4, 1, 5), 1);
        assert_eq!(scroll_to(2, 6, 5), 2); // ya visible
        assert_eq!(scroll_to(0, 3, 0), 3); // visible=0 se trata como 1
    }

    #[test]
    fn window_rect_scales_and_stays_inside() {
        let content = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(160.0, 90.0));
        let mut w = client("a", "b", "0x1", 1);
        w.x = 960;
        w.y = 540;
        w.w = 960;
        w.h = 540;
        let r = window_rect(&w, egui::vec2(1920.0, 1080.0), content, 8.0);
        assert_eq!(r.min, egui::pos2(90.0, 65.0));
        assert_eq!(r.size(), egui::vec2(80.0, 45.0));
        // Ventana fuera de la pantalla: se recorta al contenido.
        w.x = 5000;
        let r = window_rect(&w, egui::vec2(1920.0, 1080.0), content, 8.0);
        assert!(r.width() <= 0.0 || content.contains_rect(r));
    }

    #[test]
    fn active_window_is_painted_last() {
        let ws = [
            client("a", "", "0x1", 1),
            client("b", "", "0x2", 1),
            client("c", "", "0x3", 1),
        ];
        assert_eq!(paint_order(&ws, "0x1"), [1, 2, 0]);
        assert_eq!(paint_order(&ws, "0x9"), [0, 1, 2]);
    }

    #[test]
    fn window_matching_needs_every_word() {
        assert!(window_matches("", "kitty", "x"));
        assert!(window_matches("kit", "kitty", "zsh"));
        assert!(window_matches("zsh kit", "kitty", "~/proj — zsh"));
        assert!(!window_matches("zsh vim", "kitty", "zsh"));
        assert!(window_matches("FIRE", "firefox", ""));
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
