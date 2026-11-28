//! Switchers de ventana (Alt+Tab) y workspace (Super+Tab): rejilla compacta
//! sobre el estado que ya tiene el shell, sin IPC nuevo.

use std::collections::HashMap;

use smithay_client_toolkit::seat::keyboard::Modifiers;

use super::component::{close_overlays, Component, DrawCtx};
use super::overlay::hint_bar;
use super::*;
use crate::config::WindowSwitcherStyle as Style;
use crate::{approach, control};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitcherKind {
    Windows,
    Workspaces,
}

/// Modificador que mantiene el usuario mientras cicla (Alt+Tab / Super+Tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldModifier {
    Alt,
    Super,
}

#[derive(Default)]
pub struct SwitcherState {
    pub open: bool,
    pub kind: Option<SwitcherKind>,
    pub selected: usize,
    pub progress: f32,
    /// Modificador a soltar para confirmar; `None` si se abrió por CLI/panel.
    hold: Option<HoldModifier>,
    /// Solo se arma al ver el modificador pulsado: evita confirmar con una
    /// actualización de modifiers ajena a este switcher.
    armed: bool,
    /// Tiempo desde el último ciclo/evento de modificador. Si se queda abierto
    /// sin actividad, se cierra para no retener el teclado en Exclusive.
    age: f32,
}

/// Segundos de inactividad antes de soltar el teclado. Un Alt+Tab sostenido
/// cicla con cada repetición del bind, lo que reinicia este contador.
const IDLE_TIMEOUT: f32 = 3.0;

/// Referencia de diseño (16:9). Todas las medidas del switcher son fracciones de
/// la pantalla a través de este factor: se toma la menor de las dos ratios para
/// que el diseño no se estire en pantallas no 16:9.
const REF_W: f32 = 1280.0;
const REF_H: f32 = 720.0;

fn switcher_unit(screen: egui::Rect) -> f32 {
    (screen.width() / REF_W)
        .min(screen.height() / REF_H)
        .max(0.25)
}

/// Rect de una ventana dentro del preview con un único factor de escala.
/// Conserva su geometría real (tamaño y posición relativos) sin deformarla.
fn wire_rect(preview: egui::Rect, item: &Item, screen: egui::Rect) -> Option<egui::Rect> {
    let (x, y, w, h) = item.geom?;
    let k = (preview.width() / screen.width().max(1.0))
        .min(preview.height() / screen.height().max(1.0));
    let desktop = egui::vec2(screen.width() * k, screen.height() * k);
    let origin = egui::pos2(
        preview.min.x + (preview.width() - desktop.x) / 2.0,
        preview.min.y + (preview.height() - desktop.y) / 2.0,
    );
    Some(egui::Rect::from_min_size(
        egui::pos2(origin.x + x as f32 * k, origin.y + y as f32 * k),
        egui::vec2((w as f32 * k).max(1.0), (h as f32 * k).max(1.0)),
    ))
}

impl SwitcherState {
    pub fn visible(&self) -> bool {
        self.open || self.progress > 0.005
    }
}

/// Cierre común: limpia también el modificador vigilado.
fn close(state: &mut UiState) {
    state.switcher.open = false;
    state.switcher.hold = None;
    state.switcher.armed = false;
}

#[derive(Clone)]
struct Item {
    key: String,
    title: String,
    subtitle: String,
    class: String,
    /// Geometría real de la ventana, para la miniatura del switcher.
    geom: Option<(i32, i32, i32, i32)>,
    /// Workspace al que pertenece; lo usan solo los estilos que lo muestran.
    ws: Option<i32>,
}

/// Ventanas en orden MRU, o workspaces ocupados + el actual.
fn items(state: &UiState) -> Vec<Item> {
    match state.switcher.kind {
        Some(SwitcherKind::Windows) => {
            let by_address: HashMap<&str, &crate::ipc::ClientInfo> = state
                .shared
                .clients
                .iter()
                .map(|c| (c.address.as_str(), c))
                .collect();
            let mut ordered: Vec<&crate::ipc::ClientInfo> = state
                .shared
                .mru
                .iter()
                .filter_map(|address| by_address.get(address.as_str()).copied())
                .filter(|c| c.mapped)
                .collect();
            // El MRU solo se alimenta con cambios de foco: al arrancar está vacío.
            // Las ventanas aún no visitadas entran al final para que todas sean
            // alcanzables con Tab.
            let known = ordered
                .iter()
                .map(|c| c.address.clone())
                .collect::<Vec<_>>();
            for client in state
                .shared
                .clients
                .iter()
                .filter(|c| c.mapped && !known.contains(&c.address))
            {
                ordered.push(client);
            }
            ordered
                .into_iter()
                .map(|c| Item {
                    key: c.address.clone(),
                    title: truncate(&c.title, 22),
                    subtitle: format!("workspace {}", c.workspace),
                    class: c.class.clone(),
                    geom: Some((c.x, c.y, c.w, c.h)),
                    ws: Some(c.workspace),
                })
                .collect()
        }
        Some(SwitcherKind::Workspaces) => {
            let current = overview::current_ws(state);
            // Recién visitados primero (el actual suele estar al frente). Los
            // ocupados que nunca se visitaron entran al final, en orden numérico.
            let mut ids: Vec<u32> = state.shared.workspace_mru.clone();
            let mut rest = state.shared.snapshot.occupied.clone();
            if !rest.contains(&current) {
                rest.push(current);
            }
            rest.sort_unstable();
            rest.dedup();
            for id in rest {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            ids.into_iter()
                .take(20)
                .map(|id| Item {
                    key: id.to_string(),
                    geom: None,
                    ws: Some(id as i32),
                    title: state
                        .shared
                        .config
                        .workspaces
                        .name_of(id)
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("Workspace {id:02}")),
                    subtitle: format!(
                        "{} ventana{}",
                        state
                            .shared
                            .clients
                            .iter()
                            .filter(|c| c.mapped && c.workspace == id as i32)
                            .count(),
                        if state
                            .shared
                            .clients
                            .iter()
                            .filter(|c| c.mapped && c.workspace == id as i32)
                            .count()
                            == 1
                        {
                            ""
                        } else {
                            "s"
                        }
                    ),
                    class: String::new(),
                })
                .collect()
        }
        None => Vec::new(),
    }
}

pub(super) fn open(state: &mut UiState, kind: SwitcherKind, backwards: bool) {
    close_overlays(state);
    state.switcher.kind = Some(kind);
    state.switcher.open = true;
    state.switcher.hold = Some(match kind {
        SwitcherKind::Windows => HoldModifier::Alt,
        SwitcherKind::Workspaces => HoldModifier::Super,
    });
    state.switcher.armed = false;
    state.switcher.age = 0.0;
    let count = items(state).len();
    state.switcher.selected = if kind == SwitcherKind::Windows && count > 1 {
        1
    } else if backwards && count > 1 {
        count - 1
    } else {
        0
    };
}

pub(super) fn cycle(state: &mut UiState, backwards: bool) {
    let count = items(state).len();
    if count == 0 {
        return;
    }
    let delta = if backwards { count - 1 } else { 1 };
    state.switcher.selected = (state.switcher.selected + delta) % count;
    state.switcher.age = 0.0;
}

pub(super) fn activate(state: &mut UiState) {
    let kind = state.switcher.kind;
    if let (Some(kind), Some(item)) = (kind, items(state).get(state.switcher.selected).cloned()) {
        let command = match kind {
            SwitcherKind::Windows => format!("focuswindow address:{}", item.key),
            SwitcherKind::Workspaces => format!("workspace {}", item.key),
        };
        state.shared.pending_commands.push(command);
    }
    close(state);
}

