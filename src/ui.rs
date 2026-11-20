//! La UI de egui: una "isla" compacta que se expande a barra y panel.
//! Colores: base16 Black Metal (o Bathory) desde `theme.rs`.

use chrono::{Datelike, Local};
use egui::{Color32, CornerRadius, CursorIcon, Frame, Margin, RichText, Stroke, StrokeKind};
use egui_phosphor::regular as ic;

use crate::{
    config, ipc::HyprlandSnapshot, notifications::Notification, power::BatteryState,
    sysinfo::{SystemInfo, Wallpaper}, theme::Palette,
};

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

struct DockItem {
    name: String,
    command: String,
    windows: Vec<crate::ipc::ClientInfo>,
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

/// Command menu: lanzador y buscador, con pestañas de modo.
fn draw_command_menu(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    use crate::launcher::{filter, MenuMode};

    let screen = ui.max_rect();
    ui.painter()
        .rect_filled(screen, CornerRadius::same(0), Color32::from_black_alpha(205));

    let w = 560.0f32.min(screen.width() - 60.0);
    let h = 520.0f32.min(screen.height() - 60.0);
    let card = egui::Rect::from_center_size(screen.center(), egui::vec2(w, h));
    ui.painter().rect(
        card,
        CornerRadius::same(18),
        Color32::from_rgba_unmultiplied(10, 10, 11, 252),
        Stroke::new(1.0, Color32::from_gray(0x2a)),
        StrokeKind::Middle,
    );

    let x = card.min.x + 18.0;
    let mut y = card.min.y;

    // ── Buscador ──
    y += 17.0;
    ui.painter().text(
        egui::pos2(x + 9.0, y + 9.0),
        egui::Align2::CENTER_CENTER,
        ic::MAGNIFYING_GLASS,
        icon_font(ICON_MD),
        palette.muted,
    );
    let query = if state.menu_query.is_empty() {
        "Busca apps, archivos, comandos…".to_string()
    } else {
        state.menu_query.clone()
    };
    ui.painter().text(
        egui::pos2(x + 28.0, y + 9.0),
        egui::Align2::LEFT_CENTER,
        query,
        egui::FontId::new(15.0, egui::FontFamily::Proportional),
        if state.menu_query.is_empty() {
            palette.border
        } else {
            palette.text
        },
    );
    // Cursor parpadeante simple.
    if !state.menu_query.is_empty() {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(x + 30.0 + state.menu_query.len() as f32 * 7.6, y + 2.0),
                egui::vec2(1.5, 14.0),
            ),
            CornerRadius::same(0),
            palette.accent,
        );
    }
    ui.painter().text(
        egui::pos2(card.max.x - 26.0, y + 9.0),
        egui::Align2::RIGHT_CENTER,
        "ESC",
        egui::FontId::new(9.0, egui::FontFamily::Proportional),
        palette.muted,
    );
    y += 34.0;
    ui.painter().line_segment(
        [
            egui::pos2(card.min.x + 1.0, y),
            egui::pos2(card.max.x - 1.0, y),
        ],
        Stroke::new(1.0, Color32::from_gray(0x2a)),
    );

    // ── Modos ──
    y += 11.0;
    let chip_w = (w - 36.0 - 15.0) / 4.0;
    for (i, mode) in MenuMode::ALL.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            egui::pos2(x + i as f32 * (chip_w + 5.0), y),
            egui::vec2(chip_w, 28.0),
        );
        let active = *mode == state.menu_mode;
        if active {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(8), Color32::from_gray(0x1c));
        }
        let label = format!("{}  {}", mode.label(), mode.shortcut());
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::new(10.0, egui::FontFamily::Proportional),
            if active { palette.text } else { palette.muted },
        );
        let r = ui.interact(
            rect,
            ui.scope_id().with(("menu-mode", i)),
            egui::Sense::click(),
        );
        if r.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if r.clicked() {
            state.menu_mode = *mode;
            state.menu_query.clear();
            state.menu_entries.clear();
        }
    }
    y += 40.0;
    ui.painter().line_segment(
        [
            egui::pos2(card.min.x + 1.0, y),
            egui::pos2(card.max.x - 1.0, y),
        ],
        Stroke::new(1.0, Color32::from_gray(0x2a)),
    );

    // ── Resultados ──
    y += 10.0;
    if state.menu_entries.is_empty() {
        state.menu_entries = match state.menu_mode {
            MenuMode::Apps => crate::launcher::apps(),
            MenuMode::Files => crate::launcher::files(&config::get().menu.file_dirs),
            MenuMode::Workspaces => crate::launcher::workspaces(&state.snapshot.occupied),
            MenuMode::Commands => crate::launcher::commands(),
        };
    }
    let results = filter(&state.menu_entries, &state.menu_query);
    let row_h = 48.0;
    let max_rows = ((card.max.y - 52.0 - y) / row_h).floor() as usize;
    for (i, entry) in results.iter().take(max_rows).enumerate() {
        let rect = egui::Rect::from_min_size(
            egui::pos2(card.min.x + 9.0, y + i as f32 * row_h),
            egui::vec2(w - 18.0, row_h - 4.0),
        );
        let selected = i == state.menu_selected;
        if selected {
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(10),
                Color32::from_rgba_unmultiplied(
                    palette.accent.r(),
                    palette.accent.g(),
                    palette.accent.b(),
                    36,
                ),
            );
        }
        let icon_rect = egui::Rect::from_center_size(
            rect.min + egui::vec2(26.0, rect.height() / 2.0),
            egui::vec2(30.0, 30.0),
        );
        if let Some(texture) = state.app_icon_textures.get(&entry.icon_key) {
            egui::Image::new(egui::load::SizedTexture::new(texture.id(), icon_rect.size()))
                .corner_radius(8.0)
                .paint_at(ui, icon_rect);
        } else {
            ui.painter().rect_filled(icon_rect, CornerRadius::same(8), palette.badge);
            let glyph = match entry.icon_key.as_str() {
                "folder" => ic::FOLDER,
                "file" => ic::FILE,
                "workspace" => ic::SQUARES_FOUR,
                _ => ic::APP_WINDOW,
            };
            ui.painter().text(
                icon_rect.center(),
                egui::Align2::CENTER_CENTER,
                glyph,
                icon_font(14.0),
                palette.text,
            );
        }
        ui.painter().text(
            egui::pos2(rect.min.x + 48.0, rect.center().y - 7.0),
            egui::Align2::LEFT_CENTER,
            truncate(&entry.title, 40),
            egui::FontId::new(12.0, egui::FontFamily::Proportional),
            palette.text,
        );
        ui.painter().text(
            egui::pos2(rect.min.x + 48.0, rect.center().y + 7.0),
            egui::Align2::LEFT_CENTER,
            truncate(&entry.subtitle, 52),
            egui::FontId::new(9.5, egui::FontFamily::Proportional),
            palette.muted,
        );
        if selected {
            ui.painter().text(
                egui::pos2(rect.max.x - 14.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                "↵",
                egui::FontId::new(11.0, egui::FontFamily::Proportional),
                palette.accent,
            );
        }
        let r = ui.interact(
            rect,
            ui.scope_id().with(("menu-result", i)),
            egui::Sense::click(),
        );
        if r.hovered() {
            state.menu_selected = i;
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if r.clicked() {
            state.menu_action = Some(entry.action.clone());
            state.menu_open = false;
        }
    }

    // ── Pie ──
    let footer_y = card.max.y - 26.0;
    ui.painter().line_segment(
        [
            egui::pos2(card.min.x + 1.0, footer_y - 8.0),
            egui::pos2(card.max.x - 1.0, footer_y - 8.0),
        ],
        Stroke::new(1.0, Color32::from_gray(0x24)),
    );
    ui.painter().text(
        egui::pos2(x, footer_y),
        egui::Align2::LEFT_CENTER,
        "↑↓ navegar   ↵ abrir   Tab modo   Esc cerrar",
        egui::FontId::new(9.5, egui::FontFamily::Proportional),
        palette.muted,
    );
    ui.painter().text(
        egui::pos2(card.max.x - 18.0, footer_y),
        egui::Align2::RIGHT_CENTER,
        format!("{} resultados", results.len()),
        egui::FontId::new(9.5, egui::FontFamily::Proportional),
        palette.muted,
    );
}

