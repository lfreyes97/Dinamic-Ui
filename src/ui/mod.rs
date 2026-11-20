//! La UI de egui: una "isla" compacta que se expande a barra y panel.
//! Colores: base16 Black Metal (o Bathory) desde `theme.rs`.

use chrono::{Datelike, Local};
use egui::{Color32, CornerRadius, CursorIcon, Frame, Margin, RichText, Stroke, StrokeKind};
use egui_phosphor::regular as ic;

use crate::{
    config,
    ipc::HyprlandSnapshot,
    notifications::Notification,
    power::BatteryState,
    sysinfo::{SystemInfo, Wallpaper},
    theme::Palette,
};

mod bar;
mod dock;
mod menu;
mod notifications;
mod overview;
pub mod panel;

use bar::*;
use dock::*;
use menu::*;
use notifications::*;
use overview::*;
use panel::*;

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

pub struct UiState {
    /// 0.0 = isla, 1.0 = barra.
    pub expansion: f32,
    pub expanded: bool,
    pub show_panel: bool,
    /// Progreso del panel (0 cerrado, 1 abierto), animado por `main`.
    pub panel_progress: f32,
    /// Destello breve cuando cambia el workspace/ventana.
    pub pulse: f32,
    pub notification: Option<Notification>,
    pub notification_count: usize,
    pub notification_progress: f32,
    pub dismiss_notification: bool,
    /// Pestaña activa del panel de control.
    pub panel_tab: usize,
    pub dnd: bool,
    pub notification_list: Vec<Notification>,
    pub dismiss_notification_id: Option<u32>,
    pub clear_all_notifications: bool,
    pub toggle_dnd: bool,
    pub wallpaper_color: Option<String>,
    /// Fondo marcado en la rejilla de Wallpaper (solo visual; el cambio real va por `wallpaper_color`).
    pub wallpaper_selected: Option<usize>,
    /// Imagen marcada en la lista de Wallpaper (optimista; `main` la sincroniza
    /// con `awww query`) y petición pendiente de aplicarla con `awww img`.
    pub wallpaper_image: Option<std::path::PathBuf>,
    /// Imagen bajo el puntero en la galería: manda en el preview grande.
    pub wallpaper_hover: Option<std::path::PathBuf>,
    pub wallpaper_image_request: Option<std::path::PathBuf>,
    /// Vista de la galería: false = lista, true = grid de miniaturas.
    pub wallpaper_grid: bool,
    /// Imágenes de la carpeta configurada, con la carpeta y el instante del último escaneo.
    pub wallpaper_files: Vec<std::path::PathBuf>,
    pub wallpaper_scan: Option<(std::path::PathBuf, std::time::Instant)>,
    /// Generador de miniaturas (hilo propio) y sus texturas ya subidas a egui.
    pub thumbs: crate::thumbs::Thumbnails,
    pub thumb_textures: std::collections::HashMap<std::path::PathBuf, egui::TextureHandle>,
    /// Preset de layout marcado en la pestaña Workspace. `main` lo sincroniza
    /// con `general:layout` de Hyprland; el clic lo aplica con `hyprctl eval`.
    pub layout_preset: usize,
    pub battery: BatteryState,
    /// Wi-Fi, Bluetooth, niveles y dispositivos (hilo de `sysinfo.rs`).
    pub system: SystemInfo,
    /// Comandos externos (argv) que `main` lanza sin esperar: `hyprctl eval`, `xdg-open`…
    pub pending_spawns: Vec<Vec<String>>,
    pub snapshot: HyprlandSnapshot,
    pub should_exit: bool,
    /// Comandos `dispatch` pendientes de enviar al IPC de Hyprland.
    pub pending_commands: Vec<String>,
    /// Command menu (lanzador / archivos / …).
    pub menu_open: bool,
    pub menu_mode: crate::launcher::MenuMode,
    pub menu_query: String,
    pub menu_selected: usize,
    pub menu_entries: Vec<crate::launcher::Entry>,
    pub menu_action: Option<crate::launcher::EntryAction>,
    /// Modo overview (vista de todos los workspaces).
    pub overview: bool,
    pub overview_focus: Option<String>,
    pub overview_workspace: Option<u32>,
    /// Isla tipo notch: visible unos px, se despliega al acercar el cursor.
    pub island_progress: f32,
    pub island_active: bool,
    /// Compacto enganchado por clic (el hover es temporal).
    pub island_compact: bool,
    /// Dock inferior con autohide.
    pub dock_progress: f32,
    pub dock_visible: bool,
    pub dock_clients: Vec<crate::ipc::ClientInfo>,
    pub dock_action: Option<DockAction>,
    /// Ítem del dock cuyo menú de ventanas está abierto.
    pub dock_menu: Option<usize>,
    /// Iconos reales de apps del dock (desde .desktop + temas de iconos).
    pub app_icons: crate::appicons::AppIcons,
    pub app_icon_textures: std::collections::HashMap<String, egui::TextureHandle>,
}

