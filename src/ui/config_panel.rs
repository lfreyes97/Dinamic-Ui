//! Panel de configuración: páginas de ajustes con navegación lateral (feature `config-panel`,
//! comando `--config-panel`). Ver `docs/CONFIG_PANEL.md`, `docs/WIDGETS_PLAN.md` (Fase 6.5) y
//! `docs/config-panel-mockup.html` (guía de líneas, no medidas).
//!
//! **Maqueta de solo lectura**: muestra los valores de `state.shared.config` y el texto de
//! `config.toml` leído al abrir el panel. No escribe la configuración, no ejecuta comandos del
//! sistema y no inventa valores: lo que la configuración no tiene, se dice así.
//!
//! La lógica (páginas visibles, rects, navegación, comandos, lectura del archivo) son funciones
//! puras y tienen tests; el dibujo va aparte.

use std::path::PathBuf;

use egui::{pos2, vec2, Align2, Color32, Rect, Sense};
use smithay_client_toolkit::seat::keyboard::Keysym;

use super::component::{close_overlays, Component, DrawCtx, UpdateCtx};
use super::*;
use crate::approach;
use crate::config::{BarMaxState, Config, Side, StripSize, ThemeConfig, WindowSwitcherStyle};

/// Estado del panel de configuración.
#[derive(Default)]
pub struct ConfigPanelState {
    /// Objetivo: abierto o cerrado. La animación lo sigue con `progress`.
    pub open: bool,
    /// 0 (cerrado) a 1 (abierto); permite desvanecer también al cerrar.
    pub progress: f32,
    /// Índice en `PAGES` de la página seleccionada en la navegación lateral.
    pub page: usize,
    /// `config.toml` tal como estaba al abrir el panel. Se lee una vez por apertura, no por frame.
    pub file: Option<ConfigFile>,
}

impl ConfigPanelState {
    /// ¿Hay algo que dibujar? (también mientras se desvanece al cerrar).
    pub fn visible(&self) -> bool {
        self.open || self.progress > 0.005
    }
}

// ── Páginas y features ──────────────────────────────────────────────────

/// Páginas, en el orden de la navegación del mockup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Theme,
    Bars,
    Overviews,
    Dashboard,
    Workspaces,
    Notifications,
    Dock,
    Switcher,
    Keys,
    Toml,
}

/// Orden de las páginas; `ConfigPanelState::page` es un índice en esta lista.
pub const PAGES: [Page; 10] = [
    Page::Theme,
    Page::Bars,
    Page::Overviews,
    Page::Dashboard,
    Page::Workspaces,
    Page::Notifications,
    Page::Dock,
    Page::Switcher,
    Page::Keys,
    Page::Toml,
];

/// Rótulos de la navegación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Look,
    Behavior,
    System,
}

const GROUPS: [Group; 3] = [Group::Look, Group::Behavior, Group::System];

impl Group {
    fn label(self) -> &'static str {
        match self {
            Self::Look => "Apariencia y vistas",
            Self::Behavior => "Comportamiento",
            Self::System => "Sistema",
        }
    }
}

impl Page {
    pub fn title(self) -> &'static str {
        match self {
            Self::Theme => "Tema y colores",
            Self::Bars => "Barras",
            Self::Overviews => "Overviews",
            Self::Dashboard => "Dashboard",
            Self::Workspaces => "Workspaces",
            Self::Notifications => "Notificaciones",
            Self::Dock => "Dock",
            Self::Switcher => "Switcher",
            Self::Keys => "Atajos",
            Self::Toml => "config.toml",
        }
    }

    fn desc(self) -> &'static str {
        match self {
            Self::Theme => {
                "Esquema base16, corrección de contraste y los colores que usa ahora la interfaz."
            }
            Self::Bars => "Techo de la barra, topbar, taskbar y los tamaños de la isla.",
            Self::Overviews => "Lado y tamaño de las franjas de overview.",
            Self::Dashboard => "Panel de widgets a pantalla completa.",
            Self::Workspaces => "Cantidad de escritorios virtuales y sus nombres.",
            Self::Notifications => "Cuánto tiempo queda visible la tarjeta de una notificación.",
            Self::Dock => "Dock inferior: activación y apps fijadas.",
            Self::Switcher => {
                "Estilo del selector de ventanas (Alt+Tab). El cambio es inmediato y en memoria."
            }
            Self::Keys => "Comandos de dinamic-ui. Enlázalos a teclas en Hyprland.",
            Self::Toml => "El archivo de configuración tal como estaba al abrir este panel.",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Theme => ic::PALETTE,
            Self::Bars => ic::SIDEBAR_SIMPLE,
            Self::Overviews => ic::GRID_FOUR,
            Self::Dashboard => ic::LAYOUT,
            Self::Workspaces => ic::SQUARES_FOUR,
            Self::Notifications => ic::BELL,
            Self::Dock => ic::RECTANGLE,
            Self::Switcher => ic::APP_WINDOW,
            Self::Keys => ic::KEYBOARD,
            Self::Toml => ic::FILE_CODE,
        }
    }

    pub fn group(self) -> Group {
        match self {
            Self::Theme | Self::Bars | Self::Overviews | Self::Dashboard => Group::Look,
            Self::Workspaces | Self::Notifications | Self::Dock | Self::Switcher | Self::Keys => {
                Group::Behavior
            }
            Self::Toml => Group::System,
        }
    }

    /// Features de las que depende la página: basta con que una esté compilada. Vacío: siempre.
    pub fn features(self) -> &'static [&'static str] {
        match self {
            Self::Theme | Self::Keys | Self::Toml => &[],
            Self::Bars => &["bar", "topbar", "taskbar"],
            Self::Overviews => &["overview"],
            Self::Dashboard => &["dashboard"],
            // Lo usan la barra (y su panel) y los overviews.
            Self::Workspaces => &["bar", "overview"],
            Self::Notifications => &["notifications"],
            Self::Dock => &["dock"],
            Self::Switcher => &["switcher"],
        }
    }
}

/// ¿Está compilada la feature `name`? Solo cuentan las que usa el panel.
pub fn compiled(name: &str) -> bool {
    let built: [(&str, bool); 11] = [
        ("bar", cfg!(feature = "bar")),
        ("topbar", cfg!(feature = "topbar")),
        ("taskbar", cfg!(feature = "taskbar")),
        ("overview", cfg!(feature = "overview")),
        ("dashboard", cfg!(feature = "dashboard")),
        ("notifications", cfg!(feature = "notifications")),
        ("dock", cfg!(feature = "dock")),
        ("switcher", cfg!(feature = "switcher")),
        ("activities", cfg!(feature = "activities")),
        ("command-menu", cfg!(feature = "command-menu")),
        ("config-panel", cfg!(feature = "config-panel")),
    ];
    built.iter().any(|(feature, on)| *on && *feature == name)
}

/// Páginas que se muestran dado un criterio de features (en el orden de `PAGES`).
pub fn visible_pages(has: &dyn Fn(&str) -> bool) -> Vec<Page> {
    PAGES
        .into_iter()
        .filter(|p| p.features().is_empty() || p.features().iter().any(|f| has(f)))
        .collect()
}

/// Página para el índice guardado: si no está visible, la primera visible.
pub fn resolve_page(index: usize, visible: &[Page]) -> Option<Page> {
    PAGES
        .get(index)
        .copied()
        .filter(|p| visible.contains(p))
        .or(visible.first().copied())
}

/// Página visible `delta` posiciones más allá de `current`, sin salirse de la lista.
pub fn step_page(current: Page, delta: i32, visible: &[Page]) -> Option<Page> {
    let pos = visible.iter().position(|p| *p == current)? as i32;
    let last = visible.len() as i32 - 1;
    visible.get((pos + delta).clamp(0, last) as usize).copied()
}

