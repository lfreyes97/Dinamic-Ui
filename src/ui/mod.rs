//! La UI de egui: una "isla" compacta que se expande a barra y panel.
//! Colores: base16 Black Metal (o Bathory) desde `theme.rs`.

use chrono::{Datelike, Local};
#[allow(unused_imports)]
use egui::{Color32, CornerRadius, CursorIcon, Frame, Margin, RichText, Stroke, StrokeKind};
use egui_phosphor::regular as ic;

use crate::{config, theme::Palette};

#[cfg(feature = "bar")]
mod bar;
mod bar_state;
mod component;
#[cfg(all(test, feature = "overview", feature = "activities", feature = "topbar"))]
mod preview;
// Datos de ejemplo: los comparten las vistas previas PNG y el previewer interactivo.
#[cfg(all(
    test,
    feature = "overview",
    feature = "activities",
    feature = "topbar",
    feature = "config-panel"
))]
mod preview_config;
#[cfg(all(
    test,
    feature = "overview",
    feature = "activities",
    feature = "topbar",
    feature = "dashboard"
))]
mod preview_dashboard;
#[cfg(any(test, feature = "previewer"))]
pub mod sample;

pub use component::{
    dispatch_command, dispatch_key, input_rects, needs, update_components, wants_keyboard,
    UpdateCtx,
};

#[cfg(feature = "bar")]
#[allow(unused_imports)] // lo usan los módulos hermanos vía `use super::*`
use panel::*;
#[cfg(feature = "activities")]
mod activities;
#[cfg(feature = "activities")]
mod activities_model;
#[cfg(feature = "config-panel")]
mod config_panel;
#[cfg(feature = "dashboard")]
mod dashboard;
#[cfg(feature = "dock")]
mod dock;
#[cfg(feature = "command-menu")]
mod menu;
#[cfg(feature = "notifications")]
mod notifications;
// Piezas comunes de los overlays a pantalla completa.
#[cfg(any(feature = "overview", feature = "activities"))]
mod nav;
#[cfg(any(feature = "overview", feature = "activities"))]
mod overlay;
#[cfg(feature = "overview")]
mod overview;
#[cfg(feature = "overview")]
mod overview_model;
#[cfg(feature = "overview")]
mod overview_strips;
#[cfg(feature = "bar")]
pub mod panel;
#[cfg(feature = "taskbar")]
mod taskbar;
#[cfg(feature = "topbar")]
mod topbar;
#[cfg(feature = "wallpaper")]
pub mod wallpaper;

/// Ancho mínimo de la tarjeta del panel (con la isla) y separación respecto a la barra.
/// Con la barra expandida el panel crece hasta `bar_w`: ver `panel_width`.
pub const PANEL_W: u32 = 390;
pub const PANEL_GAP: f32 = 8.0;

/// Isla tipo notch: unos px en reposo, crece al acercar el cursor (solo reloj).
pub const NOTCH_W: f32 = 56.0;
/// Solo una franja fina queda visible en reposo, como el notch del teléfono.
pub const NOTCH_H: f32 = 5.0;
pub const ISLAND_ACTIVE_W: f32 = 128.0;
pub const ISLAND_ACTIVE_H: f32 = 32.0;

pub fn island_rect(screen_width: f32, progress: f32) -> egui::Rect {
    let w = NOTCH_W + (ISLAND_ACTIVE_W - NOTCH_W) * progress;
    let h = NOTCH_H + (ISLAND_ACTIVE_H - NOTCH_H) * progress;
    egui::Rect::from_min_size(
        egui::pos2(((screen_width - w) / 2.0).round(), 0.0),
        egui::vec2(w, h),
    )
}

/// Tarjeta de notificación: más corta que el panel, centrada bajo la barra.
const NOTIF_W: f32 = 320.0;
const NOTIF_H: f32 = 68.0;

/// Rect de la tarjeta de notificación en coordenadas del surface.
pub fn notification_card(screen_width: f32) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(
            ((screen_width - NOTIF_W) / 2.0).round(),
            metrics().bar_h as f32 + PANEL_GAP,
        ),
        egui::vec2(NOTIF_W, NOTIF_H),
    )
}