/// Overview: rejilla de workspaces con las ventanas en su posición relativa.
fn draw_overview(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let screen = ui.max_rect();
    let painter = ui.painter();
    painter.rect_filled(screen, CornerRadius::same(0), Color32::from_black_alpha(200));
    // Registrado antes que las tarjetas: un clic en ventana gana sobre el fondo.
    let backdrop = ui.interact(
        screen,
        ui.scope_id().with(egui::IdSalt::new("overview-backdrop")),
        egui::Sense::click(),
    );
    if backdrop.clicked() {
        state.overview = false;
    }

    let mut ids: Vec<u32> = state.snapshot.occupied.clone();
    let current = state.snapshot.workspace.parse::<u32>().unwrap_or(1);
    if !ids.contains(&current) {
        ids.push(current);
    }
    ids.sort_unstable();
    ids.dedup();
    ids.truncate(6);
    if ids.is_empty() {
        ids.push(1);
    }

    let cols = ids.len().min(3).max(1);
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

    text_at(
        ui,
        egui::pos2(screen.center().x, origin.y - 22.0),
        egui::Align2::CENTER_CENTER,
        "Overview · Esc para cerrar",
        TEXT_BODY,
        palette.muted,
    );

    let monitor_w = screen.width().max(1.0);
    let monitor_h = screen.height().max(1.0);

    for (i, id) in ids.iter().enumerate() {
        let x = origin.x + (i % cols) as f32 * (card_w + gap);
        let y = origin.y + (i / cols) as f32 * (card_h + gap);
        let card = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(card_w, card_h));
        painter.rect(
            card,
            CornerRadius::same(12),
            Color32::from_rgba_unmultiplied(14, 14, 14, 240),
            Stroke::new(
                if *id == current { 2.0 } else { 1.0 },
                if *id == current { palette.accent } else { palette.border },
            ),
            StrokeKind::Middle,
        );
        let card_response = ui.interact(
            card,
            ui.scope_id().with(("overview-card", *id)),
            egui::Sense::click(),
        );
        if card_response.clicked() {
            state.overview_workspace = Some(*id);
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
            palette.muted,
        );
        let windows: Vec<_> = state
            .dock_clients
            .iter()
            .filter(|c| c.workspace == *id as i32)
            .collect();
        for (j, win) in windows.iter().enumerate() {
            let wx = content.min.x + win.x as f32 / monitor_w * content.width();
            let wy = content.min.y + win.y as f32 / monitor_h * content.height();
            let ww = (win.w as f32 / monitor_w * content.width()).max(8.0);
            let wh = (win.h as f32 / monitor_h * content.height()).max(8.0);
            let rect = egui::Rect::from_min_size(egui::pos2(wx, wy), egui::vec2(ww, wh))
                .intersect(content);
            let active_window = win.address == state.snapshot.window_address;
            let response = ui.interact(
                rect,
                ui.scope_id().with(("overview-win", j, *id)),
                egui::Sense::click(),
            );
            painter.rect_filled(
                rect,
                CornerRadius::same(4),
                if active_window { palette.badge.gamma_multiply(1.4) } else { palette.badge },
            );
            painter.rect_stroke(
                rect,
                CornerRadius::same(4),
                Stroke::new(
                    1.0,
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
            if let Some(texture) = state.app_icon_textures.get(&win.class.to_lowercase()) {
                egui::Image::new(egui::load::SizedTexture::new(texture.id(), egui::vec2(12.0, 12.0)))
                    .paint_at(ui, egui::Rect::from_center_size(icon_center, egui::vec2(12.0, 12.0)));
            } else {
                painter.text(
                    icon_center,
                    egui::Align2::CENTER_CENTER,
                    dock_icon(&win.class),
                    icon_font(10.0),
                    palette.text,
                );
            }
            if rect.width() > 70.0 {
                painter.text(
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
                state.overview_focus = Some(win.address.clone());
            }
        }
    }
}

/// Dock inferior: se desliza desde abajo cuando el cursor llega al borde.
fn draw_dock(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let cfg = &config::get().dock;
    if !cfg.enabled || state.dock_progress <= 0.01 {
        return;
    }
    let screen = ui.max_rect();

    // Un elemento por app fijada + uno por clase en ejecución, con TODAS sus
    // ventanas para poder moverse entre ellas.
    let mut items: Vec<DockItem> = Vec::new();
    for pinned in &cfg.pinned {
        let key = pinned.to_lowercase();
        let windows: Vec<_> = state
            .dock_clients
            .iter()
            .filter(|c| c.class.to_lowercase() == key)
            .cloned()
            .collect();
        items.push(DockItem {
            name: pinned.clone(),
            command: pinned.clone(),
            windows,
        });
    }
    for client in &state.dock_clients {
        let key = client.class.to_lowercase();
        if !items.iter().any(|it| it.name.to_lowercase() == key) {
            let windows: Vec<_> = state
                .dock_clients
                .iter()
                .filter(|c| c.class.to_lowercase() == key)
                .cloned()
                .collect();
            items.push(DockItem {
                name: client.class.clone(),
                command: client.class.clone(),
                windows,
            });
        }
    }
    if items.is_empty() {
        return;
    }

    // Pedir iconos reales y subir los que ya vinieron del hilo.
    for item in &items {
        state.app_icons.request(&item.name);
    }
    for (class, rgba) in state.app_icons.drain() {
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [crate::appicons::SIZE as usize, crate::appicons::SIZE as usize],
            &rgba,
        );
        let handle = ui.ctx().load_texture(
            format!("appicon-{class}"),
            image,
            egui::TextureOptions::LINEAR,
        );
        state.app_icon_textures.insert(class, handle);
    }

    const ITEM: f32 = 42.0;
    const GAP: f32 = 6.0;
    const H: f32 = 56.0;
    const BOTTOM: f32 = 10.0;
    let w = 14.0 + items.len() as f32 * (ITEM + GAP) - GAP;
    let hidden = (1.0 - state.dock_progress) * (H + BOTTOM + 12.0);
    let dock = egui::Rect::from_min_size(
        egui::pos2(
            ((screen.width() - w) / 2.0).round(),
            screen.max.y - BOTTOM - H + hidden,
        ),
        egui::vec2(w, H),
    );

    ui.painter().rect(
        dock,
        CornerRadius::same(16),
        Color32::from_rgba_unmultiplied(10, 10, 10, 245),
        Stroke::new(1.0, Color32::from_gray(0x33)),
        StrokeKind::Middle,
    );

    // Rectángulos absolutos: el centro vertical es exactamente dock.h / 2.
    ui.set_opacity(state.dock_progress);
    let icon_y = dock.center().y - ITEM / 2.0;
    for (i, item) in items.iter().enumerate() {
        let name = &item.name;
        let running = !item.windows.is_empty();
        let rect = egui::Rect::from_min_size(
            egui::pos2(dock.min.x + 7.0 + i as f32 * (ITEM + GAP), icon_y),
            egui::vec2(ITEM, ITEM),
        );
        let active = state.snapshot.window_class.eq_ignore_ascii_case(name);
        let color = if active {
            Color32::from_gray(17)
        } else {
            palette.text
        };
        let mut response = ui.interact(
            rect,
            ui.scope_id().with(("dock-item", i)),
            egui::Sense::click(),
        );

        let pressed = response.is_pointer_button_down_on();
        let fill = if pressed {
            palette.border
        } else if active {
            palette.accent
        } else if running {
            palette.badge
        } else {
            Color32::from_gray(0x18)
        };
        ui.painter().rect_filled(rect, CornerRadius::same(12), fill);

        let icon_size = if pressed { 22.0 } else { 26.0 };
        let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(icon_size, icon_size));
        if let Some(texture) = state.app_icon_textures.get(&name.to_lowercase()) {
            egui::Image::new(egui::load::SizedTexture::new(texture.id(), icon_rect.size()))
                .paint_at(ui, icon_rect);
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                dock_icon(name),
                icon_font(if pressed { 18.0 } else { 20.0 }),
                color,
            );
        }

        // Puntos: una ventana abierta por punto (máximo 3).
        let dots = item.windows.len().min(3);
        for d in 0..dots {
            let x = rect.min.x + ITEM / 2.0 - (dots as f32 - 1.0) * 3.0 + d as f32 * 6.0;
            ui.painter().circle_filled(
                egui::pos2(x, rect.max.y - 2.0),
                1.6,
                if active { Color32::from_gray(17) } else { palette.muted },
            );
        }

        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            let tip = item
                .windows
                .first()
                .map(|c| c.title.as_str())
                .unwrap_or(name.as_str())
                .to_string();
            response = response.on_hover_text(tip);
        }

        if response.clicked() {
            if item.windows.is_empty() {
                state.dock_action = Some(DockAction::Launch(item.command.clone()));
            } else if item.windows.len() == 1 {
                state.dock_action = Some(DockAction::Focus(item.windows[0].address.clone()));
            } else if active {
                // Ya está activa: ciclar a la siguiente ventana.
                let current = item
                    .windows
                    .iter()
                    .position(|w| w.address == state.snapshot.window_address)
                    .unwrap_or(0);
                let next = (current + 1) % item.windows.len();
                state.dock_action = Some(DockAction::Focus(item.windows[next].address.clone()));
            } else {
                state.dock_menu = if state.dock_menu == Some(i) { None } else { Some(i) };
            }
        }
        if response.secondary_clicked() && running {
            state.dock_menu = Some(i);
        }
        if response.middle_clicked() {
            state.dock_action = Some(DockAction::Launch(item.command.clone()));
        }
    }

    // Menú de ventanas de la app (se dibuja sobre el dock).
    if let Some(index) = state.dock_menu {
        if let Some(item) = items.get(index) {
            let row_h = 24.0;
            let menu_w = 200.0;
            let menu_h = 26.0 + item.windows.len() as f32 * row_h + 26.0;
            let menu = egui::Rect::from_min_size(
                egui::pos2(
                    dock.min.x + 7.0 + index as f32 * (ITEM + GAP) - 40.0,
                    dock.min.y - menu_h - 6.0,
                ),
                egui::vec2(menu_w, menu_h),
            );
            ui.painter().rect(
                menu,
                CornerRadius::same(12),
                Color32::from_rgba_unmultiplied(12, 12, 12, 250),
                Stroke::new(1.0, Color32::from_gray(0x3a)),
                StrokeKind::Middle,
            );
            text_at(
                ui,
                menu.min + egui::vec2(12.0, 14.0),
                egui::Align2::LEFT_CENTER,
                truncate(&item.name, 22),
                TEXT_CAPTION,
                palette.muted,
            );
            for (j, win) in item.windows.iter().enumerate() {
                let row = egui::Rect::from_min_size(
                    menu.min + egui::vec2(6.0, 22.0 + j as f32 * row_h),
                    egui::vec2(menu_w - 12.0, row_h),
                );
                let r = ui.interact(row, ui.scope_id().with(("dock-menu-win", j)), egui::Sense::click());
                if r.hovered() {
                    ui.painter().rect_filled(row, CornerRadius::same(6), palette.badge);
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                text_at(
                    ui,
                    row.min + egui::vec2(8.0, row.height() / 2.0),
                    egui::Align2::LEFT_CENTER,
                    truncate(&win.title, 24),
                    TEXT_BODY,
                    palette.text,
                );
                if r.clicked() {
                    state.dock_action = Some(DockAction::Focus(win.address.clone()));
                    state.dock_menu = None;
                }
            }
            let new_row = egui::Rect::from_min_size(
                menu.min + egui::vec2(6.0, menu_h - 28.0),
                egui::vec2(menu_w - 12.0, 22.0),
            );
            let r = ui.interact(new_row, ui.scope_id().with("dock-menu-new"), egui::Sense::click());
            if r.hovered() {
                ui.painter().rect_filled(new_row, CornerRadius::same(6), palette.badge);
            }
            text_at(
                ui,
                new_row.min + egui::vec2(8.0, new_row.height() / 2.0),
                egui::Align2::LEFT_CENTER,
                "Nueva ventana",
                TEXT_BODY,
                palette.muted,
            );
            if r.clicked() {
                state.dock_action = Some(DockAction::Launch(item.command.clone()));
                state.dock_menu = None;
            }
        }
    }
    ui.set_opacity(1.0);
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

/// La isla solo muestra workspace, reloj y batería, con `space-between`.
fn draw_notification_card(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    state: &mut UiState,
    palette: Palette,
) {
    ui.painter().rect(
        rect,
        CornerRadius::same(14),
        Color32::from_rgba_unmultiplied(10, 10, 10, 250),
        Stroke::new(1.0, Color32::from_gray(0x33)),
        StrokeKind::Middle,
    );
    let inner = rect.shrink(10.0);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::LEFT))
            .id_salt("notification-card"),
    );
    child.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.horizontal(|ui| {
            ui.label(icon_bold(ic::BELL, ICON_SM, palette.accent));
            ui.add_space(SP_S);
            if let Some(notification) = &state.notification {
                ui.label(
                    RichText::new(truncate(&notification.app, 20))
                        .size(TEXT_CAPTION)
                        .family(egui::FontFamily::Name(FONT_SEMIBOLD.into()))
                        .color(palette.muted),
                );
                ui.label(
                    RichText::new(&notification.time)
                        .size(TEXT_CAPTION)
                        .color(palette.muted),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if state.notification_count > 0 {
                    ui.label(
                        RichText::new(format!("×{}", state.notification_count))
                            .size(TEXT_CAPTION)
                            .family(egui::FontFamily::Name(FONT_SEMIBOLD.into()))
                            .color(palette.accent),
                    )
                    .on_hover_text("Notificaciones esta sesión");
                }
                let close = egui::Button::new(icon_bold(ic::X, ICON_XS, palette.muted))
                    .frame(false)
                    .corner_radius(5.0);
                if ui.add(close).clicked() {
                    state.dismiss_notification = true;
                }
            });
        });
        if let Some(notification) = &state.notification {
            ui.label(
                RichText::new(truncate(&notification.summary, 38))
                    .size(TEXT_BODY)
                    .family(egui::FontFamily::Name(FONT_SEMIBOLD.into()))
                    .color(palette.text),
            );
            if !notification.body.is_empty() {
                ui.label(
                    RichText::new(truncate(&notification.body, 48))
                        .size(TEXT_CAPTION)
                        .color(palette.muted),
                );
            }
        }
    });
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

