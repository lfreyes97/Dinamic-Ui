//! Overview de workspaces: la rejilla de pantalla completa y las franjas acoplables, sobre un
//! mismo estado. La lógica (qué se muestra, navegación, búsqueda) vive en `overview_model.rs`;
//! las piezas visuales compartidas con Actividades, en `overlay.rs`.

use smithay_client_toolkit::seat::keyboard::Keysym;

use super::nav::{step, window_rect, Dir};
use super::overlay::{hint_bar, mini_window, pointer_moved, search_pill};
use super::overview_model as model;
use super::*;

/// Las tres vistas de overview de workspaces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OverviewKind {
    #[default]
    Workspaces,
    StripH,
    StripV,
}

impl OverviewKind {
    /// Vista modal: cubre la pantalla, toma el teclado y admite búsqueda. Las franjas no.
    pub fn is_modal(self) -> bool {
        matches!(self, Self::Workspaces)
    }
}

/// Ventana en arrastre: origen y dirección para el comando de movimiento.
#[derive(Debug, Clone)]
pub struct OverviewDrag {
    pub address: String,
    pub from_ws: u32,
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
    /// Miniaturas que caben en la franja (lo fija el dibujo según el espacio; 0 = aún no).
    pub strip_visible: usize,
    /// Búsqueda (solo la rejilla).
    pub query: String,
    /// Elemento seleccionado con el teclado o el cursor.
    pub selected: usize,
    /// Arrastre de una miniatura a otra workspace (solo la rejilla modal).
    pub drag: Option<OverviewDrag>,
    /// Workspace resaltado como destino mientras se arrastra.
    pub drop_target: Option<u32>,
}

impl OverviewState {
    /// ¿Hay algo que dibujar? (también mientras se desvanece al cerrar).
    pub fn visible(&self) -> bool {
        self.overview || self.progress > 0.005
    }

    /// Miniaturas visibles de la franja para navegar con el teclado.
    pub fn strip_window(&self) -> usize {
        if self.strip_visible == 0 {
            5
        } else {
            self.strip_visible
        }
    }
}

pub(super) fn current_ws(state: &UiState) -> u32 {
    state.shared.snapshot.workspace.parse::<u32>().unwrap_or(1)
}

/// Workspaces de la rejilla: 6 casillas, con los ocupados y el actual primero y los vacíos de
/// menor número rellenando.
pub(super) fn grid_ids(state: &UiState) -> Vec<u32> {
    let count = state.shared.config.workspaces.count.max(6);
    let ids = model::workspace_ids(&state.shared.snapshot.occupied, current_ws(state), 6);
    model::pad_workspace_ids(ids, 6, count)
}

/// Workspaces de las franjas: todos los configurados (hasta 20), con desplazamiento.
pub(super) fn strip_ids(state: &UiState) -> Vec<u32> {
    let count = state.shared.config.workspaces.count.clamp(1, 20);
    let ids = model::workspace_ids(&state.shared.snapshot.occupied, current_ws(state), 20);
    model::pad_workspace_ids(ids, count as usize, count)
}

/// Rectángulos de las cards de la rejilla, en el mismo orden que `grid_ids`.
/// Los usa el dibujo y también el hit-test del drag & drop para no desincronizarse.
pub(super) fn grid_card_rects(state: &UiState, screen: egui::Rect) -> Vec<(u32, egui::Rect)> {
    let safe = super::safe_rect(state, screen);
    let ids = grid_ids(state);
    let cols = ids.len().clamp(1, 3);
    let rows = ids.len().div_ceil(cols);
    let gap = 14.0;
    let area = egui::Rect::from_min_max(
        safe.min + egui::vec2(26.0, 76.0),
        safe.max - egui::vec2(26.0, 44.0),
    );
    let card_w = ((area.width() - gap * (cols as f32 - 1.0)) / cols as f32).min(560.0);
    let card_h = ((area.height() - gap * (rows as f32 - 1.0)) / rows as f32).min(340.0);
    let total_w = cols as f32 * card_w + gap * (cols as f32 - 1.0);
    let total_h = rows as f32 * card_h + gap * (rows as f32 - 1.0);
    let origin = egui::pos2(
        area.center().x - total_w / 2.0,
        area.center().y - total_h / 2.0,
    );
    ids.into_iter()
        .enumerate()
        .map(|(i, id)| {
            let base = egui::Rect::from_min_size(
                egui::pos2(
                    origin.x + (i % cols) as f32 * (card_w + gap),
                    origin.y + (i / cols) as f32 * (card_h + gap),
                ),
                egui::vec2(card_w, card_h),
            );
            (id, base)
        })
        .collect()
}