/// Ancho de la tarjeta del panel: `PANEL_W` con la isla y `bar_w` con la barra
/// (nunca menos de `PANEL_W`), siguiendo la animación de expansión `t` (0..1).
pub fn panel_width(t: f32) -> f32 {
    let from = PANEL_W as f32;
    let to = (metrics().bar_w as f32).max(from);
    from + (to - from) * t
}
/// Padding interior de la tarjeta del panel. El horizontal aloja además la
/// barra de scroll (ver `PANEL_SCROLLBAR`), así el contenido conserva 356 px.
const PANEL_PAD_X: f32 = 17.0;
const PANEL_PAD_Y: f32 = 10.0;
/// Hueco reservado a la derecha del contenido para la barra de scroll
/// (4 px de barra + 6 px de separación), dentro del padding derecho.
const PANEL_SCROLLBAR: f32 = 10.0;

#[derive(Clone, Copy)]
pub struct Metrics {
    pub bar_h: u32,
    pub panel_h: u32,
    pub island_w: u32,
    pub bar_w: u32,
    /// Alto fijo del layer surface: la píldora crece por dentro.
    pub surface_h: u32,
}

// ── Geometría de la barra (reloj en el centro geométrico) ───────────────
/// Margen horizontal entre el borde de la píldora y su contenido.
const BAR_INSET_X: f32 = 12.0;
/// Mitad del ancho reservado al reloj, y distancia de los separadores al centro.
const BAR_CLOCK_HALF: f32 = 34.0;
const BAR_SEP_OFFSET: f32 = BAR_CLOCK_HALF + SP_M;
/// Ancho de la zona derecha (batería, wifi, audio).
const BAR_RIGHT_W: f32 = 124.0;
/// Botones de workspace: tamaño preferido, mínimo (si no caben) y separación.
const WS_MAX: f32 = 20.0;
const WS_MIN: f32 = 14.0;
const WS_GAP: f32 = 3.0;
/// Botón del panel de control.
const OPTIONS_W: f32 = 22.0;

/// Ancho de píldora con el que `count` workspaces de tamaño preferido caben en
/// la zona izquierda (que llega hasta el reloj centrado) sin encogerse.
fn min_bar_w(count: u32) -> u32 {
    let count = count.max(1) as f32;
    let left = count * WS_MAX + (count - 1.0) * WS_GAP + SP_M + OPTIONS_W;
    (2.0 * (left + BAR_INSET_X + BAR_SEP_OFFSET + SP_M)).ceil() as u32
}

/// Margen bajo la tarjeta del panel para que el borde (trazo de 1 px) no se recorte.
const PANEL_BOTTOM_MARGIN: u32 = 2;

/// Medidas desde `config.toml` (los env vars ya se aplican al cargar la config).
pub fn metrics() -> Metrics {
    let cfg = config::get();
    Metrics {
        bar_h: cfg.sizes.bar_h,
        panel_h: cfg.sizes.panel_h,
        island_w: cfg.sizes.island_w,
        // Nunca más estrecha de lo que necesitan los workspaces configurados.
        bar_w: cfg.sizes.bar_w.max(min_bar_w(cfg.workspaces.count)),
        // La tarjeta empieza en `bar_h + PANEL_GAP` y mide `panel_h`: el surface
        // tiene que cubrir también el hueco, o se cortaban los últimos 8 px.
        surface_h: cfg.sizes.bar_h + PANEL_GAP as u32 + cfg.sizes.panel_h + PANEL_BOTTOM_MARGIN,
    }
}

/// Estado de la UI. Conserva solo datos compartidos; el estado de cada
/// componente vive en su módulo (`bar`, `dock`, `menu`, `notifications`, ...).
pub struct UiState {
    pub bar: bar_state::BarState,
    #[cfg(feature = "dock")]
    pub dock: dock::DockState,
    #[cfg(feature = "taskbar")]
    pub taskbar: taskbar::TaskbarState,
    #[cfg(feature = "topbar")]
    pub topbar: topbar::TopbarState,
    #[cfg(feature = "command-menu")]
    pub menu: menu::MenuState,
    #[cfg(feature = "notifications")]
    pub notifications: notifications::NotificationState,
    #[cfg(feature = "overview")]
    pub overview: overview::OverviewState,
    #[cfg(feature = "activities")]
    pub activities: activities::ActivitiesState,
    #[cfg(feature = "dashboard")]
    pub dashboard: dashboard::DashboardState,
    #[cfg(feature = "config-panel")]
    pub config_panel: config_panel::ConfigPanelState,
    #[cfg(feature = "wallpaper")]
    pub wallpaper: wallpaper::WallpaperState,