/// Índice de `page` en `PAGES`.
pub fn index_of(page: Page) -> usize {
    PAGES.iter().position(|p| *p == page).unwrap_or(0)
}

// ── Comandos (atajos) ───────────────────────────────────────────────────

/// Un comando de `dinamic-ui --<name>`.
pub struct CommandInfo {
    pub name: &'static str,
    pub desc: &'static str,
    /// Feature que lo implementa; `None`: lo atiende el núcleo.
    pub feature: Option<&'static str>,
}

/// Espejo de `Command` en `src/control.rs` (nombre y feature). Habría que derivarla de allí:
/// hoy es una lista escrita a mano, así que puede quedarse atrás si se añade un comando.
pub static COMMANDS: [CommandInfo; 20] = [
    CommandInfo {
        name: "toggle",
        desc: "Expande o contrae la barra",
        feature: Some("bar"),
    },
    CommandInfo {
        name: "expand",
        desc: "Expande la barra",
        feature: Some("bar"),
    },
    CommandInfo {
        name: "collapse",
        desc: "Contrae la barra y cierra el panel",
        feature: Some("bar"),
    },
    CommandInfo {
        name: "panel",
        desc: "Abre o cierra el panel",
        feature: Some("bar"),
    },
    CommandInfo {
        name: "compact",
        desc: "Pasa a la isla compacta",
        feature: Some("bar"),
    },
    CommandInfo {
        name: "notch",
        desc: "Vuelve al notch",
        feature: Some("bar"),
    },
    CommandInfo {
        name: "reload",
        desc: "Vuelve a leer config.toml y la paleta",
        feature: None,
    },
    CommandInfo {
        name: "quit",
        desc: "Cierra el shell",
        feature: None,
    },
    CommandInfo {
        name: "overview",
        desc: "Overview de workspaces",
        feature: Some("overview"),
    },
    CommandInfo {
        name: "overview-strip-h",
        desc: "Overview en franja horizontal",
        feature: Some("overview"),
    },
    CommandInfo {
        name: "overview-strip-v",
        desc: "Overview en franja vertical",
        feature: Some("overview"),
    },
    CommandInfo {
        name: "activities",
        desc: "Actividades: apps y ventanas",
        feature: Some("activities"),
    },
    CommandInfo {
        name: "dashboard",
        desc: "Dashboard de widgets",
        feature: Some("dashboard"),
    },
    CommandInfo {
        name: "config-panel",
        desc: "Este panel (también --config)",
        feature: Some("config-panel"),
    },
    CommandInfo {
        name: "window-switcher",
        desc: "Switcher de ventanas (Alt+Tab)",
        feature: Some("switcher"),
    },
    CommandInfo {
        name: "workspace-switcher",
        desc: "Switcher de workspaces (Super+Tab)",
        feature: Some("switcher"),
    },
    CommandInfo {
        name: "switcher-style-next",
        desc: "Cambia al siguiente estilo del switcher",
        feature: Some("switcher"),
    },
    CommandInfo {
        name: "taskbar",
        desc: "Activa o desactiva la taskbar",
        feature: Some("taskbar"),
    },
    CommandInfo {
        name: "topbar",
        desc: "Activa o desactiva la topbar",
        feature: Some("topbar"),
    },
    CommandInfo {
        name: "menu",
        desc: "Command menu",
        feature: Some("command-menu"),
    },
];

/// Comandos cuya feature está compilada (o no necesitan ninguna).
pub fn commands_for(has: &dyn Fn(&str) -> bool) -> Vec<&'static CommandInfo> {
    COMMANDS
        .iter()
        .filter(|c| c.feature.is_none_or(has))
        .collect()
}

// ── config.toml ─────────────────────────────────────────────────────────

/// Resultado de leer el archivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    Text(String),
    /// No existe: el shell usa los valores por defecto.
    Missing,
    Error(String),
}

/// Ruta mostrada y estado de la lectura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigFile {
    pub path: String,
    pub state: FileState,
}

/// Lee `config.toml` (una vez por apertura). La ruta se muestra con `~` en lugar de `$HOME`.
pub fn load_file(path: Option<PathBuf>, home: Option<&str>) -> ConfigFile {
    let Some(path) = path else {
        return ConfigFile {
            path: "(sin ruta)".into(),
            state: FileState::Error("no hay XDG_CONFIG_HOME ni HOME".into()),
        };
    };
    let state = match std::fs::read_to_string(&path) {
        Ok(text) => FileState::Text(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => FileState::Missing,
        Err(e) => FileState::Error(e.to_string()),
    };
    ConfigFile {
        path: shorten_home(&path.display().to_string(), home),
        state,
    }
}

fn shorten_home(path: &str, home: Option<&str>) -> String {
    match home
        .filter(|h| !h.is_empty())
        .and_then(|h| path.strip_prefix(h))
    {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_string(),
    }
}

/// Tipo de línea de `config.toml`, para colorearla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineTone {
    Comment,
    Section,
    Plain,
}

pub fn line_tone(line: &str) -> LineTone {
    let t = line.trim_start();
    if t.starts_with('#') {
        LineTone::Comment
    } else if t.starts_with('[') {
        LineTone::Section
    } else {
        LineTone::Plain
    }
}

// ── Valores de la configuración ─────────────────────────────────────────

/// Esquema configurado: `theme.scheme`, o el alias antiguo `theme.name`, o el de por defecto.
pub fn scheme_label(theme: &ThemeConfig) -> String {
    theme
        .scheme
        .as_deref()
        .or(theme.name.as_deref())
        .map(str::to_string)
        .unwrap_or_else(|| "black-metal (por defecto)".into())
}

fn side_label(side: Side) -> &'static str {
    match side {
        Side::Top => "Arriba",
        Side::Bottom => "Abajo",
        Side::Left => "Izquierda",
        Side::Right => "Derecha",
    }
}

/// Opciones de un selector de lado: `sides` en su orden, marcando el actual.
pub fn side_options(current: Side, sides: &[Side]) -> Vec<(&'static str, bool)> {
    sides
        .iter()
        .map(|s| (side_label(*s), *s == current))
        .collect()
}

/// Tamaño de una franja: porcentaje (si es fracción) y píxeles reales en esta pantalla.
pub fn strip_label(size: StripSize, total: f32) -> String {
    let px = size.resolve(total);
    match size {
        StripSize::Fraction(f) => format!("{:.0} % · {px} px", f * 100.0),
        StripSize::Pixels(_) => format!("{px} px"),
    }
}

/// Muestras de la paleta en uso, con su nombre de token (`Palette`).
pub fn palette_tokens(p: Palette) -> Vec<(&'static str, Color32)> {
    vec![
        ("fondo (bg)", p.bg),
        ("superficie", p.surface),
        ("elevada", p.raised),
        ("insignia", p.badge),
        ("selección", p.selection),
        ("borde", p.border),
        ("pista", p.track),
        ("tenue", p.faint),
        ("atenuado", p.dim),
        ("secundario", p.muted),
        ("texto", p.text),
        ("texto destacado", p.text_hi),
        ("acento", p.accent),
        ("acento suave", p.accent_soft),
        ("al pasar", p.hover),
        ("pulsado", p.pressed),
        ("hundido", p.inset),
        ("tarjeta", p.card_fill),
        ("peligro", p.danger),
        ("aviso", p.warn),
        ("correcto", p.ok),
        ("información", p.info),
    ]
}

