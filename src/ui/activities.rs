//! Actividades: lanzador a pantalla completa con las apps instaladas y las ventanas abiertas.
//!
//! No es un overview de workspaces: se parece más al modo «Apps» del command menu (lee los
//! `.desktop`, busca y lanza o enfoca). Por eso es su propia feature (`activities`) y no
//! depende del overview. La lógica pura está en `activities_model.rs`.

use smithay_client_toolkit::seat::keyboard::Keysym;

use super::activities_model::{self as model, film_layout, order_windows, Activation};
use super::nav::{step, ui_scale, window_rect, Dir};
use super::overlay::{hint_bar, mini_window, monitor_rect, pointer_moved, search_pill};
use super::*;

/// Estado de Actividades.
#[derive(Default)]
pub struct ActivitiesState {
    /// Objetivo: abierto o cerrado. La animación lo sigue con `progress`.
    pub open: bool,
    /// 0 (cerrado) a 1 (abierto); permite desvanecer también al cerrar.
    pub progress: f32,
    pub query: String,
    /// Tile seleccionada con el teclado o el cursor.
    pub selected: usize,
    /// Apps instaladas, leídas al abrir y soltadas al terminar de cerrar.
    pub apps: Vec<crate::desktop::DesktopApp>,
}

impl ActivitiesState {
    /// ¿Hay algo que dibujar? (también mientras se desvanece al cerrar).
    pub fn visible(&self) -> bool {
        self.open || self.progress > 0.005
    }
}

/// Columnas de la cuadrícula y máximo de apps mostradas.
pub(super) const COLS: usize = 6;
const CAP: usize = 24;

/// Apps según la búsqueda actual.
pub(super) fn tiles(state: &UiState) -> Vec<model::Tile> {
    model::build_tiles(
        &state.activities.apps,
        &state.shared.clients,
        &state.shared.config.dock.pinned,
        &state.activities.query,
        CAP,
    )
}

/// Abre Actividades: lee las apps instaladas y empieza con la búsqueda vacía.
pub(super) fn open(state: &mut UiState) {
    state.bar.show_panel = false;
    state.activities.open = true;
    state.activities.query.clear();
    state.activities.selected = 0;
    state.activities.apps = crate::desktop::scan();
}

/// Enfoca una ventana y cierra.
fn focus_window(state: &mut UiState, address: String) {
    state
        .shared
        .pending_commands
        .push(format!("focuswindow address:{address}"));
    state.activities.open = false;
}

/// Ejecuta una tile: enfoca (cicla entre sus ventanas) o lanza.
pub(super) fn run_tile(state: &mut UiState, tile: &model::Tile) {
    match model::activate(tile, &state.shared.snapshot.window_address) {
        Activation::Focus(address) => focus_window(state, address),
        Activation::Launch(argv) => {
            state.shared.pending_spawns.push(argv);
            state.activities.open = false;
        }
        Activation::Nothing => {}
    }
}

fn navigate(state: &mut UiState, dir: Dir) {
    let count = tiles(state).len();
    let cols = COLS.min(count.max(1));
    state.activities.selected = step(state.activities.selected, dir, cols, count);
}

fn activate_selected(state: &mut UiState) {
    if let Some(tile) = tiles(state).get(state.activities.selected).cloned() {
        run_tile(state, &tile);
    }
}

/// Teclado de Actividades abierta; `true` si consumió la tecla. `Esc` primero limpia la
/// búsqueda y después cierra; escribir filtra.
pub(super) fn handle_key(state: &mut UiState, key: Keysym) -> bool {
    if !state.activities.open {
        return false;
    }
    if key == Keysym::Escape {
        if state.activities.query.is_empty() {
            state.activities.open = false;
        } else {
            state.activities.query.clear();
            state.activities.selected = 0;
        }
        return true;
    }
    let dir = match key {
        Keysym::Left => Some(Dir::Left),
        Keysym::Right | Keysym::Tab => Some(Dir::Right),
        Keysym::Up => Some(Dir::Up),
        Keysym::Down => Some(Dir::Down),
        _ => None,
    };
    if let Some(dir) = dir {
        navigate(state, dir);
        return true;
    }
    if key == Keysym::Return || key == Keysym::KP_Enter {
        activate_selected(state);
        return true;
    }
    if key == Keysym::BackSpace {
        state.activities.query.pop();
        state.activities.selected = 0;
        return true;
    }
    let text = xkbcommon::xkb::keysym_to_utf8(key);
    let mut chars = text.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if !c.is_control() {
            state.activities.query.push(c);
            state.activities.selected = 0;
        }
    }
    // Tiene el teclado en exclusiva: no dejamos pasar teclas sueltas.
    true
}