    pub shared: crate::shared::Shared,
    /// El proceso debe terminar (lo decide `main`, no un componente).
    pub should_exit: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

impl UiState {
    pub fn new() -> Self {
        Self::with_notifications(true)
    }

    /// Como `new`, pero sin el servidor D-Bus: el previewer no debe reclamar el
    /// nombre `org.freedesktop.Notifications` de la sesión.
    pub fn new_preview() -> Self {
        Self::with_notifications(false)
    }

    fn with_notifications(#[allow(unused_variables)] online: bool) -> Self {
        Self {
            bar: bar_state::BarState::new(),
            #[cfg(feature = "dock")]
            dock: dock::DockState::new(),
            #[cfg(feature = "taskbar")]
            taskbar: taskbar::TaskbarState::new(),
            #[cfg(feature = "topbar")]
            topbar: topbar::TopbarState::new(),
            #[cfg(feature = "command-menu")]
            menu: menu::MenuState::default(),
            #[cfg(feature = "notifications")]
            notifications: if online {
                notifications::NotificationState::new()
            } else {
                notifications::NotificationState::new_offline()
            },
            #[cfg(feature = "overview")]
            overview: overview::OverviewState::default(),
            #[cfg(feature = "activities")]
            activities: activities::ActivitiesState::default(),
            #[cfg(feature = "dashboard")]
            dashboard: dashboard::DashboardState::default(),
            #[cfg(feature = "config-panel")]
            config_panel: config_panel::ConfigPanelState::default(),
            #[cfg(feature = "wallpaper")]
            wallpaper: wallpaper::WallpaperState::default(),
            shared: crate::shared::Shared::new(),
            should_exit: false,
        }
    }
}

// Los colores del panel son constantes propias (grises fijos), no la paleta base16.

// ── Escala de tipografía: 4 tamaños de texto + display ──────────────────
/// Etiquetas, metadatos y fechas secundarias.
const TEXT_CAPTION: f32 = 9.0;
/// Texto de cuerpo, valores y botones.
const TEXT_BODY: f32 = 10.5;
/// Títulos de tarjeta y de sección.
const TEXT_TITLE: f32 = 12.0;
/// Reloj de la barra y de la isla.
const TEXT_CLOCK: f32 = 13.0;
/// Valores destacados (porcentaje de batería).
const TEXT_DISPLAY_SM: f32 = 15.0;
/// Porcentaje de batería y hora de la vista previa del fondo.
const TEXT_DISPLAY: f32 = 24.0;
/// Hora grande de la pestaña Clock.
const TEXT_HERO: f32 = 32.0;

// ── Escala de iconos Phosphor ───────────────────────────────────────────
const ICON_XS: f32 = 12.0; // dentro de chips pequeños
const ICON_SM: f32 = 13.0; // botones de cabecera
const ICON_MD: f32 = 14.0; // acción principal y tarjetas
const ICON_LG: f32 = 16.0; // indicadores de sistema
const ICON_XL: f32 = 20.0; // estados vacíos

// ── Escala de espaciado (múltiplos de 2 px) ─────────────────────────────
const SP_S: f32 = 4.0;
const SP_M: f32 = 6.0;
const SP_L: f32 = 8.0;

// Aliases para mantener la intención en el código.
const ICON_ACTION: f32 = ICON_MD;
const ICON_SECONDARY: f32 = ICON_SM;
const ICON_SYSTEM: f32 = ICON_LG;
const FONT_BOLD: &str = "phosphor-bold";
/// Inter SemiBold: `.strong()` no cambia el peso en egui 0.37, solo el color.
const FONT_SEMIBOLD: &str = "inter-semibold";

fn icon_bold(symbol: &str, size: f32, color: Color32) -> RichText {
    RichText::new(symbol).font(icon_font(size)).color(color)
}

/// Fuente Phosphor Bold para pintar iconos directamente con el `Painter`.
fn icon_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(FONT_BOLD.into()))
}

/// Paleta del frame en curso, para los helpers de dibujo.
fn pal() -> Palette {
    crate::theme::current()
}

/// Parámetros de dibujo del frame: expansión de la barra, progreso del panel y
/// ancho de la píldora. Los comparten el shell y el previewer (una sola fuente).
pub fn draw_params(state: &UiState) -> (f32, f32, f32) {
    let t = crate::smootherstep(state.bar.expansion);
    let panel_t = crate::smootherstep(state.bar.panel_progress);
    let m = metrics();
    // La notificación tiene tarjeta propia: la isla no cambia de ancho.
    let pill_w = crate::lerp(m.island_w as f32, m.bar_w as f32, t);
    (t, panel_t, pill_w)
}