/// `#RRGGBB`, con el porcentaje de opacidad si el color es translúcido.
fn hex_label(c: Color32) -> String {
    // `Color32` guarda los translúcidos premultiplicados: se deshace para mostrar el color real.
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    let hex = format!("#{r:02X}{g:02X}{b:02X}");
    if a < 255 {
        format!("{hex} · {:.0} %", f32::from(a) / 255.0 * 100.0)
    } else {
        hex
    }
}

// ── Layout (puro) ───────────────────────────────────────────────────────

/// Escala local de la pantalla: 1 a 720 px de alto, hasta 1,6 en pantallas grandes.
pub fn scale(screen_h: f32) -> f32 {
    (screen_h / 720.0).clamp(1.0, 1.6)
}

/// Anchos y alturas a 720 px de alto (se multiplican por `scale`).
const WINDOW_SHARE: f32 = 0.8;
const NAV_W: f32 = 216.0;
const BRAND_H: f32 = 40.0;
const PAD: f32 = 18.0;
const HEAD_H: f32 = 52.0;
const FOOT_H: f32 = 44.0;
const NAV_LABEL_H: f32 = 24.0;
const NAV_ITEM_H: f32 = 32.0;
const ROW_H: f32 = 36.0;
const ROW_HINT_H: f32 = 46.0;
const CARD_RADIUS: f32 = 12.0;
const WINDOW_RADIUS: f32 = 18.0;
const TOKEN_COLS: usize = 4;
const TOKEN_CELL_H: f32 = 40.0;

/// Zonas de la ventana.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rects {
    pub window: Rect,
    pub side: Rect,
    pub head: Rect,
    pub body: Rect,
    pub foot: Rect,
}

/// Ventana centrada al 80 % de la pantalla, con navegación a la izquierda, cabecera, cuerpo y
/// pie a todo lo ancho.
pub fn frame_rects(screen: Rect) -> Rects {
    let s = scale(screen.height());
    let window = Rect::from_center_size(
        screen.center(),
        vec2(
            screen.width() * WINDOW_SHARE,
            screen.height() * WINDOW_SHARE,
        ),
    );
    let foot = Rect::from_min_max(pos2(window.min.x, window.max.y - FOOT_H * s), window.max);
    let side_w = (NAV_W * s).min(window.width() * 0.3);
    let side = Rect::from_min_max(window.min, pos2(window.min.x + side_w, foot.min.y));
    let content = Rect::from_min_max(
        pos2(side.max.x, window.min.y),
        pos2(window.max.x, foot.min.y),
    );
    let inner = content.shrink(PAD * s);
    let head = Rect::from_min_max(inner.min, pos2(inner.max.x, inner.min.y + HEAD_H * s));
    let body = Rect::from_min_max(pos2(inner.min.x, head.max.y + 8.0 * s), inner.max);
    Rects {
        window,
        side,
        head,
        body,
        foot,
    }
}

/// Elementos de la navegación lateral, con sus rectángulos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NavEntry {
    Group(Group, Rect),
    Item(Page, Rect),
}

/// Rótulos y elementos de la navegación; los grupos sin páginas visibles no aparecen.
pub fn nav_entries(side: Rect, s: f32, visible: &[Page]) -> Vec<NavEntry> {
    let inset = 10.0 * s;
    let x = side.min.x + inset;
    let width = (side.width() - 2.0 * inset).max(1.0);
    let mut y = side.min.y + (PAD + BRAND_H) * s;
    let mut out = Vec::new();
    for group in GROUPS {
        let pages: Vec<Page> = visible
            .iter()
            .copied()
            .filter(|p| p.group() == group)
            .collect();
        if pages.is_empty() {
            continue;
        }
        out.push(NavEntry::Group(
            group,
            Rect::from_min_size(pos2(x, y), vec2(width, NAV_LABEL_H * s)),
        ));
        y += NAV_LABEL_H * s;
        for page in pages {
            out.push(NavEntry::Item(
                page,
                Rect::from_min_size(pos2(x, y), vec2(width, NAV_ITEM_H * s)),
            ));
            y += (NAV_ITEM_H + 2.0) * s;
        }
        y += 8.0 * s;
    }
    out
}

// ── Estado de la ventana ────────────────────────────────────────────────

/// Lo que dibujan las páginas.
struct View<'a> {
    s: f32,
    p: Palette,
    screen: Rect,
    cfg: &'a Config,
    file: Option<&'a ConfigFile>,
}