/// Soltura del modificador: estilo GNOME. Cada Tab (bind repetible) cicla la
/// selección; al soltar Alt/Super se aplica. Si se abrió por CLI no hay
/// modificador vigilado y no se confirma accidentalmente.
pub fn on_modifiers(state: &mut UiState, mods: Modifiers) {
    if !state.switcher.open {
        return;
    }
    state.switcher.age = 0.0;
    let Some(hold) = state.switcher.hold else {
        return;
    };
    let held = match hold {
        HoldModifier::Alt => mods.alt,
        HoldModifier::Super => mods.logo,
    };
    if held {
        state.switcher.armed = true;
    } else if state.switcher.armed {
        activate(state);
    }
}

/// Teclado del switcher: `Esc` cierra, `↵` activa, flechas avanzan.
pub(super) fn handle_key(
    state: &mut UiState,
    key: smithay_client_toolkit::seat::keyboard::Keysym,
) -> bool {
    use smithay_client_toolkit::seat::keyboard::Keysym;
    if !state.switcher.open {
        return false;
    }
    match key {
        Keysym::Escape => close(state),
        Keysym::Return | Keysym::KP_Enter => activate(state),
        Keysym::Left | Keysym::Up => cycle(state, true),
        Keysym::Right | Keysym::Down => cycle(state, false),
        _ => return false,
    }
    true
}

pub(super) fn draw(ui: &mut egui::Ui, state: &mut UiState, _palette: Palette) {
    let screen = ui.max_rect();
    let safe = super::safe_rect(state, screen);
    // `ui::draw` ya fijó la paleta del frame: los helpers leen `pal()`.
    let p = pal();
    match state.switcher.kind {
        Some(SwitcherKind::Workspaces) => draw_workspace_dock(ui, state, safe, p),
        Some(SwitcherKind::Windows) => match state.shared.config.switcher.style {
            Style::Cards => draw_window_cards(ui, state, safe, screen, p),
            Style::Filmstrip => draw_window_filmstrip(ui, state, safe, screen, p),
            Style::Grid => draw_window_grid(ui, state, safe, screen, p),
            Style::List => draw_window_list(ui, state, safe, screen, p),
            Style::Stack => draw_window_stack(ui, state, safe, screen, p),
            Style::WindowOnly => draw_window_only(ui, state, safe, screen, p),
            Style::Cascade => draw_window_cascade(ui, state, safe, screen, p),
        },
        None => {}
    }
}

/// Mini-overview centrado, con la geometría del dock y solo los números de ws.
fn draw_workspace_dock(ui: &mut egui::Ui, state: &mut UiState, safe: egui::Rect, p: Palette) {
    ui.painter().rect_filled(
        safe,
        CornerRadius::same(Radius::of(Radius::NONE)),
        Palette::scrim(72),
    );
    let all = items(state);
    if all.is_empty() {
        hint_bar(ui, safe, &[("Esc", "cierra")]);
        return;
    }
    state.switcher.selected = state.switcher.selected.min(all.len() - 1);

    // ITEM/PAD/H son constantes del dock (no hay token de tamaño); GAP es Space::SM.
    const ITEM: f32 = 42.0;
    const GAP: f32 = Space::SM;
    const PAD: f32 = 7.0;
    const H: f32 = 56.0;
    let n = all.len().min(20);
    let w = PAD * 2.0 + n as f32 * ITEM + (n as f32 - 1.0) * GAP;
    let dock = egui::Rect::from_center_size(safe.center(), egui::vec2(w, H));

    drop_shadow(ui, dock.translate(egui::vec2(0.0, 3.0)), 10, 8, 22, 90);
    ui.painter().rect(
        dock,
        CornerRadius::same(Radius::of(Radius::XL)),
        p.card_fill,
        Stroke::new(1.0, p.border),
        StrokeKind::Middle,
    );

    let mut picked = false;
    for (i, item) in all.iter().enumerate().take(n) {
        let rect = egui::Rect::from_min_size(
            egui::pos2(
                dock.min.x + PAD + i as f32 * (ITEM + GAP),
                dock.center().y - ITEM / 2.0,
            ),
            egui::vec2(ITEM, ITEM),
        );
        let selected = i == state.switcher.selected;
        let resp = ui.interact(rect, ui.scope_id().with(("sw-ws", i)), egui::Sense::click());
        let fill = if resp.is_pointer_button_down_on() {
            p.pressed
        } else if selected {
            p.accent
        } else {
            p.badge
        };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(Radius::of(Radius::CARD)), fill);
        // `key` es el id del workspace: solo el número, sin nombre ni conteo.
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            item.key.as_str(),
            egui::FontId::proportional(TEXT_DISPLAY_SM),
            if selected { p.solid() } else { p.text },
        );
        if resp.clicked() {
            state.switcher.selected = i;
            picked = true;
        }
    }

    if picked {
        activate(state);
    } else {
        let clicks = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
        let hover = ui.input(|i| i.pointer.hover_pos());
        if clicks && hover.is_some_and(|pos| safe.contains(pos) && !dock.contains(pos)) {
            close(state);
        }
    }
    hint_bar(
        ui,
        safe,
        &[("← →", "elige"), ("↵", "abre"), ("Esc", "cierra")],
    );
}

/// Tarjeta protagonista con las dos ventanas vecinas del MRU a los lados.
fn draw_window_cards(
    ui: &mut egui::Ui,
    state: &mut UiState,
    safe: egui::Rect,
    screen: egui::Rect,
    p: Palette,
) {
    ui.painter().rect_filled(
        safe,
        CornerRadius::same(Radius::of(Radius::NONE)),
        Palette::scrim(176),
    );
    let all = items(state);
    if all.is_empty() {
        hint_bar(ui, safe, &[("Esc", "cierra")]);
        return;
    }
    state.switcher.selected = state.switcher.selected.min(all.len() - 1);
    let idx = state.switcher.selected;
    let start = idx.saturating_sub(1).min(all.len().saturating_sub(3));
    let shown = all.iter().skip(start).take(3).cloned().collect::<Vec<_>>();
    let hero = idx - start;
    let center = safe.center();
    let s = switcher_unit(screen);
    let gap = Space::MD * 2.0 * s;
    let hero_r = egui::Rect::from_center_size(center, egui::vec2(360.0 * s, 216.0 * s));
    let flank_w = 180.0 * s;
    let flank_h = 152.0 * s;
    let flank_off = 180.0 * s + gap + flank_w / 2.0;

    let mut rects = Vec::new();
    for (i, item) in shown.iter().enumerate() {
        let is_hero = i == hero;
        let rect = if is_hero {
            hero_r
        } else {
            let side = if i < hero { -1.0 } else { 1.0 };
            egui::Rect::from_center_size(
                egui::pos2(center.x + side * flank_off, center.y),
                egui::vec2(flank_w, flank_h),
            )
        };
        rects.push(rect);

        let lift =
            ui.ctx()
                .animate_bool_with_time(ui.scope_id().with(("sw-lift", i)), is_hero, 0.12);
        let rect = rect.translate(egui::vec2(0.0, -2.0 * s * lift));
        drop_shadow(ui, rect, 10, 8, 22, 90);
        ui.painter().rect(
            rect,
            CornerRadius::same(Radius::of(if is_hero { Radius::LG } else { Radius::CARD })),
            if is_hero {
                p.accent_soft
            } else {
                Palette::with_alpha(p.raised, 235)
            },
            Stroke::new(
                if is_hero { 1.5 } else { 1.0 },
                if is_hero { p.accent } else { p.border },
            ),
            StrokeKind::Middle,
        );

        let icon_size = (if is_hero {
            ICON_XL * 2.0
        } else {
            ICON_XL * 1.6
        }) * s;
        let icon_center = egui::pos2(
            rect.center().x,
            rect.min.y + (if is_hero { 46.0 } else { 34.0 }) * s,
        );
        if !item.class.is_empty() {
            if let Some(texture) = state.shared.icon(ui.ctx(), &item.class) {
                egui::Image::new(egui::load::SizedTexture::new(
                    texture.id(),
                    egui::vec2(icon_size, icon_size),
                ))
                .paint_at(
                    ui,
                    egui::Rect::from_center_size(icon_center, egui::vec2(icon_size, icon_size)),
                );
            } else {
                icon_at(ui, icon_center, dock_icon(&item.class), icon_size, p.text);
            }
        } else {
            icon_at(ui, icon_center, ic::SQUARES_FOUR, icon_size, p.text);
        }
        text_at(
            ui,
            egui::pos2(
                rect.center().x,
                rect.min.y + (if is_hero { 96.0 } else { 83.0 }) * s,
            ),
            egui::Align2::CENTER_CENTER,
            &item.title,
            (if is_hero { TEXT_DISPLAY_SM } else { TEXT_BODY }) * s,
            p.text,
        );
        text_at(
            ui,
            egui::pos2(
                rect.center().x,
                rect.min.y + (if is_hero { 120.0 } else { 104.0 }) * s,
            ),
            egui::Align2::CENTER_CENTER,
            &item.subtitle,
            TEXT_CAPTION * s,
            p.muted,
        );
        if is_hero {
            let preview = egui::Rect::from_min_size(
                hero_r.min + egui::vec2(50.0 * s, 142.0 * s),
                egui::vec2(260.0 * s, 56.0 * s),
            );
            draw_window_wire(ui, preview, item, screen, p);
        }
    }

    for (i, _item) in shown.iter().enumerate() {
        let resp = ui.interact(
            rects[i],
            ui.scope_id().with(("sw-card", i)),
            egui::Sense::click(),
        );
        if resp.clicked() {
            state.switcher.selected = start + i;
        }
        if resp.double_clicked() {
            state.switcher.selected = start + i;
            activate(state);
        }
    }
    let clicks = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
    let hover = ui.input(|i| i.pointer.hover_pos());
    if clicks
        && hover.is_some_and(|pos| safe.contains(pos) && !rects.iter().any(|r| r.contains(pos)))
    {
        close(state);
    }
    hint_bar(
        ui,
        safe,
        &[("← →", "elige"), ("↵", "abre"), ("Esc", "cierra")],
    );
}

