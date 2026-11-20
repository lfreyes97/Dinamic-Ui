//! Overview: cuatro vistas (rejilla, actividades, franja horizontal y vertical) sobre un
//! mismo estado. La lógica (qué se muestra, navegación, búsqueda) vive en `overview_model.rs`.

use smithay_client_toolkit::seat::keyboard::Keysym;

use super::overview_model::{self as model, Activation, Dir};
use super::*;

/// Las cuatro vistas de overview.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OverviewKind {
    #[default]
    Workspaces,
    Activities,
    StripH,
    StripV,
}

impl OverviewKind {
    /// Vistas modales: cubren la pantalla, toman el teclado y admiten búsqueda.
    pub fn is_modal(self) -> bool {
        matches!(self, Self::Workspaces | Self::Activities)
    }
}

/// Estado del overview.
#[derive(Default)]
pub struct OverviewState {
    /// Objetivo: abierto o cerrado. La animación lo sigue con `progress`.
    pub overview: bool,
    /// 0 (cerrado) a 1 (abierto); permite desvanecer también al cerrar.
    pub progress: f32,
    pub kind: OverviewKind,
    pub strip_offset: usize,
    /// Búsqueda (vistas modales).
    pub query: String,
    /// Elemento seleccionado con el teclado o el cursor.
    pub selected: usize,
    /// Apps instaladas, leídas al abrir Actividades y soltadas al terminar de cerrar.
    pub apps: Vec<crate::desktop::DesktopApp>,
}

impl OverviewState {
    /// ¿Hay algo que dibujar? (también mientras se desvanece al cerrar).
    pub fn visible(&self) -> bool {
        self.overview || self.progress > 0.005
    }
}

/// Columnas de la cuadrícula de Actividades y máximo de apps mostradas.
pub(super) const ACTIVITY_COLS: usize = 6;
const ACTIVITY_CAP: usize = 24;

pub(super) fn current_ws(state: &UiState) -> u32 {
    state.shared.snapshot.workspace.parse::<u32>().unwrap_or(1)
}

/// Workspaces de la rejilla (hasta 6).
pub(super) fn grid_ids(state: &UiState) -> Vec<u32> {
    model::workspace_ids(&state.shared.snapshot.occupied, current_ws(state), 6)
}

/// Workspaces de las franjas (hasta 20, con desplazamiento).
pub(super) fn strip_ids(state: &UiState) -> Vec<u32> {
    model::workspace_ids(&state.shared.snapshot.occupied, current_ws(state), 20)
}

/// Apps de Actividades según la búsqueda actual.
pub(super) fn activity_tiles(state: &UiState) -> Vec<model::Tile> {
    model::build_tiles(
        &state.overview.apps,
        &state.shared.clients,
        &state.shared.config.dock.pinned,
        &state.overview.query,
        ACTIVITY_CAP,
    )
}

/// Abre una vista (o cambia de vista si ya estaba abierta) con la selección inicial.
pub(super) fn open(state: &mut UiState, kind: OverviewKind) {
    state.bar.show_panel = false;
    state.overview.kind = kind;
    state.overview.overview = true;
    state.overview.query.clear();
    state.overview.strip_offset = 0;
    state.overview.selected = match kind {
        OverviewKind::Workspaces => model::initial_selection(&grid_ids(state), current_ws(state)),
        OverviewKind::StripH | OverviewKind::StripV => {
            let ids = strip_ids(state);
            let selected = model::initial_selection(&ids, current_ws(state));
            state.overview.strip_offset = model::scroll_to(0, selected, 5);
            selected
        }
        OverviewKind::Activities => 0,
    };
    // Las apps instaladas solo se leen mientras Actividades está abierta.
    state.overview.apps = if kind == OverviewKind::Activities {
        crate::desktop::scan()
    } else {
        Vec::new()
    };
}

/// Ejecuta una tile: enfoca (cicla entre sus ventanas) o lanza.
pub(super) fn run_tile(state: &mut UiState, tile: &model::Tile) {
    match model::activate(tile, &state.shared.snapshot.window_address) {
        Activation::Focus(address) => state.shared.overview_focus = Some(address),
        Activation::Launch(argv) => {
            state.shared.pending_spawns.push(argv);
            state.overview.overview = false;
        }
        Activation::Nothing => {}
    }
}

