//! Dashboard: rejilla bento de widgets a pantalla completa (feature `dashboard`, comando
//! `--dashboard`). Maqueta de solo lectura: ver `docs/DASHBOARD.md`, `docs/WIDGETS_PLAN.md`
//! (Fase 6.2) y la variante B de `docs/dashboard-mockup.html` (guía, no medidas).
//!
//! La disposición son datos (`PRESET` + `layout`, función pura y probada), y el dibujo solo lee
//! el estado compartido. Los accesos rápidos disparan comandos internos; aquí no hay acciones
//! del sistema.

use chrono::{Datelike, Duration, Local, NaiveDate, Timelike};
use egui::{Align2, Color32, CornerRadius, Rect, Stroke, StrokeKind};
use egui_phosphor::regular as ic;
use smithay_client_toolkit::seat::keyboard::Keysym;

use super::component::{close_overlays, Component, DrawCtx, UpdateCtx};
use super::*;
use crate::approach;
use crate::ipc::ClientInfo;

/// Estado de el dashboard.
#[derive(Default)]
pub struct DashboardState {
    /// Objetivo: abierto o cerrado. La animación lo sigue con `progress`.
    pub open: bool,
    /// 0 (cerrado) a 1 (abierto); permite desvanecer también al cerrar.
    pub progress: f32,
}

impl DashboardState {
    /// ¿Hay algo que dibujar? (también mientras se desvanece al cerrar).
    pub fn visible(&self) -> bool {
        self.open || self.progress > 0.005
    }
}

// ── Disposición: datos y función pura ───────────────────────────────────────────────────

/// Rejilla del preset: 4 columnas × 3 filas, como la variante B del mockup.
pub const GRID_COLS: u32 = 4;
pub const GRID_ROWS: u32 = 3;

/// Widgets del dashboard. Cada uno ocupa una o varias celdas del preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileId {
    Calendario,
    Sistema,
    Clima,
    Workspaces,
    Reproduciendo,
    Red,
    Procesos,
    Ventanas,
    Accesos,
}

/// Posición y tamaño de un widget en la rejilla (columna y fila desde 0, en celdas).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tile {
    pub id: TileId,
    pub col: u32,
    pub row: u32,
    pub w: u32,
    pub h: u32,
}

/// Preset actual: el calendario ocupa dos filas en la primera columna; sistema y workspaces,
/// dos columnas; y la fila inferior tiene una celda por widget. Cubre las 12 celdas sin huecos.
pub const PRESET: [Tile; 9] = [
    Tile {
        id: TileId::Calendario,
        col: 0,
        row: 0,
        w: 1,
        h: 2,
    },
    Tile {
        id: TileId::Sistema,
        col: 1,
        row: 0,
        w: 2,
        h: 1,
    },
    Tile {
        id: TileId::Clima,
        col: 3,
        row: 0,
        w: 1,
        h: 1,
    },
    Tile {
        id: TileId::Workspaces,
        col: 1,
        row: 1,
        w: 2,
        h: 1,
    },
    Tile {
        id: TileId::Reproduciendo,
        col: 3,
        row: 1,
        w: 1,
        h: 1,
    },
    Tile {
        id: TileId::Red,
        col: 0,
        row: 2,
        w: 1,
        h: 1,
    },
    Tile {
        id: TileId::Procesos,
        col: 1,
        row: 2,
        w: 1,
        h: 1,
    },
    Tile {
        id: TileId::Ventanas,
        col: 2,
        row: 2,
        w: 1,
        h: 1,
    },
    Tile {
        id: TileId::Accesos,
        col: 3,
        row: 2,
        w: 1,
        h: 1,
    },
];

/// Una rejilla de `cols` × `rows` celdas dentro de `area`, con `gap` entre celdas.
#[derive(Clone, Copy, Debug)]
pub struct Rejilla {
    pub area: Rect,
    pub gap: f32,
    pub cols: u32,
    pub rows: u32,
}

impl Rejilla {
    /// Rect de un bloque de `w` × `h` celdas que empieza en (`col`, `row`).
    pub fn rect(&self, col: u32, row: u32, w: u32, h: u32) -> Rect {
        let cols = self.cols.max(1) as f32;
        let rows = self.rows.max(1) as f32;
        let cell_w = (self.area.width() - self.gap * (cols - 1.0)) / cols;
        let cell_h = (self.area.height() - self.gap * (rows - 1.0)) / rows;
        let x = self.area.min.x + col as f32 * (cell_w + self.gap);
        let y = self.area.min.y + row as f32 * (cell_h + self.gap);
        let width = w as f32 * cell_w + (w.max(1) - 1) as f32 * self.gap;
        let height = h as f32 * cell_h + (h.max(1) - 1) as f32 * self.gap;
        Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, height))
    }
}

/// ¿Cabe el widget en la rejilla? Los que no caben se descartan (no se dibujan).
fn cabe(tile: &Tile, cols: u32, rows: u32) -> bool {
    tile.w >= 1 && tile.h >= 1 && tile.col + tile.w <= cols && tile.row + tile.h <= rows
}