/// Alto de la barra de título de las tarjetas con aspecto de ventana, a 720p.
const WINDOW_TB: f32 = 20.0;

/// Icono real de la app si está cargado; si no, glifo Phosphor según la clase.
fn draw_app_icon(
    ui: &mut egui::Ui,
    state: &mut UiState,
    class: &str,
    center: egui::Pos2,
    size: f32,
    p: Palette,
) {
    if !class.is_empty() {
        if let Some(texture) = state.shared.icon(ui.ctx(), class) {
            egui::Image::new(egui::load::SizedTexture::new(
                texture.id(),
                egui::vec2(size, size),
            ))
            .paint_at(
                ui,
                egui::Rect::from_center_size(center, egui::vec2(size, size)),
            );
            return;
        }
        icon_at(ui, center, dock_icon(class), size, p.text);
    } else {
        icon_at(ui, center, ic::SQUARES_FOUR, size, p.text);
    }
}

/// Rango de ventanas visible alrededor de la selección: `(inicio, fin)`.
fn window_range(len: usize, selected: usize, max: usize) -> (usize, usize) {
    if len == 0 {
        return (0, 0);
    }
    let take = len.min(max);
    let start = selected
        .min(len - 1)
        .saturating_sub(take / 2)
        .min(len - take);
    (start, start + take)
}

/// Índices del MRU agrupados por workspace, en orden de aparición.
fn group_items(all: &[Item]) -> Vec<(i32, Vec<usize>)> {
    let mut groups: Vec<(i32, Vec<usize>)> = Vec::new();
    for (i, item) in all.iter().enumerate() {
        let ws = item.ws.unwrap_or(0);
        if let Some(group) = groups.iter_mut().find(|(id, _)| *id == ws) {
            group.1.push(i);
        } else {
            groups.push((ws, vec![i]));
        }
    }
    groups
}

/// Tarjeta con apariencia de ventana: barra de título y cuerpo con wireframe.
/// El frente muestra la geometría real; las capas posteriores solo su título.
fn draw_window_tile(
    ui: &mut egui::Ui,
    state: &mut UiState,
    rect: egui::Rect,
    item: &Item,
    screen: egui::Rect,
    p: Palette,
    s: f32,
    front: bool,
) {
    let tb = WINDOW_TB * s;
    let titlebar = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), tb));
    ui.painter().rect_filled(
        titlebar,
        CornerRadius::same(Radius::of(Radius::MD)),
        Palette::sheen(if front { 10 } else { 6 }),
    );
    ui.painter().line_segment(
        [
            egui::pos2(rect.min.x, rect.min.y + tb),
            egui::pos2(rect.max.x, rect.min.y + tb),
        ],
        Stroke::new(1.0, p.border),
    );
    let dot_y = rect.min.y + tb / 2.0;
    let dot_x = rect.min.x + 9.0 * s;
    let dot_r = 2.0 * s;
    ui.painter()
        .circle_filled(egui::pos2(dot_x, dot_y), dot_r, p.accent);
    for n in 1..3 {
        ui.painter().circle_filled(
            egui::pos2(dot_x + n as f32 * 8.0 * s, dot_y),
            dot_r,
            p.faint,
        );
    }
    let icon_size = 12.0 * s;
    draw_app_icon(
        ui,
        state,
        &item.class,
        egui::pos2(rect.min.x + 32.0 * s + icon_size / 2.0, dot_y),
        icon_size,
        p,
    );
    text_at(
        ui,
        egui::pos2(rect.min.x + 44.0 * s + icon_size, dot_y),
        egui::Align2::LEFT_CENTER,
        &item.title,
        (if front { TEXT_BODY } else { TEXT_CAPTION }) * s,
        if front { p.text } else { p.muted },
    );

    let body = egui::Rect::from_min_size(
        egui::pos2(rect.min.x, rect.min.y + tb),
        egui::vec2(rect.width(), rect.height() - tb),
    );
    ui.painter().rect_filled(
        body.shrink(1.0),
        CornerRadius::same(Radius::of(Radius::SM)),
        if front {
            p.inset
        } else {
            Palette::with_alpha(p.inset, 160)
        },
    );
    if front {
        let preview = egui::Rect::from_min_size(
            body.min + egui::vec2(10.0 * s, 8.0 * s),
            egui::vec2(body.width() - 20.0 * s, body.height() - 30.0 * s),
        );
        draw_window_wire(ui, preview, item, screen, p);
        if let Some(label) = size_label(item.geom) {
            text_at(
                ui,
                egui::pos2(rect.max.x - 10.0 * s, body.max.y - 7.0 * s),
                egui::Align2::RIGHT_CENTER,
                label,
                TEXT_CAPTION * s,
                p.accent,
            );
        }
    }
}