/// Primera ventana que coincide con la búsqueda, recorriendo los workspaces en orden.
fn first_match(state: &UiState) -> Option<String> {
    let query = &state.overview.query;
    grid_ids(state).into_iter().find_map(|id| {
        state
            .shared
            .clients
            .iter()
            .find(|c| {
                c.mapped
                    && c.workspace == id as i32
                    && model::window_matches(query, &c.class, &c.title)
            })
            .map(|c| c.address.clone())
    })
}

/// Cuántas ventanas coinciden con la búsqueda.
fn match_count(state: &UiState) -> usize {
    let query = &state.overview.query;
    state
        .shared
        .clients
        .iter()
        .filter(|c| c.mapped && model::window_matches(query, &c.class, &c.title))
        .count()
}

fn reset_selection(state: &mut UiState) {
    state.overview.selected = match state.overview.kind {
        OverviewKind::Workspaces => model::initial_selection(&grid_ids(state), current_ws(state)),
        _ => 0,
    };
}

fn navigate(state: &mut UiState, dir: Dir) {
    let selected = state.overview.selected;
    match state.overview.kind {
        OverviewKind::Workspaces => {
            let count = grid_ids(state).len();
            state.overview.selected = model::step(selected, dir, count.clamp(1, 3), count);
        }
        OverviewKind::Activities => {
            let count = activity_tiles(state).len();
            let cols = ACTIVITY_COLS.min(count.max(1));
            state.overview.selected = model::step(selected, dir, cols, count);
        }
        kind @ (OverviewKind::StripH | OverviewKind::StripV) => {
            // En una franja solo hay un eje: arriba/izquierda retrocede, abajo/derecha avanza.
            let dir = match dir {
                Dir::Left | Dir::Up => Dir::Left,
                Dir::Right | Dir::Down => Dir::Right,
            };
            let count = strip_ids(state).len();
            let next = model::step(selected, dir, count, count);
            let visible = if kind == OverviewKind::StripH { 5 } else { 6 };
            state.overview.selected = next;
            state.overview.strip_offset =
                model::scroll_to(state.overview.strip_offset, next, visible);
        }
    }
}

fn activate_selected(state: &mut UiState) {
    let selected = state.overview.selected;
    match state.overview.kind {
        OverviewKind::Workspaces => {
            if !state.overview.query.is_empty() {
                if let Some(address) = first_match(state) {
                    state.shared.overview_focus = Some(address);
                }
            } else if let Some(id) = grid_ids(state).get(selected) {
                state.shared.overview_workspace = Some(*id);
            }
        }
        OverviewKind::Activities => {
            if let Some(tile) = activity_tiles(state).get(selected).cloned() {
                run_tile(state, &tile);
            }
        }
        OverviewKind::StripH | OverviewKind::StripV => {
            if let Some(id) = strip_ids(state).get(selected) {
                state.shared.overview_workspace = Some(*id);
            }
        }
    }
}

fn digit(key: Keysym) -> Option<u32> {
    match key {
        Keysym::_1 => Some(1),
        Keysym::_2 => Some(2),
        Keysym::_3 => Some(3),
        Keysym::_4 => Some(4),
        Keysym::_5 => Some(5),
        Keysym::_6 => Some(6),
        Keysym::_7 => Some(7),
        Keysym::_8 => Some(8),
        Keysym::_9 => Some(9),
        _ => None,
    }
}

