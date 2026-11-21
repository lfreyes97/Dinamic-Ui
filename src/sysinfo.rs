//! Datos del sistema para la pestaña System del panel (Wi-Fi, Bluetooth,
//! brillo, volumen, dispositivos y layout de Hyprland).
//!
//! Se leen con comandos de Arch (`nmcli`, `bluetoothctl`, `pactl`,
//! `brightnessctl`, `hyprctl`, `awww`) en un hilo propio, solo mientras el panel está
//! solo los grupos que algún componente pide (`Needs`) y como mucho cada `INTERVAL`. Si un
//! comando no existe, falla o tarda más de `TIMEOUT`, el campo queda en `None` y la UI muestra
//! "Sin datos".

use std::{
    io::Read,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use crate::needs::{Need, Needs};

const INTERVAL: Duration = Duration::from_secs(5);
const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wifi {
    pub enabled: bool,
    /// Red a la que está conectada la interfaz Wi-Fi, si hay.
    pub ssid: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bluetooth {
    pub powered: bool,
    pub connected: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Volume {
    pub percent: u8,
    pub muted: bool,
}

/// Fondo de escritorio actual según `awww query`.
#[cfg(feature = "wallpaper")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wallpaper {
    Image(String),
    /// Color sólido en `rrggbb`.
    Color(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemInfo {
    pub wifi: Option<Wifi>,
    pub bluetooth: Option<Bluetooth>,
    pub brightness: Option<u8>,
    pub volume: Option<Volume>,
    /// Descripción de la salida de audio por defecto.
    pub audio_output: Option<String>,
    /// Teclado principal: "distribución · nombre".
    pub keyboard: Option<String>,
    /// Monitor enfocado: "nombre · 1366×768 @ 60 Hz".
    pub display: Option<String>,
    /// Valor actual de `general:layout` en Hyprland.
    pub layout: Option<String>,
    /// Fondo actual del primer monitor (`awww query`).
    #[cfg(feature = "wallpaper")]
    pub wallpaper: Option<Wallpaper>,
}

/// Hilo de lectura + último resultado compartido.
pub struct SystemMonitor {
    shared: Arc<Mutex<(SystemInfo, u64)>>,
    needs: Arc<AtomicU8>,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let shared = Arc::new(Mutex::new((SystemInfo::default(), 0)));
        let needs = Arc::new(AtomicU8::new(0));
        let (thread_shared, thread_needs) = (Arc::clone(&shared), Arc::clone(&needs));
        thread::Builder::new()
            .name("dinamic-ui-sysinfo".into())
            .spawn(move || worker(thread_shared, thread_needs))
            .ok();
        Self { shared, needs }
    }

    /// Solo se consultan los datos que algún componente ha pedido.
    pub fn set_needs(&self, needs: Needs) {
        self.needs.store(needs.bits(), Ordering::Relaxed);
    }

    /// Devuelve el último estado y su revisión (cambia cuando cambian los datos).
    pub fn poll(&self) -> Option<(SystemInfo, u64)> {
        self.shared.lock().ok().map(|s| s.clone())
    }
}

fn worker(shared: Arc<Mutex<(SystemInfo, u64)>>, needs: Arc<AtomicU8>) {
    let mut last: Option<Instant> = None;
    let mut was = Needs::NONE;
    loop {
        let requested = Needs::from_bits(needs.load(Ordering::Relaxed));
        // Cuando cambia lo pedido se lee en seguida; después, cada INTERVAL.
        let due = !requested.is_empty()
            && (requested != was || last.is_none_or(|t| t.elapsed() >= INTERVAL));
        was = requested;
        if due {
            last = Some(Instant::now());
            let info = read(requested);
            if let Ok(mut s) = shared.lock() {
                if s.0 != info {
                    s.0 = info;
                    s.1 = s.1.wrapping_add(1);
                }
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
}

/// Lee solo los grupos de datos pedidos; los demás quedan en `None`.
fn read(needs: Needs) -> SystemInfo {
    let mut info = SystemInfo::default();
    if needs.get(Need::Audio) {
        let (volume, audio_output) = read_audio();
        info.volume = volume;
        info.audio_output = audio_output;
    }
    if needs.get(Need::Network) {
        info.wifi = read_wifi();
        info.bluetooth = read_bluetooth();
    }
    if needs.get(Need::Display) {
        let (keyboard, display) = read_hypr_devices();
        info.keyboard = keyboard;
        info.display = display;
        info.brightness = read_brightness();
        #[cfg(feature = "wallpaper")]
        {
            info.wallpaper = read_wallpaper();
        }
    }
    if needs.get(Need::Layout) {
        info.layout = read_layout();
    }
    info
}

/// Ejecuta un comando con `LC_ALL=C` y límite de tiempo. `None` si no existe,
/// falla o no termina a tiempo (en ese caso se mata al hijo).
fn run(program: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    status.success().then_some(out)
}

/// Lanza un comando sin esperar al resultado (un hilo recoge al hijo para
/// no dejar procesos zombi). Errores solo por stderr.
pub fn spawn_detached(argv: Vec<String>) {
    let Some((program, args)) = argv.split_first() else {
        return;
    };
    let (program, args) = (program.clone(), args.to_vec());
    thread::spawn(move || {
        let result = Command::new(&program)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if let Err(e) = result {
            eprintln!("[dinamic-ui] {program}: {e}");
        }
    });
}

fn read_wifi() -> Option<Wifi> {
    let enabled = run("nmcli", &["radio", "wifi"])?.trim() == "enabled";
    let devices = run("nmcli", &["-t", "-f", "TYPE,STATE,CONNECTION", "device"])?;
    let ssid = devices.lines().find_map(|line| {
        let mut parts = line.splitn(3, ':');
        let (kind, state, name) = (parts.next()?, parts.next()?, parts.next()?);
        (kind == "wifi" && state == "connected" && !name.is_empty())
            .then(|| name.replace("\\:", ":"))
    });
    Some(Wifi { enabled, ssid })
}

fn read_bluetooth() -> Option<Bluetooth> {
    let show = run("bluetoothctl", &["show"])?;
    let powered = show
        .lines()
        .find_map(|l| l.trim().strip_prefix("Powered:"))
        .map(|v| v.trim() == "yes")?;
    let connected = run("bluetoothctl", &["devices", "Connected"])
        .map(|out| out.lines().filter(|l| l.starts_with("Device ")).count())
        .unwrap_or(0);
    Some(Bluetooth { powered, connected })
}

/// `brightnessctl -m` → "amdgpu_bl1,backlight,18154,28%,65535".
fn read_brightness() -> Option<u8> {
    let out = run("brightnessctl", &["-m"])?;
    let field = out.lines().next()?.split(',').nth(3)?;
    field.trim().trim_end_matches('%').parse().ok()
}

fn read_audio() -> (Option<Volume>, Option<String>) {
    let volume = (|| {
        let out = run("pactl", &["get-sink-volume", "@DEFAULT_SINK@"])?;
        let percent = out
            .split_whitespace()
            .find_map(|w| w.strip_suffix('%')?.parse::<u32>().ok())?;
        let muted =
            run("pactl", &["get-sink-mute", "@DEFAULT_SINK@"]).is_some_and(|m| m.contains("yes"));
        Some(Volume {
            percent: percent.min(255) as u8,
            muted,
        })
    })();

    // La descripción legible requiere el JSON de pactl (PulseAudio/PipeWire ≥ 16).
    let output = (|| {
        let default = run("pactl", &["get-default-sink"])?;
        let default = default.trim();
        let json = run("pactl", &["-f", "json", "list", "sinks"])?;
        let sinks: Vec<serde_json::Value> = serde_json::from_str(&json).ok()?;
        let sink = sinks.iter().find(|s| s["name"].as_str() == Some(default))?;
        Some(sink["description"].as_str().unwrap_or(default).to_string())
    })();
    (volume, output)
}

fn read_hypr_devices() -> (Option<String>, Option<String>) {
    let keyboard = (|| {
        let json: serde_json::Value =
            serde_json::from_str(&run("hyprctl", &["-j", "devices"])?).ok()?;
        let keyboards = json["keyboards"].as_array()?;
        let main = keyboards
            .iter()
            .find(|k| k["main"].as_bool() == Some(true))
            .or_else(|| keyboards.first())?;
        let name = main["name"].as_str().unwrap_or("teclado");
        Some(match main["active_keymap"].as_str() {
            Some(keymap) => format!("{keymap} · {name}"),
            None => name.to_string(),
        })
    })();
    let display = (|| {
        let json: serde_json::Value =
            serde_json::from_str(&run("hyprctl", &["-j", "monitors"])?).ok()?;
        let monitors = json.as_array()?;
        let m = monitors
            .iter()
            .find(|m| m["focused"].as_bool() == Some(true))
            .or_else(|| monitors.first())?;
        Some(format!(
            "{} · {}×{} @ {:.0} Hz",
            m["name"].as_str().unwrap_or("?"),
            m["width"].as_u64()?,
            m["height"].as_u64()?,
            m["refreshRate"].as_f64()?,
        ))
    })();
    (keyboard, display)
}

#[cfg(feature = "wallpaper")]
/// `awww query` → ": eDP-1: 1366x768, scale: 1, currently displaying: image: /ruta.png"
/// (o `color: rrggbb`). Solo se lee el primer monitor.
fn read_wallpaper() -> Option<Wallpaper> {
    parse_wallpaper(&run("awww", &["query"])?)
}

#[cfg(feature = "wallpaper")]
fn parse_wallpaper(out: &str) -> Option<Wallpaper> {
    let shown = out
        .lines()
        .next()?
        .split_once("currently displaying:")?
        .1
        .trim();
    if let Some(path) = shown.strip_prefix("image:") {
        return Some(Wallpaper::Image(path.trim().to_string()));
    }
    let color = shown.strip_prefix("color:")?.trim();
    Some(Wallpaper::Color(color.to_lowercase()))
}

fn read_layout() -> Option<String> {
    let json: serde_json::Value =
        serde_json::from_str(&run("hyprctl", &["-j", "getoption", "general:layout"])?).ok()?;
    json["str"].as_str().map(|s| s.trim().to_string())
}

#[cfg(all(test, feature = "wallpaper"))]
mod tests {
    use super::*;

    #[test]
    fn parses_awww_query() {
        let image =
            ": eDP-1: 1366x768, scale: 1, currently displaying: image: /home/u/Pictures/a b.png\n";
        assert_eq!(
            parse_wallpaper(image),
            Some(Wallpaper::Image("/home/u/Pictures/a b.png".into()))
        );
        let color = ": eDP-1: 1366x768, scale: 1, currently displaying: color: DD9999\n";
        assert_eq!(
            parse_wallpaper(color),
            Some(Wallpaper::Color("dd9999".into()))
        );
        assert_eq!(parse_wallpaper(""), None);
        assert_eq!(parse_wallpaper("algo raro"), None);
    }
}