/// 01 · Tira de miniaturas del MRU: la seleccionada crece, las demás se atenúan.
fn draw_window_filmstrip(
    ui: &mut egui::Ui,
    state: &mut UiState,
    safe: egui::Rect,
    screen: egui::Rect,
    p: Palette,
) {
    ui.painter().rect_filled(
        safe,
        CornerRadius::same(Radius::of(Radius::NONE)),
        Palette::scrim(176),
    );
    let all = items(state);
    if all.is_empty() {
        hint_bar(ui, safe, &[("Esc", "cierra")]);
        return;
    }
    state.switcher.selected = state.switcher.selected.min(all.len() - 1);
    let idx = state.switcher.selected;
    let (start, end) = window_range(all.len(), idx, 5);
    let s = switcher_unit(screen);
    let (small_w, small_h) = (76.0 * s, 100.0 * s);
    let (big_w, big_h) = (132.0 * s, 134.0 * s);
    let gap = 8.0 * s;
    let count = end - start;
    let total = count as f32 * small_w + (big_w - small_w) + (count.saturating_sub(1)) as f32 * gap;
    let mut x = safe.center().x - total / 2.0;
    let bottom = safe.center().y + big_h / 2.0;

    let mut hits = Vec::new();
    let mut picked = None;
    let mut double = false;
    for i in start..end {
        let selected = i == idx;
        let item = &all[i];
        let w = if selected { big_w } else { small_w };
        let h = if selected { big_h } else { small_h };
        let rect = egui::Rect::from_min_size(egui::pos2(x, bottom - h), egui::vec2(w, h));
        x += w + gap;
        hits.push((i, rect));

        drop_shadow(ui, rect, 10, 8, 22, 90);
        ui.painter().rect(
            rect,
            CornerRadius::same(Radius::of(if selected { Radius::LG } else { Radius::CARD })),
            if selected {
                p.accent_soft
            } else {
                Palette::with_alpha(p.raised, 235)
            },
            Stroke::new(
                if selected { 1.5 } else { 1.0 },
                if selected { p.accent } else { p.border },
            ),
            StrokeKind::Middle,
        );
        let preview = egui::Rect::from_min_size(
            rect.min + egui::vec2(8.0 * s, 8.0 * s),
            egui::vec2(w - 16.0 * s, h - 42.0 * s),
        );
        draw_window_wire(ui, preview, item, screen, p);
        text_at(
            ui,
            egui::pos2(rect.center().x, rect.max.y - 20.0 * s),
            egui::Align2::CENTER_CENTER,
            &item.title,
            TEXT_CAPTION * s,
            if selected { p.text } else { p.muted },
        );
        text_at(
            ui,
            egui::pos2(rect.center().x, rect.max.y - 8.0 * s),
            egui::Align2::CENTER_CENTER,
            &item.subtitle,
            TEXT_CAPTION * s,
            p.muted,
        );

        let resp = ui.interact(
            rect,
            ui.scope_id().with(("sw-film", i)),
            egui::Sense::click(),
        );
        if resp.clicked() {
            picked = Some(i);
        }
        if resp.double_clicked() {
            picked = Some(i);
            double = true;
        }
    }

    if let Some(i) = picked {
        state.switcher.selected = i;
        if double {
            activate(state);
        }
    } else {
        let clicks = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
        let hover = ui.input(|i| i.pointer.hover_pos());
        if clicks
            && hover
                .is_some_and(|pos| safe.contains(pos) && !hits.iter().any(|(_, r)| r.contains(pos)))
        {
            close(state);
        }
    }
    hint_bar(
        ui,
        safe,
        &[("← →", "elige"), ("↵", "abre"), ("Esc", "cierra")],
    );
}

/// 02 · Rejilla de ventanas agrupada por workspace.
fn draw_window_grid(
    ui: &mut egui::Ui,
    state: &mut UiState,
    safe: egui::Rect,
    screen: egui::Rect,
    p: Palette,
) {
    ui.painter().rect_filled(
        safe,
        CornerRadius::same(Radius::of(Radius::NONE)),
        Palette::scrim(176),
    );
    let all = items(state);
    if all.is_empty() {
        hint_bar(ui, safe, &[("Esc", "cierra")]);
        return;
    }
    state.switcher.selected = state.switcher.selected.min(all.len() - 1);
    let idx = state.switcher.selected;
    let s = switcher_unit(screen);
    text_at(
        ui,
        egui::pos2(safe.center().x, safe.min.y + 22.0 * s),
        egui::Align2::CENTER_CENTER,
        "VENTANAS · AGRUPADO POR WORKSPACE",
        TEXT_CAPTION * s,
        p.muted,
    );

    let all_groups = group_items(&all);
    let sel_ws = all[idx].ws.unwrap_or(0);
    let mut groups = all_groups.iter().take(3).cloned().collect::<Vec<_>>();
    if !groups.iter().any(|(ws, _)| *ws == sel_ws) {
        if groups.len() == 3 {
            groups.pop();
        }
        if let Some(group) = all_groups.iter().find(|(ws, _)| *ws == sel_ws) {
            groups.push(group.clone());
        }
    }

    let tile_w = 104.0 * s;
    let tile_h = 74.0 * s;
    let gap = 8.0 * s;
    let mut y = safe.min.y + 44.0 * s;
    let mut hits = Vec::new();

    for (ws, indices) in &groups {
        let name = state
            .shared
            .config
            .workspaces
            .name_of(*ws as u32)
            .unwrap_or("");
        let header = if name.is_empty() {
            format!("{ws:02}")
        } else {
            format!("{ws:02} {name}")
        };
        text_at(
            ui,
            egui::pos2(safe.min.x + 30.0 * s, y),
            egui::Align2::LEFT_CENTER,
            format!(
                "{} · {} ventana{}",
                header,
                indices.len(),
                if indices.len() == 1 { "" } else { "s" }
            ),
            TEXT_CAPTION * s,
            p.muted,
        );
        y += 14.0 * s;

        let first = indices
            .iter()
            .position(|&i| i == idx)
            .unwrap_or(0)
            .saturating_sub(4);
        let shown = &indices[first..indices.len().min(first + 5)];
        let total = shown.len() as f32 * tile_w + (shown.len().saturating_sub(1)) as f32 * gap;
        let mut x = safe.center().x - total / 2.0;
        for (n, &i) in shown.iter().enumerate() {
            let selected = i == idx;
            let item = &all[i];
            let rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(tile_w, tile_h));
            x += tile_w + gap;
            hits.push((i, rect));
            ui.painter().rect(
                rect,
                CornerRadius::same(Radius::of(Radius::CARD)),
                if selected {
                    p.accent_soft
                } else {
                    Palette::with_alpha(p.card_fill, 225)
                },
                Stroke::new(
                    if selected { 1.5 } else { 1.0 },
                    if selected { p.accent } else { p.border },
                ),
                StrokeKind::Middle,
            );
            let preview = egui::Rect::from_min_size(
                rect.min + egui::vec2(6.0 * s, 6.0 * s),
                egui::vec2(tile_w - 12.0 * s, 38.0 * s),
            );
            draw_window_wire(ui, preview, item, screen, p);
            text_at(
                ui,
                egui::pos2(rect.center().x, rect.max.y - 6.0 * s),
                egui::Align2::CENTER_CENTER,
                &item.title,
                TEXT_CAPTION * s,
                if selected { p.text } else { p.muted },
            );
            text_at(
                ui,
                egui::pos2(rect.max.x - 6.0 * s, rect.min.y + 7.0 * s),
                egui::Align2::RIGHT_CENTER,
                (n + 1).to_string(),
                TEXT_CAPTION * s,
                p.faint,
            );
        }
        y += tile_h + 12.0 * s;
    }

    let mut picked = None;
    let mut double = false;
    for (i, rect) in &hits {
        let resp = ui.interact(
            *rect,
            ui.scope_id().with(("sw-grid", *i)),
            egui::Sense::click(),
        );
        if resp.clicked() {
            picked = Some(*i);
        }
        if resp.double_clicked() {
            picked = Some(*i);
            double = true;
        }
    }
    if let Some(i) = picked {
        state.switcher.selected = i;
        if double {
            activate(state);
        }
    } else {
        let clicks = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
        let hover = ui.input(|i| i.pointer.hover_pos());
        if clicks
            && hover
                .is_some_and(|pos| safe.contains(pos) && !hits.iter().any(|(_, r)| r.contains(pos)))
        {
            close(state);
        }
    }
    hint_bar(
        ui,
        safe,
        &[("← → ↑ ↓", "elige"), ("↵", "abre"), ("Esc", "cierra")],
    );
}