#[derive(Debug, Clone)]
pub enum DockAction {
    Focus(String),
    Launch(String),
}

impl UiState {
    pub fn new() -> Self {
        Self {
            expansion: 0.0,
            expanded: std::env::var_os("DINAMIC_UI_EXPANDED").is_some(),
            show_panel: std::env::var_os("DINAMIC_UI_PANEL").is_some(),
            panel_progress: if std::env::var_os("DINAMIC_UI_PANEL").is_some() {
                1.0
            } else {
                0.0
            },
            pulse: 0.0,
            notification: None,
            notification_count: 0,
            notification_progress: 0.0,
            dismiss_notification: false,
            panel_tab: std::env::var("DINAMIC_UI_PANEL_TAB")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0)
                .min(PANEL_TABS.len() - 1),
            dnd: false,
            notification_list: Vec::new(),
            dismiss_notification_id: None,
            clear_all_notifications: false,
            toggle_dnd: false,
            wallpaper_color: None,
            wallpaper_selected: None,
            wallpaper_image: None,
            wallpaper_hover: None,
            wallpaper_image_request: None,
            wallpaper_grid: false,
            wallpaper_files: Vec::new(),
            wallpaper_scan: None,
            thumbs: crate::thumbs::Thumbnails::new(),
            thumb_textures: std::collections::HashMap::new(),
            layout_preset: 0,
            battery: BatteryState::default(),
            system: SystemInfo::default(),
            pending_spawns: Vec::new(),
            snapshot: HyprlandSnapshot::default(),
            should_exit: false,
            pending_commands: Vec::new(),
            menu_open: false,
            menu_mode: crate::launcher::MenuMode::default(),
            menu_query: String::new(),
            menu_selected: 0,
            menu_entries: Vec::new(),
            menu_action: None,
            overview: false,
            overview_focus: None,
            overview_workspace: None,
            island_progress: 0.0,
            island_active: std::env::var_os("DINAMIC_UI_ISLAND").is_some(),
            island_compact: false,
            dock_progress: if std::env::var_os("DINAMIC_UI_DOCK").is_some() {
                1.0
            } else {
                0.0
            },
            dock_visible: std::env::var_os("DINAMIC_UI_DOCK").is_some(),
            dock_clients: Vec::new(),
            dock_action: None,
            dock_menu: None,
            app_icons: crate::appicons::AppIcons::new(),
            app_icon_textures: std::collections::HashMap::new(),
        }
    }
}

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