/// Icono de estado de la barra con tooltip.
fn bar_icon(ui: &mut egui::Ui, icon: &str, color: Color32, tip: String) {
    ui.add(egui::Label::new(icon_bold(icon, ICON_SYSTEM, color)).sense(egui::Sense::hover()))
        .on_hover_text(tip);
}

/// Modo largo: workspaces · opciones | reloj+fecha | sistema.
///
/// El reloj va siempre en el centro geométrico de la píldora (w/2), no en el
/// espacio que sobra entre los bloques. A su izquierda y derecha quedan dos
/// zonas de ancho fijo respecto a ese centro; si los workspaces no caben en la
/// izquierda, sus botones se encogen (mínimo `WS_MIN`) en vez de empujar el reloj.
fn draw_bar_row(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let rect = ui.max_rect();
    let h = rect.height();
    let cx = rect.center().x;

    // Separadores a ambos lados del reloj.
    let separator = Stroke::new(1.0, palette.border);
    for x in [cx - BAR_SEP_OFFSET, cx + BAR_SEP_OFFSET] {
        ui.painter()
            .vline(x, (rect.min.y + 5.0)..=(rect.max.y - 5.0), separator);
    }

    // Zona izquierda: workspaces (1..N) como círculos, con el activo invertido,
    // y el botón del panel.
    let left = egui::Rect::from_min_max(
        rect.min,
        egui::Pos2::new(cx - BAR_SEP_OFFSET - SP_M, rect.max.y),
    );
    island_slot(
        ui,
        "bar-left",
        left,
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_clip_rect(left.intersect(ui.clip_rect()));
            ui.spacing_mut().item_spacing.x = WS_GAP;
            let count = config::get().workspaces.count.max(1);
            let free = left.width() - OPTIONS_W - SP_M;
            let size = ((free - WS_GAP * (count as f32 - 1.0)) / count as f32).clamp(WS_MIN, WS_MAX);
            for n in 1..=count {
                let active = state.snapshot.workspace == n.to_string();
                // Círculo de tamaño exacto (un `Button` de egui impone su mínimo
                // y desbordaría la zona izquierda).
                let (rect, response) =
                    ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::click());
                let response = response.on_hover_cursor(CursorIcon::PointingHand);
                let (bg, fg) = if active {
                    (palette.text, Color32::from_gray(17))
                } else if response.hovered() {
                    (palette.border, palette.text)
                } else {
                    (palette.badge, palette.muted)
                };
                ui.painter().circle_filled(rect.center(), size / 2.0, bg);
                if state.snapshot.occupied.contains(&n) {
                    ui.painter().circle_filled(
                        egui::Pos2::new(rect.center().x, rect.max.y - 3.5),
                        1.75,
                        occupied_color(palette, active),
                    );
                }
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    n.to_string(),
                    egui::FontId::proportional(TEXT_BODY),
                    fg,
                );
                if response.clicked() {
                    state.pending_commands.push(switch_workspace_cmd(n));
                }
            }
            ui.add_space(SP_M);
            panel_toggle_button(ui, state, palette);
        },
    );

    // Zona central: reloj + fecha centrados en w/2.
    island_slot(
        ui,
        "bar-clock",
        egui::Rect::from_center_size(
            egui::Pos2::new(cx, rect.center().y),
            egui::Vec2::new(BAR_CLOCK_HALF * 2.0, h),
        ),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(
                RichText::new(Local::now().format("%H:%M").to_string())
                    .size(TEXT_CLOCK)
                    .family(egui::FontFamily::Name(FONT_SEMIBOLD.into()))
                    .color(palette.text),
            );
            ui.label(
                RichText::new(short_date())
                    .size(TEXT_CAPTION)
                    .color(palette.muted),
            );
        },
    );

    // Zona derecha: batería · % · wifi · audio, con el estado real de `sysinfo`.
    let system_color = Color32::from_rgb(0xf0, 0xf0, 0xf0);
    let battery_color = battery_tone(state.battery.percent, state.battery.charging);
    let (wifi_icon, wifi_color, wifi_tip) = match &state.system.wifi {
        None => (ic::WIFI_HIGH, palette.muted, "Wi-Fi: sin datos".to_string()),
        Some(w) if !w.enabled => (ic::WIFI_SLASH, palette.muted, "Wi-Fi desactivado".to_string()),
        Some(w) => match &w.ssid {
            Some(ssid) => (ic::WIFI_HIGH, system_color, format!("Wi-Fi: {ssid}")),
            None => (ic::WIFI_X, palette.muted, "Wi-Fi sin conexión".to_string()),
        },
    };
    let (audio_icon, audio_color, audio_tip) = match &state.system.volume {
        None => (ic::SPEAKER_HIGH, palette.muted, "Audio: sin datos".to_string()),
        Some(v) if v.muted => (ic::SPEAKER_SLASH, palette.muted, "Audio silenciado".to_string()),
        Some(v) => {
            let icon = match v.percent {
                0 => ic::SPEAKER_NONE,
                1..=49 => ic::SPEAKER_LOW,
                _ => ic::SPEAKER_HIGH,
            };
            (icon, system_color, format!("Volumen {}%", v.percent))
        }
    };
    island_slot(
        ui,
        "bar-system",
        egui::Rect::from_min_max(egui::Pos2::new(rect.max.x - BAR_RIGHT_W, rect.min.y), rect.max),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = SP_L;
            // En right_to_left el primer widget queda a la derecha.
            bar_icon(ui, audio_icon, audio_color, audio_tip);
            bar_icon(ui, wifi_icon, wifi_color, wifi_tip);
            if state.battery.present {
                ui.label(
                    RichText::new(format!("{}%", state.battery.percent))
                        .size(TEXT_BODY)
                        .color(battery_color),
                );
            }
            battery_glyph(ui, state.battery.percent, battery_color);
        },
    );
}


// ── Panel de control ────────────────────────────────────────────────────
// Los colores del panel son constantes propias (grises fijos), no la paleta base16.
const P_SURFACE: Color32 = Color32::from_gray(0x15);
const P_BORDER: Color32 = Color32::from_gray(0x29);
const P_TEXT: Color32 = Color32::from_gray(0xf5);
const P_MUTED: Color32 = Color32::from_gray(0x77);
const P_TABBAR: Color32 = Color32::from_gray(0x10);
const P_HEADING: Color32 = Color32::from_gray(0xaa);
const P_DIM: Color32 = Color32::from_gray(0x66);
const P_SOFT: Color32 = Color32::from_gray(0xcc);
const P_INVERT: Color32 = Color32::from_gray(0xee);
const P_INK: Color32 = Color32::from_gray(0x11);
const P_ICON_BOX: Color32 = Color32::from_gray(0x25);
const P_TRACK: Color32 = Color32::from_gray(0x30);
const P_OK: Color32 = Color32::from_rgb(0x8b, 0xe0, 0x9b);

const PANEL_TABS: [&str; 5] = ["Workspace", "Clock", "System", "Wallpaper", "Alertas"];

const DAYS_LONG: [&str; 7] = [
    "lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo",
];
const MONTHS_LONG: [&str; 12] = [
    "enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre",
    "octubre", "noviembre", "diciembre",
];

/// Presets de layout: nombre + forma de la mini-vista previa.
const LAYOUT_PRESETS: [&str; 4] = ["Dwindle", "Master", "Scrolling", "Master Stack"];
/// Valor de `general:layout` para cada preset. "Master Stack" no tiene un
/// layout propio en Hyprland: queda sin aplicar.
const LAYOUT_IDS: [Option<&str>; 4] = [Some("dwindle"), Some("master"), Some("scrolling"), None];

/// Comando que cambia `general:layout` en tiempo de ejecución (config Lua).
fn layout_command(id: &str) -> Vec<String> {
    vec![
        "hyprctl".to_string(),
        "eval".to_string(),
        format!("hl.config({{ general = {{ layout = \"{id}\" }} }})"),
    ]
}

/// Índice del preset que corresponde a un valor de `general:layout`.
pub fn layout_preset_index(layout: &str) -> Option<usize> {
    LAYOUT_IDS.iter().position(|id| *id == Some(layout))
}

/// Abre una ruta con la aplicación por defecto (vía `main`, sin bloquear).
fn open_path(state: &mut UiState, path: &std::path::Path) {
    state
        .pending_spawns
        .push(vec!["xdg-open".to_string(), path.to_string_lossy().into_owned()]);
}

/// Carpeta de imágenes del usuario (`XDG_PICTURES_DIR` o `~/Pictures`).
fn pictures_dir() -> Option<std::path::PathBuf> {
    let home = std::path::PathBuf::from(std::env::var_os("HOME")?);
    let dir = std::env::var_os("XDG_PICTURES_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| home.join("Pictures"));
    Some(if dir.is_dir() { dir } else { home })
}

/// Carpeta de wallpapers: `[wallpapers] dir` de la config (con `~`), o
/// `~/Pictures/Unsplash`, o la carpeta de imágenes del usuario.
fn wallpaper_dir() -> Option<std::path::PathBuf> {
    let home = std::path::PathBuf::from(std::env::var_os("HOME")?);
    let configured = config::get().wallpapers.dir.clone().map(|raw| {
        match raw.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None if raw == "~" => home.clone(),
            None => std::path::PathBuf::from(raw),
        }
    });
    let dir = configured.unwrap_or_else(|| home.join("Pictures/Unsplash"));
    if dir.is_dir() {
        Some(dir)
    } else {
        pictures_dir()
    }
}

const IMAGE_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "webp", "gif", "bmp"];

/// Imágenes (no recursivo) de `dir`, ordenadas por nombre sin distinguir mayúsculas.
fn list_images(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        })
        .collect();
    files.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    files
}

/// Título y subtítulo de una imagen. Los nombres de Unsplash
/// (`autor-apellido-ID11-unsplash.jpg`) se muestran como "Autor Apellido" + ID.
fn wallpaper_label(path: &std::path::Path) -> (String, String) {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    if let Some(base) = stem.strip_suffix("-unsplash") {
        let chars: Vec<char> = base.chars().collect();
        if chars.len() > 12 && chars[chars.len() - 12] == '-' {
            let author: String = chars[..chars.len() - 12]
                .iter()
                .collect::<String>()
                .split('-')
                .map(capitalize)
                .collect::<Vec<_>>()
                .join(" ");
            let id: String = chars[chars.len() - 11..].iter().collect();
            return (author, format!("Unsplash · {id}"));
        }
    }
    (stem, "Imagen".to_string())
}

/// Colores sólidos que se aplican con `awww clear`.
const WALLPAPERS: [(&str, &str); 6] = [
    ("Negro", "000000"),
    ("Pizarra", "222222"),
    ("Piedra", "444444"),
    ("Gris", "777777"),
    ("Claro", "dddddd"),
    ("Metal", "dd9999"),
];

