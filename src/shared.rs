//! Datos y colas compartidos por todos los componentes del shell.
//!
//! Fase 2 del plan de modularización: ningún componente consulta a otro; todos
//! leen/escriben aquí. Los iconos viven aquí (no en el dock) para que overview y
//! command menu los tengan aunque el dock no esté compilado.

#[cfg(feature = "icons")]
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "icons")]
use egui::TextureHandle;

use crate::{
    config,
    ipc::{ClientInfo, HyprlandSnapshot},
    power::BatteryState,
    sysinfo::SystemInfo,
    theme::Palette,
};

/// Acción diferida del dock; el propio dock la traduce a órdenes en cola.
#[cfg(feature = "dock")]
#[derive(Debug, Clone)]
pub enum DockAction {
    Focus(String),
    Launch(String),
}

/// Estado común: datos del compositor, sistema, iconos y colas de acciones.
pub struct Shared {
    /// Reservados para el registro de componentes de Fase 3.
    #[allow(dead_code)]
    pub config: Arc<config::Config>,
    #[allow(dead_code)]
    pub palette: Palette,
    pub snapshot: HyprlandSnapshot,
    /// Clientes mapeados, consultados por el núcleo (no por el dock).
    pub clients: Vec<ClientInfo>,
    pub battery: BatteryState,
    pub system: SystemInfo,
    /// Sube cada vez que el hilo de sistema publica datos nuevos.
    pub system_revision: u64,
    /// Un componente pidió recargar config y paleta (lo ejecuta el núcleo).
    pub reload_requested: bool,
    /// Acciones de QuickSettings con su semáforo (ámbar) y log de ciclo de vida.
    pub settings: crate::actions::SettingsState,
    #[cfg(feature = "icons")]
    icons: IconCache,
    /// Destello breve cuando cambia el workspace/ventana.
    pub pulse: f32,
    // ── Colas diferidas ───────────────────────────────────────────────
    pub pending_commands: Vec<String>,
    pub pending_spawns: Vec<Vec<String>>,
    #[cfg(feature = "dock")]
    pub dock_action: Option<DockAction>,
    #[cfg(feature = "overview")]
    pub overview_focus: Option<String>,
    #[cfg(feature = "overview")]
    pub overview_workspace: Option<u32>,
    #[cfg(feature = "command-menu")]
    pub menu_action: Option<crate::launcher::EntryAction>,
}

/// Iconos de app: carga en un hilo y sube a textura bajo demanda.
#[cfg(feature = "icons")]
struct IconCache {
    loader: crate::appicons::AppIcons,
    textures: HashMap<String, TextureHandle>,
}

#[cfg(feature = "icons")]
impl IconCache {
    fn new() -> Self {
        Self {
            loader: crate::appicons::AppIcons::new(),
            textures: HashMap::new(),
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

impl Shared {
    /// Textura del icono de una clase/nombre de app. La primera llamada lo pide al
    /// hilo de carga; mientras no esté listo (o si solo existe en SVG) devuelve
    /// `None` y quien llama dibuja su glifo de reserva.
    #[cfg(feature = "icons")]
    pub fn icon(&mut self, ctx: &egui::Context, class: &str) -> Option<TextureHandle> {
        let cache = &mut self.icons;
        cache.loader.request(class);
        for (name, rgba) in cache.loader.drain() {
            let size = crate::appicons::SIZE as usize;
            let image = egui::ColorImage::from_rgba_unmultiplied([size, size], &rgba);
            let handle = ctx.load_texture(
                format!("appicon-{name}"),
                image,
                egui::TextureOptions::LINEAR,
            );
            cache.textures.insert(name, handle);
        }
        cache.textures.get(&class.to_lowercase()).cloned()
    }

    pub fn new() -> Self {
        Self {
            config: config::get(),
            palette: Palette::load(),
            snapshot: HyprlandSnapshot::default(),
            clients: Vec::new(),
            battery: BatteryState::default(),
            system: SystemInfo::default(),
            system_revision: 0,
            reload_requested: false,
            settings: crate::actions::SettingsState::default(),
            #[cfg(feature = "icons")]
            icons: IconCache::new(),
            pulse: 0.0,
            pending_commands: Vec::new(),
            pending_spawns: Vec::new(),
            #[cfg(feature = "dock")]
            dock_action: None,
            #[cfg(feature = "overview")]
            overview_focus: None,
            #[cfg(feature = "overview")]
            overview_workspace: None,
            #[cfg(feature = "command-menu")]
            menu_action: None,
        }
    }
}