pub fn draw(
    ui: &mut egui::Ui,
    state: &mut UiState,
    palette: Palette,
    t: f32,
    panel_t: f32,
    pill_w: f32,
) {
    crate::theme::set_current(palette);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::TRANSPARENT;
    visuals.window_fill = Color32::TRANSPARENT;
    visuals.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
    ui.ctx().set_visuals(visuals);

    let ctx = component::DrawCtx { t, panel_t, pill_w };
    for mut component in component::build_components() {
        component.draw(ui, state, ctx, palette);
    }
}

/// Icono Phosphor según la clase de la ventana (o el comando fijado).
fn dock_icon(name: &str) -> &'static str {
    let n = name.to_lowercase();
    if n.contains("firefox") || n.contains("chrome") || n.contains("chromium") {
        ic::GLOBE
    } else if n.contains("code") || n.contains("codium") {
        ic::CODE
    } else if n.contains("kitty") || n.contains("term") || n.contains("alacritty") {
        ic::TERMINAL
    } else if n.contains("mail") || n.contains("thunderbird") {
        ic::ENVELOPE
    } else if n.contains("spotify") || n.contains("music") {
        ic::MUSIC_NOTES
    } else if n.contains("nautilus") || n.contains("thunar") || n.contains("files") {
        ic::FOLDER
    } else if n.contains("obs") || n.contains("camera") {
        ic::CAMERA
    } else {
        ic::APP_WINDOW
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Chip de control de modo (▲/▼). Con hover y cursor, con tokens del tema.
/// Devuelve `true` si se hizo clic; el llamador decide qué comando despachar.
pub(super) fn mode_chip(
    ui: &egui::Ui,
    rect: egui::Rect,
    icon: &str,
    palette: Palette,
    id: &'static str,
) -> bool {
    let response = ui.interact(
        rect,
        ui.scope_id().with(egui::IdSalt::new(id)),
        egui::Sense::click(),
    );
    let fill = if response.hovered() {
        Palette::sheen(22)
    } else {
        Palette::sheen(10)
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(7),
        fill,
        egui::Stroke::new(1.0, palette.border),
        egui::StrokeKind::Middle,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        icon_font(ICON_XS),
        palette.text,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    response.clicked()
}

fn island_slot(
    ui: &mut egui::Ui,
    id: &'static str,
    rect: egui::Rect,
    layout: egui::Layout,
    content: impl FnOnce(&mut egui::Ui),
) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(layout)
            .id_salt(id),
    );
    content(&mut child);
}

/// Cambia al workspace `n`. Hyprland 0.57 con config en Lua no acepta el
/// `workspace N` antiguo: hay que enviar la expresión Lua del dispatcher.
fn switch_workspace_cmd(n: u32) -> String {
    format!("hl.dsp.focus({{ workspace = {n} }})")
}

/// Color del indicador de workspace ocupado: el acento del tema, oscurecido
/// sobre el fondo claro del workspace activo.
fn occupied_color(palette: Palette, active: bool) -> Color32 {
    if active {
        palette.accent.gamma_multiply(0.55)
    } else {
        palette.accent
    }
}

/// Botón de la cuadrícula: abre/cierra el panel de control. Se usa en la isla
/// y en la barra; resaltado mientras el panel está abierto.
fn panel_toggle_button(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let active = state.bar.show_panel;
    let (fg, bg) = if active {
        (palette.solid(), palette.text)
    } else {
        (palette.text, palette.badge)
    };
    let response = ui
        .add_sized(
            egui::Vec2::new(22.0, 22.0),
            egui::Button::new(icon_bold(ic::SQUARES_FOUR, ICON_ACTION, fg))
                .corner_radius(11.0)
                .fill(bg),
        )
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text("Panel de control");
    if response.clicked() {
        state.bar.show_panel = !state.bar.show_panel;
    }
}

fn truncate(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_string();
    }
    let out: String = chars[..max.saturating_sub(1)].iter().collect();
    format!("{out}...")
}