/// Valor de un control de solo lectura en una fila.
enum Control {
    None,
    Text(String),
    /// Valor que falta (p. ej. un workspace sin nombre): en tono secundario.
    Muted(String),
    Switch(bool),
    Options(Vec<(&'static str, bool)>),
}

/// Dibuja el panel de configuración a pantalla completa.
pub(super) fn draw(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let screen = ui.max_rect();
    ui.painter().rect_filled(
        screen,
        CornerRadius::same(Radius::of(Radius::NONE)),
        Palette::scrim(200),
    );
    // Registrado antes que el contenido: un clic en un widget gana sobre el fondo.
    let backdrop = ui.interact(
        screen,
        ui.scope_id().with("config_panel-backdrop"),
        Sense::click(),
    );
    if backdrop.clicked() {
        state.config_panel.open = false;
    }

    let rects = frame_rects(screen);
    let s = scale(screen.height());
    let p = palette;
    let visible = visible_pages(&compiled);
    let page = resolve_page(state.config_panel.page, &visible).unwrap_or(Page::Theme);

    window_chrome(ui, &rects, s, p);
    // La ventana absorbe los clics que no caen en un widget: solo el fondo cuenta como «fuera».
    let _window = ui.interact(
        rects.window,
        ui.scope_id().with("config_panel-window"),
        Sense::click(),
    );
    let picked = sidebar(ui, &rects, s, &visible, page, p);
    let view = View {
        s,
        p,
        screen,
        cfg: state.shared.config.as_ref(),
        file: state.config_panel.file.as_ref(),
    };
    if let Some(style) = content(ui, &rects, &view, page) {
        state
            .shared
            .pending_tune
            .push(format!("switcher.style={style}"));
    }
    footer(ui, &rects, s, p);
    if let Some(next) = picked {
        state.config_panel.page = index_of(next);
    }
}

fn window_chrome(ui: &egui::Ui, r: &Rects, s: f32, p: Palette) {
    let radius = (WINDOW_RADIUS * s).round() as u8;
    drop_shadow(ui, r.window, radius, 12, 40, 150);
    ui.painter().rect(
        r.window,
        CornerRadius::same(radius),
        p.card_fill,
        Stroke::new(1.0, p.border),
        StrokeKind::Inside,
    );
    // Navegación: más oscura, con las esquinas izquierdas de la ventana.
    let side_radius = CornerRadius {
        nw: radius,
        sw: radius,
        ne: 0,
        se: 0,
    };
    ui.painter().rect_filled(r.side, side_radius, p.inset);
    ui.painter().line_segment(
        [r.side.right_top(), r.side.right_bottom()],
        Stroke::new(1.0, p.border),
    );
}

fn sidebar(
    ui: &mut egui::Ui,
    r: &Rects,
    s: f32,
    visible: &[Page],
    current: Page,
    p: Palette,
) -> Option<Page> {
    brand(ui, r.side, s, p);
    let mut picked = None;
    for entry in nav_entries(r.side, s, visible) {
        match entry {
            NavEntry::Group(group, rect) => {
                text_at(
                    ui,
                    pos2(rect.min.x + 8.0 * s, rect.center().y),
                    Align2::LEFT_CENTER,
                    group.label().to_uppercase(),
                    TEXT_CAPTION * s,
                    p.dim,
                );
            }
            NavEntry::Item(page, rect) => {
                if nav_item(ui, rect, page, page == current, s, p).clicked() {
                    picked = Some(page);
                }
            }
        }
    }
    picked
}

fn brand(ui: &egui::Ui, side: Rect, s: f32, p: Palette) {
    let logo = Rect::from_min_size(
        pos2(side.min.x + 16.0 * s, side.min.y + PAD * s),
        vec2(28.0 * s, 28.0 * s),
    );
    ui.painter().rect(
        logo,
        CornerRadius::same((8.0 * s).round() as u8),
        Palette::with_alpha(p.accent, 40),
        Stroke::new(1.0, Palette::with_alpha(p.accent, 90)),
        StrokeKind::Inside,
    );
    icon_at(ui, logo.center(), ic::GEAR, ICON_MD * s, p.accent);
    text_at(
        ui,
        pos2(logo.max.x + 10.0 * s, logo.min.y + 2.0 * s),
        Align2::LEFT_TOP,
        "Dinamic UI",
        TEXT_TITLE * s,
        p.text_hi,
    );
    text_at(
        ui,
        pos2(logo.max.x + 10.0 * s, logo.min.y + 18.0 * s),
        Align2::LEFT_TOP,
        "configuración del shell",
        TEXT_CAPTION * s,
        p.muted,
    );
}

fn nav_item(
    ui: &egui::Ui,
    rect: Rect,
    page: Page,
    active: bool,
    s: f32,
    p: Palette,
) -> egui::Response {
    let response = ui
        .interact(
            rect,
            ui.scope_id().with(("config-nav", page.title())),
            Sense::click(),
        )
        .on_hover_cursor(CursorIcon::PointingHand);
    let (fill, color) = if active {
        (p.accent_soft, p.accent)
    } else if response.hovered() {
        (p.hover, p.text_hi)
    } else {
        (Color32::TRANSPARENT, p.muted)
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same((8.0 * s).round() as u8), fill);
    icon_at(
        ui,
        pos2(rect.min.x + 18.0 * s, rect.center().y),
        page.icon(),
        ICON_MD * s,
        color,
    );
    text_at(
        ui,
        pos2(rect.min.x + 36.0 * s, rect.center().y),
        Align2::LEFT_CENTER,
        page.title(),
        TEXT_BODY * s,
        color,
    );
    response
}

fn header(ui: &egui::Ui, rect: Rect, v: &View, page: Page) {
    text_at(
        ui,
        rect.min,
        Align2::LEFT_TOP,
        page.title(),
        TEXT_DISPLAY_SM * v.s,
        v.p.text_hi,
    );
    text_at(
        ui,
        pos2(rect.min.x, rect.min.y + 24.0 * v.s),
        Align2::LEFT_TOP,
        page.desc(),
        TEXT_BODY * v.s,
        v.p.muted,
    );
}

fn footer(ui: &egui::Ui, r: &Rects, s: f32, p: Palette) {
    ui.painter().line_segment(
        [r.foot.left_top(), r.foot.right_top()],
        Stroke::new(1.0, p.border),
    );
    text_at(
        ui,
        pos2(r.foot.min.x + PAD * s, r.foot.center().y),
        Align2::LEFT_CENTER,
        "Switcher: cambios en vivo · el resto es solo lectura",
        TEXT_CAPTION * s,
        p.muted,
    );
    text_at(
        ui,
        pos2(r.foot.max.x - PAD * s, r.foot.center().y),
        Align2::RIGHT_CENTER,
        "Esc cierra · las flechas cambian de página",
        TEXT_CAPTION * s,
        p.dim,
    );
}

/// Devuelve el estilo elegido en una página interactiva, si la hubo.
fn content(ui: &mut egui::Ui, r: &Rects, v: &View, page: Page) -> Option<&'static str> {
    header(ui, r.head, v, page);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(r.body)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(r.body);
    if page == Page::Toml {
        // El texto de config.toml tiene su propio scroll: no va dentro del de la página.
        page_body(&mut child, page, v)
    } else {
        egui::ScrollArea::vertical()
            .id_salt(page.title())
            .auto_shrink([false, false])
            .show(&mut child, |ui| page_body(ui, page, v))
            .inner
    }
}

fn page_body(ui: &mut egui::Ui, page: Page, v: &View) -> Option<&'static str> {
    match page {
        Page::Theme => theme_page(ui, v),
        Page::Bars => bars_page(ui, v),
        Page::Overviews => overviews_page(ui, v),
        Page::Dashboard => dashboard_page(ui, v),
        Page::Workspaces => workspaces_page(ui, v),
        Page::Notifications => notifications_page(ui, v),
        Page::Dock => dock_page(ui, v),
        Page::Switcher => return switcher_page(ui, v),
        Page::Keys => keys_page(ui, v),
        Page::Toml => toml_page(ui, v),
    }
    None
}

// ── Páginas ─────────────────────────────────────────────────────────────

fn theme_page(ui: &mut egui::Ui, v: &View) {
    let (s, p, t) = (v.s, v.p, &v.cfg.theme);
    card(ui, s, "Esquema", p, |ui| {
        setting_row(
            ui,
            s,
            "Esquema",
            "theme.scheme (el alias antiguo es theme.name)",
            Control::Text(scheme_label(t)),
            p,
        );
        setting_row(
            ui,
            s,
            "Corrección de contraste",
            "Sube texto e indicadores hasta 4,5:1 y 3:1 (WCAG)",
            Control::Switch(t.contrast_correction),
            p,
        );
        setting_row(
            ui,
            s,
            "Acento",
            "theme.accent: un slot base16 o RRGGBB",
            Control::Text(
                t.accent
                    .clone()
                    .unwrap_or_else(|| "base0B (por defecto)".into()),
            ),
            p,
        );
    });
    card(ui, s, "Overrides de token", p, |ui| {
        let overrides = [
            ("Superficie", "theme.surface", &t.surface),
            ("Borde", "theme.border", &t.border),
            ("Insignia", "theme.badge", &t.badge),
            ("Texto", "theme.text", &t.text),
            ("Texto secundario", "theme.muted", &t.muted),
        ];
        for (label, key, value) in overrides {
            let control = match value {
                Some(hex) => Control::Text(hex.clone()),
                None => Control::Muted("sin override".into()),
            };
            setting_row(ui, s, label, key, control, p);
        }
    });
    card(ui, s, "Paleta en uso", p, |ui| palette_grid(ui, s, p));
}

fn palette_grid(ui: &mut egui::Ui, s: f32, p: Palette) {
    let width = ui.available_width();
    let cell_w = width / TOKEN_COLS as f32;
    let tokens = palette_tokens(p);
    for row in tokens.chunks(TOKEN_COLS) {
        let (rect, _) = ui.allocate_exact_size(vec2(width, TOKEN_CELL_H * s), Sense::hover());
        for (i, (name, color)) in row.iter().enumerate() {
            let cell = Rect::from_min_size(
                pos2(rect.min.x + i as f32 * cell_w, rect.min.y),
                vec2(cell_w, rect.height()),
            );
            let swatch =
                Rect::from_min_size(cell.min + vec2(0.0, 7.0 * s), vec2(22.0 * s, 22.0 * s));
            ui.painter().rect(
                swatch,
                CornerRadius::same((6.0 * s).round() as u8),
                *color,
                Stroke::new(1.0, p.border),
                StrokeKind::Inside,
            );
            let text_x = swatch.max.x + 8.0 * s;
            text_at(
                ui,
                pos2(text_x, cell.min.y + 7.0 * s),
                Align2::LEFT_TOP,
                *name,
                TEXT_BODY * s,
                p.text,
            );
            text_at(
                ui,
                pos2(text_x, cell.min.y + 22.0 * s),
                Align2::LEFT_TOP,
                hex_label(*color),
                TEXT_CAPTION * s,
                p.muted,
            );
        }
    }
}

