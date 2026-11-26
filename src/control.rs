//! Control remoto del shell: un socket Unix + un modo CLI (`dinamic-ui --toggle`).
//! Así se puede manejar la UI con keybinds de Hyprland sin depender de que
//! el surface layer tenga el foco del teclado.

use std::{
    env, fs,
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Toggle,
    Expand,
    Collapse,
    Panel,
    Quit,
    Reload,
    Compact,
    Notch,
    Overview,
    Activities,
    Dashboard,
    ConfigPanel,
    OverviewStripH,
    OverviewStripV,
    WindowSwitcher,
    WindowSwitcherPrev,
    WorkspaceSwitcher,
    WorkspaceSwitcherPrev,
    Taskbar,
    Topbar,
    Menu,
    OsdVolume,
    OsdBrightness,
    Screenshots,
}

impl Command {
    pub fn name(self) -> &'static str {
        match self {
            Self::Toggle => "toggle",
            Self::Expand => "expand",
            Self::Collapse => "collapse",
            Self::Panel => "panel",
            Self::Quit => "quit",
            Self::Reload => "reload",
            Self::Compact => "compact",
            Self::Notch => "notch",
            Self::Overview => "overview",
            Self::Activities => "activities",
            Self::Dashboard => "dashboard",
            Self::ConfigPanel => "config-panel",
            Self::OverviewStripH => "overview-strip-h",
            Self::OverviewStripV => "overview-strip-v",
            Self::WindowSwitcher => "window-switcher",
            Self::WindowSwitcherPrev => "window-switcher-prev",
            Self::WorkspaceSwitcher => "workspace-switcher",
            Self::WorkspaceSwitcherPrev => "workspace-switcher-prev",
            Self::Taskbar => "taskbar",
            Self::Topbar => "topbar",
            Self::Menu => "menu",
            Self::OsdVolume => "osd-volume",
            Self::OsdBrightness => "osd-brightness",
            Self::Screenshots => "screenshots",
        }
    }

    /// Feature de Cargo que implementa el comando y si este binario la incluye.
    /// `None`: lo atiende el núcleo (siempre disponible).
    pub(crate) fn feature(self) -> Option<(&'static str, bool)> {
        match self {
            Self::Toggle
            | Self::Expand
            | Self::Collapse
            | Self::Panel
            | Self::Compact
            | Self::Notch => Some(("bar", cfg!(feature = "bar"))),
            Self::Overview | Self::OverviewStripH | Self::OverviewStripV => {
                Some(("overview", cfg!(feature = "overview")))
            }
            Self::WindowSwitcher
            | Self::WindowSwitcherPrev
            | Self::WorkspaceSwitcher
            | Self::WorkspaceSwitcherPrev => Some(("switcher", cfg!(feature = "switcher"))),
            Self::Activities => Some(("activities", cfg!(feature = "activities"))),
            Self::Dashboard => Some(("dashboard", cfg!(feature = "dashboard"))),
            Self::ConfigPanel => Some(("config-panel", cfg!(feature = "config-panel"))),
            Self::Taskbar => Some(("taskbar", cfg!(feature = "taskbar"))),
            Self::Topbar => Some(("topbar", cfg!(feature = "topbar"))),
            Self::Menu => Some(("command-menu", cfg!(feature = "command-menu"))),
            Self::OsdVolume | Self::OsdBrightness => Some(("osd", cfg!(feature = "osd"))),
            Self::Screenshots => Some(("screenshots", cfg!(feature = "screenshots"))),
            Self::Quit | Self::Reload => None,
        }
    }

    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "toggle" => Some(Self::Toggle),
            "expand" => Some(Self::Expand),
            "collapse" => Some(Self::Collapse),
            "panel" => Some(Self::Panel),
            "quit" => Some(Self::Quit),
            "reload" => Some(Self::Reload),
            "compact" => Some(Self::Compact),
            "notch" => Some(Self::Notch),
            "overview" => Some(Self::Overview),
            // `overview-activities` era el nombre antiguo: se acepta para no romper atajos.
            "activities" | "overview-activities" => Some(Self::Activities),
            "dashboard" => Some(Self::Dashboard),
            // `config` es el nombre corto para el atajo.
            "config-panel" | "config" => Some(Self::ConfigPanel),
            "overview-strip-h" => Some(Self::OverviewStripH),
            "overview-strip-v" => Some(Self::OverviewStripV),
            "window-switcher" => Some(Self::WindowSwitcher),
            "window-switcher-prev" => Some(Self::WindowSwitcherPrev),
            "workspace-switcher" => Some(Self::WorkspaceSwitcher),
            "workspace-switcher-prev" => Some(Self::WorkspaceSwitcherPrev),
            "taskbar" => Some(Self::Taskbar),
            "topbar" => Some(Self::Topbar),
            "menu" => Some(Self::Menu),
            "osd-volume" => Some(Self::OsdVolume),
            "osd-brightness" => Some(Self::OsdBrightness),
            "screenshots" => Some(Self::Screenshots),
            _ => None,
        }
    }
}

pub fn socket_path() -> Option<PathBuf> {
    let runtime = env::var("XDG_RUNTIME_DIR").ok()?;
    Some(PathBuf::from(runtime).join("dinamic-ui.sock"))
}

/// Si el binario se llamó con un comando, lo envía y devuelve el código de salida.
pub fn handle_cli() -> Option<i32> {
    let arg = env::args().nth(1)?;
    let arg = arg.trim_start_matches('-');
    let command = Command::parse(arg)?;
    if let Some((feature, false)) = command.feature() {
        eprintln!(
            "[dinamic-ui] `--{arg}` no disponible: este binario se compiló sin la feature `{feature}`"
        );
        return Some(1);
    }

    let Some(path) = socket_path() else {
        eprintln!("[dinamic-ui] XDG_RUNTIME_DIR no definido");
        return Some(1);
    };
    match UnixStream::connect(&path) {
        Ok(mut stream) => {
            let _ = stream.write_all(command.name().as_bytes());
            Some(0)
        }
        Err(e) => {
            eprintln!("[dinamic-ui] no se pudo conectar con el shell: {e}");
            Some(1)
        }
    }
}

pub struct ControlServer {
    listener: UnixListener,
    path: PathBuf,
}

impl ControlServer {
    pub fn new() -> std::io::Result<Self> {
        let path = socket_path().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "XDG_RUNTIME_DIR no definido")
        })?;
        let _ = fs::remove_file(&path);
        let listener = UnixListener::bind(&path)?;
        listener.set_nonblocking(true)?;
        Ok(Self { listener, path })
    }

    /// Clonar el listener para registrarlo como fuente de calloop (level-triggered).
    pub fn try_clone_listener(&self) -> std::io::Result<UnixListener> {
        self.listener.try_clone()
    }

    pub fn poll(&mut self) -> Vec<Command> {
        let mut commands = Vec::new();
        while let Ok((mut stream, _)) = self.listener.accept() {
            let mut buf = [0u8; 64];
            if let Ok(n) = stream.read(&mut buf) {
                let text = String::from_utf8_lossy(&buf[..n]);
                if let Some(command) = Command::parse(&text) {
                    commands.push(command);
                }
            }
        }
        commands
    }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