/// Color de la batería según nivel, con los slots de estado del esquema base16:
/// `ok` / `warn` / `danger`, y `info` si está cargando. En esquemas monocromos (black-metal)
/// esos slots apenas se distinguen por tono: el nivel también se lee por el relleno del glifo.
fn battery_tone(percent: u8, charging: bool) -> Color32 {
    let p = pal();
    if charging {
        return p.info;
    }
    match percent {
        0..=20 => p.danger,
        21..=60 => p.warn,
        _ => p.ok,
    }
}

/// Batería estilo SVG: cuerpo redondeado, terminal y barra de relleno.
fn battery_glyph(ui: &mut egui::Ui, percent: u8, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(18.0, 14.0), egui::Sense::hover());
    battery_glyph_at(
        ui.painter(),
        egui::Pos2::new(rect.min.x + 8.5, rect.center().y),
        percent,
        color,
    );
}

fn battery_glyph_at(painter: &egui::Painter, center: egui::Pos2, percent: u8, color: Color32) {
    // Halo suave: da vida al indicador sin añadir widgets.
    painter.circle_filled(center, 13.0, color.gamma_multiply(0.12));
    let body = egui::Rect::from_center_size(
        egui::Pos2::new(center.x - 1.5, center.y),
        egui::Vec2::new(15.0, 10.0),
    );
    painter.rect_stroke(
        body,
        CornerRadius::same(2),
        Stroke::new(1.5, color),
        StrokeKind::Middle,
    );

    let terminal = egui::Rect::from_min_size(
        egui::Pos2::new(body.max.x, body.center().y - 2.0),
        egui::Vec2::new(2.0, 4.0),
    );
    painter.rect_filled(terminal, CornerRadius::same(1), color);

    let fill_w = (11.0 * (percent.clamp(0, 100) as f32 / 100.0)).max(1.0);
    let fill = egui::Rect::from_min_size(
        body.min + egui::Vec2::new(2.0, 2.0),
        egui::Vec2::new(fill_w, 6.0),
    );
    painter.rect_filled(fill, CornerRadius::same(1), color);
}

/// Sombra suave bajo una tarjeta flotante.
fn drop_shadow(ui: &egui::Ui, rect: egui::Rect, radius: u8, offset: i8, blur: u8, alpha: u8) {
    let shadow = egui::Shadow {
        offset: [0, offset],
        blur,
        spread: 0,
        color: Palette::scrim(alpha),
    };
    ui.painter()
        .add(shadow.as_shape(rect, CornerRadius::same(radius)));
}

/// Icono de batería según el nivel (o «cargando»). Lo comparten el topbar y la taskbar.
fn battery_icon(percent: u8, charging: bool) -> &'static str {
    if charging {
        return ic::BATTERY_CHARGING;
    }
    match percent {
        85..=100 => ic::BATTERY_FULL,
        60..=84 => ic::BATTERY_HIGH,
        35..=59 => ic::BATTERY_MEDIUM,
        10..=34 => ic::BATTERY_LOW,
        _ => ic::BATTERY_EMPTY,
    }
}

/// Icono, color y descripción del Wi-Fi según los datos reales de `sysinfo`.
fn wifi_status(
    system: &crate::sysinfo::SystemInfo,
    palette: Palette,
) -> (&'static str, Color32, String) {
    match &system.wifi {
        None => (ic::WIFI_HIGH, palette.muted, "Wi-Fi: sin datos".to_string()),
        Some(w) if !w.enabled => (
            ic::WIFI_SLASH,
            palette.muted,
            "Wi-Fi desactivado".to_string(),
        ),
        Some(w) => match &w.ssid {
            Some(ssid) => (ic::WIFI_HIGH, palette.text_hi, format!("Wi-Fi: {ssid}")),
            None => (ic::WIFI_X, palette.muted, "Wi-Fi sin conexión".to_string()),
        },
    }
}

/// Icono, color y descripción del audio según los datos reales de `sysinfo`.
fn audio_status(
    system: &crate::sysinfo::SystemInfo,
    palette: Palette,
) -> (&'static str, Color32, String) {
    match &system.volume {
        None => (
            ic::SPEAKER_HIGH,
            palette.muted,
            "Audio: sin datos".to_string(),
        ),
        Some(v) if v.muted => (
            ic::SPEAKER_SLASH,
            palette.muted,
            "Audio silenciado".to_string(),
        ),
        Some(v) => {
            let icon = match v.percent {
                0 => ic::SPEAKER_NONE,
                1..=49 => ic::SPEAKER_LOW,
                _ => ic::SPEAKER_HIGH,
            };
            (icon, palette.text_hi, format!("Volumen {}%", v.percent))
        }
    }
}