/// Teclado del overview abierto; `true` si consumió la tecla.
///
/// `Esc` primero limpia la búsqueda y después cierra. En las vistas modales escribir filtra;
/// en la rejilla, un número salta a ese workspace mientras no haya búsqueda.
pub(super) fn handle_key(state: &mut UiState, key: Keysym) -> bool {
    if !state.overview.overview {
        return false;
    }
    if key == Keysym::Escape {
        if state.overview.query.is_empty() {
            state.overview.overview = false;
        } else {
            state.overview.query.clear();
            reset_selection(state);
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

    let kind = state.overview.kind;
    let jump_allowed = match kind {
        OverviewKind::Workspaces => state.overview.query.is_empty(),
        OverviewKind::StripH | OverviewKind::StripV => true,
        OverviewKind::Activities => false,
    };
    if jump_allowed {
        if let Some(n) = digit(key) {
            state.shared.overview_workspace = Some(n);
            return true;
        }
    }
    if !kind.is_modal() {
        return false;
    }
    if key == Keysym::BackSpace {
        state.overview.query.pop();
        reset_selection(state);
        return true;
    }
    let text = xkbcommon::xkb::keysym_to_utf8(key);
    let mut chars = text.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if !c.is_control() {
            state.overview.query.push(c);
            reset_selection(state);
        }
    }
    // Una vista modal tiene el teclado en exclusiva: no dejamos pasar teclas sueltas.
    true
}

/// ¿El cursor se movió en este frame? Evita que una tarjeta quieta bajo el cursor robe la
/// selección que se acaba de mover con el teclado.
pub(super) fn pointer_moved(ui: &egui::Ui) -> bool {
    ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO)
}