/// Workspace cuya card contiene `pos`, si alguna.
pub(super) fn card_at(cards: &[(u32, egui::Rect)], pos: egui::Pos2) -> Option<u32> {
    cards
        .iter()
        .find(|(_, rect)| rect.contains(pos))
        .map(|(id, _)| *id)
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
            state.overview.strip_offset =
                model::scroll_to(0, selected, state.overview.strip_window());
            selected
        }
    };
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
            state.overview.selected = step(selected, dir, count.clamp(1, 3), count);
        }
        OverviewKind::StripH | OverviewKind::StripV => {
            // En una franja solo hay un eje: arriba/izquierda retrocede, abajo/derecha avanza.
            let dir = match dir {
                Dir::Left | Dir::Up => Dir::Left,
                Dir::Right | Dir::Down => Dir::Right,
            };
            let count = strip_ids(state).len();
            let next = step(selected, dir, count, count);
            let visible = state.overview.strip_window();
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
/// `Esc` primero limpia la búsqueda y después cierra. En la rejilla escribir filtra y un número
/// salta a ese workspace mientras no haya búsqueda; las franjas solo navegan y saltan.
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
    let jump_allowed = !kind.is_modal() || state.overview.query.is_empty();
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
    // La rejilla tiene el teclado en exclusiva: no dejamos pasar teclas sueltas.
    true
}