/// Fecha corta en español: "Mié. 18 nov."
fn short_date() -> String {
    let now = Local::now();
    const DAYS: [&str; 7] = ["Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom"];
    const MONTHS: [&str; 12] = [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
    ];
    let weekday = DAYS[now.weekday().num_days_from_monday() as usize];
    let month = MONTHS[(now.month() - 1) as usize];
    format!("{weekday}. {} {month}.", now.day())
}

fn text_at(
    ui: &egui::Ui,
    pos: egui::Pos2,
    align: egui::Align2,
    text: impl ToString,
    size: f32,
    color: Color32,
) -> egui::Rect {
    ui.painter().text(
        pos,
        align,
        text.to_string(),
        egui::FontId::proportional(size),
        color,
    )
}

fn icon_at(ui: &egui::Ui, center: egui::Pos2, icon: &str, size: f32, color: Color32) {
    ui.painter().text(
        center,
        egui::Align2::CENTER_CENTER,
        icon,
        icon_font(size),
        color,
    );
}

/// Botón circular con borde.
fn round_icon_button_at(
    ui: &egui::Ui,
    rect: egui::Rect,
    id_salt: &str,
    icon: &str,
) -> egui::Response {
    let response = ui
        .interact(rect, ui.scope_id().with(id_salt), egui::Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    ui.painter().circle(
        rect.center(),
        rect.width() / 2.0 - 0.5,
        pal().solid(),
        Stroke::new(1.0, pal().border),
    );
    let color = if response.hovered() {
        pal().text_hi
    } else {
        pal().muted
    };
    icon_at(ui, rect.center(), icon, ICON_SECONDARY, color);
    response
}

/// Caja redondeada con un icono (`.item-icon`, `.device-icon`, `.app-icon`…).
fn icon_box_at(ui: &egui::Ui, rect: egui::Rect, icon: &str, fill: Color32) {
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    icon_at(ui, rect.center(), icon, ICON_MD, pal().text_hi);
}

fn card_at(ui: &egui::Ui, rect: egui::Rect, radius: u8) {
    ui.painter().rect(
        rect,
        CornerRadius::same(radius),
        pal().solid(),
        Stroke::new(1.0, pal().border),
        StrokeKind::Inside,
    );
}

fn bar_at(ui: &egui::Ui, rect: egui::Rect, fraction: f32, track: Color32, fill: Color32) {
    let radius = CornerRadius::same((rect.height() / 2.0).round() as u8);
    ui.painter().rect_filled(rect, radius, track);
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let filled = egui::Rect::from_min_size(
            rect.min,
            egui::Vec2::new(rect.width() * fraction, rect.height()),
        );
        ui.painter().rect_filled(filled, radius, fill);
    }
}

/// `.section-heading`: título a la izquierda y texto secundario a la derecha.
fn section_heading(ui: &mut egui::Ui, left: &str, right: &str) {
    let (rect, _) = ui.allocate_exact_size(
        egui::Vec2::new(ui.available_width(), 16.0),
        egui::Sense::hover(),
    );
    let y = rect.center().y;
    text_at(
        ui,
        egui::Pos2::new(rect.min.x + 1.0, y),
        egui::Align2::LEFT_CENTER,
        left,
        TEXT_BODY,
        pal().muted,
    );
    text_at(
        ui,
        egui::Pos2::new(rect.max.x - 1.0, y),
        egui::Align2::RIGHT_CENTER,
        right,
        TEXT_BODY,
        pal().dim,
    );
    ui.add_space(SP_M);
}

/// `.section-heading` con un botón redondo a la derecha.
fn section_heading_action(ui: &mut egui::Ui, left: &str, icon: &str) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(
        egui::Vec2::new(ui.available_width(), 20.0),
        egui::Sense::hover(),
    );
    text_at(
        ui,
        egui::Pos2::new(rect.min.x + 1.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        left,
        TEXT_BODY,
        pal().muted,
    );
    let button = egui::Rect::from_min_size(
        egui::Pos2::new(rect.max.x - rect.height(), rect.min.y),
        egui::Vec2::splat(rect.height()),
    );
    let response = round_icon_button_at(ui, button, left, icon);
    ui.add_space(SP_M);
    response
}

