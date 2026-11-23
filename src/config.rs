//! Configuración por archivo con recarga en caliente.
//!
//! Ruta: `$XDG_CONFIG_HOME/dinamic-ui/config.toml` (o `~/.config/dinamic-ui/config.toml`).
//! Las variables de entorno `DINAMIC_UI_*` siguen teniendo prioridad.

use std::{
    env, fs,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub bar: BarConfig,
    pub theme: ThemeConfig,
    pub sizes: Sizes,
    pub workspaces: Workspaces,
    pub notifications: NotificationConfig,
    pub clock: ClockConfig,
    pub wallpapers: WallpaperConfig,
    pub dock: DockConfig,
    pub menu: MenuConfig,
    pub icons: IconsConfig,
    pub taskbar: TaskbarConfig,
    pub topbar: TopbarConfig,
    pub overview: OverviewConfig,
    pub activities: ActivitiesConfig,
}

/// Techo de expansión de la barra: notch → compacto → barra → panel.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum BarMaxState {
    Notch,
    Compact,
    Bar,
    #[default]
    Panel,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct BarConfig {
    pub max_state: BarMaxState,
    /// Mostrar el botón `▲` dentro del notch activo (atajo mouse para seguir subiendo).
    pub notch_control: bool,
    /// Mostrar `▲▼` en el hueco entre el reloj y el estado de la barra expandida.
    pub hueco_control: bool,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            max_state: BarMaxState::Panel,
            notch_control: true,
            hueco_control: true,
        }
    }
}

/// Lado de acople de una barra o franja.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}

impl Side {
    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

/// Taskbar estilo Windows (feature `taskbar`).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct TaskbarConfig {
    pub enabled: bool,
    pub side: Side,
    pub size: u32,
    pub autohide: bool,
}

impl Default for TaskbarConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            side: Side::Bottom,
            size: 48,
            autohide: false,
        }
    }
}

/// Topbar estilo GNOME (feature `topbar`).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct TopbarConfig {
    pub enabled: bool,
    pub height: u32,
    /// Mostrar el botón `▼` para volver al estado anterior con mouse.
    pub mode_control: bool,
}

impl Default for TopbarConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            height: 34,
            mode_control: true,
        }
    }
}

/// Actividades (feature `activities`).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct ActivitiesConfig {
    /// Alto de la tira de ventanas abiertas: fracción del alto de pantalla (`0.2` = 1/5, por
    /// defecto) o píxeles (`154`). Como las franjas del overview, no es una barra de estado.
    pub strip_height: StripSize,
}

impl Default for ActivitiesConfig {
    fn default() -> Self {
        Self {
            strip_height: StripSize::Fraction(1.0 / 5.0),
        }
    }
}

/// Estilo y acople de los overviews de franja (feature `overview`).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct OverviewConfig {
    pub strip_h_side: Side,
    pub strip_v_side: Side,
    /// Alto de la franja horizontal: fracción del alto de pantalla (`0.2` = 1/5, por defecto)
    /// o píxeles (`154`).
    pub strip_h_height: StripSize,
    /// Ancho de la franja vertical: fracción del ancho de pantalla (`0.1667` = 1/6, por
    /// defecto) o píxeles (`228`).
    pub strip_v_width: StripSize,
}

impl Default for OverviewConfig {
    fn default() -> Self {
        Self {
            strip_h_side: Side::Top,
            strip_v_side: Side::Right,
            strip_h_height: StripSize::Fraction(1.0 / 5.0),
            strip_v_width: StripSize::Fraction(1.0 / 6.0),
        }
    }
}

/// Tamaño de una franja: píxeles fijos (`154`, entero) o fracción de la pantalla (`0.2`).
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum StripSize {
    Pixels(u32),
    Fraction(f32),
}

impl StripSize {
    /// Píxeles que ocupa sobre `total` (alto o ancho de pantalla): entre 40 px y la mitad.
    pub fn resolve(self, total: f32) -> i32 {
        let px = match self {
            Self::Pixels(p) => p as f32,
            Self::Fraction(f) => total * f.clamp(0.05, 0.5),
        };
        px.round().clamp(40.0, (total * 0.5).max(40.0)) as i32
    }
}