/// Rect de cada widget dentro de `area`. Función pura: sin egui ni estado, para poder probarla
/// y, más adelante, para una rejilla editable que sustituya a `PRESET`.
pub fn layout(area: Rect, gap: f32, cols: u32, rows: u32, tiles: &[Tile]) -> Vec<(TileId, Rect)> {
    let rejilla = Rejilla {
        area,
        gap,
        cols,
        rows,
    };
    tiles
        .iter()
        .filter(|t| cabe(t, cols, rows))
        .map(|t| (t.id, rejilla.rect(t.col, t.row, t.w, t.h)))
        .collect()
}

/// Escala de la interfaz según la altura de la pantalla (720 px de referencia, como `nav.rs`).
pub fn escala(alto: f32) -> f32 {
    (alto / 720.0).clamp(1.0, 1.6)
}

// ── Textos y datos de calendario (puros) ────────────────────────────────────────────────

const DIAS: [&str; 7] = [
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
    "domingo",
];
const MESES: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

/// Fecha larga en español: «viernes, 20 de noviembre de 2026».
pub fn fecha_larga(fecha: NaiveDate) -> String {
    format!(
        "{}, {} de {} de {}",
        DIAS[fecha.weekday().num_days_from_monday() as usize],
        fecha.day(),
        MESES[fecha.month0() as usize],
        fecha.year()
    )
}

/// Saludo según la hora local.
pub fn saludo(hora: u32) -> &'static str {
    match hora {
        6..=11 => "Buenos días",
        12..=19 => "Buenas tardes",
        _ => "Buenas noches",
    }
}

/// Las 42 celdas (6 semanas, de lunes a domingo) del mes de `hoy`. El `bool` indica si el día
/// pertenece a ese mes; los de los meses vecinos se pintan atenuados.
pub fn celdas_mes(hoy: NaiveDate) -> Vec<(NaiveDate, bool)> {
    let primero = hoy - Duration::days(hoy.day0() as i64);
    let inicio = primero - Duration::days(primero.weekday().num_days_from_monday() as i64);
    (0..42)
        .map(|i| {
            let dia = inicio + Duration::days(i);
            (dia, dia.month() == hoy.month())
        })
        .collect()
}

/// Rect de una ventana dentro de la miniatura `area`. Las coordenadas de la ventana son del
/// monitor (`monitor`, en píxeles lógicos). `None` si queda fuera de la miniatura.
pub fn ventana_en_miniatura(
    win: &ClientInfo,
    monitor: egui::Vec2,
    area: Rect,
    min: f32,
) -> Option<Rect> {
    let (mw, mh) = (monitor.x.max(1.0), monitor.y.max(1.0));
    let x = area.min.x + win.x as f32 / mw * area.width();
    let y = area.min.y + win.y as f32 / mh * area.height();
    let w = (win.w as f32 / mw * area.width()).max(min);
    let h = (win.h as f32 / mh * area.height()).max(min);
    let r = Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h)).intersect(area);
    (r.width() > 0.0 && r.height() > 0.0).then_some(r)
}

/// Nombre de usuario desde la variable `USER` (vacía o ausente: sin nombre en el saludo).
fn usuario() -> Option<String> {
    std::env::var("USER")
        .ok()
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
}

/// Primera letra en mayúscula («luis» → «Luis»).
fn capitalizar(texto: &str) -> String {
    let mut chars = texto.chars();
    match chars.next() {
        Some(primera) => primera.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Accesos rápidos: comando interno, icono y rótulo. Los comandos son los del `--comando`.
const ACCESOS: [(&str, &str, &str); 6] = [
    ("menu", ic::MAGNIFYING_GLASS, "Menú"),
    ("overview", ic::SQUARES_FOUR, "Vista general"),
    ("activities", ic::APP_WINDOW, "Actividades"),
    ("config-panel", ic::GEAR_SIX, "Configuración"),
    ("topbar", ic::RECTANGLE, "Barra superior"),
    ("taskbar", ic::LIST_BULLETS, "Barra de tareas"),
];

// ── Dibujo ──────────────────────────────────────────────────────────────────────────────

/// Radio de tarjetas y botones, a 720 px (se escala).
const RADIO: f32 = 14.0;
/// Relleno interior de las tarjetas, a 720 px.
const PAD: f32 = 14.0;
/// Alto de la línea de cabecera de cada tarjeta, a 720 px.
const CABECERA: f32 = 18.0;
/// Margen de la pantalla y hueco entre tarjetas, a 720 px.
const MARGEN: f32 = 28.0;
const HUECO: f32 = 12.0;

/// Lo que comparten todas las tarjetas: la escala, el hueco y el momento de dibujo.
struct Escena {
    pantalla: Rect,
    k: f32,
    gap: f32,
    hoy: NaiveDate,
}

/// Cabecera de una tarjeta: icono, título y un dato secundario a la derecha.
struct Marco<'a> {
    titulo: &'a str,
    icono: &'a str,
    color: Color32,
    aside: &'a str,
}

/// Una medida con etiqueta, valor y barra opcional (fracción 0..1 y su color).
struct Medida<'a> {
    icono: &'a str,
    color: Color32,
    etiqueta: &'a str,
    valor: &'a str,
    barra: Option<(f32, Color32)>,
}