/// 03 · Lista MRU a la izquierda y vista previa de la ventana a la derecha.
fn draw_window_list(
    ui: &mut egui::Ui,
    state: &mut UiState,
    safe: egui::Rect,
    screen: egui::Rect,
    p: Palette,
) {
    ui.painter().rect_filled(
        safe,
        CornerRadius::same(Radius::of(Radius::NONE)),
        Palette::scrim(176),
    );
    let all = items(state);
    if all.is_empty() {
        hint_bar(ui, safe, &[("Esc", "cierra")]);
        return;
    }
    state.switcher.selected = state.switcher.selected.min(all.len() - 1);
    let idx = state.switcher.selected;
    let (start, end) = window_range(all.len(), idx, 6);
    let s = switcher_unit(screen);
    let list_w = (safe.width() * 0.55).min(520.0 * s);
    let preview_w = 224.0 * s;
    let gap = 16.0 * s;
    let left = safe.center().x - (list_w + gap + preview_w) / 2.0;
    let row_h = 36.0 * s;
    let row_gap = 5.0 * s;
    let rows_h = (end - start) as f32 * (row_h + row_gap) - row_gap;
    let top = safe.center().y - rows_h / 2.0;

    let mut picked = None;
    let mut double = false;
    for (j, i) in (start..end).enumerate() {
        let selected = i == idx;
        let item = &all[i];
        let rect = egui::Rect::from_min_size(
            egui::pos2(left, top + j as f32 * (row_h + row_gap)),
            egui::vec2(list_w, row_h),
        );
        ui.painter().rect(
            rect,
            CornerRadius::same(Radius::of(Radius::CARD)),
            if selected {
                p.accent_soft
            } else {
                Palette::sheen(6)
            },
            Stroke::new(
                if selected { 1.5 } else { 1.0 },
                if selected { p.accent } else { p.border },
            ),
            StrokeKind::Middle,
        );
        draw_app_icon(
            ui,
            state,
            &item.class,
            egui::pos2(left + 20.0 * s, rect.center().y),
            16.0 * s,
            p,
        );
        text_at(
            ui,
            egui::pos2(left + 36.0 * s, rect.center().y),
            egui::Align2::LEFT_CENTER,
            &item.title,
            TEXT_BODY * s,
            if selected { p.text } else { p.muted },
        );
        text_at(
            ui,
            egui::pos2(rect.max.x - 10.0 * s, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            &item.subtitle,
            TEXT_CAPTION * s,
            p.muted,
        );
        let resp = ui.interact(
            rect,
            ui.scope_id().with(("sw-list", i)),
            egui::Sense::click(),
        );
        if resp.clicked() {
            picked = Some(i);
        }
        if resp.double_clicked() {
            picked = Some(i);
            double = true;
        }
    }

    let preview = egui::Rect::from_min_size(
        egui::pos2(left + list_w + gap, safe.center().y - 93.0 * s),
        egui::vec2(preview_w, 186.0 * s),
    );
    ui.painter().rect(
        preview,
        CornerRadius::same(Radius::of(Radius::LG)),
        p.card_fill,
        Stroke::new(1.5, p.accent),
        StrokeKind::Middle,
    );
    let item = &all[idx];
    draw_app_icon(
        ui,
        state,
        &item.class,
        egui::pos2(preview.center().x, preview.min.y + 26.0 * s),
        30.0 * s,
        p,
    );
    text_at(
        ui,
        egui::pos2(preview.center().x, preview.min.y + 58.0 * s),
        egui::Align2::CENTER_CENTER,
        &item.title,
        TEXT_BODY * s,
        p.text,
    );
    text_at(
        ui,
        egui::pos2(preview.center().x, preview.min.y + 74.0 * s),
        egui::Align2::CENTER_CENTER,
        &item.subtitle,
        TEXT_CAPTION * s,
        p.muted,
    );
    let wire = egui::Rect::from_min_size(
        egui::pos2(preview.min.x + 12.0 * s, preview.min.y + 90.0 * s),
        egui::vec2(preview_w - 24.0 * s, 70.0 * s),
    );
    draw_window_wire(ui, wire, item, screen, p);
    if let Some(label) = size_label(item.geom) {
        text_at(
            ui,
            egui::pos2(preview.max.x - 10.0 * s, preview.max.y - 10.0 * s),
            egui::Align2::RIGHT_CENTER,
            label,
            TEXT_CAPTION * s,
            p.accent,
        );
    }

    if let Some(i) = picked {
        state.switcher.selected = i;
        if double {
            activate(state);
        }
    } else {
        let clicks = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
        let hover = ui.input(|i| i.pointer.hover_pos());
        if clicks
            && hover.is_some_and(|pos| {
                safe.contains(pos)
                    && !(start..end).enumerate().any(|(j, _)| {
                        egui::Rect::from_min_size(
                            egui::pos2(left, top + j as f32 * (row_h + row_gap)),
                            egui::vec2(list_w, row_h),
                        )
                        .contains(pos)
                    })
                    && !preview.contains(pos)
            })
        {
            close(state);
        }
    }
    hint_bar(
        ui,
        safe,
        &[("↑ ↓", "elige"), ("↵", "abre"), ("Esc", "cierra")],
    );
}

/// 04 · Tres capas escaladas en profundidad; la activa al frente.
fn draw_window_stack(
    ui: &mut egui::Ui,
    state: &mut UiState,
    safe: egui::Rect,
    screen: egui::Rect,
    p: Palette,
) {
    ui.painter().rect_filled(
        safe,
        CornerRadius::same(Radius::of(Radius::NONE)),
        Palette::scrim(176),
    );
    let all = items(state);
    if all.is_empty() {
        hint_bar(ui, safe, &[("Esc", "cierra")]);
        return;
    }
    state.switcher.selected = state.switcher.selected.min(all.len() - 1);
    let idx = state.switcher.selected;
    let (start, end) = window_range(all.len(), idx, 3);
    let s = switcher_unit(screen);
    let mut shown = (start..end).collect::<Vec<_>>();
    // Orden estable: la seleccionada se dibuja la última (al frente).
    shown.sort_by_key(|&i| i == idx);
    let base = egui::Rect::from_center_size(safe.center(), egui::vec2(300.0 * s, 190.0 * s));
    let mut hits = Vec::new();
    let mut full_rects = Vec::new();

    for (layer, &i) in shown.iter().enumerate() {
        let front = layer == shown.len() - 1;
        let back = (shown.len() - 1 - layer) as f32;
        let shrink = 1.0 - back * 0.05;
        let rect = egui::Rect::from_center_size(
            base.center() + egui::vec2(0.0, back * 11.0 * s),
            egui::vec2(base.width() * shrink, base.height() * shrink),
        );
        // Las capas traseras asoman por abajo: solo ahí son clicables.
        let hit = if front {
            rect
        } else {
            egui::Rect::from_min_size(
                egui::pos2(rect.min.x, rect.max.y - 11.0 * s),
                egui::vec2(rect.width(), 11.0 * s),
            )
        };
        hits.push((i, hit));
        full_rects.push(rect);
        let item = &all[i];
        drop_shadow(ui, rect, 10, 8, 22, if front { 90 } else { 55 });
        ui.painter().rect(
            rect,
            CornerRadius::same(Radius::of(Radius::LG)),
            if front {
                p.accent_soft
            } else {
                Palette::with_alpha(p.card_fill, 210)
            },
            Stroke::new(
                if front { 1.5 } else { 1.0 },
                if front { p.accent } else { p.border },
            ),
            StrokeKind::Middle,
        );
        draw_app_icon(
            ui,
            state,
            &item.class,
            egui::pos2(rect.center().x, rect.min.y + 24.0 * s),
            28.0 * s,
            p,
        );
        text_at(
            ui,
            egui::pos2(rect.center().x, rect.min.y + 52.0 * s),
            egui::Align2::CENTER_CENTER,
            &item.title,
            (if front { TEXT_DISPLAY_SM } else { TEXT_CAPTION }) * s,
            if front { p.text } else { p.muted },
        );
        text_at(
            ui,
            egui::pos2(rect.center().x, rect.min.y + 70.0 * s),
            egui::Align2::CENTER_CENTER,
            &item.subtitle,
            TEXT_CAPTION * s,
            p.muted,
        );
        if front {
            let wire = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + 24.0 * s, rect.min.y + 92.0 * s),
                egui::vec2(rect.width() - 48.0 * s, 60.0 * s),
            );
            draw_window_wire(ui, wire, item, screen, p);
        }
    }

    text_at(
        ui,
        egui::pos2(safe.max.x - 24.0 * s, safe.min.y + 22.0 * s),
        egui::Align2::RIGHT_CENTER,
        format!("{:02} / {:02}", idx + 1, all.len()),
        TEXT_CAPTION * s,
        p.muted,
    );

    let mut picked = None;
    let mut double = false;
    for (i, rect) in &hits {
        let resp = ui.interact(
            *rect,
            ui.scope_id().with(("sw-stack", *i)),
            egui::Sense::click(),
        );
        if resp.clicked() {
            picked = Some(*i);
        }
        if resp.double_clicked() {
            picked = Some(*i);
            double = true;
        }
    }
    if let Some(i) = picked {
        state.switcher.selected = i;
        if double {
            activate(state);
        }
    } else {
        let clicks = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
        let hover = ui.input(|i| i.pointer.hover_pos());
        if clicks
            && hover.is_some_and(|pos| {
                safe.contains(pos) && !full_rects.iter().any(|r| r.contains(pos))
            })
        {
            close(state);
        }
    }
    hint_bar(
        ui,
        safe,
        &[("← →", "elige"), ("↵", "abre"), ("Esc", "cierra")],
    );
}