fn draw_panel(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let width = ui.available_width();
    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

    // Cabecera: título + acciones redondas (ajustes, salir y volver a la isla).
    let (header, _) = ui.allocate_exact_size(egui::Vec2::new(width, 24.0), egui::Sense::hover());
    text_at(
        ui,
        egui::Pos2::new(header.min.x, header.center().y),
        egui::Align2::LEFT_CENTER,
        "Control Center",
        TEXT_DISPLAY_SM,
        P_TEXT,
    );
    let button = egui::Vec2::splat(header.height());
    let slot = |i: f32| {
        egui::Rect::from_min_size(
            egui::Pos2::new(header.max.x - button.x - i * (button.x + 7.0), header.min.y),
            button,
        )
    };
    if round_icon_button_at(ui, slot(0.0), "panel-gear", ic::GEAR).clicked() {
        // Comportamiento previo: el engranaje lleva a Alertas (no hay vista de ajustes).
        state.panel_tab = 4;
    }
    if round_icon_button_at(ui, slot(1.0), "panel-power", ic::POWER).clicked() {
        state.should_exit = true;
    }
    if round_icon_button_at(ui, slot(2.0), "panel-back", ic::ARROW_LEFT).clicked() {
        state.expanded = false;
        state.show_panel = false;
    }
    ui.add_space(SP_L);

    // Barra de pestañas: fondo #101010, activa #292929.
    let (bar, _) = ui.allocate_exact_size(egui::Vec2::new(width, 26.0), egui::Sense::hover());
    ui.painter().rect(
        bar,
        CornerRadius::same(10),
        P_TABBAR,
        Stroke::new(1.0, P_BORDER),
        StrokeKind::Inside,
    );
    let inner = bar.shrink(3.0);
    let gap = 3.0;
    let tab_w = (inner.width() - gap * (PANEL_TABS.len() as f32 - 1.0)) / PANEL_TABS.len() as f32;
    for (i, label) in PANEL_TABS.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            egui::Pos2::new(inner.min.x + i as f32 * (tab_w + gap), inner.min.y),
            egui::Vec2::new(tab_w, inner.height()),
        );
        let response = ui
            .interact(rect, ui.scope_id().with(("panel-tab", i)), egui::Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let active = i == state.panel_tab;
        if active {
            ui.painter().rect_filled(rect, CornerRadius::same(7), P_BORDER);
        }
        let color = if active || response.hovered() { P_TEXT } else { P_MUTED };
        text_at(ui, rect.center(), egui::Align2::CENTER_CENTER, *label, TEXT_BODY, color);
        if response.clicked() {
            state.panel_tab = i;
        }
    }
    ui.add_space(SP_L);

    // Con panel_h = 300 quedan ~216 px para la página: Workspace cabe entera;
    // Clock, System, Wallpaper y Alertas son más altas y hacen scroll.
    // La página ocupa el resto de la tarjeta y se extiende hacia el padding
    // derecho para que la barra de scroll (sólida, fina) no tape el contenido:
    // el contenido mantiene el mismo ancho que la cabecera.
    let rest = ui.available_rect_before_wrap();
    let page_rect = egui::Rect::from_min_max(
        rest.min,
        egui::Pos2::new(rest.max.x + PANEL_SCROLLBAR, rest.max.y.max(rest.min.y)),
    );
    let mut page = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(page_rect)
            .layout(egui::Layout::top_down(egui::Align::Min))
            .id_salt("panel-page-area"),
    );
    page.spacing_mut().scroll = egui::style::ScrollStyle {
        bar_width: 4.0,
        bar_inner_margin: PANEL_SCROLLBAR - 4.0,
        bar_outer_margin: 0.0,
        // Visible aunque no se pase el ratón: indica que hay más contenido.
        dormant_handle_opacity: 0.35,
        ..egui::style::ScrollStyle::solid()
    };
    let tab = state.panel_tab.min(PANEL_TABS.len() - 1);
    egui::ScrollArea::vertical()
        .id_salt(("panel-page", tab))
        .auto_shrink([false, false])
        .show(&mut page, |ui| {
            // Mismo ancho con o sin barra de scroll visible.
            ui.set_max_width(width);
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            match tab {
                0 => panel_workspaces(ui, state, palette),
                1 => panel_clock(ui, state),
                2 => panel_system(ui, state),
                3 => panel_wallpaper(ui, state),
                _ => panel_notifications(ui, state),
            }
        });
}

// ── Piezas comunes del panel ────────────────────────────────────────────

fn text_at(
    ui: &egui::Ui,
    pos: egui::Pos2,
    align: egui::Align2,
    text: impl ToString,
    size: f32,
    color: Color32,
) -> egui::Rect {
    ui.painter()
        .text(pos, align, text.to_string(), egui::FontId::proportional(size), color)
}

fn icon_at(ui: &egui::Ui, center: egui::Pos2, icon: &str, size: f32, color: Color32) {
    ui.painter()
        .text(center, egui::Align2::CENTER_CENTER, icon, icon_font(size), color);
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
    let color = if response.hovered() { P_TEXT } else { P_HEADING };
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
    let (rect, _) =
        ui.allocate_exact_size(egui::Vec2::new(ui.available_width(), 16.0), egui::Sense::hover());
    let y = rect.center().y;
    text_at(ui, egui::Pos2::new(rect.min.x + 1.0, y), egui::Align2::LEFT_CENTER, left, TEXT_BODY, P_HEADING);
    text_at(ui, egui::Pos2::new(rect.max.x - 1.0, y), egui::Align2::RIGHT_CENTER, right, TEXT_BODY, P_DIM);
    ui.add_space(SP_M);
}

/// `.section-heading` con un botón redondo a la derecha.
fn section_heading_action(ui: &mut egui::Ui, left: &str, icon: &str) -> egui::Response {
    let (rect, _) =
        ui.allocate_exact_size(egui::Vec2::new(ui.available_width(), 20.0), egui::Sense::hover());
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
    let (rect, _) =
        ui.allocate_exact_size(egui::Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
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
    let (rect, _) =
        ui.allocate_exact_size(egui::Vec2::new(ui.available_width(), 24.0), egui::Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.min.y, Stroke::new(1.0, Color32::from_gray(0x24)));
    let y = rect.min.y + 14.0;
    text_at(ui, egui::Pos2::new(rect.min.x + 2.0, y), egui::Align2::LEFT_CENTER, left, TEXT_CAPTION, P_DIM);
    let hit = egui::Rect::from_min_max(
        egui::Pos2::new(rect.max.x - if right_is_icon { 22.0 } else { 120.0 }, rect.min.y + 4.0),
        rect.max,
    );
    let sense = if action { egui::Sense::click() } else { egui::Sense::hover() };
    let mut response = ui.interact(hit, ui.scope_id().with(("panel-footer", left)), sense);
    if action {
        response = response.on_hover_cursor(CursorIcon::PointingHand);
    }
    let right_color = if action && response.hovered() { P_TEXT } else { Color32::from_gray(0x99) };
    if right_is_icon {
        icon_at(ui, egui::Pos2::new(rect.max.x - 8.0, y), right, ICON_SM, right_color);
    } else {
        text_at(ui, egui::Pos2::new(rect.max.x - 2.0, y), egui::Align2::RIGHT_CENTER, right, TEXT_CAPTION, right_color);
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
    text_at(ui, egui::Pos2::new(x, rect.center().y - 6.0), egui::Align2::LEFT_CENTER, title, TEXT_TITLE, P_TEXT);
    text_at(ui, egui::Pos2::new(x, rect.center().y + 8.0), egui::Align2::LEFT_CENTER, subtitle, TEXT_CAPTION, P_MUTED);
}

fn text_link(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).size(TEXT_CAPTION).color(color)).frame(false))
        .on_hover_cursor(CursorIcon::PointingHand)
}

/// Reparte `rect` en columnas (o filas) proporcionales con un hueco fijo.
fn split(rect: egui::Rect, weights: &[f32], gap: f32, horizontal: bool) -> Vec<egui::Rect> {
    let total: f32 = weights.iter().sum();
    let length = if horizontal { rect.width() } else { rect.height() };
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

// ── Workspace ───────────────────────────────────────────────────────────

fn panel_workspaces(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let count = config::get().workspaces.count as usize;
    section_heading(ui, "Workspaces", &format!("{count} disponibles"));

    // Rejilla de 5 columnas; el activo va invertido (#eee sobre #111).
    // Medidas para que la pestaña entera quepa en panel_h = 300 sin scroll
    // (con 10 workspaces): 2 filas de 34 px + 4 presets en una sola fila.
    let columns = 5usize;
    let gap = 4.0;
    let cell_h = 34.0;
    let rows = count.div_ceil(columns);
    let width = ui.available_width();
    let cell_w = (width - gap * (columns as f32 - 1.0)) / columns as f32;
    let (grid, _) = ui.allocate_exact_size(
        egui::Vec2::new(width, rows as f32 * cell_h + (rows as f32 - 1.0) * gap),
        egui::Sense::hover(),
    );
    for i in 0..count {
        let n = i + 1;
        let rect = egui::Rect::from_min_size(
            grid.min
                + egui::Vec2::new(
                    (i % columns) as f32 * (cell_w + gap),
                    (i / columns) as f32 * (cell_h + gap),
                ),
            egui::Vec2::new(cell_w, cell_h),
        );
        let response = ui
            .interact(rect, ui.scope_id().with(("panel-ws", n)), egui::Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let active = state.snapshot.workspace == n.to_string();
        let occupied = state.snapshot.occupied.contains(&(n as u32));
        let (fill, stroke, fg) = if active {
            (P_INVERT, P_INVERT, P_INK)
        } else if response.hovered() {
            (P_SURFACE, Color32::from_gray(0x44), P_SOFT)
        } else {
            (P_SURFACE, P_BORDER, P_SOFT)
        };
        ui.painter().rect(rect, CornerRadius::same(10), fill, Stroke::new(1.0, stroke), StrokeKind::Inside);
        let c = rect.center();
        text_at(ui, c - egui::Vec2::new(0.0, 5.0), egui::Align2::CENTER_CENTER, n, TEXT_DISPLAY_SM, fg);
        text_at(
            ui,
            c + egui::Vec2::new(0.0, 9.0),
            egui::Align2::CENTER_CENTER,
            if active {
                "ACTIVO"
            } else if occupied {
                "OCUPADO"
            } else {
                "WS"
            },
            TEXT_CAPTION - 1.0,
            if occupied && !active { palette.accent } else { fg.gamma_multiply(0.65) },
        );
        if occupied {
            ui.painter().circle_filled(
                rect.max - egui::Vec2::new(10.0, rect.height() - 10.0),
                3.0,
                occupied_color(palette, active),
            );
        }
        if response.clicked() {
            state.pending_commands.push(switch_workspace_cmd(n as u32));
        }
    }
    ui.add_space(SP_L);

    // Placeholder: crear presets propios no está implementado.
    let _ = section_heading_action(ui, "Layout presets", ic::PLUS)
        .on_hover_text("Sin implementar: no hay presets propios");

    // Una fila de 4 tarjetas (antes 2×2 de 84 px): el activo se marca con
    // relleno y borde, sin icono de check (no cabe junto a "Master Stack").
    let gap = 6.0;
    let card_h = 52.0;
    let columns = LAYOUT_PRESETS.len();
    let width = ui.available_width();
    let card_w = (width - gap * (columns as f32 - 1.0)) / columns as f32;
    let (grid, _) =
        ui.allocate_exact_size(egui::Vec2::new(width, card_h), egui::Sense::hover());
    for (i, name) in LAYOUT_PRESETS.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            grid.min + egui::Vec2::new(i as f32 * (card_w + gap), 0.0),
            egui::Vec2::new(card_w, card_h),
        );
        let layout_id = LAYOUT_IDS[i];
        let mut response =
            ui.interact(rect, ui.scope_id().with(("panel-preset", i)), egui::Sense::click());
        response = match layout_id {
            Some(id) => response
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(format!("general.layout = \"{id}\"")),
            None => response.on_hover_text("Sin equivalente en Hyprland"),
        };
        let active = state.layout_preset == i;
        let (fill, stroke) = if active { (P_BORDER, P_MUTED) } else { (P_SURFACE, P_BORDER) };
        ui.painter().rect(rect, CornerRadius::same(10), fill, Stroke::new(1.0, stroke), StrokeKind::Inside);
        let inner = rect.shrink(SP_L);
        let name_color = if active { P_TEXT } else { P_SOFT };
        text_at(ui, egui::Pos2::new(inner.min.x, inner.min.y + 6.0), egui::Align2::LEFT_CENTER, *name, TEXT_BODY, name_color);
        let preview = egui::Rect::from_min_max(egui::Pos2::new(inner.min.x, inner.min.y + 15.0), inner.max);
        paint_layout_preview(ui, preview, i);
        if response.clicked() {
            // `general:layout` es una opción de config, no un dispatcher. Con la
            // config en Lua de Hyprland 0.57 `hyprctl keyword` ya no existe
            // ("unknown request"): se aplica con `hyprctl eval` (proceso aparte,
            // lanzado por `main`). Hyprland acepta cualquier cadena sin validarla,
            // por eso solo se envían ids de `LAYOUT_IDS`.
            if let Some(id) = layout_id {
                state.layout_preset = i;
                state.pending_spawns.push(layout_command(id));
            }
        }
    }

    let ws = if state.snapshot.workspace.is_empty() { "-" } else { &state.snapshot.workspace };
    // Layout real de Hyprland si se pudo leer; si no, el preset marcado.
    let layout = match &state.system.layout {
        Some(layout) => capitalize(layout),
        None => LAYOUT_PRESETS[state.layout_preset.min(3)].to_string(),
    };
    let footer = format!("Workspace {ws} · {layout}");
    // Placeholder: no hay vista de ajustes de workspaces.
    let _ = panel_footer(ui, &footer, ic::GEAR, true, false);
}