/// Dibuja el dashboard a pantalla completa. Solo lectura: no ejecuta acciones del sistema.
pub(super) fn draw(ui: &mut egui::Ui, state: &mut UiState, palette: Palette) {
    let screen = ui.max_rect();
    ui.painter()
        .rect_filled(screen, CornerRadius::same(0), Palette::scrim(200));
    // Registrado antes que el contenido: un clic en un widget gana sobre el fondo.
    let backdrop = ui.interact(
        screen,
        ui.scope_id().with("dashboard-backdrop"),
        egui::Sense::click(),
    );
    if backdrop.clicked() {
        state.dashboard.open = false;
    }

    let k = escala(screen.height());
    let margen = MARGEN * k;
    let ahora = Local::now();
    let hoy = ahora.date_naive();

    // Cabecera: saludo, fecha larga y hora.
    let saludo_txt = match usuario() {
        Some(nombre) => format!("{}, {}", saludo(ahora.hour()), capitalizar(&nombre)),
        None => saludo(ahora.hour()).to_string(),
    };
    let saludo_rect = text_at(
        ui,
        egui::pos2(screen.min.x + margen, screen.min.y + margen),
        Align2::LEFT_TOP,
        saludo_txt,
        TEXT_HERO * k,
        palette.text_hi,
    );
    let fecha = text_at(
        ui,
        egui::pos2(screen.min.x + margen, saludo_rect.max.y + 2.0 * k),
        Align2::LEFT_TOP,
        fecha_larga(hoy),
        TEXT_TITLE * k,
        palette.muted,
    );
    text_at(
        ui,
        egui::pos2(screen.max.x - margen, screen.min.y + margen),
        Align2::RIGHT_TOP,
        ahora.format("%H:%M").to_string(),
        TEXT_DISPLAY * k,
        palette.text_hi,
    );

    // Rejilla bento: lo que queda bajo la cabecera, con los márgenes de la pantalla.
    let grid = Rect::from_min_max(
        egui::pos2(screen.min.x + margen, fecha.max.y + 18.0 * k),
        egui::pos2(screen.max.x - margen, screen.max.y - margen),
    );
    let escena = Escena {
        pantalla: screen,
        k,
        gap: HUECO * k,
        hoy,
    };
    for (id, rect) in layout(grid, escena.gap, GRID_COLS, GRID_ROWS, &PRESET) {
        contenido(ui, state, palette, &escena, id, rect);
    }
}

/// Dibuja una tarjeta y su contenido según el widget.
fn contenido(
    ui: &mut egui::Ui,
    state: &mut UiState,
    palette: Palette,
    e: &Escena,
    id: TileId,
    rect: Rect,
) {
    let k = e.k;
    tarjeta(ui, palette, k, rect, &format!("{id:?}"));
    match id {
        TileId::Calendario => calendario(ui, palette, e, rect),
        TileId::Sistema => sistema(ui, state, palette, e, rect),
        TileId::Workspaces => workspaces(ui, state, palette, e, rect),
        TileId::Red => red(ui, state, palette, k, rect),
        TileId::Ventanas => ventanas(ui, state, palette, k, rect),
        TileId::Accesos => accesos(ui, state, palette, k, rect),
        // Sin fuente de datos todavía: estado vacío honesto, nunca valores de ejemplo.
        TileId::Clima => sin_fuente(
            ui,
            palette,
            k,
            rect,
            &Marco {
                titulo: "Clima",
                icono: ic::CLOUD,
                color: palette.muted,
                aside: "sin fuente",
            },
        ),
        TileId::Reproduciendo => sin_fuente(
            ui,
            palette,
            k,
            rect,
            &Marco {
                titulo: "Reproduciendo",
                icono: ic::MUSIC_NOTES,
                color: palette.muted,
                aside: "sin fuente",
            },
        ),
        TileId::Procesos => sin_fuente(
            ui,
            palette,
            k,
            rect,
            &Marco {
                titulo: "Procesos",
                icono: ic::CPU,
                color: palette.muted,
                aside: "sin fuente",
            },
        ),
    }
}

/// Fondo de la tarjeta. Registra además la tarjeta como objetivo de clic: así un clic dentro
/// de una tarjeta no cierra el dashboard (solo lo hace el fondo).
fn tarjeta(ui: &egui::Ui, palette: Palette, k: f32, rect: Rect, id: &str) {
    drop_shadow(ui, rect, (RADIO * k).round() as u8, 4, 18, 90);
    ui.painter().rect(
        rect,
        CornerRadius::same((RADIO * k).round() as u8),
        palette.surface,
        Stroke::new(1.0, palette.border),
        StrokeKind::Inside,
    );
    ui.interact(
        rect,
        ui.scope_id().with(("dash-card", id)),
        egui::Sense::click(),
    );
}

/// Dibuja la cabecera de la tarjeta y devuelve el área del cuerpo, debajo de ella.
fn cuerpo(ui: &egui::Ui, palette: Palette, k: f32, rect: Rect, m: &Marco) -> Rect {
    let inner = rect.shrink(PAD * k);
    let cy = inner.min.y + CABECERA * k / 2.0;
    icon_at(
        ui,
        egui::pos2(inner.min.x + ICON_MD * k / 2.0, cy),
        m.icono,
        ICON_MD * k,
        m.color,
    );
    text_at(
        ui,
        egui::pos2(inner.min.x + (ICON_MD + 6.0) * k, cy),
        Align2::LEFT_CENTER,
        m.titulo,
        TEXT_TITLE * k,
        palette.text_hi,
    );
    if !m.aside.is_empty() {
        text_at(
            ui,
            egui::pos2(inner.max.x, cy),
            Align2::RIGHT_CENTER,
            m.aside,
            TEXT_CAPTION * k,
            palette.muted,
        );
    }
    Rect::from_min_max(
        egui::pos2(inner.min.x, inner.min.y + (CABECERA + 10.0) * k),
        inner.max,
    )
}

