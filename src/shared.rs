//! Datos y colas compartidos por todos los componentes del shell.
//!
//! Fase 2 del plan de modularización: ningún componente consulta a otro; todos
//! leen/escriben aquí. En Fase 3 estos campos se repartirán en `Shared` con
//! `#[cfg]` por feature.

use std::{collections::HashMap, sync::Arc};

use egui::TextureHandle;

use crate::{
    appicons::AppIcons,
    config,
    ipc::{ClientInfo, HyprlandSnapshot},
    power::BatteryState,
    sysinfo::SystemInfo,
    theme::Palette,
};

/// Acción diferida del dock: `main` la ejecuta fuera del render.
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
    pub app_icons: AppIcons,
    pub app_icon_textures: HashMap<String, TextureHandle>,
    /// Destello breve cuando cambia el workspace/ventana.
    pub pulse: f32,
    // ── Colas diferidas ───────────────────────────────────────────────
    pub pending_commands: Vec<String>,
    pub pending_spawns: Vec<Vec<String>>,
    pub dock_action: Option<DockAction>,
    pub overview_focus: Option<String>,
    pub overview_workspace: Option<u32>,
    pub menu_action: Option<crate::launcher::EntryAction>,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            config: config::get(),
            palette: Palette::load(),
            snapshot: HyprlandSnapshot::default(),
            clients: Vec::new(),
            battery: BatteryState::default(),
            system: SystemInfo::default(),
            app_icons: AppIcons::new(),
            app_icon_textures: HashMap::new(),
            pulse: 0.0,
            pending_commands: Vec::new(),
            pending_spawns: Vec::new(),
            dock_action: None,
            overview_focus: None,
            overview_workspace: None,
            menu_action: None,
        }
    }
}
