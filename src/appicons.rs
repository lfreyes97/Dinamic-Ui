//! Iconos reales de apps para el dock.
//!
//! Resuelve `class` de Hyprland → `Icon=` del `.desktop` → PNG del tema
//! (hicolor, Adwaita…). La decodificación va en un hilo propio, igual que las
//! miniaturas de wallpaper, para no congelar el render.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread,
};

use image::imageops::FilterType;

pub const SIZE: u32 = 48;

type ReadyIcons = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

#[derive(Default)]
pub struct AppIcons {
    names: HashMap<String, String>,
    ready: ReadyIcons,
    tx: Option<Sender<(String, PathBuf)>>,
    requested: HashSet<String>,
}

impl AppIcons {
    pub fn new() -> Self {
        let mut this = Self::default();
        this.scan_desktop_files();
        this
    }

    fn scan_desktop_files(&mut self) {
        let mut dirs = vec![PathBuf::from("/usr/share/applications")];
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(home).join(".local/share/applications"));
        }
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("desktop") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let mut icon = None;
                let mut classes = vec![path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_lowercase()];
                for line in text.lines() {
                    if let Some(v) = line.strip_prefix("Icon=") {
                        icon = Some(v.trim().to_string());
                    } else if let Some(v) = line.strip_prefix("StartupWMClass=") {
                        classes.push(v.trim().to_lowercase());
                    }
                }
                if let Some(icon) = icon {
                    for class in classes {
                        if !class.is_empty() {
                            self.names.entry(class).or_insert_with(|| icon.clone());
                        }
                    }
                }
            }
        }
    }

    pub fn request(&mut self, class: &str) {
        let key = class.to_lowercase();
        if !self.requested.insert(key.clone()) {
            return;
        }
        let Some(icon_name) = self.names.get(&key).cloned() else {
            return;
        };
        let tx = self.tx.get_or_insert_with(|| {
            let (tx, rx) = mpsc::channel::<(String, PathBuf)>();
            let ready = Arc::clone(&self.ready);
            thread::Builder::new()
                .name("dinamic-ui-appicons".into())
                .spawn(move || {
                    for (class, path) in rx {
                        let decoded = if path.extension().and_then(|s| s.to_str()) == Some("svg") {
                            // Sin añadir resvg: convertimos con el `magick` del sistema.
                            std::process::Command::new("magick")
                                .arg(&path)
                                .arg(format!("{SIZE}x{SIZE}"))
                                .arg("png:-")
                                .output()
                                .ok()
                                .and_then(|out| image::load_from_memory(&out.stdout).ok())
                        } else {
                            image::open(&path).ok()
                        };
                        if let Some(img) = decoded {
                            let rgba = img
                                .resize_exact(SIZE, SIZE, FilterType::Lanczos3)
                                .to_rgba8()
                                .into_raw();
                            if let Ok(mut ready) = ready.lock() {
                                ready.push((class, rgba));
                            }
                        }
                    }
                })
                .ok();
            tx
        });
        if let Some(path) = icon_path(&icon_name) {
            let _ = tx.send((key, path));
        }
    }

    pub fn drain(&self) -> Vec<(String, Vec<u8>)> {
        self.ready
            .lock()
            .map(|mut r| std::mem::take(&mut *r))
            .unwrap_or_default()
    }
}

fn icon_path(name: &str) -> Option<PathBuf> {
    if Path::new(name).is_absolute() {
        return Some(PathBuf::from(name));
    }
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".local/share/icons"));
    }
    roots.push(PathBuf::from("/usr/share/icons"));
    roots.push(PathBuf::from("/usr/share/pixmaps"));

    for root in roots {
        // Temas y tamaños típicos; primero los PNG de 48/32/22.
        for theme in ["hicolor", "Adwaita", "AdwaitaLegacy"] {
            for size in [
                "48x48", "128x128", "64x64", "32x32", "256x256", "22x22", "16x16", "scalable",
            ] {
                let dir = root.join(theme).join(size).join("apps");
                let png = dir.join(format!("{name}.png"));
                if png.is_file() {
                    return Some(png);
                }
                let svg = dir.join(format!("{name}.svg"));
                if svg.is_file() {
                    return Some(svg);
                }
            }
        }
        let direct = root.join(format!("{name}.png"));
        if direct.is_file() {
            return Some(direct);
        }
    }
    None
}