/// Estado vacío: icono, un mensaje y una nota. «Sin datos» solo cuando falta la fuente; una
/// lista vacía es un dato real («Sin ventanas»), no una falta de datos.
fn vacio(
    ui: &egui::Ui,
    palette: Palette,
    k: f32,
    body: Rect,
    (icono, mensaje, nota): (&str, &str, &str),
) {
    let c = body.center();
    icon_at(
        ui,
        egui::pos2(c.x, c.y - 18.0 * k),
        icono,
        ICON_XL * k,
        palette.muted,
    );
    text_at(
        ui,
        egui::pos2(c.x, c.y + 6.0 * k),
        Align2::CENTER_TOP,
        mensaje,
        TEXT_BODY * k,
        palette.text,
    );
    text_at(
        ui,
        egui::pos2(c.x, c.y + 24.0 * k),
        Align2::CENTER_TOP,
        nota,
        TEXT_CAPTION * k,
        palette.muted,
    );
}

/// Tarjeta sin fuente de datos: cabecera y estado vacío.
fn sin_fuente(ui: &egui::Ui, palette: Palette, k: f32, rect: Rect, marco: &Marco) {
    let body = cuerpo(ui, palette, k, rect, marco);
    vacio(
        ui,
        palette,
        k,
        body,
        (marco.icono, "Sin datos", "Fuente de datos pendiente"),
    );
}

/// Una medida: etiqueta e icono a la izquierda, valor a la derecha y barra debajo.
fn medida(ui: &egui::Ui, palette: Palette, k: f32, r: Rect, m: &Medida) {
    let linea = r.min.y + 9.0 * k;
    icon_at(
        ui,
        egui::pos2(r.min.x + ICON_SM * k / 2.0, linea),
        m.icono,
        ICON_SM * k,
        m.color,
    );
    text_at(
        ui,
        egui::pos2(r.min.x + (ICON_SM + 5.0) * k, linea),
        Align2::LEFT_CENTER,
        m.etiqueta,
        TEXT_CAPTION * k,
        palette.muted,
    );
    text_at(
        ui,
        egui::pos2(r.max.x, linea),
        Align2::RIGHT_CENTER,
        m.valor,
        TEXT_BODY * k,
        palette.text_hi,
    );
    if let Some((fraccion, color)) = m.barra {
        let barra = Rect::from_min_size(
            egui::pos2(r.min.x, r.min.y + 22.0 * k),
            egui::vec2(r.width(), 6.0 * k),
        );
        bar_at(ui, barra, fraccion, palette.track, color);
    }
}

/// Sistema: batería, Wi-Fi, volumen y brillo en una cuadrícula 2×2.
fn sistema(ui: &egui::Ui, state: &UiState, palette: Palette, e: &Escena, rect: Rect) {
    let k = e.k;
    let body = cuerpo(
        ui,
        palette,
        k,
        rect,
        &Marco {
            titulo: "Sistema",
            icono: ic::CPU,
            color: palette.text_hi,
            aside: "",
        },
    );
    let sys = &state.shared.system;
    let bat = &state.shared.battery;

    let (bat_icono, bat_color, bat_valor, bat_barra) = if bat.present {
        let tono = battery_tone(bat.percent, bat.charging);
        let valor = if bat.charging {
            format!("{}% · cargando", bat.percent)
        } else {
            format!("{}%", bat.percent)
        };
        (
            battery_icon(bat.percent, bat.charging),
            tono,
            valor,
            Some((bat.percent as f32 / 100.0, tono)),
        )
    } else {
        (
            ic::BATTERY_EMPTY,
            palette.muted,
            "No detectada".to_string(),
            None,
        )
    };

    let (wifi_icono, wifi_color, wifi_texto) = wifi_status(sys, palette);
    let wifi_valor = wifi_texto
        .strip_prefix("Wi-Fi: ")
        .unwrap_or(&wifi_texto)
        .to_string();

    let (audio_icono, audio_color, audio_texto) = audio_status(sys, palette);
    let audio_valor = audio_texto
        .strip_prefix("Volumen ")
        .unwrap_or(&audio_texto)
        .to_string();
    let audio_barra = match &sys.volume {
        Some(v) if !v.muted => Some((v.percent as f32 / 100.0, palette.accent)),
        _ => None,
    };

    let (brillo_valor, brillo_barra, brillo_color) = match sys.brightness {
        Some(p) => (
            format!("{p}%"),
            Some((p as f32 / 100.0, palette.accent)),
            palette.text_hi,
        ),
        None => ("sin datos".to_string(), None, palette.muted),
    };

    let cuadricula = Rejilla {
        area: body,
        gap: e.gap * 2.5,
        cols: 2,
        rows: 2,
    };
    let celdas = [
        (
            cuadricula.rect(0, 0, 1, 1),
            Medida {
                icono: bat_icono,
                color: bat_color,
                etiqueta: "Batería",
                valor: &bat_valor,
                barra: bat_barra,
            },
        ),
        (
            cuadricula.rect(1, 0, 1, 1),
            Medida {
                icono: wifi_icono,
                color: wifi_color,
                etiqueta: "Wi-Fi",
                valor: &wifi_valor,
                barra: None,
            },
        ),
        (
            cuadricula.rect(0, 1, 1, 1),
            Medida {
                icono: audio_icono,
                color: audio_color,
                etiqueta: "Volumen",
                valor: &audio_valor,
                barra: audio_barra,
            },
        ),
        (
            cuadricula.rect(1, 1, 1, 1),
            Medida {
                icono: ic::SUN,
                color: brillo_color,
                etiqueta: "Brillo",
                valor: &brillo_valor,
                barra: brillo_barra,
            },
        ),
    ];
    for (r, m) in &celdas {
        medida(ui, palette, k, *r, m);
    }
}