fn divider(ui: &mut egui::Ui) {
    ui.add_space(SP_L);
    let (rect, _) = ui.allocate_exact_size(
        egui::Vec2::new(ui.available_width(), 1.0),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(rect, 0.0, pal().border);
    ui.add_space(SP_L);
}

/// `.footer`: línea superior, texto a la izquierda y acción a la derecha.
/// Con `action = false` la acción es solo visual (sin integración); con
/// `true` es clicable y el llamador decide qué hace con la respuesta.
fn panel_footer(
    ui: &mut egui::Ui,
    left: &str,
    right: &str,
    right_is_icon: bool,
    action: bool,
) -> egui::Response {
    ui.add_space(SP_L);
    let (rect, _) = ui.allocate_exact_size(
        egui::Vec2::new(ui.available_width(), 24.0),
        egui::Sense::hover(),
    );
    ui.painter()
        .hline(rect.x_range(), rect.min.y, Stroke::new(1.0, pal().border));
    let y = rect.min.y + 14.0;
    text_at(
        ui,
        egui::Pos2::new(rect.min.x + 2.0, y),
        egui::Align2::LEFT_CENTER,
        left,
        TEXT_CAPTION,
        pal().dim,
    );
    let hit = egui::Rect::from_min_max(
        egui::Pos2::new(
            rect.max.x - if right_is_icon { 22.0 } else { 120.0 },
            rect.min.y + 4.0,
        ),
        rect.max,
    );
    let sense = if action {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let mut response = ui.interact(hit, ui.scope_id().with(("panel-footer", left)), sense);
    if action {
        response = response.on_hover_cursor(CursorIcon::PointingHand);
    }
    let right_color = if action && response.hovered() {
        pal().text_hi
    } else {
        pal().muted
    };
    if right_is_icon {
        icon_at(
            ui,
            egui::Pos2::new(rect.max.x - 8.0, y),
            right,
            ICON_SM,
            right_color,
        );
    } else {
        text_at(
            ui,
            egui::Pos2::new(rect.max.x - 2.0, y),
            egui::Align2::RIGHT_CENTER,
            right,
            TEXT_CAPTION,
            right_color,
        );
    }
    response
}

/// Fila de lista con icono, título y subtítulo (`.list-item` / `.device-row`).
fn list_row_at(ui: &egui::Ui, rect: egui::Rect, icon: &str, title: &str, subtitle: &str) {
    let box_size = 28.0;
    let icon_rect = egui::Rect::from_min_size(
        egui::Pos2::new(rect.min.x, rect.center().y - box_size / 2.0),
        egui::Vec2::splat(box_size),
    );
    icon_box_at(ui, icon_rect, icon, pal().badge);
    let x = icon_rect.max.x + 10.0;
    text_at(
        ui,
        egui::Pos2::new(x, rect.center().y - 6.0),
        egui::Align2::LEFT_CENTER,
        title,
        TEXT_TITLE,
        pal().text_hi,
    );
    text_at(
        ui,
        egui::Pos2::new(x, rect.center().y + 8.0),
        egui::Align2::LEFT_CENTER,
        subtitle,
        TEXT_CAPTION,
        pal().muted,
    );
}

fn text_link(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).size(TEXT_CAPTION).color(color)).frame(false))
        .on_hover_cursor(CursorIcon::PointingHand)
}

/// Reparte `rect` en columnas (o filas) proporcionales con un hueco fijo.
fn split(rect: egui::Rect, weights: &[f32], gap: f32, horizontal: bool) -> Vec<egui::Rect> {
    let total: f32 = weights.iter().sum();
    let length = if horizontal {
        rect.width()
    } else {
        rect.height()
    };
    let free = length - gap * (weights.len() as f32 - 1.0);
    let mut offset = 0.0;
    weights
        .iter()
        .map(|w| {
            let size = free * w / total;
            let r = if horizontal {
                egui::Rect::from_min_size(
                    rect.min + egui::Vec2::new(offset, 0.0),
                    egui::Vec2::new(size, rect.height()),
                )
            } else {
                egui::Rect::from_min_size(
                    rect.min + egui::Vec2::new(0.0, offset),
                    egui::Vec2::new(rect.width(), size),
                )
            };
            offset += size + gap;
            r
        })
        .collect()
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// `true` mientras la taskbar esté desplegada: la barra y el dock se ocultan.
#[cfg(feature = "taskbar")]
pub fn taskbar_active(state: &UiState) -> bool {
    state.taskbar.enabled && state.taskbar.progress > 0.5
}