/// Mini-vista previa de cada preset.
fn paint_layout_preview(ui: &egui::Ui, rect: egui::Rect, preset: usize) {
    let dark = Color32::from_gray(0x66);
    let light = Color32::from_gray(0x99);
    let gap = 3.0;
    let mut blocks: Vec<(egui::Rect, Color32)> = Vec::new();
    match preset {
        // Dwindle y Master comparten la misma mini-vista.
        0 | 1 => {
            let cols = split(rect, &[2.0, 1.0], gap, true);
            blocks.push((cols[0], light));
            let rows = split(cols[1], &[1.0, 1.0], gap, false);
            blocks.push((rows[0], dark));
            blocks.push((rows[1], light));
        }
        2 => {
            for (i, col) in split(rect, &[1.0; 4], gap, true).into_iter().enumerate() {
                blocks.push((col, if i % 2 == 0 { light } else { dark }));
            }
        }
        _ => {
            let cols = split(rect, &[1.3, 1.0], gap, true);
            blocks.push((cols[0], light));
            for (i, row) in split(cols[1], &[1.0; 3], gap, false).into_iter().enumerate() {
                blocks.push((row, if i == 1 { light } else { dark }));
            }
        }
    }
    for (block, color) in blocks {
        ui.painter().rect_filled(block, CornerRadius::same(3), color);
    }
}

// ── Clock ───────────────────────────────────────────────────────────────

fn panel_clock(ui: &mut egui::Ui, state: &mut UiState) {
    let now = Local::now();
    let width = ui.available_width();
    let weekday = DAYS_LONG[now.weekday().num_days_from_monday() as usize];
    let month = MONTHS_LONG[(now.month() - 1) as usize];

    ui.add_space(SP_S);
    let (time, _) = ui.allocate_exact_size(egui::Vec2::new(width, 38.0), egui::Sense::hover());
    text_at(ui, time.center(), egui::Align2::CENTER_CENTER, now.format("%H:%M:%S"), TEXT_HERO, P_TEXT);
    let (date, _) = ui.allocate_exact_size(egui::Vec2::new(width, 18.0), egui::Sense::hover());
    text_at(
        ui,
        date.center(),
        egui::Align2::CENTER_CENTER,
        format!("{}, {} de {month}", capitalize(weekday), now.day()),
        TEXT_CLOCK,
        Color32::from_gray(0xbb),
    );
    let (zone, _) = ui.allocate_exact_size(egui::Vec2::new(width, 18.0), egui::Sense::hover());
    let offset_h = now.offset().local_minus_utc() as f32 / 3600.0;
    text_at(
        ui,
        zone.center(),
        egui::Align2::CENTER_CENTER,
        format!("{} · UTC{}", timezone_name(), format_offset(offset_h)),
        TEXT_BODY,
        P_MUTED,
    );
    divider(ui);

    // Calendario del mes actual con navegación ‹ › (el desplazamiento vive en
    // la memoria de egui, no en `UiState`).
    let offset_id = ui.scope_id().with("panel-calendar-offset");
    let mut month_offset: i32 = ui.data(|d| d.get_temp(offset_id)).unwrap_or(0);
    let total = now.year() * 12 + now.month0() as i32 + month_offset;
    let (year, month0) = (total.div_euclid(12), total.rem_euclid(12) as u32);

    section_heading(ui, "Calendario", &year.to_string());
    let (header, _) = ui.allocate_exact_size(egui::Vec2::new(width, 20.0), egui::Sense::hover());
    text_at(
        ui,
        egui::Pos2::new(header.min.x, header.center().y),
        egui::Align2::LEFT_CENTER,
        format!("{} {year}", capitalize(MONTHS_LONG[month0 as usize])),
        TEXT_TITLE,
        P_SOFT,
    );
    for (i, (icon, step)) in [(ic::CARET_RIGHT, 1), (ic::CARET_LEFT, -1)].into_iter().enumerate() {
        let rect = egui::Rect::from_center_size(
            egui::Pos2::new(header.max.x - 8.0 - i as f32 * 26.0, header.center().y),
            egui::Vec2::splat(20.0),
        );
        let response = ui
            .interact(rect, ui.scope_id().with(("panel-month", i)), egui::Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let color = if response.hovered() { P_TEXT } else { Color32::from_gray(0x99) };
        icon_at(ui, rect.center(), icon, ICON_SM, color);
        if response.clicked() {
            month_offset += step;
            ui.data_mut(|d| d.insert_temp(offset_id, month_offset));
        }
    }
    ui.add_space(SP_L);

    let first = chrono::NaiveDate::from_ymd_opt(year, month0 + 1, 1).unwrap_or_default();
    let lead = first.weekday().num_days_from_monday() as usize;
    let days = days_in_month(year, month0 + 1);
    let today = (month_offset == 0).then(|| now.day());

    let gap = 2.0;
    let cell_w = (width - gap * 6.0) / 7.0;
    let row_h = 20.0;
    let rows = (lead + days as usize).div_ceil(7);
    let (grid, _) = ui.allocate_exact_size(
        egui::Vec2::new(width, row_h * (rows + 1) as f32 + gap * rows as f32),
        egui::Sense::hover(),
    );
    let cell = |slot: usize| {
        egui::Rect::from_min_size(
            grid.min
                + egui::Vec2::new((slot % 7) as f32 * (cell_w + gap), (slot / 7) as f32 * (row_h + gap)),
            egui::Vec2::new(cell_w, row_h),
        )
    };
    for (i, day) in ["L", "M", "X", "J", "V", "S", "D"].iter().enumerate() {
        text_at(ui, cell(i).center(), egui::Align2::CENTER_CENTER, *day, TEXT_CAPTION, P_MUTED);
    }
    for day in 1..=days {
        let rect = cell(7 + lead + day as usize - 1);
        let is_today = today == Some(day);
        if is_today {
            ui.painter().rect_filled(rect, CornerRadius::same(6), P_INVERT);
        }
        let color = if is_today { P_INK } else { P_SOFT };
        text_at(ui, rect.center(), egui::Align2::CENTER_CENTER, day, TEXT_BODY, color);
    }
    divider(ui);

    // Placeholder: no hay sistema de alarmas (nada las haría sonar); filas de
    // ejemplo marcadas como tales.
    let _ = section_heading_action(ui, "Alarmas", ic::PLUS)
        .on_hover_text("Sin implementar: no hay sistema de alarmas");
    for (time, days, active) in [("07:00", "Lunes a viernes", true), ("08:30", "Sábados", false)] {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, 36.0), egui::Sense::hover());
        list_row_at(ui, rect, ic::ALARM, time, &format!("{days} · ejemplo"));
        let (label, color) = if active { ("Activa", P_OK) } else { ("Inactiva", P_MUTED) };
        text_at(ui, egui::Pos2::new(rect.max.x, rect.center().y), egui::Align2::RIGHT_CENTER, label, TEXT_CAPTION, color);
    }
    divider(ui);

    // Reloj mundial: ciudades de `[clock] cities` en config.toml (por defecto
    // Bogotá, Madrid y Tokio), con desplazamientos fijos (sin base de datos de zonas).
    // ＋ y el engranaje del pie abren config.toml para editarlas.
    let config_path = config::path().filter(|p| p.is_file());
    let add = section_heading_action(ui, "Reloj mundial", ic::PLUS).on_hover_text(
        if config_path.is_some() {
            "Editar [[clock.cities]] en config.toml"
        } else {
            "Crea ~/.config/dinamic-ui/config.toml para añadir ciudades"
        },
    );
    let utc = chrono::Utc::now();
    let summer = eu_summer_time(utc);
    for city in &config::get().clock.cities {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, 24.0), egui::Sense::hover());
        let y = rect.center().y;
        text_at(ui, egui::Pos2::new(rect.min.x, y), egui::Align2::LEFT_CENTER, &city.name, TEXT_TITLE, P_TEXT);
        let dst = if summer && city.dst.as_deref() == Some("eu") { 60 } else { 0 };
        let minutes = (city.utc_offset * 60.0).round() as i64 + dst;
        let local = utc + chrono::Duration::minutes(minutes);
        text_at(
            ui,
            egui::Pos2::new(rect.max.x, y),
            egui::Align2::RIGHT_CENTER,
            local.format("%H:%M"),
            TEXT_BODY,
            Color32::from_gray(0xdd),
        );
    }

    let settings = panel_footer(ui, "Clock settings", ic::GEAR, true, config_path.is_some());
    if let Some(path) = &config_path {
        if add.clicked() || settings.clicked() {
            open_path(state, path);
        }
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_y, next_m) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    chrono::NaiveDate::from_ymd_opt(next_y, next_m, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(30)
}

/// Zona horaria del sistema según `/etc/localtime` (p. ej. "America/Bogota").
fn timezone_name() -> &'static str {
    static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAME.get_or_init(|| {
        let from_link = std::fs::read_link("/etc/localtime").ok().and_then(|path| {
            let path = path.to_string_lossy().into_owned();
            path.split_once("zoneinfo/").map(|(_, zone)| zone.to_string())
        });
        from_link
            .or_else(|| std::env::var("TZ").ok())
            .unwrap_or_else(|| "Hora local".to_string())
            .replace('_', " ")
    })
}