/// Workspaces: miniaturas de los ocupados, con sus ventanas en la posición relativa real.
fn workspaces(ui: &egui::Ui, state: &UiState, palette: Palette, e: &Escena, rect: Rect) {
    let k = e.k;
    let body = cuerpo(
        ui,
        palette,
        k,
        rect,
        &Marco {
            titulo: "Workspaces",
            icono: ic::SQUARES_FOUR,
            color: palette.text_hi,
            aside: "",
        },
    );
    let mut ids = state.shared.snapshot.occupied.clone();
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        vacio(
            ui,
            palette,
            k,
            body,
            (
                ic::SQUARES_FOUR,
                "Sin workspaces",
                "Ningún workspace ocupado",
            ),
        );
        return;
    }

    let etiqueta_h = 18.0 * k;
    let area = Rect::from_min_max(
        body.min,
        egui::pos2(body.max.x, body.max.y - etiqueta_h - 6.0 * k),
    );
    let aspecto = (e.pantalla.width() / e.pantalla.height().max(1.0)).max(0.1);
    // Cada miniatura necesita unos 96 px (a 720 px); las que no caben se resumen.
    let minimo = 96.0 * k;
    let cap = ((rect.width() - 2.0 * PAD * k + e.gap) / (minimo + e.gap))
        .floor()
        .max(1.0) as usize;
    let visibles = ids.len().min(cap);
    let activo = state.shared.snapshot.workspace.parse::<u32>().ok();
    let ventana_activa = state.shared.snapshot.window_address.as_str();
    let ancho_hueco = if visibles > 1 {
        (body.width() - e.gap * (visibles as f32 - 1.0)) / visibles as f32
    } else {
        body.width()
    };

    for (i, id) in ids.iter().take(visibles).enumerate() {
        let x = body.min.x + i as f32 * (ancho_hueco + e.gap);
        let slot = Rect::from_min_size(
            egui::pos2(x, area.min.y),
            egui::vec2(ancho_hueco, area.height()),
        );
        let ancho = ancho_hueco.min(area.height() * aspecto);
        let alto = ancho / aspecto;
        // Miniatura centrada en vertical en su franja; la etiqueta va pegada debajo.
        let mini = Rect::from_center_size(
            egui::pos2(slot.center().x, area.center().y),
            egui::vec2(ancho, alto),
        );
        let es_activo = activo == Some(*id);
        ui.painter().rect(
            mini,
            CornerRadius::same((6.0 * k).round() as u8),
            palette.inset,
            Stroke::new(
                if es_activo { 1.5 } else { 1.0 },
                if es_activo {
                    palette.accent
                } else {
                    palette.border
                },
            ),
            StrokeKind::Inside,
        );
        for win in state
            .shared
            .clients
            .iter()
            .filter(|c| c.mapped && c.workspace == *id as i32)
        {
            if let Some(r) = ventana_en_miniatura(win, e.pantalla.size(), mini, 3.0 * k) {
                let enfocada = win.address == ventana_activa;
                ui.painter().rect(
                    r,
                    CornerRadius::same((3.0 * k).round() as u8),
                    if enfocada {
                        palette.accent_soft
                    } else {
                        palette.raised
                    },
                    Stroke::new(
                        1.0,
                        if enfocada {
                            palette.accent
                        } else {
                            palette.border
                        },
                    ),
                    StrokeKind::Inside,
                );
            }
        }
        let nombre = id
            .checked_sub(1)
            .and_then(|i| state.shared.config.workspaces.names.get(i as usize));
        let etiqueta = match nombre {
            Some(n) => format!("{id} · {}", truncate(n, 12)),
            None => id.to_string(),
        };
        text_at(
            ui,
            egui::pos2(slot.center().x, mini.max.y + 6.0 * k + etiqueta_h / 2.0),
            Align2::CENTER_CENTER,
            etiqueta,
            TEXT_CAPTION * k,
            if es_activo {
                palette.text
            } else {
                palette.muted
            },
        );
    }
    if ids.len() > visibles {
        text_at(
            ui,
            egui::pos2(body.max.x, body.min.y - 4.0 * k),
            Align2::RIGHT_BOTTOM,
            format!("+{} más", ids.len() - visibles),
            TEXT_CAPTION * k,
            palette.muted,
        );
    }
}