/// 05 · La propia ventana es la tarjeta, sin ninguna referencia a workspace.
fn draw_window_only(
    ui: &mut egui::Ui,
    state: &mut UiState,
    safe: egui::Rect,
    screen: egui::Rect,
    p: Palette,
) {
    ui.painter().rect_filled(
        safe,
        CornerRadius::same(Radius::of(Radius::NONE)),
        Palette::scrim(176),
    );
    let all = items(state);
    if all.is_empty() {
        hint_bar(ui, safe, &[("Esc", "cierra")]);
        return;
    }
    state.switcher.selected = state.switcher.selected.min(all.len() - 1);
    let idx = state.switcher.selected;
    let (start, end) = window_range(all.len(), idx, 3);
    let hero = idx - start;
    let s = switcher_unit(screen);
    let center = safe.center();
    let front_rect = egui::Rect::from_center_size(center, egui::vec2(260.0 * s, 158.0 * s));
    let flank_w = 138.0 * s;
    let flank_h = 112.0 * s;
    let flank_off = 130.0 * s + 12.0 * s + flank_w / 2.0;

    let mut hits = Vec::new();
    let mut picked = None;
    let mut double = false;
    for (j, i) in (start..end).enumerate() {
        let front = j == hero;
        let rect = if front {
            front_rect
        } else {
            let side = if j < hero { -1.0 } else { 1.0 };
            egui::Rect::from_center_size(
                egui::pos2(center.x + side * flank_off, center.y),
                egui::vec2(flank_w, flank_h),
            )
        };
        hits.push((i, rect));
        drop_shadow(ui, rect, 10, 8, 22, 90);
        ui.painter().rect(
            rect,
            CornerRadius::same(Radius::of(Radius::LG)),
            if front {
                p.accent_soft
            } else {
                Palette::with_alpha(p.card_fill, 225)
            },
            Stroke::new(
                if front { 1.5 } else { 1.0 },
                if front { p.accent } else { p.border },
            ),
            StrokeKind::Middle,
        );
        draw_window_tile(ui, state, rect, &all[i], screen, p, s, front);

        let resp = ui.interact(
            rect,
            ui.scope_id().with(("sw-window", i)),
            egui::Sense::click(),
        );
        if resp.clicked() {
            picked = Some(i);
        }
        if resp.double_clicked() {
            picked = Some(i);
            double = true;
        }
    }

    if let Some(i) = picked {
        state.switcher.selected = i;
        if double {
            activate(state);
        }
    } else {
        let clicks = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
        let hover = ui.input(|i| i.pointer.hover_pos());
        if clicks
            && hover
                .is_some_and(|pos| safe.contains(pos) && !hits.iter().any(|(_, r)| r.contains(pos)))
        {
            close(state);
        }
    }
    hint_bar(
        ui,
        safe,
        &[("← →", "elige"), ("↵", "abre"), ("Esc", "cierra")],
    );
}

/// Capas de la cascada y su desplazamiento, medidos a 720p.
const CASCADE_LAYERS: usize = 4;
const CASCADE_W: f32 = 300.0;
const CASCADE_H: f32 = 126.0;
const CASCADE_DX: f32 = 28.0;
const CASCADE_DY: f32 = 16.0;

/// Rects de la pila, de atrás hacia delante, centrados en `center`.
fn cascade_rects(center: egui::Pos2, layers: usize, s: f32) -> Vec<egui::Rect> {
    let w = CASCADE_W * s;
    let h = CASCADE_H * s;
    let dx = CASCADE_DX * s;
    let dy = CASCADE_DY * s;
    let span = (layers.saturating_sub(1) as f32) * dx;
    let span_y = (layers.saturating_sub(1) as f32) * dy;
    let left = center.x - w / 2.0 - span / 2.0;
    let top = center.y - h / 2.0 - span_y / 2.0;
    (0..layers)
        .map(|i| {
            egui::Rect::from_min_size(
                egui::pos2(left + i as f32 * dx, top + i as f32 * dy),
                egui::vec2(w, h),
            )
        })
        .collect()
}

/// Etiqueta puramente de ventana: `1280 × 720`. Sin workspace, sin nombre.
fn size_label(geom: Option<(i32, i32, i32, i32)>) -> Option<String> {
    geom.map(|(_, _, w, h)| format!("{w} × {h}"))
}