pub fn draw(
    ui: &mut egui::Ui,
    state: &mut UiState,
    palette: Palette,
    t: f32,
    panel_t: f32,
    pill_w: f32,
) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::TRANSPARENT;
    visuals.window_fill = Color32::TRANSPARENT;
    visuals.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
    ui.ctx().set_visuals(visuals);

    // El borde se enciende con decaimiento suave tras un evento de Hyprland.
    let glow = state.pulse * state.pulse;
    let accent = palette.accent;
    let border = if glow > 0.01 {
        let a = (70.0 + 150.0 * glow).min(255.0) as u8;
        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), a)
    } else {
        palette.border
    };

    // El surface es ancho completo y fijo; los elementos se dibujan centrados.
    let screen = ui.max_rect();
    // Caída con aceleración (gravedad), no lineal: la gota cae y se posa.
    let p = state.island_progress;
    let island_t = p * p;
    let in_island_mode = t < 0.5;
    let pill = if in_island_mode {
        island_rect(screen.width(), island_t)
    } else {
        egui::Rect::from_min_size(
            egui::Pos2::new(((screen.width() - pill_w) / 2.0).round(), 0.0),
            egui::Vec2::new(pill_w.round(), metrics().bar_h as f32),
        )
    };
    let radius = (pill.height() / 2.0).round().min(16.0) as u8;
    // En modo isla las esquinas superiores son rectas (tocan el borde de la
    // pantalla); solo se redondean las inferiores.
    let corners = if in_island_mode {
        CornerRadius {
            nw: 0,
            ne: 0,
            sw: radius,
            se: radius,
        }
    } else {
        CornerRadius::same(radius)
    };
    ui.painter().rect(
        pill,
        corners,
        palette.surface,
        Stroke::new(1.0 + glow * 1.5, border),
        StrokeKind::Middle,
    );

    let inner = egui::Rect::from_min_max(
        pill.min + egui::Vec2::new(BAR_INSET_X, 5.0),
        pill.max - egui::Vec2::new(BAR_INSET_X, 5.0),
    );
    let header_h = metrics().bar_h as f32 - 8.0;
    let header = egui::Rect::from_min_size(
        inner.min,
        egui::Vec2::new(inner.width(), header_h.min(inner.height())),
    );

    // Crossfade isla <-> barra sobre el mismo rectángulo.
    let island_alpha = 1.0 - smoothstep(0.15, 0.75, t);
    let bar_alpha = smoothstep(0.25, 0.85, t);

    if island_alpha > 0.01 && in_island_mode {
        if island_t > 0.1 {
            // Icono + hora con más presencia, agrupados y centrados.
            let now = Local::now().format("%H:%M").to_string();
            let icon_pos = pill.center() - egui::vec2(30.0, 0.0);
            let time_pos = pill.center() + egui::vec2(8.0, 0.0);
            ui.painter().text(
                icon_pos,
                egui::Align2::CENTER_CENTER,
                ic::CLOCK,
                icon_font(ICON_MD),
                palette.accent,
            );
            text_at(
                ui,
                time_pos,
                egui::Align2::CENTER_CENTER,
                now,
                TEXT_DISPLAY_SM,
                palette.text,
            );
        }
        // Un punto de acento en el notch en reposo.
        if island_t < 0.5 {
            ui.painter().circle_filled(
                pill.center(),
                1.4,
                palette.accent.gamma_multiply(0.5 + island_t),
            );
        }
        // Clic: notch/compacto -> compacto enganchado -> barra.
        let response = ui.interact(
            pill,
            ui.scope_id().with(egui::IdSalt::new("island-click")),
            egui::Sense::click(),
        );
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if response.clicked() {
            if state.island_compact {
                state.expanded = true;
                state.island_compact = false;
            } else {
                state.island_compact = true;
            }
        }
    }
    if bar_alpha > 0.01 {
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header)
                .layout(egui::Layout::left_to_right(egui::Align::Center))
                .id_salt("bar-layer"),
        );
        child.set_opacity(bar_alpha);
        draw_bar_row(&mut child, state, palette);
    }

    // Notificación: tarjeta propia, separada de la barra. Se oculta al abrir el panel.
    let notif_t = smoothstep(0.0, 1.0, state.notification_progress);
    if notif_t > 0.01 && panel_t < 0.5 && state.notification.is_some() {
        // Slide in desde arriba / slide out hacia arriba: la tarjeta se desplaza
        // desde detrás de la barra hasta su sitio (y viceversa al salir).
        let base = notification_card(screen.width());
        let slide = (1.0 - notif_t) * (metrics().bar_h as f32 + PANEL_GAP + base.height());
        let card = base.translate(egui::vec2(0.0, -slide));
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(card)
                .id_salt("notification-layer"),
        );
        child.set_clip_rect(card.intersect(ui.clip_rect()));
        child.set_opacity(smoothstep(0.0, 0.6, state.notification_progress));
        draw_notification_card(&mut child, card, state, palette);
    }

    if panel_t > 0.01 {
        // Tarjeta independiente, centrada bajo la píldora y tan ancha como el modo.
        let card_w = panel_width(t).round();
        let card_h = metrics().panel_h as f32 * panel_t;
        let card = egui::Rect::from_min_size(
            egui::Pos2::new(
                ((screen.width() - card_w) / 2.0).round(),
                metrics().bar_h as f32 + PANEL_GAP,
            ),
            egui::Vec2::new(card_w, card_h),
        );
        let card_fill = Color32::from_rgba_unmultiplied(8, 8, 8, 245);
        let card_border = Color32::from_rgba_unmultiplied(41, 41, 41, 255);
        ui.painter().rect(
            card,
            CornerRadius::same(18),
            card_fill,
            Stroke::new(1.0, card_border),
            StrokeKind::Middle,
        );
        let inner_min = card.min + egui::Vec2::new(PANEL_PAD_X, PANEL_PAD_Y);
        // Al abrir, la tarjeta mide menos que el padding: sin rect invertido.
        let panel_inner = egui::Rect::from_min_max(
            inner_min,
            (card.max - egui::Vec2::new(PANEL_PAD_X, PANEL_PAD_Y)).max(inner_min),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(panel_inner)
                .id_salt("panel-layer"),
        );
        // Durante la animación de apertura el contenido no sale de la tarjeta.
        child.set_clip_rect(card.intersect(ui.clip_rect()));
        child.set_opacity(panel_t);
        draw_panel(&mut child, state, palette);
    }

    draw_dock(ui, state, palette);
    if state.overview {
        draw_overview(ui, state, palette);
    }
    if state.menu_open {
        draw_command_menu(ui, state, palette);
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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
    let active = state.show_panel;
    let (fg, bg) = if active {
        (Color32::from_gray(17), palette.text)
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
        state.show_panel = !state.show_panel;
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

/// Color de la batería según nivel: verde / ámbar / rojo, azul si cargando.
fn battery_tone(percent: u8, charging: bool) -> Color32 {
    if charging {
        return Color32::from_rgb(0x7f, 0xc8, 0xff);
    }
    match percent {
        0..=20 => Color32::from_rgb(0xff, 0x7b, 0x72),
        21..=60 => Color32::from_rgb(0xf5, 0xc2, 0x6b),
        _ => Color32::from_rgb(0x8b, 0xe0, 0x9b),
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
        P_SURFACE,
        Stroke::new(1.0, P_BORDER),
    );
    let color = if response.hovered() {
        P_TEXT
    } else {
        P_HEADING
    };
    icon_at(ui, rect.center(), icon, ICON_SECONDARY, color);
    response
}

/// Caja redondeada con un icono (`.item-icon`, `.device-icon`, `.app-icon`…).
fn icon_box_at(ui: &egui::Ui, rect: egui::Rect, icon: &str, fill: Color32) {
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    icon_at(ui, rect.center(), icon, ICON_MD, Color32::from_gray(0xdd));
}

fn card_at(ui: &egui::Ui, rect: egui::Rect, radius: u8) {
    ui.painter().rect(
        rect,
        CornerRadius::same(radius),
        P_SURFACE,
        Stroke::new(1.0, P_BORDER),
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
        P_HEADING,
    );
    text_at(
        ui,
        egui::Pos2::new(rect.max.x - 1.0, y),
        egui::Align2::RIGHT_CENTER,
        right,
        TEXT_BODY,
        P_DIM,
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
        P_HEADING,
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
    ui.painter().rect_filled(rect, 0.0, P_BORDER);
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
    ui.painter().hline(
        rect.x_range(),
        rect.min.y,
        Stroke::new(1.0, Color32::from_gray(0x24)),
    );
    let y = rect.min.y + 14.0;
    text_at(
        ui,
        egui::Pos2::new(rect.min.x + 2.0, y),
        egui::Align2::LEFT_CENTER,
        left,
        TEXT_CAPTION,
        P_DIM,
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
        P_TEXT
    } else {
        Color32::from_gray(0x99)
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
    icon_box_at(ui, icon_rect, icon, P_ICON_BOX);
    let x = icon_rect.max.x + 10.0;
    text_at(
        ui,
        egui::Pos2::new(x, rect.center().y - 6.0),
        egui::Align2::LEFT_CENTER,
        title,
        TEXT_TITLE,
        P_TEXT,
    );
    text_at(
        ui,
        egui::Pos2::new(x, rect.center().y + 8.0),
        egui::Align2::LEFT_CENTER,
        subtitle,
        TEXT_CAPTION,
        P_MUTED,
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