/// Calendario: el mes actual en una cuadrícula de 7 columnas; hoy resaltado.
fn calendario(ui: &egui::Ui, palette: Palette, e: &Escena, rect: Rect) {
    let k = e.k;
    let hoy = e.hoy;
    let mes = capitalizar(MESES[hoy.month0() as usize]);
    let aside = format!("{mes} {}", hoy.year());
    let body = cuerpo(
        ui,
        palette,
        k,
        rect,
        &Marco {
            titulo: "Calendario",
            icono: ic::CALENDAR_BLANK,
            color: palette.text_hi,
            aside: &aside,
        },
    );
    let cab_h = 16.0 * k;
    let ancho = body.width() / 7.0;
    let alto = (body.height() - cab_h) / 6.0;
    for (i, letra) in ["L", "M", "X", "J", "V", "S", "D"].iter().enumerate() {
        text_at(
            ui,
            egui::pos2(
                body.min.x + (i as f32 + 0.5) * ancho,
                body.min.y + cab_h / 2.0,
            ),
            Align2::CENTER_CENTER,
            letra,
            TEXT_CAPTION * k,
            palette.muted,
        );
    }
    for (i, (dia, en_mes)) in celdas_mes(hoy).into_iter().enumerate() {
        let col = (i % 7) as f32;
        let fila = (i / 7) as f32;
        let c = egui::pos2(
            body.min.x + (col + 0.5) * ancho,
            body.min.y + cab_h + (fila + 0.5) * alto,
        );
        let color = if dia == hoy {
            let radio = ancho.min(alto) * 0.42;
            ui.painter().circle(
                c,
                radio,
                palette.accent_soft,
                Stroke::new(1.0, palette.accent),
            );
            palette.text_hi
        } else if en_mes {
            palette.text
        } else {
            palette.dim
        };
        text_at(
            ui,
            c,
            Align2::CENTER_CENTER,
            dia.day().to_string(),
            TEXT_BODY * k,
            color,
        );
    }
}

/// Red: estado del Wi-Fi según `sysinfo` (solo lectura).
fn red(ui: &egui::Ui, state: &UiState, palette: Palette, k: f32, rect: Rect) {
    let (icono, color, texto) = wifi_status(&state.shared.system, palette);
    let body = cuerpo(
        ui,
        palette,
        k,
        rect,
        &Marco {
            titulo: "Red",
            icono,
            color,
            aside: "",
        },
    );
    let valor = texto.strip_prefix("Wi-Fi: ").unwrap_or(&texto).to_string();
    text_at(
        ui,
        egui::pos2(body.min.x, body.min.y + 10.0 * k),
        Align2::LEFT_TOP,
        valor,
        TEXT_DISPLAY_SM * k,
        palette.text_hi,
    );
    text_at(
        ui,
        egui::pos2(body.min.x, body.min.y + 40.0 * k),
        Align2::LEFT_TOP,
        "Estado de la conexión inalámbrica",
        TEXT_CAPTION * k,
        palette.muted,
    );
    // Icono grande del estado, abajo a la derecha: la tarjeta es de una sola línea de datos.
    icon_at(
        ui,
        egui::pos2(body.max.x - 34.0 * k, body.max.y - 34.0 * k),
        icono,
        ICON_XL * k * 2.4,
        color,
    );
}

/// Textura de la app de una clase, si el build tiene iconos reales (`icons`).
#[cfg(feature = "icons")]
fn textura_app(
    state: &mut UiState,
    ctx: &egui::Context,
    clase: &str,
) -> Option<egui::TextureHandle> {
    state.shared.icon(ctx, clase)
}

/// Sin iconos reales en este build: se usa el glifo de reserva.
#[cfg(not(feature = "icons"))]
fn textura_app(
    _state: &mut UiState,
    _ctx: &egui::Context,
    _clase: &str,
) -> Option<egui::TextureHandle> {
    None
}

/// Ventanas abiertas: clase y título de cada una (solo lectura).
fn ventanas(ui: &mut egui::Ui, state: &mut UiState, palette: Palette, k: f32, rect: Rect) {
    let lista: Vec<(String, String)> = state
        .shared
        .clients
        .iter()
        .filter(|c| c.mapped)
        .map(|c| (c.class.clone(), c.title.clone()))
        .collect();
    let aside = lista.len().to_string();
    let body = cuerpo(
        ui,
        palette,
        k,
        rect,
        &Marco {
            titulo: "Ventanas abiertas",
            icono: ic::APP_WINDOW,
            color: palette.text_hi,
            aside: &aside,
        },
    );
    if lista.is_empty() {
        vacio(
            ui,
            palette,
            k,
            body,
            (ic::APP_WINDOW, "Sin ventanas", "Ninguna ventana abierta"),
        );
        return;
    }
    let fila_h = 34.0 * k;
    let caben = ((body.height() / fila_h).floor() as usize).max(1);
    let visibles = if lista.len() > caben {
        (caben - 1).max(1)
    } else {
        lista.len()
    };
    let max_chars = ((body.width() - 34.0 * k) / (6.0 * k)).max(8.0) as usize;
    for (i, (clase, titulo)) in lista.iter().take(visibles).enumerate() {
        let fila = Rect::from_min_size(
            egui::pos2(body.min.x, body.min.y + i as f32 * fila_h),
            egui::vec2(body.width(), fila_h),
        );
        let centro = egui::pos2(fila.min.x + 11.0 * k, fila.center().y);
        match textura_app(state, ui.ctx(), clase) {
            Some(textura) => {
                let tam = 22.0 * k;
                egui::Image::new(egui::load::SizedTexture::new(
                    textura.id(),
                    egui::vec2(tam, tam),
                ))
                .paint_at(ui, Rect::from_center_size(centro, egui::vec2(tam, tam)));
            }
            None => icon_at(ui, centro, dock_icon(clase), ICON_MD * k, palette.muted),
        }
        text_at(
            ui,
            egui::pos2(fila.min.x + 30.0 * k, fila.min.y + 2.0 * k),
            Align2::LEFT_TOP,
            clase,
            TEXT_BODY * k,
            palette.text_hi,
        );
        text_at(
            ui,
            egui::pos2(fila.min.x + 30.0 * k, fila.min.y + 16.0 * k),
            Align2::LEFT_TOP,
            truncate(titulo, max_chars),
            TEXT_CAPTION * k,
            palette.muted,
        );
    }
    if lista.len() > visibles {
        text_at(
            ui,
            egui::pos2(body.min.x, body.min.y + visibles as f32 * fila_h + 4.0 * k),
            Align2::LEFT_TOP,
            format!("+{} más", lista.len() - visibles),
            TEXT_CAPTION * k,
            palette.muted,
        );
    }
}

