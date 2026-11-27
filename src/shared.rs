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
    /// Orden MRU de ventanas por dirección: la más reciente, primero.
    pub mru: Vec<String>,
    /// Orden MRU de workspaces por id: el visitado más recientemente, primero.
    pub workspace_mru: Vec<u32>,
    /// Margen horizontal de la franja de activación del dock (px).
    pub dock_activate_margin: i32,
    /// Margen de permanencia alrededor del dock desplegado (px).
    pub dock_persist_margin: i32,
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
    /// RGBA listo esperando su turno para subir a GPU.
    pending: Vec<(String, Vec<u8>)>,
    textures: HashMap<String, TextureHandle>,
    theme: String,
    theme_config: String,
    /// Presupuesto de subidas por frame: reparte el tirón al abrir un overlay.
    budget: usize,
}

/// Texturas nuevas como máximo por frame (a 60 fps, ~480/s).
#[cfg(feature = "icons")]
const ICON_UPLOAD_BUDGET: usize = 8;

#[cfg(feature = "icons")]
impl IconCache {
    fn new() -> Self {
        Self {
            loader: crate::appicons::AppIcons::new(),
            pending: Vec::new(),
            textures: HashMap::new(),
            theme: crate::appicons::AppIcons::theme(),
            theme_config: config::get().icons.theme.clone().unwrap_or_default(),
            budget: ICON_UPLOAD_BUDGET,
        }
    }

    fn begin_frame(&mut self) {
        self.budget = ICON_UPLOAD_BUDGET;
        // Comparamos la config en crudo: `AppIcons::theme()` consulta GTK y es
        // mejor no hacerlo en cada frame.
        let raw = config::get().icons.theme.clone().unwrap_or_default();
        if raw != self.theme_config {
            self.theme_config = raw;
            self.loader = crate::appicons::AppIcons::new();
            self.textures.clear();
            self.pending.clear();
            self.theme = crate::appicons::AppIcons::theme();
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

impl Shared {
    /// Mueve una ventana al frente del MRU, sin duplicados.
    pub fn touch_window(&mut self, address: &str) {
        if address.is_empty() {
            return;
        }
        self.mru.retain(|a| a != address);
        self.mru.insert(0, address.to_string());
        if self.mru.len() > 64 {
            self.mru.truncate(64);
        }
    }

    /// Ventana más recientemente enfocada entre las de una clase, según el MRU
    /// global. Así un clic en el dock/taskbar vuelve a la ventana que dejaste,
    /// en vez de elegir siempre la primera.
    pub fn last_focused_of(
        &self,
        class: &str,
        windows: &[crate::ipc::ClientInfo],
    ) -> Option<String> {
        self.mru.iter().find_map(|address| {
            windows
                .iter()
                .find(|w| &w.address == address && w.class.eq_ignore_ascii_case(class))
                .map(|w| w.address.clone())
        })
    }

    /// Mueve un workspace al frente del MRU, sin duplicados.
    pub fn touch_workspace(&mut self, id: u32) {
        self.workspace_mru.retain(|&w| w != id);
        self.workspace_mru.insert(0, id);
        if self.workspace_mru.len() > 32 {
            self.workspace_mru.truncate(32);
        }
    }

    /// Textura del icono de una clase/nombre de app. La primera llamada lo pide al
    /// hilo de carga; mientras no esté listo devuelve `None` y quien llama dibuja
    /// su glifo de reserva. Las subidas a GPU se reparten por frames.
    #[cfg(feature = "icons")]
    pub fn icon(&mut self, ctx: &egui::Context, class: &str) -> Option<TextureHandle> {
        let cache = &mut self.icons;
        cache.loader.request(class);
        cache.pending.extend(cache.loader.drain());
        let size = crate::appicons::SIZE as usize;
        while cache.budget > 0 && !cache.pending.is_empty() {
            let (name, rgba) = cache.pending.remove(0);
            let image = egui::ColorImage::from_rgba_unmultiplied([size, size], &rgba);
            let handle = ctx.load_texture(
                format!("appicon-{name}"),
                image,
                egui::TextureOptions::LINEAR,
            );
            cache.textures.insert(name, handle);
            cache.budget -= 1;
        }
        cache.textures.get(&class.to_lowercase()).cloned()
    }

    /// Prepara el frame en curso (presupuesto de iconos y tema).
    /// Lo llama `ui::draw` antes de dibujar cualquier componente.
    #[cfg(feature = "icons")]
    pub fn begin_frame(&mut self) {
        self.icons.begin_frame();
    }

    #[cfg(not(feature = "icons"))]
    pub fn begin_frame(&mut self) {}

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
            mru: Vec::new(),
            workspace_mru: Vec::new(),
            dock_activate_margin: 32,
            dock_persist_margin: 12,
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
