//! Switchers de ventana (Alt+Tab) y workspace (Super+Tab): rejilla compacta
//! sobre el estado que ya tiene el shell, sin IPC nuevo.

use std::collections::HashMap;

use smithay_client_toolkit::seat::keyboard::Modifiers;

use super::component::{close_overlays, Component, DrawCtx};
use super::overlay::hint_bar;
use super::*;
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
            state
                .shared
                .mru
                .iter()
                .filter_map(|address| by_address.get(address.as_str()).copied())
                .filter(|c| c.mapped)
                .map(|c| Item {
                    key: c.address.clone(),
                    title: truncate(&c.title, 22),
                    subtitle: format!("workspace {}", c.workspace),
                    class: c.class.clone(),
                    geom: Some((c.x, c.y, c.w, c.h)),
                })
                .collect()
        }
        Some(SwitcherKind::Workspaces) => {
            let mut ids = state.shared.snapshot.occupied.clone();
            let current = overview::current_ws(state);
            if !ids.contains(&current) {
                ids.push(current);
            }
            ids.sort_unstable();
            ids.dedup();
            ids.into_iter()
                .take(20)
                .map(|id| Item {
                    key: id.to_string(),
                    geom: None,
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
        _ => draw_window_cards(ui, state, safe, screen, p),
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
    let gap = Space::MD * 2.0;
    let hero_r = egui::Rect::from_center_size(center, egui::vec2(360.0, 216.0));
    let flank_w = 180.0;
    let flank_h = 152.0;
    let flank_off = 180.0 + gap + flank_w / 2.0;

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
        let rect = rect.translate(egui::vec2(0.0, -2.0 * lift));
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

        let icon_size = if is_hero {
            ICON_XL * 2.0
        } else {
            ICON_XL * 1.6
        };
        let icon_center = egui::pos2(
            rect.center().x,
            rect.min.y + if is_hero { 46.0 } else { 34.0 },
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
                rect.min.y + if is_hero { 96.0 } else { 83.0 },
            ),
            egui::Align2::CENTER_CENTER,
            &item.title,
            if is_hero { TEXT_DISPLAY_SM } else { TEXT_BODY },
            p.text,
        );
        text_at(
            ui,
            egui::pos2(
                rect.center().x,
                rect.min.y + if is_hero { 120.0 } else { 104.0 },
            ),
            egui::Align2::CENTER_CENTER,
            &item.subtitle,
            TEXT_CAPTION,
            p.muted,
        );
        if is_hero {
            draw_window_wire(ui, hero_r, item, screen, p);
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

/// Miniatura de la geometría real de la ventana (`ClientInfo.x/y/w/h`).
fn draw_window_wire(
    ui: &mut egui::Ui,
    hero: egui::Rect,
    item: &Item,
    screen: egui::Rect,
    p: Palette,
) {
    let preview = egui::Rect::from_min_size(
        egui::pos2(hero.min.x + 50.0, hero.min.y + 142.0),
        egui::vec2(260.0, 56.0),
    );
    let round = CornerRadius::same(Radius::of(Radius::XS));
    ui.painter().rect_filled(preview, round, p.inset);
    ui.painter().rect(
        preview,
        round,
        egui::Color32::TRANSPARENT,
        Stroke::new(1.0, p.border),
        StrokeKind::Middle,
    );
    let Some((x, y, w, h)) = item.geom else {
        return;
    };
    let sx = preview.width() / screen.width().max(1.0);
    let sy = preview.height() / screen.height().max(1.0);
    let wire = egui::Rect::from_min_size(
        egui::pos2(preview.min.x + x as f32 * sx, preview.min.y + y as f32 * sy),
        egui::vec2((w as f32 * sx).max(6.0), (h as f32 * sy).max(5.0)),
    );
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
        let s = &mut state.switcher;
        let before = s.progress;
        s.progress = approach(s.progress, if s.open { 1.0 } else { 0.0 }, ctx.dt * 9.0);
        (s.progress - before).abs() > 0.0005
    }

    fn wants_keyboard(&self, state: &UiState) -> bool {
        state.switcher.open
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