/// Cascada del MRU: cada ventana se apila en diagonal y la elegida queda al
/// frente. Solo usa `class`, `título` y geometría; deliberadamente sin workspace.
fn draw_window_cascade(
    ui: &mut egui::Ui,
    state: &mut UiState,
    safe: egui::Rect,
    screen: egui::Rect,
    p: Palette,
) {
    ui.painter().rect_filled(
        safe,
        CornerRadius::same(Radius::of(Radius::NONE)),
        Palette::scrim(176),
    );
    let all = items(state);
    if all.is_empty() {
        hint_bar(ui, safe, &[("Esc", "cierra")]);
        return;
    }
    state.switcher.selected = state.switcher.selected.min(all.len() - 1);
    let idx = state.switcher.selected;

    let take = all.len().min(CASCADE_LAYERS);
    let start = idx.saturating_sub(take - 1).min(all.len() - take);
    let mut shown: Vec<(usize, Item)> =
        (start..start + take).map(|i| (i, all[i].clone())).collect();
    // Orden estable: la seleccionada se dibuja la última (al frente).
    shown.sort_by_key(|(i, _)| *i == idx);

    let s = switcher_unit(screen);
    let rects = cascade_rects(safe.center(), shown.len(), s);
    let tb = WINDOW_TB * s;
    let mut picked = None;
    let mut double = false;

    for (layer, ((original, item), rect)) in shown.iter().zip(rects.iter()).enumerate() {
        let front = layer == shown.len() - 1;
        let fill = if front {
            p.accent_soft
        } else {
            Palette::with_alpha(p.card_fill, 225)
        };
        drop_shadow(
            ui,
            rect.translate(egui::vec2(0.0, 2.0)),
            10,
            8,
            22,
            if front { 90 } else { 55 },
        );
        ui.painter().rect(
            *rect,
            CornerRadius::same(Radius::of(if front { Radius::LG } else { Radius::MD })),
            fill,
            Stroke::new(
                if front { 1.5 } else { 1.0 },
                if front { p.accent } else { p.border },
            ),
            StrokeKind::Middle,
        );

        // Barra de título: tres puntos, icono y título completo.
        let titlebar = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), tb));
        ui.painter().rect_filled(
            titlebar,
            CornerRadius::same(Radius::of(Radius::MD)),
            Palette::sheen(if front { 10 } else { 6 }),
        );
        ui.painter().line_segment(
            [
                egui::pos2(rect.min.x, rect.min.y + tb),
                egui::pos2(rect.max.x, rect.min.y + tb),
            ],
            Stroke::new(1.0, p.border),
        );
        let dot_y = rect.min.y + tb / 2.0;
        let dot_x = rect.min.x + 9.0 * s;
        let dot_r = 2.0 * s;
        ui.painter()
            .circle_filled(egui::pos2(dot_x, dot_y), dot_r, p.accent);
        for n in 1..3 {
            ui.painter().circle_filled(
                egui::pos2(dot_x + n as f32 * 8.0 * s, dot_y),
                dot_r,
                p.faint,
            );
        }
        let icon_size = 12.0 * s;
        let icon_center = egui::pos2(rect.min.x + 32.0 * s + icon_size / 2.0, dot_y);
        if !item.class.is_empty() {
            if let Some(texture) = state.shared.icon(ui.ctx(), &item.class) {
                egui::Image::new(egui::load::SizedTexture::new(
                    texture.id(),
                    egui::vec2(icon_size, icon_size),
                ))
                .paint_at(
                    ui,
                    egui::Rect::from_center_size(icon_center, egui::vec2(icon_size, icon_size)),
                );
            } else {
                icon_at(ui, icon_center, dock_icon(&item.class), icon_size, p.text);
            }
        } else {
            icon_at(ui, icon_center, ic::SQUARES_FOUR, icon_size, p.text);
        }
        text_at(
            ui,
            egui::pos2(rect.min.x + 44.0 * s + icon_size, dot_y),
            egui::Align2::LEFT_CENTER,
            &item.title,
            (if front { TEXT_BODY } else { TEXT_CAPTION }) * s,
            if front { p.text } else { p.muted },
        );

        // Cuerpo: solo la ventana activa muestra su geometría real.
        let body = egui::Rect::from_min_size(
            egui::pos2(rect.min.x, rect.min.y + tb),
            egui::vec2(rect.width(), rect.height() - tb),
        );
        ui.painter().rect_filled(
            body.shrink(1.0),
            CornerRadius::same(Radius::of(Radius::SM)),
            if front {
                p.inset
            } else {
                Palette::with_alpha(p.inset, 160)
            },
        );
        if front {
            let preview = egui::Rect::from_min_size(
                body.min + egui::vec2(10.0 * s, 8.0 * s),
                egui::vec2(body.width() - 20.0 * s, body.height() - 30.0 * s),
            );
            draw_window_wire(ui, preview, item, screen, p);
            if let Some(label) = size_label(item.geom) {
                text_at(
                    ui,
                    egui::pos2(rect.max.x - 10.0 * s, body.max.y - 7.0 * s),
                    egui::Align2::RIGHT_CENTER,
                    label,
                    TEXT_CAPTION * s,
                    p.accent,
                );
            }
        }

        // El frente es clicable entero; las capas detrás solo por la franja visible.
        let hit = if front {
            *rect
        } else {
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), CASCADE_DY * s))
        };
        let resp = ui.interact(
            hit,
            ui.scope_id().with(("sw-cascade", original)),
            egui::Sense::click(),
        );
        if resp.clicked() {
            picked = Some(*original);
        }
        if resp.double_clicked() {
            picked = Some(*original);
            double = true;
        }
    }

    if let Some(i) = picked {
        state.switcher.selected = i;
        if double {
            activate(state);
        }
    } else {
        let clicks = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
        let hover = ui.input(|i| i.pointer.hover_pos());
        if clicks
            && hover.is_some_and(|pos| safe.contains(pos) && !rects.iter().any(|r| r.contains(pos)))
        {
            close(state);
        }
    }
    hint_bar(
        ui,
        safe,
        &[("← →", "sube capa"), ("↵", "abre"), ("Esc", "cierra")],
    );
}

/// Miniatura de la geometría real de la ventana (`ClientInfo.x/y/w/h`).
fn draw_window_wire(
    ui: &mut egui::Ui,
    preview: egui::Rect,
    item: &Item,
    screen: egui::Rect,
    p: Palette,
) {
    let round = CornerRadius::same(Radius::of(Radius::XS));
    ui.painter().rect_filled(preview, round, p.inset);
    ui.painter().rect(
        preview,
        round,
        egui::Color32::TRANSPARENT,
        Stroke::new(1.0, p.border),
        StrokeKind::Middle,
    );
    let Some(wire) = wire_rect(preview, item, screen) else {
        return;
    };
    ui.painter().rect(
        wire,
        round,
        Palette::with_alpha(p.accent, 60),
        Stroke::new(1.0, p.accent),
        StrokeKind::Middle,
    );
}

#[cfg(feature = "switcher")]
pub(super) struct SwitcherComponent;