fn bars_page(ui: &mut egui::Ui, v: &View) {
    let (s, p, cfg) = (v.s, v.p, v.cfg);
    if compiled("bar") {
        card(ui, s, "Isla y barra", p, |ui| {
            let max = cfg.bar.max_state;
            setting_row(
                ui,
                s,
                "Techo de expansión",
                "bar.max_state: hasta dónde se expande la barra",
                Control::Options(vec![
                    ("Notch", max == BarMaxState::Notch),
                    ("Compacto", max == BarMaxState::Compact),
                    ("Barra", max == BarMaxState::Bar),
                    ("Panel", max == BarMaxState::Panel),
                ]),
                p,
            );
            setting_row(
                ui,
                s,
                "Alto de la barra",
                "sizes.bar_h (24 a 48)",
                Control::Text(px(cfg.sizes.bar_h)),
                p,
            );
            setting_row(
                ui,
                s,
                "Alto del panel",
                "sizes.panel_h (80 a 480)",
                Control::Text(px(cfg.sizes.panel_h)),
                p,
            );
            setting_row(
                ui,
                s,
                "Ancho de la isla",
                "sizes.island_w",
                Control::Text(px(cfg.sizes.island_w)),
                p,
            );
            setting_row(
                ui,
                s,
                "Ancho de la barra",
                "sizes.bar_w (se amplía sola si los workspaces no caben)",
                Control::Text(px(cfg.sizes.bar_w)),
                p,
            );
        });
    }
    if compiled("topbar") {
        card(ui, s, "Topbar (estilo GNOME)", p, |ui| {
            setting_row(
                ui,
                s,
                "Activa",
                "topbar.enabled: barra superior",
                Control::Switch(cfg.topbar.enabled),
                p,
            );
            setting_row(
                ui,
                s,
                "Altura",
                "topbar.height (24 a 48)",
                Control::Text(px(cfg.topbar.height)),
                p,
            );
        });
    }
    if compiled("taskbar") {
        card(ui, s, "Taskbar (estilo Windows)", p, |ui| {
            let t = cfg.taskbar;
            setting_row(
                ui,
                s,
                "Activa",
                "taskbar.enabled: muestra la barra en lugar de la isla",
                Control::Switch(t.enabled),
                p,
            );
            setting_row(
                ui,
                s,
                "Lado",
                "taskbar.side: ancla de la barra",
                Control::Options(side_options(
                    t.side,
                    &[Side::Bottom, Side::Top, Side::Left, Side::Right],
                )),
                p,
            );
            setting_row(
                ui,
                s,
                "Grosor",
                "taskbar.size (40 a 72)",
                Control::Text(px(t.size)),
                p,
            );
            setting_row(
                ui,
                s,
                "Autohide",
                "taskbar.autohide: aparece al acercar el cursor al borde",
                Control::Switch(t.autohide),
                p,
            );
        });
    }
}

fn overviews_page(ui: &mut egui::Ui, v: &View) {
    let (s, p, o) = (v.s, v.p, v.cfg.overview);
    card(ui, s, "Franjas", p, |ui| {
        setting_row(
            ui,
            s,
            "Franja horizontal · lado",
            "overview.strip_h_side",
            Control::Options(side_options(o.strip_h_side, &[Side::Top, Side::Bottom])),
            p,
        );
        setting_row(
            ui,
            s,
            "Franja horizontal · alto",
            "overview.strip_h_height: fracción de la pantalla o píxeles",
            Control::Text(strip_label(o.strip_h_height, v.screen.height())),
            p,
        );
        setting_row(
            ui,
            s,
            "Franja vertical · lado",
            "overview.strip_v_side",
            Control::Options(side_options(o.strip_v_side, &[Side::Left, Side::Right])),
            p,
        );
        setting_row(
            ui,
            s,
            "Franja vertical · ancho",
            "overview.strip_v_width: fracción de la pantalla o píxeles",
            Control::Text(strip_label(o.strip_v_width, v.screen.width())),
            p,
        );
    });
}

fn dashboard_page(ui: &mut egui::Ui, v: &View) {
    let (s, p) = (v.s, v.p);
    card(ui, s, "Estado", p, |ui| {
        ui.label(
            egui::RichText::new(
                "El dashboard todavía no tiene ajustes en config.toml. Su diseño y sus widgets \
                 se decidirán con layout.toml (fase 6.3 del plan de widgets).",
            )
            .size(TEXT_BODY * s)
            .color(p.muted),
        );
        ui.add_space(SP_S * s);
        setting_row(ui, s, "Comando", "", Control::Text("--dashboard".into()), p);
    });
}

fn workspaces_page(ui: &mut egui::Ui, v: &View) {
    let (s, p, ws) = (v.s, v.p, &v.cfg.workspaces);
    card(ui, s, "General", p, |ui| {
        setting_row(
            ui,
            s,
            "Número de workspaces",
            "workspaces.count (4 a 20): lo usan la barra y los overviews",
            Control::Text(ws.count.to_string()),
            p,
        );
    });
    card(ui, s, "Nombres", p, |ui| {
        for id in 1..=ws.count {
            let hint = if id == 1 {
                "workspaces.names: el primero es el workspace 1"
            } else {
                ""
            };
            let control = match ws.name_of(id) {
                Some(name) => Control::Text(name.to_string()),
                None => Control::Muted("sin nombre".into()),
            };
            setting_row(ui, s, &format!("Workspace {id}"), hint, control, p);
        }
    });
}

fn notifications_page(ui: &mut egui::Ui, v: &View) {
    let (s, p) = (v.s, v.p);
    card(ui, s, "Tarjeta", p, |ui| {
        setting_row(
            ui,
            s,
            "Tiempo en la isla",
            "notifications.ttl_secs: segundos que la tarjeta queda visible",
            Control::Text(format!("{} s", v.cfg.notifications.ttl_secs)),
            p,
        );
    });
}

fn dock_page(ui: &mut egui::Ui, v: &View) {
    let (s, p, dock) = (v.s, v.p, &v.cfg.dock);
    card(ui, s, "Dock inferior", p, |ui| {
        setting_row(
            ui,
            s,
            "Activado",
            "dock.enabled: autohide al borde inferior",
            Control::Switch(dock.enabled),
            p,
        );
    });
    card(ui, s, "Apps fijadas", p, |ui| {
        if dock.pinned.is_empty() {
            setting_row(
                ui,
                s,
                "Ninguna app fijada",
                "dock.pinned: comandos o clases de Hyprland",
                Control::None,
                p,
            );
        }
        for (i, app) in dock.pinned.iter().enumerate() {
            let hint = if i == 0 {
                "dock.pinned: si no hay ventana abierta, se lanza el comando"
            } else {
                ""
            };
            setting_row(
                ui,
                s,
                &format!("Fijada {}", i + 1),
                hint,
                Control::Text(app.clone()),
                p,
            );
        }
    });
}

/// Los siete estilos llevados a código, con su valor de configuración.
const STYLE_CHIPS: [(&str, WindowSwitcherStyle); 7] = [
    ("cards", WindowSwitcherStyle::Cards),
    ("filmstrip", WindowSwitcherStyle::Filmstrip),
    ("grid", WindowSwitcherStyle::Grid),
    ("list", WindowSwitcherStyle::List),
    ("stack", WindowSwitcherStyle::Stack),
    ("windows", WindowSwitcherStyle::WindowOnly),
    ("cascade", WindowSwitcherStyle::Cascade),
];