fn format_offset(hours: f32) -> String {
    let sign = if hours < 0.0 { "-" } else { "+" };
    let abs = hours.abs();
    if abs.fract() == 0.0 {
        format!("{sign}{}", abs as i32)
    } else {
        format!("{sign}{}:{:02}", abs as i32, (abs.fract() * 60.0).round() as i32)
    }
}

/// Horario de verano de la UE: del último domingo de marzo al último de
/// octubre, ambos a las 01:00 UTC.
fn eu_summer_time(utc: chrono::DateTime<chrono::Utc>) -> bool {
    let last_sunday = |month: u32| {
        let last = chrono::NaiveDate::from_ymd_opt(utc.year(), month, days_in_month(utc.year(), month))
            .unwrap_or_default();
        let back = last.weekday().num_days_from_sunday() as i64;
        (last - chrono::Duration::days(back))
            .and_hms_opt(1, 0, 0)
            .unwrap_or_default()
            .and_utc()
    };
    utc >= last_sunday(3) && utc < last_sunday(10)
}

// ── System ──────────────────────────────────────────────────────────────

fn panel_system(ui: &mut egui::Ui, state: &mut UiState) {
    let width = ui.available_width();
    let battery = &state.battery;

    section_heading(ui, "BATTERY", "Power status");
    let (card, _) = ui.allocate_exact_size(egui::Vec2::new(width, 78.0), egui::Sense::hover());
    card_at(ui, card, 12);
    let inner = card.shrink(12.0);
    let icon_rect = egui::Rect::from_min_size(inner.min, egui::Vec2::splat(34.0));
    let icon = if battery.charging { ic::LIGHTNING } else { ic::BATTERY_HIGH };
    ui.painter().rect_filled(icon_rect, CornerRadius::same(10), P_ICON_BOX);
    icon_at(ui, icon_rect.center(), icon, ICON_LG, P_TEXT);
    let status = match (battery.present, battery.charging) {
        (false, _) => "Sin batería detectada".to_string(),
        (true, charging) => {
            let verb = if charging { "Cargando" } else { "Descargando" };
            match battery.minutes_left {
                Some(m) if charging => format!("{verb} · {} para completar", format_minutes(m)),
                Some(m) => format!("{verb} · {} restantes", format_minutes(m)),
                None => verb.to_string(),
            }
        }
    };
    let x = icon_rect.max.x + 10.0;
    text_at(ui, egui::Pos2::new(x, icon_rect.center().y - 7.0), egui::Align2::LEFT_CENTER, "Battery", TEXT_CLOCK, P_TEXT);
    text_at(ui, egui::Pos2::new(x, icon_rect.center().y + 9.0), egui::Align2::LEFT_CENTER, status, TEXT_CAPTION, P_MUTED);
    let tone = battery_tone(battery.percent, battery.charging);
    let percent = if battery.present { format!("{}%", battery.percent) } else { "--".to_string() };
    text_at(
        ui,
        egui::Pos2::new(inner.max.x, icon_rect.center().y),
        egui::Align2::RIGHT_CENTER,
        percent,
        TEXT_DISPLAY,
        tone,
    );
    let bar = egui::Rect::from_min_size(
        egui::Pos2::new(inner.min.x, inner.max.y - 5.0),
        egui::Vec2::new(inner.width(), 5.0),
    );
    bar_at(ui, bar, battery.percent as f32 / 100.0, P_TRACK, tone);
    ui.add_space(15.0);

    // Wi-Fi (nmcli) y Bluetooth (bluetoothctl), leídos por `sysinfo.rs`.
    let system = &state.system;
    section_heading(ui, "CONNECTIVITY", "Network status");
    let wifi = match &system.wifi {
        None => (None, "Sin datos".to_string()),
        Some(w) if !w.enabled => (Some(false), "Desactivado".to_string()),
        Some(w) => match &w.ssid {
            Some(ssid) => (Some(true), truncate(ssid, 20)),
            None => (Some(false), "Sin conexión".to_string()),
        },
    };
    let bluetooth = match &system.bluetooth {
        None => (None, "Sin datos".to_string()),
        Some(b) if !b.powered => (Some(false), "Apagado".to_string()),
        Some(b) => (
            Some(true),
            match b.connected {
                0 => "Encendido · sin dispositivos".to_string(),
                1 => "1 dispositivo".to_string(),
                n => format!("{n} dispositivos"),
            },
        ),
    };
    let gap = 8.0;
    let (row, _) = ui.allocate_exact_size(egui::Vec2::new(width, 72.0), egui::Sense::hover());
    for (rect, (icon, name, (on, detail))) in split(row, &[1.0, 1.0], gap, true)
        .into_iter()
        .zip([(ic::WIFI_HIGH, "Wi-Fi", wifi), (ic::BLUETOOTH, "Bluetooth", bluetooth)])
    {
        card_at(ui, rect, 11);
        let inner = rect.shrink(11.0);
        icon_at(ui, inner.min + egui::Vec2::new(8.0, 8.0), icon, ICON_LG, Color32::from_gray(0xdd));
        // Punto: verde activo, gris oscuro apagado, gris medio desconocido.
        let dot = match on {
            Some(true) => P_OK,
            Some(false) => Color32::from_gray(0x44),
            None => Color32::from_gray(0x33),
        };
        ui.painter().circle_filled(egui::Pos2::new(inner.max.x - 3.0, inner.min.y + 8.0), 3.0, dot);
        text_at(ui, egui::Pos2::new(inner.min.x, inner.max.y - 14.0), egui::Align2::LEFT_CENTER, name, TEXT_TITLE, P_TEXT);
        text_at(ui, egui::Pos2::new(inner.min.x, inner.max.y - 2.0), egui::Align2::LEFT_CENTER, detail, TEXT_CAPTION, P_MUTED);
    }
    ui.add_space(15.0);

    // Brillo (brightnessctl) y volumen (pactl): solo lectura.
    section_heading(ui, "LEVELS", "Output controls");
    let (card, _) = ui.allocate_exact_size(egui::Vec2::new(width, 66.0), egui::Sense::hover());
    card_at(ui, card, 12);
    let inner = card.shrink2(egui::Vec2::new(13.0, 12.0));
    let volume_icon = match &system.volume {
        Some(v) if v.muted => ic::SPEAKER_SLASH,
        _ => ic::SPEAKER_HIGH,
    };
    let levels = [
        (ic::SUN, system.brightness.map(|b| (b, false))),
        (volume_icon, system.volume.as_ref().map(|v| (v.percent, v.muted))),
    ];
    for (rect, (icon, level)) in split(inner, &[1.0, 1.0], 12.0, false).into_iter().zip(levels) {
        let y = rect.center().y;
        icon_at(ui, egui::Pos2::new(rect.min.x + 8.0, y), icon, ICON_MD, P_HEADING);
        let track = egui::Rect::from_min_max(
            egui::Pos2::new(rect.min.x + 27.0, y - 2.5),
            egui::Pos2::new(rect.max.x - 38.0, y + 2.5),
        );
        let (fraction, label) = match level {
            Some((_, true)) => (0.0, "Mute".to_string()),
            Some((percent, false)) => (percent as f32 / 100.0, format!("{percent}%")),
            None => (0.0, "--".to_string()),
        };
        bar_at(ui, track, fraction, Color32::from_gray(0x33), Color32::from_gray(0xdd));
        text_at(ui, egui::Pos2::new(rect.max.x, y), egui::Align2::RIGHT_CENTER, label, TEXT_CAPTION, Color32::from_gray(0x99));
    }
    ui.add_space(15.0);

    // Dispositivos: salida de audio (pactl), teclado y pantalla (hyprctl).
    section_heading(ui, "DEVICES", "Hardware");
    let or_none = |v: &Option<String>| truncate(v.as_deref().unwrap_or("Sin datos"), 44);
    let devices = [
        (ic::SPEAKER_HIGH, "Audio output", or_none(&system.audio_output)),
        (ic::KEYBOARD, "Keyboard", or_none(&system.keyboard)),
        (ic::MONITOR, "Display", or_none(&system.display)),
    ];
    let row_h = 44.0;
    let (card, _) = ui.allocate_exact_size(
        egui::Vec2::new(width, row_h * devices.len() as f32 + 12.0),
        egui::Sense::hover(),
    );
    card_at(ui, card, 12);
    let inner = card.shrink2(egui::Vec2::new(13.0, 6.0));
    for (i, (icon, name, detail)) in devices.into_iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            inner.min + egui::Vec2::new(0.0, i as f32 * row_h),
            egui::Vec2::new(inner.width(), row_h),
        );
        if i > 0 {
            ui.painter().hline(rect.x_range(), rect.min.y, Stroke::new(1.0, P_BORDER));
        }
        list_row_at(ui, rect, icon, name, &detail);
        icon_at(ui, egui::Pos2::new(rect.max.x - 6.0, rect.center().y), ic::CARET_RIGHT, ICON_SM, P_DIM);
    }

    // Placeholder: Hyprland no tiene una aplicación de ajustes estándar.
    let _ = panel_footer(ui, "System overview", "System settings ↗", false, false);
}

/// "2 h 05 min" / "40 min".
fn format_minutes(minutes: u32) -> String {
    if minutes >= 60 {
        format!("{} h {:02} min", minutes / 60, minutes % 60)
    } else {
        format!("{minutes} min")
    }
}

// ── Wallpaper ───────────────────────────────────────────────────────────

fn is_light(color: Color32) -> bool {
    (color.r() as u32 * 299 + color.g() as u32 * 587 + color.b() as u32 * 114) / 1000 > 150
}

/// Índice del swatch cuyo color es `hex` (`rrggbb`).
pub fn wallpaper_swatch_index(hex: &str) -> Option<usize> {
    WALLPAPERS.iter().position(|(_, h)| h.eq_ignore_ascii_case(hex))
}