/// Iconos de aplicación (feature `icons`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct IconsConfig {
    /// Tema de iconos. Vacío: el de GTK (`gsettings`) y, si no hay, `hicolor`.
    pub theme: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MenuConfig {
    /// Directorios que indexa el buscador de archivos del command menu.
    pub file_dirs: Vec<String>,
}

impl Default for MenuConfig {
    fn default() -> Self {
        Self {
            file_dirs: vec![
                "~/Pictures".into(),
                "~/Documents".into(),
                "~/Downloads".into(),
            ],
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DockConfig {
    pub enabled: bool,
    /// Comandos/appids fijados: se lanzan con `Command::new(...)` si no están abiertos.
    pub pinned: Vec<String>,
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pinned: vec![],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ThemeConfig {
    /// Esquema base16: uno incluido (`black-metal`, `bathory`, `venom`…; ver
    /// `docs/DESIGN_TOKENS.md`) o la ruta a un archivo base16 (`~/.config/…/esquema.yaml`).
    pub scheme: Option<String>,
    /// Alias antiguo de `scheme`.
    pub name: Option<String>,
    /// Corrige los colores de texto e indicadores que no alcancen el contraste WCAG
    /// (4,5:1 texto, 3:1 elementos). Apagado: se respeta el esquema y solo se avisa en el log.
    pub contrast_correction: bool,
    /// Sustituye slots del esquema: `base0B = "ddeecc"`.
    pub slots: std::collections::HashMap<String, String>,
    /// Colores hexadecimales opcionales (RRGGBB) que sobrescriben un token. `accent` también
    /// acepta un slot (`"base0A"`); por defecto es `base0B`.
    pub surface: Option<String>,
    pub border: Option<String>,
    pub badge: Option<String>,
    pub text: Option<String>,
    pub muted: Option<String>,
    pub accent: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct Sizes {
    pub bar_h: u32,
    pub panel_h: u32,
    pub island_w: u32,
    pub bar_w: u32,
}

/// Carpeta de imágenes que lista la pestaña Wallpaper (`~` se expande).
/// Sin esta sección: `~/Pictures/Unsplash`, y si no existe, `~/Pictures`.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct WallpaperConfig {
    pub dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Workspaces {
    pub count: u32,
    /// Nombres opcionales: el primero es el workspace 1 (`["Principal", "Desarrollo"]`).
    pub names: Vec<String>,
}

impl Workspaces {
    /// Nombre configurado del workspace `id`, si lo hay.
    pub fn name_of(&self, id: u32) -> Option<&str> {
        let name = self.names.get(id.checked_sub(1)? as usize)?.trim();
        (!name.is_empty()).then_some(name)
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct NotificationConfig {
    pub ttl_secs: u64,
}

/// Pestaña Clock del panel. Si falta la sección se usan Bogotá, Madrid y Tokio.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ClockConfig {
    pub cities: Vec<City>,
}

/// Ciudad del reloj mundial con desplazamiento fijo respecto a UTC (no hay
/// base de datos de zonas). `dst = "eu"` suma 1 h en el horario de verano europeo.
#[derive(Debug, Clone, Deserialize)]
pub struct City {
    pub name: String,
    pub utc_offset: f32,
    #[serde(default)]
    pub dst: Option<String>,
}

impl Default for ClockConfig {
    fn default() -> Self {
        let city = |name: &str, utc_offset: f32, dst: Option<&str>| City {
            name: name.to_string(),
            utc_offset,
            dst: dst.map(str::to_string),
        };
        Self {
            cities: vec![
                city("Bogotá", -5.0, None),
                city("Madrid", 1.0, Some("eu")),
                city("Tokio", 9.0, None),
            ],
        }
    }
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self { ttl_secs: 6 }
    }
}

impl Default for Sizes {
    fn default() -> Self {
        Self {
            bar_h: 32,
            panel_h: 360,
            island_w: 260,
            bar_w: 640,
        }
    }
}

impl Default for Workspaces {
    fn default() -> Self {
        Self {
            count: 10,
            names: Vec::new(),
        }
    }
}

static CONFIG: RwLock<Option<Arc<Config>>> = RwLock::new(None);

pub fn init() {
    if let Err(e) = reload() {
        eprintln!("[dinamic-ui] config: {e}");
    }
}

pub fn get() -> Arc<Config> {
    CONFIG
        .read()
        .expect("config lock envenenado")
        .clone()
        .unwrap_or_default()
}

/// Fija la configuración en memoria sin tocar el archivo (previewer: sliders en vivo).
pub fn set(config: Config) {
    *CONFIG.write().expect("config lock envenenado") = Some(Arc::new(config));
}

/// Recarga el archivo. Si falla, mantiene la configuración anterior.
pub fn reload() -> Result<(), String> {
    let path = path().ok_or_else(|| "XDG_CONFIG_HOME no definido".to_string())?;
    let mut config: Config = match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };

    // Los env vars siguen mandando (útil para probar sin tocar el archivo).
    if let Ok(v) = env::var("DINAMIC_UI_BAR_H") {
        if let Ok(n) = v.parse() {
            config.sizes.bar_h = n;
        }
    }
    if let Ok(v) = env::var("DINAMIC_UI_PANEL_H") {
        if let Ok(n) = v.parse() {
            config.sizes.panel_h = n;
        }
    }
    if let Ok(v) = env::var("DINAMIC_UI_THEME") {
        config.theme.name = Some(v);
    }

    config.sizes.bar_h = config.sizes.bar_h.clamp(24, 48);
    config.sizes.panel_h = config.sizes.panel_h.clamp(80, 480);
    config.workspaces.count = config.workspaces.count.clamp(4, 20);
    config.taskbar.size = config.taskbar.size.clamp(40, 72);
    config.topbar.height = config.topbar.height.clamp(24, 48);

    *CONFIG.write().expect("config lock envenenado") = Some(Arc::new(config));
    Ok(())
}

pub fn path() -> Option<PathBuf> {
    let base = env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env::var("HOME").unwrap_or_default()).join(".config"));
    Some(base.join("dinamic-ui").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_size_resolves_fractions_and_pixels() {
        // 1366×768: 1/5 del alto = 154 px y 1/6 del ancho = 228 px.
        assert_eq!(StripSize::Fraction(0.2).resolve(768.0), 154);
        assert_eq!(StripSize::Fraction(1.0 / 6.0).resolve(1366.0), 228);
        assert_eq!(StripSize::Fraction(0.25).resolve(1080.0), 270);
        assert_eq!(StripSize::Pixels(200).resolve(768.0), 200);
        // Límites: nunca menos de 40 px ni más de la mitad de la pantalla.
        assert_eq!(StripSize::Pixels(5).resolve(768.0), 40);
        assert_eq!(StripSize::Pixels(9999).resolve(768.0), 384);
        assert_eq!(StripSize::Fraction(0.9).resolve(1000.0), 500);
        assert_eq!(StripSize::Fraction(0.0).resolve(1000.0), 50);
    }

    #[test]
    fn activities_strip_height_defaults_to_a_fifth_and_accepts_both_forms() {
        let default: Config = toml::from_str("").unwrap();
        assert_eq!(default.activities.strip_height, StripSize::Fraction(0.2));
        let frac: Config = toml::from_str("[activities]\nstrip_height = 0.3\n").unwrap();
        assert_eq!(frac.activities.strip_height, StripSize::Fraction(0.3));
        let px: Config = toml::from_str("[activities]\nstrip_height = 180\n").unwrap();
        assert_eq!(px.activities.strip_height, StripSize::Pixels(180));
        // 1/5 de 768 px = 154 px: una tira de verdad.
        assert_eq!(default.activities.strip_height.resolve(768.0), 154);
    }

    #[test]
    fn overview_config_parses_fraction_and_pixels_and_has_defaults() {
        let cfg: Config =
            toml::from_str("[overview]\nstrip_h_height = 0.25\nstrip_v_width = 240\n").unwrap();
        assert_eq!(cfg.overview.strip_h_height, StripSize::Fraction(0.25));
        assert_eq!(cfg.overview.strip_v_width, StripSize::Pixels(240));
        let default: Config = toml::from_str("").unwrap();
        assert_eq!(default.overview.strip_h_height, StripSize::Fraction(0.2));
        assert_eq!(default.overview.strip_h_side, Side::Top);
    }
}
