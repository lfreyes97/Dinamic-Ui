//! Modelo del overview de workspaces (rejilla y franjas): qué se muestra y cómo se mueve la
//! selección, sin dibujo y con tests. El dibujo vive en `overview.rs` y `overview_strips.rs`.

use crate::ipc::ClientInfo;

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

// ── Dimensiones: se derivan de la pantalla (el mockup es una guía, no una medida) ──
// El grosor de las franjas (fracción o píxeles) lo resuelve `config::StripSize`.

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
}