/// Overview: rejilla de workspaces con las ventanas en su posición relativa.
fn draw_workspaces(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let screen = ui.max_rect();
    ui.painter()
        .rect_filled(screen, CornerRadius::same(0), Palette::scrim(200));
    // Registrado antes que las tarjetas: un clic en ventana gana sobre el fondo.
    let backdrop = ui.interact(
        screen,
        ui.scope_id().with(egui::IdSalt::new("overview-backdrop")),
        egui::Sense::click(),
    );
    if backdrop.clicked() {
        state.overview.overview = false;
    }

    let ids = grid_ids(state);
    let current = current_ws(state);
    state.overview.selected = state.overview.selected.min(ids.len() - 1);

    let cols = ids.len().clamp(1, 3);
    let rows = ids.len().div_ceil(cols);
    let gap = 14.0;
    let area = screen.shrink2(egui::vec2(40.0, 56.0));
    let card_h = ((area.height() - gap * (rows as f32 - 1.0)) / rows as f32).min(240.0);
    let card_w =
        (card_h * 16.0 / 9.0).min((area.width() - gap * (cols as f32 - 1.0)) / cols as f32);
    let card_h = card_w * 9.0 / 16.0;
    let total_w = cols as f32 * card_w + gap * (cols as f32 - 1.0);
    let total_h = rows as f32 * card_h + gap * (rows as f32 - 1.0);
    let origin = egui::pos2(
        screen.center().x - total_w / 2.0,
        screen.center().y - total_h / 2.0 + 10.0,
    );

    let query = state.overview.query.clone();
    let header = if query.is_empty() {
        "Escribe para buscar ventanas  ·  ←→↑↓ navega  ·  ↵ cambia  ·  1–9 salta  ·  Esc cierra"
            .to_string()
    } else {
        format!(
            "Buscar: {query}|   ·   {} coincidencias   ·   ↵ enfoca la primera   ·   Esc limpia",
            match_count(state)
        )
    };
    text_at(
        ui,
        egui::pos2(screen.center().x, origin.y - 22.0),
        egui::Align2::CENTER_CENTER,
        header,
        TEXT_BODY,
        if query.is_empty() {
            palette.muted
        } else {
            palette.text
        },
    );

    let monitor = screen.size();
    let moved = pointer_moved(ui);
    let active_address = state.shared.snapshot.window_address.clone();

    for (i, id) in ids.iter().enumerate() {
        let x = origin.x + (i % cols) as f32 * (card_w + gap);
        let y = origin.y + (i / cols) as f32 * (card_h + gap);
        let card = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(card_w, card_h));
        let card_response = ui.interact(
            card,
            ui.scope_id().with(("overview-card", *id)),
            egui::Sense::click(),
        );
        if card_response.hovered() && moved {
            state.overview.selected = i;
        }
        let selected = state.overview.selected == i;
        let is_current = *id == current;
        let (width, color) = if is_current {
            (2.0, palette.accent)
        } else if selected {
            (2.0, palette.text)
        } else if card_response.hovered() {
            (1.0, palette.muted)
        } else {
            (1.0, palette.border)
        };
        ui.painter().rect(
            card,
            CornerRadius::same(12),
            if selected || card_response.hovered() {
                palette.hover
            } else {
                pal().card_fill
            },
            Stroke::new(width, color),
            StrokeKind::Middle,
        );
        if card_response.clicked() {
            state.shared.overview_workspace = Some(*id);
        }

        // Cabecera reservada: las ventanas nunca tapan el título del workspace.
        let content = egui::Rect::from_min_max(
            card.min + egui::vec2(6.0, 22.0),
            card.max - egui::vec2(6.0, 6.0),
        );
        text_at(
            ui,
            card.min + egui::vec2(10.0, 11.0),
            egui::Align2::LEFT_CENTER,
            format!("Workspace {id}"),
            TEXT_CAPTION,
            if is_current {
                palette.accent
            } else {
                palette.muted
            },
        );
        let windows: Vec<crate::ipc::ClientInfo> = state
            .shared
            .clients
            .iter()
            .filter(|c| c.mapped && c.workspace == *id as i32)
            .cloned()
            .collect();
        if windows.is_empty() {
            text_at(
                ui,
                content.center(),
                egui::Align2::CENTER_CENTER,
                "Vacío",
                TEXT_CAPTION,
                palette.dim,
            );
        }
        // La ventana activa se pinta al final: queda encima si se solapan.
        for j in model::paint_order(&windows, &active_address) {
            let win = &windows[j];
            let rect = model::window_rect(win, monitor, content, 8.0);
            if rect.width() <= 0.0 || rect.height() <= 0.0 {
                continue;
            }
            // Con búsqueda, lo que no coincide se atenúa.
            let dimmed = !model::window_matches(&query, &win.class, &win.title);
            let previous = ui.opacity();
            if dimmed {
                ui.set_opacity(previous * 0.28);
            }
            let active_window = win.address == active_address;
            let response = ui.interact(
                rect,
                ui.scope_id().with(("overview-win", &win.address)),
                egui::Sense::click(),
            );
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(4),
                if active_window {
                    palette.badge.gamma_multiply(1.4)
                } else {
                    palette.badge
                },
            );
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(4),
                Stroke::new(
                    if active_window { 1.5 } else { 1.0 },
                    if active_window {
                        palette.accent
                    } else if response.hovered() {
                        palette.text
                    } else {
                        palette.border
                    },
                ),
                StrokeKind::Middle,
            );
            let icon_center = rect.min + egui::vec2(9.0, 9.0);
            if let Some(texture) = state.shared.icon(ui.ctx(), &win.class) {
                egui::Image::new(egui::load::SizedTexture::new(
                    texture.id(),
                    egui::vec2(12.0, 12.0),
                ))
                .paint_at(
                    ui,
                    egui::Rect::from_center_size(icon_center, egui::vec2(12.0, 12.0)),
                );
            } else {
                ui.painter().text(
                    icon_center,
                    egui::Align2::CENTER_CENTER,
                    dock_icon(&win.class),
                    icon_font(10.0),
                    palette.text,
                );
            }
            if rect.width() > 70.0 {
                ui.painter().text(
                    rect.min + egui::vec2(18.0, 9.0),
                    egui::Align2::LEFT_CENTER,
                    truncate(&win.title, 18),
                    egui::FontId::new(9.0, egui::FontFamily::Proportional),
                    palette.text,
                );
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if response.clicked() {
                state.shared.overview_focus = Some(win.address.clone());
            }
            ui.set_opacity(previous);
        }
    }
}

/// Punto de entrada: despacha a la vista de overview elegida.
pub(super) fn draw_overview(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    match state.overview.kind {
        OverviewKind::Workspaces => draw_workspaces(ui, state, palette),
        OverviewKind::Activities => super::overview_collection::draw_activities(ui, state, palette),
        OverviewKind::StripH => super::overview_collection::draw_strip_h(ui, state, palette),
        OverviewKind::StripV => super::overview_collection::draw_strip_v(ui, state, palette),
    }
}