fn panel_wallpaper(ui: &mut egui::Ui, state: &mut UiState) {
    let width = ui.available_width();
    let selected = state.wallpaper_selected.and_then(|i| WALLPAPERS.get(i));
    let real = state.system.wallpaper.clone();
    // Miniaturas terminadas: subirlas como texturas.
    for thumb in state.thumbs.drain() {
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [crate::thumbs::THUMB_W as usize, crate::thumbs::THUMB_H as usize],
            &thumb.rgba,
        );
        let name = format!("wallpaper-{}", thumb.path.display());
        let handle = ui.ctx().load_texture(name, image, egui::TextureOptions::LINEAR);
        state.thumb_textures.insert(thumb.path, handle);
    }
    let color_of = |hex: &str| crate::theme::parse_hex(Some(hex)).unwrap_or(Color32::BLACK);
    // El hover manda: al pasar por una miniatura se ve ahí, sin aplicarla.
    let preview_path = state
        .wallpaper_hover
        .as_ref()
        .or(state.wallpaper_image.as_ref());

    section_heading(
        ui,
        if state.wallpaper_hover.is_some() {
            "PREVIEW"
        } else {
            "CURRENT WALLPAPER"
        },
        "Desktop",
    );
    let (preview, _) = ui.allocate_exact_size(egui::Vec2::new(width, 130.0), egui::Sense::hover());
    let base = match (selected, &real) {
        (Some((_, hex)), _) => color_of(hex),
        (None, Some(Wallpaper::Color(hex))) => color_of(hex),
        _ => Color32::from_gray(0x16),
    };
    ui.painter().rect(
        preview,
        CornerRadius::same(12),
        base,
        Stroke::new(1.0, Color32::from_gray(0x33)),
        StrokeKind::Inside,
    );
    // Si el fondo es una imagen con miniatura, se usa de fondo de la vista previa
    // (recortada al aspecto del recuadro) bajo un velo oscuro para leer la hora.
    let has_image = match (selected, preview_path.and_then(|p| state.thumb_textures.get(p))) {
        (None, Some(texture)) => {
            let target = preview.width() / preview.height();
            let source = crate::thumbs::THUMB_W as f32 / crate::thumbs::THUMB_H as f32;
            let uv = if target < source {
                let w = target / source;
                egui::Rect::from_min_max(egui::pos2((1.0 - w) / 2.0, 0.0), egui::pos2((1.0 + w) / 2.0, 1.0))
            } else {
                let h = source / target;
                egui::Rect::from_min_max(egui::pos2(0.0, (1.0 - h) / 2.0), egui::pos2(1.0, (1.0 + h) / 2.0))
            };
            egui::Image::new(egui::load::SizedTexture::new(texture.id(), preview.size()))
                .uv(uv)
                .corner_radius(12)
                .paint_at(ui, preview);
            ui.painter()
                .rect_filled(preview, CornerRadius::same(12), Color32::from_black_alpha(110));
            true
        }
        _ => false,
    };
    // Aproximación de los degradados CSS: un círculo claro y otro oscuro.
    if !has_image {
        ui.painter().circle_filled(
            egui::Pos2::new(preview.max.x - 70.0, preview.min.y + 62.0),
            50.0,
            Color32::from_white_alpha(14),
        );
        ui.painter().circle_filled(
            egui::Pos2::new(preview.min.x + 80.0, preview.max.y - 30.0),
            26.0,
            Color32::from_black_alpha(40),
        );
    }
    let ink = if is_light(base) { P_INK } else { P_TEXT };
    let now = Local::now();
    text_at(ui, preview.center() - egui::Vec2::new(0.0, 10.0), egui::Align2::CENTER_CENTER, now.format("%H:%M"), TEXT_DISPLAY, ink);
    text_at(
        ui,
        preview.center() + egui::Vec2::new(0.0, 12.0),
        egui::Align2::CENTER_CENTER,
        format!(
            "{}, {} de {}",
            capitalize(DAYS_LONG[now.weekday().num_days_from_monday() as usize]),
            now.day(),
            MONTHS_LONG[(now.month() - 1) as usize]
        ),
        TEXT_CAPTION,
        ink.gamma_multiply(0.8),
    );
    let caption = match (selected, &real) {
        (Some((name, hex)), _) => format!("{name} · #{hex}"),
        (None, _) if preview_path.is_some() => {
            let (title, _) = wallpaper_label(preview_path.unwrap());
            let label = truncate(&title, 34);
            if state.wallpaper_hover.is_some() {
                format!("{label} · vista previa")
            } else {
                label
            }
        }
        (None, Some(Wallpaper::Color(hex))) => format!("Color · #{hex}"),
        (None, Some(Wallpaper::Image(path))) => std::path::Path::new(path)
            .file_name()
            .map(|n| truncate(&n.to_string_lossy(), 34))
            .unwrap_or_else(|| "Imagen".to_string()),
        (None, None) => "Fondo actual · sin datos".to_string(),
    };
    text_at(
        ui,
        egui::Pos2::new(preview.min.x + 12.0, preview.max.y - 12.0),
        egui::Align2::LEFT_CENTER,
        caption,
        TEXT_CAPTION,
        ink.gamma_multiply(0.75),
    );
    // Placeholder: "Preview ↗" no abre nada (la vista previa no renderiza imágenes).
    let menu = egui::Rect::from_min_max(
        egui::Pos2::new(preview.max.x - 72.0, preview.max.y - 26.0),
        egui::Pos2::new(preview.max.x - 9.0, preview.max.y - 8.0),
    );
    ui.painter().rect(
        menu,
        CornerRadius::same(6),
        Color32::from_black_alpha(80),
        Stroke::new(1.0, Color32::from_white_alpha(48)),
        StrokeKind::Inside,
    );
    text_at(ui, menu.center(), egui::Align2::CENTER_CENTER, "Preview ↗", TEXT_CAPTION, Color32::WHITE);
    ui.add_space(15.0);

    // Imágenes de la carpeta configurada (`[wallpapers] dir`); clic → `awww img`
    // (vía main). La carpeta se reescanea cada 5 s o al cambiarla en la config.
    let dir = wallpaper_dir();
    let stale = match (&dir, &state.wallpaper_scan) {
        (Some(d), Some((scanned, at))) => {
            scanned != d || at.elapsed() > std::time::Duration::from_secs(5)
        }
        (Some(_), None) => true,
        (None, _) => false,
    };
    if let (true, Some(d)) = (stale, &dir) {
        state.wallpaper_files = list_images(d);
        state.wallpaper_scan = Some((d.clone(), std::time::Instant::now()));
        for path in &state.wallpaper_files {
            state.thumbs.request(path);
        }
    }
    let dir_name = dir
        .as_ref()
        .and_then(|d| d.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "—".to_string());
    let files = std::mem::take(&mut state.wallpaper_files);
    let view_icon = if state.wallpaper_grid {
        ic::LIST
    } else {
        ic::SQUARES_FOUR
    };
    if section_heading_action(
        ui,
        &format!("IMAGES · {} en {}", files.len(), truncate(&dir_name, 16)),
        view_icon,
    )
    .clicked()
    {
        state.wallpaper_grid = !state.wallpaper_grid;
    }
    if files.is_empty() {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, 44.0), egui::Sense::hover());
        card_at(ui, rect, 10);
        text_at(
            ui,
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Sin imágenes en esta carpeta",
            TEXT_BODY,
            P_MUTED,
        );
    }
    let mut chosen: Option<std::path::PathBuf> = None;
    let mut hover_candidate: Option<std::path::PathBuf> = None;
    if !state.wallpaper_grid {
    for (i, path) in files.iter().enumerate() {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, 48.0), egui::Sense::hover());
        let response = ui
            .interact(rect, ui.scope_id().with(("panel-wp-image", i)), egui::Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(path.to_string_lossy());
        if response.hovered() {
            hover_candidate = Some(path.clone());
        }
        let current = state.wallpaper_image.as_deref() == Some(path.as_path());
        card_at(ui, rect, 10);
        if current {
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(10),
                Stroke::new(1.5, P_INVERT),
                StrokeKind::Inside,
            );
        } else if response.hovered() {
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(10),
                Stroke::new(1.0, Color32::from_gray(0x44)),
                StrokeKind::Inside,
            );
        }
        let (title, subtitle) = wallpaper_label(path);
        // Miniatura 16:9 a la izquierda; mientras se genera, un icono de imagen.
        let thumb = egui::Rect::from_min_size(
            egui::Pos2::new(rect.min.x + 6.0, rect.center().y - 18.0),
            egui::Vec2::new(64.0, 36.0),
        );
        match state.thumb_textures.get(path) {
            Some(texture) => {
                egui::Image::new(egui::load::SizedTexture::new(texture.id(), thumb.size()))
                    .corner_radius(6)
                    .paint_at(ui, thumb);
            }
            None => {
                ui.painter().rect_filled(thumb, CornerRadius::same(6), P_ICON_BOX);
                icon_at(ui, thumb.center(), ic::IMAGE, ICON_MD, P_MUTED);
            }
        }
        let text_x = thumb.max.x + 10.0;
        text_at(ui, egui::Pos2::new(text_x, rect.center().y - 7.0), egui::Align2::LEFT_CENTER, truncate(&title, 28), TEXT_TITLE, P_TEXT);
        text_at(ui, egui::Pos2::new(text_x, rect.center().y + 8.0), egui::Align2::LEFT_CENTER, &subtitle, TEXT_CAPTION, P_MUTED);
        if current {
            let check = egui::Pos2::new(rect.max.x - 18.0, rect.center().y);
            ui.painter().circle_filled(check, 8.5, P_INVERT);
            icon_at(ui, check, ic::CHECK, ICON_XS - 2.0, P_INK);
        }
        if response.clicked() {
            chosen = Some(path.clone());
        }
        ui.add_space(SP_S);
    }
    } else {
        // Misma fuente de datos, vista de grid: 3 columnas de miniaturas 16:9.
        let gap = 8.0;
        let columns = 3usize;
        let tile_w = (width - gap * (columns - 1) as f32) / columns as f32;
        let tile_h = tile_w * 9.0 / 16.0;
        let row_h = tile_h + 16.0 + gap;
        let rows = files.len().div_ceil(columns);
        let (area, _) =
            ui.allocate_exact_size(egui::Vec2::new(width, rows as f32 * row_h), egui::Sense::hover());
        for (i, path) in files.iter().enumerate() {
            let x = area.min.x + (i % columns) as f32 * (tile_w + gap);
            let y = area.min.y + (i / columns) as f32 * row_h;
            let image_rect =
                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(tile_w, tile_h));
            let response = ui
                .interact(
                    image_rect,
                    ui.scope_id().with(("panel-wp-grid", i)),
                    egui::Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(path.to_string_lossy());
            if response.hovered() {
                hover_candidate = Some(path.clone());
            }
            let current = state.wallpaper_image.as_deref() == Some(path.as_path());
            card_at(ui, image_rect, 8);
            match state.thumb_textures.get(path) {
                Some(texture) => {
                    egui::Image::new(egui::load::SizedTexture::new(
                        texture.id(),
                        image_rect.size(),
                    ))
                    .corner_radius(8)
                    .paint_at(ui, image_rect);
                }
                None => {
                    ui.painter()
                        .rect_filled(image_rect, CornerRadius::same(8), P_ICON_BOX);
                    icon_at(ui, image_rect.center(), ic::IMAGE, ICON_MD, P_MUTED);
                }
            }
            if current {
                ui.painter().rect_stroke(
                    image_rect,
                    CornerRadius::same(8),
                    Stroke::new(2.0, P_INVERT),
                    StrokeKind::Inside,
                );
                let check = egui::pos2(image_rect.max.x - 12.0, image_rect.min.y + 12.0);
                ui.painter().circle_filled(check, 8.0, P_INVERT);
                icon_at(ui, check, ic::CHECK, ICON_XS - 2.0, P_INK);
            } else if response.hovered() {
                ui.painter().rect_stroke(
                    image_rect,
                    CornerRadius::same(8),
                    Stroke::new(1.0, Color32::from_gray(0x66)),
                    StrokeKind::Inside,
                );
            }
            let (title, _) = wallpaper_label(path);
            text_at(
                ui,
                egui::pos2(x + 2.0, y + tile_h + 7.0),
                egui::Align2::LEFT_TOP,
                truncate(&title, 20),
                TEXT_CAPTION,
                P_TEXT,
            );
            if response.clicked() {
                chosen = Some(path.clone());
            }
        }
    }
    state.wallpaper_files = files;
    state.wallpaper_hover = hover_candidate;
    if let Some(path) = chosen {
        state.wallpaper_image = Some(path.clone());
        state.wallpaper_selected = None;
        state.wallpaper_image_request = Some(path);
    }
    ui.add_space(11.0);

    // Colores sólidos: 3×2; clic → `awww clear` (vía main).
    section_heading(ui, "COLORS", "Solid fill");
    let gap = 8.0;
    let tile_h = 62.0;
    let (grid, _) = ui.allocate_exact_size(
        egui::Vec2::new(width, tile_h * 2.0 + gap + 6.0),
        egui::Sense::hover(),
    );
    // Margen de 3 px para que el contorno del seleccionado no se recorte.
    let grid = grid.shrink(3.0);
    let tile_w = (grid.width() - gap * 2.0) / 3.0;
    for (i, (name, hex)) in WALLPAPERS.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            grid.min + egui::Vec2::new((i % 3) as f32 * (tile_w + gap), (i / 3) as f32 * (tile_h + gap)),
            egui::Vec2::new(tile_w, tile_h),
        );
        let response = ui
            .interact(rect, ui.scope_id().with(("panel-wallpaper", i)), egui::Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(format!("{name} · #{hex}"));
        let color = color_of(hex);
        ui.painter().rect(
            rect,
            CornerRadius::same(9),
            color,
            Stroke::new(1.0, Color32::from_gray(0x33)),
            StrokeKind::Inside,
        );
        ui.painter().circle_filled(
            egui::Pos2::new(rect.min.x + rect.width() * 0.68, rect.min.y + 20.0),
            12.0,
            Color32::from_white_alpha(16),
        );
        if state.wallpaper_selected == Some(i) {
            ui.painter().rect_stroke(
                rect.expand(3.0),
                CornerRadius::same(11),
                Stroke::new(2.0, Color32::from_gray(0xdd)),
                StrokeKind::Middle,
            );
            let check = egui::Pos2::new(rect.max.x - 14.5, rect.min.y + 14.5);
            ui.painter().circle_filled(check, 8.5, P_INVERT);
            icon_at(ui, check, ic::CHECK, ICON_XS - 2.0, P_INK);
        }
        if response.clicked() {
            state.wallpaper_selected = Some(i);
            state.wallpaper_image = None;
            state.wallpaper_color = Some(hex.to_string());
        }
    }
    ui.add_space(15.0);

    // Valores de solo lectura: tipo de fondo según `awww query` (sin ajuste ni
    // rotación) y esquema de color del tema de config.
    section_heading(ui, "APPEARANCE", "Display options");
    let scheme = capitalize(
        &config::get()
            .theme
            .name
            .clone()
            .unwrap_or_else(|| "black-metal".to_string())
            .replace('-', " "),
    );
    for (i, (name, value)) in [
        (
            "Fit mode",
            match &real {
                Some(Wallpaper::Image(_)) => "Imagen",
                Some(Wallpaper::Color(_)) => "Color sólido",
                None => "Sin datos",
            }
            .to_string(),
        ),
        ("Color scheme", scheme),
        ("Wallpaper rotation", "No disponible".to_string()),
    ]
    .into_iter()
    .enumerate()
    {
        if i > 0 {
            ui.add_space(7.0);
        }
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, 36.0), egui::Sense::hover());
        card_at(ui, rect, 10);
        let y = rect.center().y;
        text_at(ui, egui::Pos2::new(rect.min.x + 12.0, y), egui::Align2::LEFT_CENTER, name, TEXT_BODY, P_TEXT);
        icon_at(ui, egui::Pos2::new(rect.max.x - 16.0, y), ic::CARET_RIGHT, ICON_XS, Color32::from_gray(0x88));
        text_at(ui, egui::Pos2::new(rect.max.x - 26.0, y), egui::Align2::RIGHT_CENTER, value, TEXT_CAPTION, Color32::from_gray(0x88));
    }

    let library = wallpaper_dir();
    let browse = panel_footer(ui, "Wallpaper library", "Browse files ↗", false, library.is_some());
    if let (true, Some(dir)) = (browse.clicked(), library) {
        open_path(state, &dir);
    }
}

