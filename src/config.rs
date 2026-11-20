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
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            max_state: BarMaxState::Panel,
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
}

impl Default for TopbarConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            height: 34,
        }
    }
}

/// Estilo y acople de los overviews de franja (feature `overview`).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct OverviewConfig {
    pub strip_h_side: Side,
    pub strip_v_side: Side,
}

impl Default for OverviewConfig {
    fn default() -> Self {
        Self {
            strip_h_side: Side::Top,
            strip_v_side: Side::Right,
        }
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
    /// "black-metal" (default) o "bathory".
    pub name: Option<String>,
    /// Colores hexadecimales opcionales (RRGGBB) que sobrescriben la paleta.
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

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct Workspaces {
    pub count: u32,
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
        Self { count: 10 }
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