/// Overview: rejilla de workspaces con las ventanas en su posición relativa.
fn draw_workspaces(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let screen = ui.max_rect();
    // El velo y el contenido dejan libres las zonas reservadas por topbar/taskbar.
    let safe = super::safe_rect(state, screen);
    ui.painter().rect_filled(
        safe,
        CornerRadius::same(Radius::of(Radius::NONE)),
        Palette::scrim(168),
    );
    // Registrado antes que las tarjetas: un clic en ventana gana sobre el fondo.
    let backdrop = ui.interact(
        safe,
        ui.scope_id().with(egui::IdSalt::new("overview-backdrop")),
        egui::Sense::click(),
    );
    if backdrop.clicked() {
        state.overview.overview = false;
    }

    let query = state.overview.query.clone();
    search_pill(ui, safe, &query, "Busca apps, ventanas, workspaces…");
    if !query.is_empty() {
        text_at(
            ui,
            egui::pos2(safe.center().x, safe.min.y + 60.0),
            egui::Align2::CENTER_CENTER,
            format!("{} coincidencias", match_count(state)),
            9.0,
            palette.dim,
        );
    }

    let cards = grid_card_rects(state, screen);
    let current = current_ws(state);
    state.overview.selected = state.overview.selected.min(cards.len().saturating_sub(1));

    let monitor = safe.size();
    let aspect = monitor.x.max(1.0) / monitor.y.max(1.0);
    let moved = pointer_moved(ui);
    let active_address = state.shared.snapshot.window_address.clone();

    for (i, (id, base)) in cards.iter().enumerate() {
        let card_response = ui.interact(
            *base,
            ui.scope_id().with(("overview-card", *id)),
            egui::Sense::click(),
        );
        if card_response.hovered() && moved {
            state.overview.selected = i;
        }
        let selected = state.overview.selected == i;
        let is_current = *id == current;
        // Al pasar el cursor la tarjeta se levanta 2 px, con animación.
        let lift = ui.ctx().animate_bool_with_time(
            ui.scope_id().with(("overview-lift", *id)),
            card_response.hovered() || selected,
            0.12,
        );
        let card = base.translate(egui::vec2(0.0, -2.0 * lift));

        drop_shadow(ui, card, 9, 8, 22, 90);
        let is_target = state.overview.drop_target == Some(*id);
        let fill = if is_target || is_current {
            crate::theme::mix(palette.raised, palette.accent, 0.12)
        } else {
            palette.raised
        };
        let (width, color) = if is_target {
            (2.0, palette.accent)
        } else if is_current {
            (1.0, Palette::with_alpha(palette.accent, 140))
        } else if selected {
            (1.5, palette.text)
        } else if card_response.hovered() {
            (1.0, palette.muted)
        } else {
            (1.0, palette.border)
        };
        ui.painter().rect(
            card,
            CornerRadius::same(Radius::of(Radius::MD)),
            Palette::with_alpha(fill, 224),
            Stroke::new(width, color),
            StrokeKind::Middle,
        );
        if is_current {
            // Halo de 1 px alrededor de la tarjeta del workspace actual.
            ui.painter().rect_stroke(
                card.expand(1.5),
                CornerRadius::same(Radius::of(Radius::MD)),
                Stroke::new(1.0, Palette::with_alpha(palette.accent, 50)),
                StrokeKind::Middle,
            );
        }
        if card_response.clicked() {
            state.shared.overview_workspace = Some(*id);
        }

        let windows: Vec<crate::ipc::ClientInfo> = state
            .shared
            .clients
            .iter()
            .filter(|c| c.mapped && c.workspace == *id as i32)
            .cloned()
            .collect();

        // Etiqueta arriba a la izquierda y nombre (o recuento) abajo, centrado.
        text_at(
            ui,
            card.min + egui::vec2(11.0, 13.0),
            egui::Align2::LEFT_CENTER,
            format!("{id:02}"),
            8.5,
            if is_current {
                palette.accent
            } else {
                palette.muted
            },
        );
        let caption = match state.shared.config.workspaces.name_of(*id) {
            Some(name) => name.to_string(),
            None => match windows.len() {
                0 => "Vacío".to_string(),
                1 => "1 ventana".to_string(),
                n => format!("{n} ventanas"),
            },
        };
        text_at(
            ui,
            egui::pos2(card.center().x, card.max.y - 12.0),
            egui::Align2::CENTER_CENTER,
            caption,
            9.0,
            palette.muted,
        );

        // Vista del monitor: mismo aspecto que la pantalla, centrada entre etiqueta y nombre.
        let inner = egui::Rect::from_min_max(
            card.min + egui::vec2(12.0, 28.0),
            card.max - egui::vec2(12.0, 28.0),
        );
        let (mw, mh) = if inner.width() / inner.height().max(1.0) > aspect {
            (inner.height() * aspect, inner.height())
        } else {
            (inner.width(), inner.width() / aspect)
        };
        let mon = egui::Rect::from_center_size(inner.center(), egui::vec2(mw, mh));
        ui.painter().rect_filled(
            mon,
            CornerRadius::same(Radius::of(Radius::XS)),
            Palette::sheen(6),
        );

        // La ventana activa se pinta al final: queda encima si se solapan.
        for j in model::paint_order(&windows, &active_address) {
            let win = &windows[j];
            let rect = window_rect(win, monitor, mon, 8.0);
            if rect.width() <= 0.0 || rect.height() <= 0.0 {
                continue;
            }
            // Con búsqueda, lo que no coincide se atenúa.
            let dimmed = !model::window_matches(&query, &win.class, &win.title);
            let previous = ui.opacity();
            if dimmed {
                ui.set_opacity(previous * 0.28);
            }
            let response = ui.interact(
                rect,
                ui.scope_id().with(("overview-win", &win.address)),
                egui::Sense::click_and_drag(),
            );
            mini_window(
                ui,
                rect,
                win.address == active_address,
                response.hovered(),
                3,
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
            if response.drag_started() {
                state.overview.drag = Some(OverviewDrag {
                    address: win.address.clone(),
                    from_ws: *id,
                });
            }
            if response.dragged() {
                let pos = ui.input(|i| i.pointer.latest_pos());
                state.overview.drop_target = pos.and_then(|p| card_at(&cards, p)).filter(|ws| {
                    state
                        .overview
                        .drag
                        .as_ref()
                        .is_some_and(|d| d.from_ws != *ws)
                });
            }
            if response.drag_stopped() {
                let drag = state.overview.drag.take();
                let target = state.overview.drop_target.take();
                if let (Some(drag), Some(target)) = (drag, target) {
                    if drag.from_ws != target {
                        state.shared.overview_move = Some((drag.address, target));
                    }
                }
            }
            if response.clicked() {
                state.shared.overview_focus = Some(win.address.clone());
            }
            ui.set_opacity(previous);
        }
    }

    // Fantasma del arrastre: sigue al puntero y muestra qué ventana se mueve.
    if let Some(drag) = state.overview.drag.clone() {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            let title = state
                .shared
                .clients
                .iter()
                .find(|c| c.address == drag.address)
                .map(|c| c.title.clone())
                .unwrap_or_default();
            let ghost =
                egui::Rect::from_center_size(pos + egui::vec2(0.0, -18.0), egui::vec2(150.0, 32.0));
            ui.painter().rect_filled(
                ghost,
                CornerRadius::same(Radius::of(Radius::SM)),
                Palette::with_alpha(palette.raised, 240),
            );
            ui.painter().rect_stroke(
                ghost,
                CornerRadius::same(Radius::of(Radius::SM)),
                Stroke::new(1.5, palette.accent),
                StrokeKind::Middle,
            );
            text_at(
                ui,
                ghost.center(),
                egui::Align2::CENTER_CENTER,
                truncate(&title, 20),
                9.0,
                palette.text,
            );
        }
    }

    if query.is_empty() {
        hint_bar(
            ui,
            safe,
            &[
                ("← →", "navega"),
                ("↵", "abre"),
                ("arrastra", "mueve de workspace"),
                ("1–9", "salta"),
                ("Esc", "cierra"),
            ],
        );
    } else {
        hint_bar(ui, safe, &[("↵", "enfoca la primera"), ("Esc", "limpia")]);
    }
}

/// Punto de entrada: despacha a la vista de overview elegida.
pub(super) fn draw_overview(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    match state.overview.kind {
        OverviewKind::Workspaces => draw_workspaces(ui, state, palette),
        OverviewKind::StripH => super::overview_strips::draw_strip_h(ui, state, palette),
        OverviewKind::StripV => super::overview_strips::draw_strip_v(ui, state, palette),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_at_devuelve_el_workspace_bajo_el_puntero() {
        let cards = vec![
            (
                1u32,
                egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 80.0)),
            ),
            (
                2u32,
                egui::Rect::from_min_size(egui::pos2(110.0, 0.0), egui::vec2(100.0, 80.0)),
            ),
        ];
        assert_eq!(card_at(&cards, egui::pos2(10.0, 10.0)), Some(1));
        assert_eq!(card_at(&cards, egui::pos2(150.0, 40.0)), Some(2));
        assert_eq!(card_at(&cards, egui::pos2(105.0, 40.0)), None);
        assert_eq!(card_at(&[], egui::pos2(0.0, 0.0)), None);
    }
}