/// Página interactiva del switcher: los chips encolan un `--tune` en `Shared`.
fn switcher_page(ui: &mut egui::Ui, v: &View) -> Option<&'static str> {
    let (s, p) = (v.s, v.p);
    let current = v.cfg.switcher.style;
    let mut picked = None;
    card(ui, s, "Estilo del switcher de ventanas", p, |ui| {
        setting_row(
            ui,
            s,
            "Estilo activo",
            "switcher.style · igual que dinamic-ui --tune=switcher.style=<estilo>",
            Control::Text(current.as_str().to_string()),
            p,
        );
        ui.add_space(6.0 * s);
        for row in [&STYLE_CHIPS[..4], &STYLE_CHIPS[4..]] {
            if let Some(style) = style_chips(ui, s, p, row, current) {
                picked = Some(style);
            }
            ui.add_space(8.0 * s);
        }
        setting_row(
            ui,
            s,
            "Aplicación",
            "Cambio inmediato en memoria; para conservarlo, edita [switcher] en config.toml",
            Control::Muted("sin guardar en disco".to_string()),
            p,
        );
    });
    picked
}

/// Fila de chips; devuelve el valor del que se hizo clic.
fn style_chips(
    ui: &mut egui::Ui,
    s: f32,
    p: Palette,
    styles: &'static [(&'static str, WindowSwitcherStyle)],
    current: WindowSwitcherStyle,
) -> Option<&'static str> {
    let h = 30.0 * s;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let gap = 6.0 * s;
    let widths = styles
        .iter()
        .map(|(name, _)| (name.len() as f32 * 6.2 + 20.0) * s)
        .collect::<Vec<_>>();
    let mut x = rect.min.x;
    let mut picked = None;
    for ((name, style), w) in styles.iter().zip(widths) {
        let chip = Rect::from_min_size(pos2(x, rect.min.y), vec2(w, h));
        x += w + gap;
        let selected = *style == current;
        let resp = ui.interact(
            chip,
            ui.scope_id().with(("config-style", *name)),
            Sense::click(),
        );
        let fill = if resp.is_pointer_button_down_on() {
            p.pressed
        } else if selected {
            p.accent
        } else if resp.hovered() {
            p.hover
        } else {
            p.badge
        };
        ui.painter().rect(
            chip,
            CornerRadius::same(Radius::of(Radius::SM)),
            fill,
            Stroke::new(1.0, if selected { p.accent } else { p.border }),
            StrokeKind::Middle,
        );
        text_at(
            ui,
            chip.center(),
            Align2::CENTER_CENTER,
            *name,
            TEXT_CAPTION * s,
            if selected { p.solid() } else { p.text },
        );
        if resp.clicked() {
            picked = Some(*name);
        }
    }
    picked
}

fn keys_page(ui: &mut egui::Ui, v: &View) {
    let (s, p) = (v.s, v.p);
    card(ui, s, "Comandos: dinamic-ui --<comando>", p, |ui| {
        for command in commands_for(&compiled) {
            setting_row(
                ui,
                s,
                &format!("--{}", command.name),
                command.desc,
                Control::None,
                p,
            );
        }
    });
}

fn toml_page(ui: &mut egui::Ui, v: &View) {
    let (s, p) = (v.s, v.p);
    let (path, state) = match v.file {
        Some(file) => (file.path.clone(), Some(&file.state)),
        None => ("(sin leer)".into(), None),
    };
    card(ui, s, "Archivo", p, |ui| {
        setting_row(
            ui,
            s,
            "Ruta",
            "Se lee al abrir este panel; no cambia mientras está abierto",
            Control::Text(path),
            p,
        );
    });
    match state {
        None => {}
        Some(FileState::Missing) => {
            ui.label(
                egui::RichText::new("No hay config.toml: se usan los valores por defecto.")
                    .size(TEXT_BODY * s)
                    .color(p.muted),
            );
        }
        Some(FileState::Error(error)) => {
            ui.label(
                egui::RichText::new(format!("No se pudo leer el archivo: {error}"))
                    .size(TEXT_BODY * s)
                    .color(p.warn),
            );
        }
        Some(FileState::Text(text)) => code_box(ui, text, v),
    }
}

/// Texto de `config.toml` en monoespaciado, solo lectura, con scroll propio.
fn code_box(ui: &mut egui::Ui, text: &str, v: &View) {
    let (s, p) = (v.s, v.p);
    egui::Frame::new()
        .fill(p.inset)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(10.0 * s)
        .inner_margin(Margin::same((12.0 * s) as i8))
        .show(ui, |ui| {
            let height = ui.available_height().max(80.0 * s);
            egui::ScrollArea::vertical()
                .id_salt("config-toml-text")
                .max_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for line in text.lines() {
                        if line.trim().is_empty() {
                            ui.add_space(TEXT_BODY * s * 0.6);
                            continue;
                        }
                        let color = match line_tone(line) {
                            LineTone::Comment => p.dim,
                            LineTone::Section => p.accent,
                            LineTone::Plain => p.text,
                        };
                        ui.label(
                            egui::RichText::new(line)
                                .font(egui::FontId::monospace(TEXT_BODY * s))
                                .color(color),
                        );
                    }
                });
        });
}

// ── Piezas de las páginas ───────────────────────────────────────────────

fn px(value: u32) -> String {
    format!("{value} px")
}

/// Tarjeta con rótulo en mayúsculas, como `.card` del mockup.
fn card(ui: &mut egui::Ui, s: f32, title: &str, p: Palette, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(Palette::sheen(7))
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CARD_RADIUS * s)
        .inner_margin(Margin::symmetric((16.0 * s) as i8, (12.0 * s) as i8))
        .show(ui, |ui| {
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 18.0 * s), Sense::hover());
            text_at(
                ui,
                pos2(rect.min.x, rect.center().y),
                Align2::LEFT_CENTER,
                title.to_uppercase(),
                TEXT_CAPTION * s,
                p.dim,
            );
            content(ui);
        });
    ui.add_space(8.0 * s);
}

/// Fila «etiqueta · valor»: el nombre de la clave va como pista debajo de la etiqueta.
fn setting_row(ui: &mut egui::Ui, s: f32, label: &str, hint: &str, control: Control, p: Palette) {
    let height = if hint.is_empty() { ROW_H } else { ROW_HINT_H } * s;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let cy = rect.center().y;
    if hint.is_empty() {
        text_at(
            ui,
            pos2(rect.min.x, cy),
            Align2::LEFT_CENTER,
            label,
            TEXT_BODY * s,
            p.text,
        );
    } else {
        text_at(
            ui,
            pos2(rect.min.x, rect.min.y + 8.0 * s),
            Align2::LEFT_TOP,
            label,
            TEXT_BODY * s,
            p.text,
        );
        text_at(
            ui,
            pos2(rect.min.x, rect.min.y + 26.0 * s),
            Align2::LEFT_TOP,
            hint,
            TEXT_CAPTION * s,
            p.muted,
        );
    }
    let right = rect.max.x;
    match control {
        Control::None => {}
        Control::Text(value) => {
            text_at(
                ui,
                pos2(right, cy),
                Align2::RIGHT_CENTER,
                value,
                TEXT_BODY * s,
                p.text_hi,
            );
        }
        Control::Muted(value) => {
            text_at(
                ui,
                pos2(right, cy),
                Align2::RIGHT_CENTER,
                value,
                TEXT_BODY * s,
                p.muted,
            );
        }
        Control::Switch(on) => {
            let track = Rect::from_min_size(
                pos2(right - 34.0 * s, cy - 9.5 * s),
                vec2(34.0 * s, 19.0 * s),
            );
            switch_at(ui, track, on, p);
        }
        Control::Options(options) => options_at(ui, right, cy, s, &options, p),
    }
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, p.faint),
    );
}