#[cfg(feature = "switcher")]
impl Component for SwitcherComponent {
    fn name(&self) -> &'static str {
        "switcher"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, _ctx: DrawCtx, palette: Palette) {
        if state.switcher.visible() {
            draw(ui, state, palette);
        }
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        if state.switcher.visible() {
            vec![super::safe_rect_ints(state, screen_w, screen_h)]
        } else {
            Vec::new()
        }
    }

    fn update(&mut self, state: &mut UiState, ctx: &UpdateCtx) -> bool {
        let before = state.switcher.progress;
        if state.switcher.open {
            state.switcher.age += ctx.dt;
            // Nunca retener el teclado en Exclusive si no llega la soltura del
            // modificador (p. ej. switcher abierto por CLI o evento perdido).
            if state.switcher.age >= IDLE_TIMEOUT {
                close(state);
            }
        }
        let s = &mut state.switcher;
        s.progress = approach(s.progress, if s.open { 1.0 } else { 0.0 }, ctx.dt * 9.0);
        (s.progress - before).abs() > 0.0005
    }

    /// El switcher NO pide teclado: en esta máquina Hyprland no suelta bien el
    /// foco Exclusive de un layer surface. Las teclas las maneja Hyprland
    /// (binds repetibles) y la confirmación llega como comando `switcher-confirm`.
    fn wants_keyboard(&self, _state: &UiState) -> bool {
        false
    }

    fn on_key(
        &mut self,
        state: &mut UiState,
        key: smithay_client_toolkit::seat::keyboard::Keysym,
    ) -> bool {
        handle_key(state, key)
    }

    fn on_command(&mut self, command: &str, state: &mut UiState) -> bool {
        use control::Command;
        if command == "switcher-confirm" {
            if state.switcher.open {
                activate(state);
            }
            return true;
        }
        let (kind, backwards) = match control::Command::parse(command) {
            Some(Command::WindowSwitcher) => (SwitcherKind::Windows, false),
            Some(Command::WindowSwitcherPrev) => (SwitcherKind::Windows, true),
            Some(Command::WorkspaceSwitcher) => (SwitcherKind::Workspaces, false),
            Some(Command::WorkspaceSwitcherPrev) => (SwitcherKind::Workspaces, true),
            _ => return false,
        };
        if state.switcher.open && state.switcher.kind == Some(kind) {
            cycle(state, backwards);
        } else {
            open(state, kind, backwards);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactividad_cierra_y_no_retiene_el_teclado() {
        let mut state = UiState::new_preview();
        open(&mut state, SwitcherKind::Windows, false);
        assert!(state.switcher.open);
        let ctx = UpdateCtx {
            dt: 1.0,
            screen_w: 1920,
            screen_h: 1080,
            cursor: None,
        };
        let mut component = super::SwitcherComponent;
        for _ in 0..4 {
            component.update(&mut state, &ctx);
        }
        assert!(!state.switcher.open, "tras 3 s de inactividad se cierra");
        assert!(!component.wants_keyboard(&state));
    }

    #[test]
    fn mru_orders_windows_from_shared_state() {
        let mut state = UiState::new_preview();
        state.shared.touch_window("a");
        state.shared.touch_window("b");
        state.shared.touch_window("a");
        assert_eq!(state.shared.mru, vec!["a", "b"]);
        state.shared.clients = vec![
            crate::ipc::ClientInfo {
                class: "A".into(),
                title: "a".into(),
                address: "a".into(),
                mapped: true,
                workspace: 1,
                x: 0,
                y: 0,
                w: 10,
                h: 10,
            },
            crate::ipc::ClientInfo {
                class: "B".into(),
                title: "b".into(),
                address: "b".into(),
                mapped: true,
                workspace: 2,
                x: 0,
                y: 0,
                w: 10,
                h: 10,
            },
        ];
        state.switcher.kind = Some(SwitcherKind::Windows);
        let keys: Vec<String> = items(&state).into_iter().map(|i| i.key).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn windows_sin_mru_incluyen_todas_las_ventanas_mapeadas() {
        let mut state = UiState::new_preview();
        let client = |address: &str, mapped: bool| crate::ipc::ClientInfo {
            class: address.to_string(),
            title: address.to_string(),
            address: address.to_string(),
            mapped,
            workspace: 1,
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        };
        state.shared.clients = vec![client("a", true), client("b", true), client("c", false)];
        state.switcher.kind = Some(SwitcherKind::Windows);
        let keys: Vec<String> = items(&state).into_iter().map(|i| i.key).collect();
        assert_eq!(
            keys,
            vec!["a", "b"],
            "solo las mapeadas, aunque el MRU esté vacío"
        );
    }

    #[test]
    fn el_switcher_no_robe_el_teclado_y_confirma_por_comando() {
        let mut state = UiState::new_preview();
        state.shared.snapshot.occupied = vec![1, 2];
        let mut component = super::SwitcherComponent;
        assert!(!component.wants_keyboard(&state));
        component.on_command("workspace-switcher", &mut state);
        assert!(state.switcher.open);
        assert!(
            !component.wants_keyboard(&state),
            "ni abierto toma el teclado"
        );
        component.on_command("switcher-confirm", &mut state);
        assert!(!state.switcher.open);
        assert_eq!(state.shared.pending_commands.len(), 1);
    }

    #[test]
    fn workspaces_se_ordenan_por_visita_reciente() {
        let mut state = UiState::new_preview();
        state.shared.snapshot.occupied = vec![1, 2, 3, 5];
        state.shared.snapshot.workspace = "3".into();
        state.shared.touch_workspace(5);
        state.shared.touch_workspace(2);
        state.switcher.kind = Some(SwitcherKind::Workspaces);
        let keys: Vec<String> = items(&state).into_iter().map(|i| i.key).collect();
        assert_eq!(keys, vec!["2", "5", "1", "3"]);
    }

    #[test]
    fn switcher_unit_follows_the_smaller_screen_ratio() {
        let screen = |w, h| egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, h));
        assert!((switcher_unit(screen(1920.0, 1080.0)) - 1.5).abs() < 0.001);
        assert!((switcher_unit(screen(1366.0, 768.0)) - 768.0 / 720.0).abs() < 0.001);
        assert!((switcher_unit(screen(3840.0, 2160.0)) - 3.0).abs() < 0.001);
        // En ultrawide manda la altura: el diseño no se estira.
        assert!((switcher_unit(screen(3440.0, 1080.0)) - 1.5).abs() < 0.001);
    }

    #[test]
    fn wire_rect_preserves_window_aspect_ratio() {
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1920.0, 1080.0));
        let preview = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(260.0, 56.0));
        let item = Item {
            key: "a".into(),
            title: String::new(),
            subtitle: String::new(),
            class: String::new(),
            geom: Some((0, 0, 1920, 1080)),
            ws: None,
        };
        let wire = wire_rect(preview, &item, screen).unwrap();
        assert!((wire.width() / wire.height() - 16.0 / 9.0).abs() < 0.01);
        assert!(preview.contains_rect(wire));
    }

    #[test]
    fn window_range_keeps_selection_visible_and_in_bounds() {
        assert_eq!(window_range(0, 0, 5), (0, 0));
        assert_eq!(window_range(8, 0, 5), (0, 5));
        assert_eq!(window_range(8, 4, 5), (2, 7));
        assert_eq!(window_range(8, 7, 5), (3, 8));
        assert_eq!(window_range(2, 1, 5), (0, 2));
    }

    #[test]
    fn group_items_splits_by_workspace_in_mru_order() {
        let item = |ws: i32| Item {
            key: ws.to_string(),
            title: String::new(),
            subtitle: String::new(),
            class: String::new(),
            geom: None,
            ws: Some(ws),
        };
        let all = vec![item(2), item(1), item(2), item(3)];
        assert_eq!(
            group_items(&all),
            vec![(2, vec![0, 2]), (1, vec![1]), (3, vec![3])]
        );
    }

    #[test]
    fn cascade_rects_form_a_centered_diagonal_stack() {
        let rects = cascade_rects(egui::pos2(0.0, 0.0), CASCADE_LAYERS, 1.0);
        assert_eq!(rects.len(), CASCADE_LAYERS);
        assert_eq!(rects[1].min.x - rects[0].min.x, CASCADE_DX);
        assert_eq!(rects[1].min.y - rects[0].min.y, CASCADE_DY);
        let union = rects.iter().fold(rects[0], |acc, rect| acc.union(*rect));
        assert!(union.center().x.abs() < 0.001);
        assert!(union.center().y.abs() < 0.001);
    }

    #[test]
    fn size_label_is_pure_window_geometry() {
        assert_eq!(
            size_label(Some((12, 34, 1280, 720))).as_deref(),
            Some("1280 × 720")
        );
        assert_eq!(size_label(None), None);
    }

    #[test]
    fn cycle_wraps_and_skips_when_empty() {
        let mut state = UiState::new_preview();
        state.switcher.kind = Some(SwitcherKind::Workspaces);
        state.shared.snapshot.occupied = vec![1, 2, 3];
        cycle(&mut state, false);
        assert_eq!(state.switcher.selected, 1);
        cycle(&mut state, true);
        assert_eq!(state.switcher.selected, 0);
    }

    #[test]
    fn release_confirms_only_after_the_modifier_was_held() {
        use smithay_client_toolkit::seat::keyboard::Modifiers;

        let mut state = UiState::new_preview();
        state.shared.snapshot.occupied = vec![1, 2];
        open(&mut state, SwitcherKind::Workspaces, false);

        // Un evento de modifiers ajeno no confirma si nunca vimos Super pulsado.
        on_modifiers(&mut state, Modifiers::default());
        assert!(state.switcher.open);
        assert!(state.shared.pending_commands.is_empty());

        on_modifiers(
            &mut state,
            Modifiers {
                logo: true,
                ..Default::default()
            },
        );
        on_modifiers(&mut state, Modifiers::default());
        assert!(!state.switcher.open);
        assert_eq!(
            state.shared.pending_commands.last().map(String::as_str),
            Some("workspace 1")
        );
    }

    #[test]
    fn repeated_tab_cycles_without_reopening() {
        let mut state = UiState::new_preview();
        state.shared.snapshot.occupied = vec![1, 2, 3];
        let mut component = SwitcherComponent;
        assert!(component.on_command("workspace-switcher", &mut state));
        assert!(component.on_command("workspace-switcher", &mut state));
        assert_eq!(state.switcher.selected, 1);
        assert!(state.switcher.open);
    }
}
