//! Navegación en rejilla, escala de la UI y geometría de miniaturas, compartidas por los
//! overlays (overview y actividades). Puro, con tests.

use crate::ipc::ClientInfo;

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

/// Escala de la UI de los overviews respecto a 720p, entre 1× y 1,6×.
pub fn ui_scale(screen_h: f32) -> f32 {
    (screen_h / 720.0).clamp(1.0, 1.6)
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn ui_scale_is_clamped() {
        assert_eq!(ui_scale(720.0), 1.0);
        assert_eq!(ui_scale(360.0), 1.0);
        assert!((ui_scale(1080.0) - 1.5).abs() < 1e-6);
        assert_eq!(ui_scale(4000.0), 1.6);
    }

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
}