/// Accesos rápidos: botones que ejecutan comandos internos y cierran el dashboard.
fn accesos(ui: &egui::Ui, state: &mut UiState, palette: Palette, k: f32, rect: Rect) {
    let body = cuerpo(
        ui,
        palette,
        k,
        rect,
        &Marco {
            titulo: "Accesos rápidos",
            icono: ic::LIGHTNING,
            color: palette.text_hi,
            aside: "",
        },
    );
    let rejilla = Rejilla {
        area: body,
        gap: 6.0 * k,
        cols: 2,
        rows: 3,
    };
    for (i, (comando, icono, etiqueta)) in ACCESOS.iter().enumerate() {
        let r = rejilla.rect((i % 2) as u32, (i / 2) as u32, 1, 1);
        let resp = ui.interact(
            r,
            ui.scope_id().with(("dash-acceso", *comando)),
            egui::Sense::click(),
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        ui.painter().rect(
            r,
            CornerRadius::same((10.0 * k).round() as u8),
            if resp.hovered() {
                palette.hover
            } else {
                palette.badge
            },
            Stroke::new(1.0, palette.border),
            StrokeKind::Inside,
        );
        icon_at(
            ui,
            egui::pos2(r.min.x + 14.0 * k, r.center().y),
            icono,
            ICON_MD * k,
            palette.text_hi,
        );
        text_at(
            ui,
            egui::pos2(r.min.x + 30.0 * k, r.center().y),
            Align2::LEFT_CENTER,
            etiqueta,
            TEXT_BODY * k,
            palette.text,
        );
        if resp.clicked() && dispatch_command(comando, state) {
            state.dashboard.open = false;
        }
    }
}

pub struct Dashboard;

impl Component for Dashboard {
    fn name(&self) -> &'static str {
        "dashboard"
    }

    fn draw(&mut self, ui: &mut egui::Ui, state: &mut UiState, _ctx: DrawCtx, palette: Palette) {
        if !state.dashboard.visible() {
            return;
        }
        // Fundido de entrada y salida; se restaura la opacidad para los componentes siguientes.
        let fade = crate::smootherstep(state.dashboard.progress);
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
        if state.dashboard.open {
            vec![(0, 0, screen_w, screen_h)]
        } else {
            Vec::new()
        }
    }

    fn update(&mut self, state: &mut UiState, ctx: &UpdateCtx) -> bool {
        let s = &mut state.dashboard;
        let before = s.progress;
        s.progress = approach(s.progress, if s.open { 1.0 } else { 0.0 }, ctx.dt * 7.0);
        (s.progress - before).abs() > 0.0005
    }

    fn on_key(&mut self, state: &mut UiState, key: Keysym) -> bool {
        if state.dashboard.open && key == Keysym::Escape {
            state.dashboard.open = false;
            return true;
        }
        false
    }

    fn wants_keyboard(&self, state: &UiState) -> bool {
        state.dashboard.open
    }

    fn on_command(&mut self, command: &str, state: &mut UiState) -> bool {
        if command != "dashboard" {
            return false;
        }
        // El mismo atajo abre y cierra. Solo hay un overlay a pantalla completa a la vez.
        let was_open = state.dashboard.open;
        close_overlays(state);
        state.dashboard.open = !was_open;
        state.bar.show_panel = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
    }

    /// Dos rects se solapan si su intersección tiene área positiva.
    fn solapan(a: Rect, b: Rect) -> bool {
        let i = a.intersect(b);
        i.width() > 1e-3 && i.height() > 1e-3
    }

    fn tile_ids_unicos() -> bool {
        let mut ids: Vec<String> = PRESET.iter().map(|t| format!("{:?}", t.id)).collect();
        ids.sort();
        ids.dedup();
        ids.len() == PRESET.len()
    }

    #[test]
    fn preset_cubre_las_doce_celdas_sin_solapes() {
        // Cada celda de la rejilla pertenece a exactamente un widget.
        let mut ocupadas = [[0u32; 4]; 3];
        for t in &PRESET {
            assert!(cabe(t, GRID_COLS, GRID_ROWS), "{t:?} no cabe en la rejilla");
            for r in t.row..t.row + t.h {
                for c in t.col..t.col + t.w {
                    ocupadas[r as usize][c as usize] += 1;
                }
            }
        }
        assert!(ocupadas.iter().flatten().all(|&n| n == 1));
        assert!(tile_ids_unicos());
    }

    #[test]
    fn layout_sin_solapes_dentro_del_area_y_con_huecos() {
        for a in [
            area(30.0, 110.0, 1306.0, 630.0), // 1366×768 con márgenes
            area(42.0, 150.0, 1836.0, 888.0), // 1920×1080 con márgenes
        ] {
            let gap = 12.0;
            let rects = layout(a, gap, GRID_COLS, GRID_ROWS, &PRESET);
            assert_eq!(rects.len(), PRESET.len());
            for (i, (_, r)) in rects.iter().enumerate() {
                assert!(a.contains_rect(*r), "{r:?} fuera de {a:?}");
                for (_, otro) in rects.iter().skip(i + 1) {
                    assert!(!solapan(*r, *otro), "{r:?} solapa {otro:?}");
                }
            }
            // El conjunto ocupa exactamente el área (ni más ni menos).
            let min = rects.iter().fold(a.max, |m, (_, r)| m.min(r.min));
            let max = rects.iter().fold(a.min, |m, (_, r)| m.max(r.max));
            assert!((min - a.min).length() < 1e-3);
            assert!((max - a.max).length() < 1e-3);
            // Huecos: calendario y sistema, y sistema y workspaces, separados por `gap`.
            let rect_de = |id: TileId| rects.iter().find(|(i, _)| *i == id).unwrap().1;
            let cal = rect_de(TileId::Calendario);
            let sis = rect_de(TileId::Sistema);
            let ws = rect_de(TileId::Workspaces);
            assert!((sis.min.x - cal.max.x - gap).abs() < 1e-3);
            assert!((ws.min.y - sis.max.y - gap).abs() < 1e-3);
        }
    }

    #[test]
    fn layout_descarta_widgets_que_no_caben() {
        let a = area(0.0, 0.0, 400.0, 300.0);
        let tiles = [
            Tile {
                id: TileId::Clima,
                col: 3,
                row: 0,
                w: 2,
                h: 1,
            },
            Tile {
                id: TileId::Red,
                col: 0,
                row: 0,
                w: 1,
                h: 1,
            },
        ];
        let rects = layout(a, 10.0, 4, 3, &tiles);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].0, TileId::Red);
    }

    #[test]
    fn ventana_en_miniatura_escala_y_recorta() {
        let miniatura = area(100.0, 50.0, 160.0, 90.0);
        let mut win = ClientInfo {
            class: "a".into(),
            title: "b".into(),
            address: "0x1".into(),
            mapped: true,
            workspace: 1,
            x: 960,
            y: 540,
            w: 960,
            h: 540,
        };
        let r = ventana_en_miniatura(&win, egui::vec2(1920.0, 1080.0), miniatura, 2.0).unwrap();
        assert_eq!(r.min, egui::pos2(180.0, 95.0));
        assert_eq!(r.size(), egui::vec2(80.0, 45.0));
        assert!(miniatura.contains_rect(r));
        // Fuera de la pantalla: no hay rect.
        win.x = 5000;
        assert!(ventana_en_miniatura(&win, egui::vec2(1920.0, 1080.0), miniatura, 2.0).is_none());
    }

    #[test]
    fn escala_con_referencia_720() {
        assert_eq!(escala(720.0), 1.0);
        assert_eq!(escala(360.0), 1.0);
        assert!((escala(1080.0) - 1.5).abs() < 1e-6);
        assert_eq!(escala(4000.0), 1.6);
    }

    #[test]
    fn fecha_larga_en_espanol() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        assert_eq!(fecha_larga(d), "viernes, 20 de noviembre de 2026");
        let d = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert_eq!(fecha_larga(d), "jueves, 1 de enero de 2026");
    }

    #[test]
    fn saludo_segun_la_hora() {
        assert_eq!(saludo(7), "Buenos días");
        assert_eq!(saludo(13), "Buenas tardes");
        assert_eq!(saludo(22), "Buenas noches");
        assert_eq!(saludo(3), "Buenas noches");
    }

    #[test]
    fn calendario_de_42_celdas_empieza_en_lunes() {
        // 2026-11-01 es domingo: la primera celda es el lunes 26 de octubre.
        let hoy = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        let celdas = celdas_mes(hoy);
        assert_eq!(celdas.len(), 42);
        assert_eq!(celdas[0].0, NaiveDate::from_ymd_opt(2026, 10, 26).unwrap());
        assert_eq!(celdas[0].0.weekday().num_days_from_monday(), 0);
        assert_eq!(celdas.iter().filter(|(_, en)| *en).count(), 30);
        assert!(celdas.contains(&(hoy, true)));
    }

    #[test]
    fn capitaliza_el_nombre() {
        assert_eq!(capitalizar("luis"), "Luis");
        assert_eq!(capitalizar(""), "");
    }
}