// ── Alertas (centro de notificaciones, cola de la Fase B) ───────────────

fn panel_notifications(ui: &mut egui::Ui, state: &mut UiState) {
    let width = ui.available_width();

    // DND: tarjeta con interruptor (suprime la isla vía `notifications.rs`).
    let (card, _) = ui.allocate_exact_size(egui::Vec2::new(width, 50.0), egui::Sense::hover());
    card_at(ui, card, 10);
    let icon_rect = egui::Rect::from_center_size(
        egui::Pos2::new(card.min.x + 12.0 + 14.0, card.center().y),
        egui::Vec2::splat(28.0),
    );
    icon_box_at(ui, icon_rect, ic::MOON, P_BORDER);
    let x = icon_rect.max.x + 9.0;
    text_at(ui, egui::Pos2::new(x, card.center().y - 6.0), egui::Align2::LEFT_CENTER, "Do Not Disturb", TEXT_BODY, P_TEXT);
    text_at(
        ui,
        egui::Pos2::new(x, card.center().y + 8.0),
        egui::Align2::LEFT_CENTER,
        "Silence incoming notifications",
        TEXT_CAPTION,
        P_MUTED,
    );
    let toggle = egui::Rect::from_center_size(
        egui::Pos2::new(card.max.x - 12.0 - 17.0, card.center().y),
        egui::Vec2::new(34.0, 19.0),
    );
    let response = ui
        .interact(toggle, ui.scope_id().with("panel-dnd"), egui::Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let (track, knob, knob_x) = if state.dnd {
        (P_HEADING, P_INK, toggle.max.x - 9.5)
    } else {
        (Color32::from_gray(0x44), P_INVERT, toggle.min.x + 9.5)
    };
    ui.painter().rect_filled(toggle, CornerRadius::same(10), track);
    ui.painter().circle_filled(egui::Pos2::new(knob_x, toggle.center().y), 7.5, knob);
    if response.clicked() {
        state.toggle_dnd = true;
    }
    ui.add_space(17.0);

    // Cabecera de la lista: fecha, contador y limpiar todo.
    let (header, _) = ui.allocate_exact_size(egui::Vec2::new(width, 24.0), egui::Sense::hover());
    let y = header.center().y;
    text_at(
        ui,
        egui::Pos2::new(header.min.x + 1.0, y),
        egui::Align2::LEFT_CENTER,
        Local::now().format("TODAY · %B %-d").to_string().to_uppercase(),
        TEXT_BODY,
        P_HEADING,
    );
    let clear = egui::Rect::from_min_size(
        egui::Pos2::new(header.max.x - header.height(), header.min.y),
        egui::Vec2::splat(header.height()),
    );
    if round_icon_button_at(ui, clear, "panel-clear-all", ic::BACKSPACE).clicked() {
        state.clear_all_notifications = true;
    }
    text_at(
        ui,
        egui::Pos2::new(clear.min.x - 8.0, y),
        egui::Align2::RIGHT_CENTER,
        format!("{} new", state.notification_list.len()),
        TEXT_BODY,
        P_DIM,
    );
    ui.add_space(SP_L + 1.0);

    if state.notification_list.is_empty() {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, 90.0), egui::Sense::hover());
        icon_at(ui, rect.center() - egui::Vec2::new(0.0, 10.0), ic::MOON, ICON_XL, P_MUTED);
        text_at(
            ui,
            rect.center() + egui::Vec2::new(0.0, 16.0),
            egui::Align2::CENTER_CENTER,
            "You're all caught up.",
            TEXT_BODY,
            P_MUTED,
        );
    }

    let mut dismiss = None;
    let mut open = None;
    for notification in &state.notification_list {
        Frame::new()
            .fill(P_SURFACE)
            .stroke(Stroke::new(1.0, P_BORDER))
            .corner_radius(11.0)
            .inner_margin(Margin::symmetric(11, 7))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing = egui::Vec2::new(SP_L, 0.0);
                ui.horizontal(|ui| {
                    let (icon_rect, _) =
                        ui.allocate_exact_size(egui::Vec2::splat(28.0), egui::Sense::hover());
                    icon_box_at(ui, icon_rect, ic::BELL, P_BORDER);
                    ui.label(
                        RichText::new(truncate(&notification.app, 28))
                            .size(TEXT_BODY)
                            .color(P_TEXT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(&notification.time).size(TEXT_CAPTION).color(P_DIM));
                    });
                });
                ui.label(
                    RichText::new(truncate(&notification.summary, 60))
                        .size(TEXT_TITLE)
                        .color(P_TEXT),
                );
                if !notification.body.is_empty() {
                    ui.label(
                        RichText::new(truncate(&notification.body, 140))
                            .size(TEXT_BODY)
                            .color(Color32::from_gray(0x99)),
                    );
                }
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width(), 16.0),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if text_link(ui, "Abrir aplicación ↗", P_INVERT).clicked() {
                            open = Some(notification.app.clone());
                        }
                        if text_link(ui, "Descartar", Color32::from_gray(0x99)).clicked() {
                            dismiss = Some(notification.id);
                        }
                    },
                );
            });
        ui.add_space(SP_S);
    }
    if let Some(id) = dismiss {
        state.dismiss_notification_id = Some(id);
    }
    if let Some(app) = open {
        // Comportamiento previo: enfocar la ventana por clase = nombre de la app.
        state.pending_commands.push(format!("focuswindow class:{app}"));
    }

    // Placeholder: historial y ajustes de notificaciones sin implementar
    // (Mako no tiene interfaz de ajustes).
    // No hay agrupación por app (Downloads…): no hay fuente de datos.
    let _ = panel_footer(ui, "Notification history", "Notification settings ↗", false, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_minimum_grows_with_workspaces() {
        // 10 × 20 + 9 × 3 + 6 + 22 = 255 → 2 × (255 + 12 + 40 + 6) = 626.
        assert_eq!(min_bar_w(10), 626);
        assert!(min_bar_w(20) > min_bar_w(10));
        assert_eq!(min_bar_w(0), min_bar_w(1));
    }

    #[test]
    fn unsplash_labels() {
        let p = std::path::Path::new("/x/steve-a-johnson-rji55XQsGkw-unsplash.jpg");
        assert_eq!(
            wallpaper_label(p),
            ("Steve A Johnson".to_string(), "Unsplash · rji55XQsGkw".to_string())
        );
        // ID con guion y guion bajo (alfabeto base64url).
        let p = std::path::Path::new("/x/marek-pavlik-_MoM-kvI_a1-unsplash.jpg");
        assert_eq!(wallpaper_label(p).0, "Marek Pavlik");
        let p = std::path::Path::new("/x/wallhaven-ogjgyl.png");
        assert_eq!(wallpaper_label(p), ("wallhaven-ogjgyl".to_string(), "Imagen".to_string()));
    }

    #[test]
    fn lists_only_images_sorted() {
        let dir = std::env::temp_dir().join(format!("dinamic-ui-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["b.JPG", "a.png", "notas.txt", "c.webp"] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        let names: Vec<String> = list_images(&dir)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(names, vec!["a.png", "b.JPG", "c.webp"]);
    }
}