/// Overview de actividades: buscador, apps instaladas y tira de ventanas abiertas.
pub(super) fn draw(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let screen = ui.max_rect();
    ui.painter()
        .rect_filled(screen, CornerRadius::same(0), Palette::scrim(176));
    let backdrop = ui.interact(
        screen,
        ui.scope_id().with("ov-act-backdrop"),
        egui::Sense::click(),
    );
    if backdrop.clicked() {
        state.activities.open = false;
    }

    let query = state.activities.query.clone();
    search_pill(ui, screen, &query, "Busca aplicaciones…");

    let tiles = tiles(state);
    state.activities.selected = state.activities.selected.min(tiles.len().saturating_sub(1));
    let moved = pointer_moved(ui);

    // Zona de la cuadrícula: entre el buscador y la tira de ventanas.
    // Las medidas del mockup son para 720p: se escalan con la pantalla.
    let k = ui_scale(screen.height());
    // La tira de ventanas es una fracción de la pantalla (1/5 por defecto), como las franjas del
    // overview: con medidas fijas se quedaba en una barra de ~70 px.
    let film_h = state
        .shared
        .config
        .activities
        .strip_height
        .resolve(screen.height()) as f32;
    let film_y = screen.max.y - 40.0 - film_h;
    let top = screen.min.y + 92.0 * k.sqrt();
    let cols = COLS.min(tiles.len().max(1));
    let rows = tiles.len().div_ceil(cols.max(1)).max(1);
    let (pitch_x, pitch_y, tile) = (104.0 * k, 96.0 * k, 56.0 * k);
    let block_h = rows as f32 * pitch_y;
    let free = (film_y - 24.0 - top - block_h).max(0.0);
    let origin = egui::pos2(
        screen.center().x - cols as f32 * pitch_x / 2.0,
        top + free * 0.4,
    );

    if tiles.is_empty() {
        let message = if query.is_empty() {
            "No hay aplicaciones".to_string()
        } else {
            format!("Sin resultados para «{query}»")
        };
        text_at(
            ui,
            egui::pos2(screen.center().x, top + 60.0),
            egui::Align2::CENTER_CENTER,
            message,
            TEXT_BODY,
            palette.muted,
        );
    }

    for (i, t) in tiles.iter().enumerate() {
        let c = egui::pos2(
            origin.x + (i % cols) as f32 * pitch_x + pitch_x / 2.0,
            origin.y + (i / cols) as f32 * pitch_y + tile / 2.0,
        );
        let cell = egui::Rect::from_center_size(
            egui::pos2(c.x, c.y + 10.0),
            egui::vec2(pitch_x - 8.0, pitch_y - 6.0),
        );
        let resp = ui.interact(
            cell,
            ui.scope_id().with(("ov-app", &t.id)),
            egui::Sense::click(),
        );
        if resp.hovered() && moved {
            state.activities.selected = i;
        }
        let selected = state.activities.selected == i;
        let box_rect = egui::Rect::from_center_size(c, egui::vec2(tile, tile));
        let lift = ui.ctx().animate_bool_with_time(
            ui.scope_id().with(("ov-app-lift", &t.id)),
            resp.hovered() || selected,
            0.12,
        );
        let box_rect = box_rect.translate(egui::vec2(0.0, -2.0 * lift));
        ui.painter().rect(
            box_rect,
            CornerRadius::same(14),
            if selected {
                palette.accent_soft
            } else {
                Palette::with_alpha(palette.raised, 235)
            },
            Stroke::new(
                if selected { 1.5 } else { 1.0 },
                if selected {
                    palette.accent
                } else {
                    palette.border
                },
            ),
            StrokeKind::Middle,
        );
        if let Some(texture) = state.shared.icon(ui.ctx(), &t.id) {
            egui::Image::new(egui::load::SizedTexture::new(
                texture.id(),
                egui::vec2(30.0 * k, 30.0 * k),
            ))
            .paint_at(
                ui,
                egui::Rect::from_center_size(box_rect.center(), egui::vec2(30.0 * k, 30.0 * k)),
            );
        } else {
            icon_at(
                ui,
                box_rect.center(),
                dock_icon(&t.name),
                ICON_LG,
                palette.text,
            );
        }
        // Un punto por ventana abierta (hasta tres).
        let dots = t.windows.len().min(3);
        for k in 0..dots {
            let dx = (k as f32 - (dots as f32 - 1.0) / 2.0) * 6.0;
            ui.painter().circle_filled(
                egui::pos2(box_rect.center().x + dx, box_rect.max.y - 6.0),
                1.9,
                palette.accent,
            );
        }
        text_at(
            ui,
            egui::pos2(c.x, c.y + tile / 2.0 + 14.0),
            egui::Align2::CENTER_CENTER,
            truncate(&t.name, 13),
            9.5 * k.sqrt(),
            if selected {
                palette.text
            } else {
                palette.muted
            },
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() {
            run_tile(state, t);
        }
    }

    // Tira de ventanas abiertas: la activa primero y tantas como caben, con una vista esquemática
    // grande de cada una (tiene la proporción del monitor).
    let active = state.shared.snapshot.window_address.clone();
    let monitor = screen.size();
    let aspect = monitor.x.max(1.0) / monitor.y.max(1.0);
    let film = film_layout(screen.width(), film_h, aspect);
    let all = order_windows(&state.shared.clients, &active);
    let shown = all.len().min(film.capacity);
    let (card_w, gap, pad) = (film.card.0, film.gap, film.pad);
    let film_w = shown as f32 * (card_w + gap) - gap;
    let caption_size = 9.0 * film.scale;
    for (i, win) in all.iter().take(shown).enumerate() {
        let r = egui::Rect::from_min_size(
            egui::pos2(
                screen.center().x - film_w / 2.0 + i as f32 * (card_w + gap),
                film_y,
            ),
            egui::vec2(card_w, film_h),
        );
        let resp = ui.interact(
            r,
            ui.scope_id().with(("ov-film", &win.address)),
            egui::Sense::click(),
        );
        let is_active = win.address == active;
        drop_shadow(ui, r, 8, 6, 16, 80);
        ui.painter().rect(
            r,
            CornerRadius::same(8),
            Palette::with_alpha(palette.raised, 240),
            Stroke::new(
                if is_active { 1.5 } else { 1.0 },
                if is_active {
                    palette.accent
                } else if resp.hovered() {
                    palette.muted
                } else {
                    palette.border
                },
            ),
            StrokeKind::Middle,
        );
        // Vista esquemática de la ventana dentro del monitor.
        let pic = egui::Rect::from_min_size(
            r.min + egui::vec2(pad, pad),
            egui::vec2(film.pic.0, film.pic.1),
        );
        let mon = monitor_rect(pic, aspect);
        ui.painter()
            .rect_filled(mon, CornerRadius::same(3), Palette::sheen(8));
        let wr = window_rect(win, monitor, mon, 6.0);
        if wr.width() > 0.0 && wr.height() > 0.0 {
            mini_window(ui, wr, is_active, resp.hovered(), 2);
        }
        // Pie: icono y título (recortado a lo que cabe en la tarjeta).
        let icon_px = 13.0 * film.scale;
        let cy = r.max.y - film.caption / 2.0 - pad * 0.25;
        let icon_c = egui::pos2(r.min.x + pad + icon_px / 2.0, cy);
        if let Some(texture) = state.shared.icon(ui.ctx(), &win.class) {
            egui::Image::new(egui::load::SizedTexture::new(
                texture.id(),
                egui::vec2(icon_px, icon_px),
            ))
            .paint_at(
                ui,
                egui::Rect::from_center_size(icon_c, egui::vec2(icon_px, icon_px)),
            );
        } else {
            icon_at(
                ui,
                icon_c,
                dock_icon(&win.class),
                icon_px * 0.85,
                palette.text,
            );
        }
        let text_x = r.min.x + pad + icon_px + 6.0 * film.scale;
        let max_chars = (((r.max.x - pad - text_x) / (caption_size * 0.55)) as usize).clamp(6, 48);
        text_at(
            ui,
            egui::pos2(text_x, cy),
            egui::Align2::LEFT_CENTER,
            truncate(&win.title, max_chars),
            caption_size,
            palette.muted,
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() {
            focus_window(state, win.address.clone());
        }
    }
    if all.len() > shown {
        text_at(
            ui,
            egui::pos2(screen.max.x - 60.0, film_y - 10.0),
            egui::Align2::RIGHT_CENTER,
            match all.len() - shown {
                1 => "+1 ventana más".to_string(),
                n => format!("+{n} ventanas más"),
            },
            caption_size,
            palette.dim,
        );
    }

    hint_bar(
        ui,
        screen,
        &[
            ("← → ↑ ↓", "navega"),
            ("↵", "abre o enfoca"),
            ("Esc", if query.is_empty() { "cierra" } else { "limpia" }),
        ],
    );
}