/// Interruptor de solo lectura: no responde al clic.
fn switch_at(ui: &egui::Ui, track: Rect, on: bool, p: Palette) {
    let radius = (track.height() / 2.0).round() as u8;
    let (fill, stroke) = if on {
        (p.accent_soft, Palette::with_alpha(p.accent, 90))
    } else {
        (p.track, p.border)
    };
    ui.painter().rect(
        track,
        CornerRadius::same(radius),
        fill,
        Stroke::new(1.0, stroke),
        StrokeKind::Inside,
    );
    let knob_r = track.height() * 0.34;
    let knob_x = if on {
        track.max.x - track.height() / 2.0
    } else {
        track.min.x + track.height() / 2.0
    };
    let knob = if on { p.accent } else { p.muted };
    ui.painter()
        .circle_filled(pos2(knob_x, track.center().y), knob_r, knob);
}

/// Selector de solo lectura: resalta la opción vigente y no responde al clic.
fn options_at(ui: &egui::Ui, right: f32, cy: f32, s: f32, options: &[(&str, bool)], p: Palette) {
    let pad = 10.0 * s;
    let inner = 3.0 * s;
    let height = 26.0 * s;
    let widths: Vec<f32> = options
        .iter()
        .map(|(label, _)| measure(ui, label, TEXT_BODY * s) + 2.0 * pad)
        .collect();
    let total = widths.iter().sum::<f32>() + 2.0 * inner;
    let container =
        Rect::from_min_size(pos2(right - total, cy - height / 2.0), vec2(total, height));
    ui.painter().rect(
        container,
        CornerRadius::same((8.0 * s).round() as u8),
        Palette::sheen(10),
        Stroke::new(1.0, p.border),
        StrokeKind::Inside,
    );
    let mut x = container.min.x + inner;
    for ((label, active), width) in options.iter().zip(widths) {
        let seg = Rect::from_min_size(
            pos2(x, container.min.y + inner),
            vec2(width, height - 2.0 * inner),
        );
        let (fill, color) = if *active {
            (p.accent_soft, p.accent)
        } else {
            (Color32::TRANSPARENT, p.muted)
        };
        ui.painter()
            .rect_filled(seg, CornerRadius::same((6.0 * s).round() as u8), fill);
        text_at(
            ui,
            seg.center(),
            Align2::CENTER_CENTER,
            *label,
            TEXT_BODY * s,
            color,
        );
        x += width;
    }
}

/// Ancho en píxeles de un texto, para colocar los controles a la derecha.
fn measure(ui: &egui::Ui, text: &str, size: f32) -> f32 {
    ui.painter()
        .layout_no_wrap(
            text.to_string(),
            egui::FontId::proportional(size),
            Color32::WHITE,
        )
        .size()
        .x
}

// ── Teclado y componente ────────────────────────────────────────────────

/// Pasa a la página visible contigua (`delta` = -1 o 1).
fn move_page(state: &mut UiState, delta: i32) {
    let visible = visible_pages(&compiled);
    let current = resolve_page(state.config_panel.page, &visible);
    if let Some(next) = current.and_then(|c| step_page(c, delta, &visible)) {
        state.config_panel.page = index_of(next);
    }
}

pub struct ConfigPanel;

impl Component for ConfigPanel {
    fn name(&self) -> &'static str {
        "config_panel"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, _ctx: DrawCtx, palette: Palette) {
        if !state.config_panel.visible() {
            return;
        }
        // Fundido de entrada y salida; se restaura la opacidad para los componentes siguientes.
        let fade = crate::smootherstep(state.config_panel.progress);
        let previous = ui.opacity();
        ui.set_opacity(previous * fade);
        draw(ui, state, palette);
        ui.set_opacity(previous);
    }

    fn input_rects(
        &self,
        state: &UiState,
        screen_w: i32,
        screen_h: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        if state.config_panel.open {
            vec![(0, 0, screen_w, screen_h)]
        } else {
            Vec::new()
        }
    }

    fn update(&mut self, state: &mut UiState, ctx: &UpdateCtx) -> bool {
        let s = &mut state.config_panel;
        let before = s.progress;
        s.progress = approach(s.progress, if s.open { 1.0 } else { 0.0 }, ctx.dt * 7.0);
        (s.progress - before).abs() > 0.0005
    }

    fn on_key(&mut self, state: &mut UiState, key: Keysym) -> bool {
        if !state.config_panel.open {
            return false;
        }
        match key {
            Keysym::Escape => {
                state.config_panel.open = false;
                true
            }
            Keysym::Up => {
                move_page(state, -1);
                true
            }
            Keysym::Down => {
                move_page(state, 1);
                true
            }
            _ => false,
        }
    }

    fn wants_keyboard(&self, state: &UiState) -> bool {
        state.config_panel.open
    }

    fn on_command(&mut self, command: &str, state: &mut UiState) -> bool {
        if command != "config-panel" {
            return false;
        }
        // El mismo atajo abre y cierra. Solo hay un overlay a pantalla completa a la vez.
        let was_open = state.config_panel.open;
        close_overlays(state);
        state.config_panel.open = !was_open;
        if !was_open {
            // Se lee una vez al abrir: no se relee el disco en cada frame.
            let home = std::env::var("HOME").ok();
            state.config_panel.file = Some(load_file(crate::config::path(), home.as_deref()));
        }
        state.bar.show_panel = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> bool {
        false
    }

    fn all(_: &str) -> bool {
        true
    }

    fn only(feature: &'static str) -> impl Fn(&str) -> bool {
        move |f| f == feature
    }

    #[test]
    fn with_every_feature_all_pages_are_visible_in_order() {
        assert_eq!(visible_pages(&all), PAGES.to_vec());
    }

    #[test]
    fn pages_without_features_are_always_visible() {
        assert_eq!(
            visible_pages(&none),
            vec![Page::Theme, Page::Keys, Page::Toml]
        );
    }

    #[test]
    fn a_page_shows_only_when_one_of_its_features_is_compiled() {
        assert_eq!(
            visible_pages(&only("dock")),
            vec![Page::Theme, Page::Dock, Page::Keys, Page::Toml]
        );
        // Espacio de trabajo: lo usan la barra o los overviews.
        assert!(visible_pages(&only("overview")).contains(&Page::Workspaces));
        assert!(visible_pages(&only("taskbar")).contains(&Page::Bars));
        assert!(!visible_pages(&only("taskbar")).contains(&Page::Workspaces));
    }

    #[test]
    fn switcher_page_only_shows_with_its_feature() {
        assert_eq!(
            visible_pages(&only("switcher")),
            vec![Page::Theme, Page::Switcher, Page::Keys, Page::Toml]
        );
        assert!(!visible_pages(&none).contains(&Page::Switcher));
        assert_eq!(Page::Switcher.group(), Group::Behavior);
    }

    #[test]
    fn style_chips_cover_every_style_once() {
        let names = STYLE_CHIPS
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 7);
        assert!(names
            .iter()
            .all(|name| names.iter().filter(|n| *n == name).count() == 1));
        for (name, style) in STYLE_CHIPS {
            assert_eq!(style.as_str(), name);
        }
    }

    #[test]
    fn resolve_page_falls_back_to_the_first_visible_page() {
        let visible = [Page::Theme, Page::Keys, Page::Toml];
        // `PAGES` incluye `Switcher` en el índice 7: `Keys` pasó al 8.
        assert_eq!(resolve_page(8, &visible), Some(Page::Keys));
        // «Bars» (índice 1) no está visible: se muestra la primera.
        assert_eq!(resolve_page(1, &visible), Some(Page::Theme));
        assert_eq!(resolve_page(99, &visible), Some(Page::Theme));
        assert_eq!(resolve_page(0, &[]), None);
    }

    #[test]
    fn step_page_moves_between_visible_pages_and_stops_at_the_ends() {
        let visible = [Page::Theme, Page::Keys, Page::Toml];
        assert_eq!(step_page(Page::Theme, 1, &visible), Some(Page::Keys));
        assert_eq!(step_page(Page::Keys, -1, &visible), Some(Page::Theme));
        assert_eq!(step_page(Page::Toml, 1, &visible), Some(Page::Toml));
        assert_eq!(step_page(Page::Theme, -1, &visible), Some(Page::Theme));
        // Una página que no está en la lista no tiene vecinos.
        assert_eq!(step_page(Page::Bars, 1, &visible), None);
    }

    #[test]
    fn commands_follow_the_compiled_features() {
        assert_eq!(command_names(&none), vec!["reload", "quit"]);
        assert_eq!(command_names(&all).len(), COMMANDS.len());
        assert!(command_names(&only("bar")).contains(&"toggle"));
        assert!(!command_names(&only("bar")).contains(&"overview"));
    }

    fn command_names(has: &dyn Fn(&str) -> bool) -> Vec<&'static str> {
        commands_for(has).iter().map(|c| c.name).collect()
    }

    #[test]
    fn command_mirror_has_unique_names() {
        let mut names: Vec<&str> = COMMANDS.iter().map(|c| c.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), COMMANDS.len());
    }

    #[test]
    fn scale_is_clamped_between_one_and_one_and_a_half_k() {
        assert_eq!(scale(720.0), 1.0);
        assert_eq!(scale(100.0), 1.0);
        assert!((scale(768.0) - 768.0 / 720.0).abs() < 1e-6);
        assert!((scale(1080.0) - 1.5).abs() < 1e-6);
        assert_eq!(scale(4000.0), 1.6);
    }

    fn screen(w: f32, h: f32) -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h))
    }

    #[test]
    fn frame_fits_the_screen_at_common_sizes() {
        for (w, h) in [(1366.0, 768.0), (1920.0, 1080.0)] {
            let screen = screen(w, h);
            let r = frame_rects(screen);
            assert!(screen.contains_rect(r.window), "ventana fuera de {w}x{h}");
            assert!(r.window.width() > 0.8 * w - 1.0 && r.window.width() < 0.8 * w + 1.0);
            assert!(r.side.max.x <= r.window.max.x);
            assert_eq!(r.foot.max.y, r.window.max.y);
            assert!(r.side.max.y <= r.foot.min.y + 0.5);
            assert!(r.body.height() >= 300.0, "cuerpo demasiado bajo a {w}x{h}");
            assert!(
                r.body.width() >= 500.0,
                "cuerpo demasiado estrecho a {w}x{h}"
            );
            assert!(r.window.contains_rect(r.body));
            assert!(r.head.max.y <= r.body.min.y);
        }
    }

    #[test]
    fn nav_entries_are_ordered_inside_the_side_column() {
        let side = frame_rects(screen(1366.0, 768.0)).side;
        let s = scale(768.0);
        let entries = nav_entries(side, s, &visible_pages(&all));
        let labels = entries
            .iter()
            .filter(|e| matches!(e, NavEntry::Group(..)))
            .count();
        assert_eq!(labels, 3);
        let mut last_y = f32::MIN;
        for entry in &entries {
            let rect = match entry {
                NavEntry::Group(_, r) | NavEntry::Item(_, r) => *r,
            };
            assert!(side.contains_rect(rect));
            assert!(rect.min.y > last_y);
            last_y = rect.min.y;
        }
    }

    #[test]
    fn nav_hides_groups_without_visible_pages() {
        let side = frame_rects(screen(1366.0, 768.0)).side;
        let entries = nav_entries(side, 1.0, &[Page::Theme]);
        // Solo «Apariencia y vistas» tiene página: un rótulo y un elemento.
        assert_eq!(entries.len(), 2);
        assert!(matches!(entries[0], NavEntry::Group(Group::Look, _)));
    }

    #[test]
    fn strip_label_shows_percent_and_real_pixels() {
        assert_eq!(
            strip_label(StripSize::Fraction(0.2), 768.0),
            "20 % · 154 px"
        );
        assert_eq!(strip_label(StripSize::Pixels(200), 768.0), "200 px");
        // Por debajo del mínimo de 40 px: se muestra lo que realmente ocupa.
        assert_eq!(strip_label(StripSize::Pixels(5), 768.0), "40 px");
    }

    #[test]
    fn scheme_label_prefers_scheme_then_the_old_alias() {
        let mut theme = ThemeConfig::default();
        assert_eq!(scheme_label(&theme), "black-metal (por defecto)");
        theme.name = Some("bathory".into());
        assert_eq!(scheme_label(&theme), "bathory");
        theme.scheme = Some("venom".into());
        assert_eq!(scheme_label(&theme), "venom");
    }

    #[test]
    fn side_options_mark_only_the_current_side() {
        assert_eq!(
            side_options(Side::Top, &[Side::Bottom, Side::Top]),
            vec![("Abajo", false), ("Arriba", true)]
        );
    }

    #[test]
    fn palette_tokens_are_named_and_unique() {
        let tokens = palette_tokens(crate::theme::current());
        assert_eq!(tokens.len(), 22);
        let mut names: Vec<&str> = tokens.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 22);
    }

    #[test]
    fn hex_label_reports_opacity_only_when_translucent() {
        assert_eq!(hex_label(Color32::BLACK), "#000000");
        assert_eq!(
            hex_label(Palette::with_alpha(Color32::WHITE, 128)),
            "#FFFFFF · 50 %"
        );
    }

    #[test]
    fn line_tone_classifies_toml_lines() {
        assert_eq!(line_tone("# comentario"), LineTone::Comment);
        assert_eq!(line_tone("  [overview]"), LineTone::Section);
        assert_eq!(line_tone("enabled = true"), LineTone::Plain);
    }

    #[test]
    fn load_file_reports_missing_text_and_errors() {
        let missing = concat!(env!("CARGO_MANIFEST_DIR"), "/no-existe/config.toml");
        assert_eq!(
            load_file(Some(missing.into()), None).state,
            FileState::Missing
        );

        let cargo = load_file(
            Some(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml").into()),
            None,
        );
        match cargo.state {
            FileState::Text(text) => assert!(text.contains("[package]")),
            other => panic!("se esperaba texto, llegó {other:?}"),
        }

        // Un directorio no se puede leer como archivo.
        let dir = load_file(Some(env!("CARGO_MANIFEST_DIR").into()), None);
        assert!(matches!(dir.state, FileState::Error(_)));

        assert!(matches!(load_file(None, None).state, FileState::Error(_)));
    }

    #[test]
    fn config_path_shows_the_home_as_tilde() {
        let file = load_file(
            Some("/home/ana/.config/dinamic-ui/config.toml".into()),
            Some("/home/ana"),
        );
        assert_eq!(file.path, "~/.config/dinamic-ui/config.toml");
        // Un prefijo parcial («/home/anabel») no es el home.
        assert_eq!(
            shorten_home("/home/anabel/x", Some("/home/ana")),
            "/home/anabel/x"
        );
    }
}
