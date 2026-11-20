//! Iconos reales de apps (feature `icons`).
//!
//! Resuelve `class` de Hyprland → `Icon=` del `.desktop` → PNG del tema activo
//! (con herencia, vía `freedesktop-icons`). Solo PNG: sin SVG ni procesos externos;
//! si un icono es SVG, no hay textura y la UI cae a un glifo Phosphor.
//! La decodificación va en un hilo propio para no congelar el render.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread,
};

pub const SIZE: u32 = 48;

type ReadyIcons = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

#[derive(Default)]
pub struct AppIcons {
    names: HashMap<String, String>,
    ready: ReadyIcons,
    tx: Option<Sender<(String, String)>>,
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
            let (tx, rx) = mpsc::channel::<(String, String)>();
            let ready = Arc::clone(&self.ready);
            thread::Builder::new()
                .name("dinamic-ui-appicons".into())
                .spawn(move || {
                    let theme = resolve_theme();
                    for (class, name) in rx {
                        let Some(path) = icon_path(&name, &theme) else {
                            continue;
                        };
                        if let Some(rgba) = decode_png_icon(&path) {
                            if let Ok(mut ready) = ready.lock() {
                                ready.push((class, rgba));
                            }
                        }
                    }
                })
                .ok();
            tx
        });
        let _ = tx.send((key, icon_name));
    }

    pub fn drain(&self) -> Vec<(String, Vec<u8>)> {
        self.ready
            .lock()
            .map(|mut r| std::mem::take(&mut *r))
            .unwrap_or_default()
    }
}

/// Tema activo: `[icons] theme` de la config, si no el de GTK, si no `hicolor`.
fn resolve_theme() -> String {
    crate::config::get()
        .icons
        .theme
        .clone()
        .filter(|t| !t.trim().is_empty())
        .or_else(freedesktop_icons::default_theme_gtk)
        .unwrap_or_else(|| "hicolor".to_string())
}

/// Ruta del PNG del icono, o `None` si no existe o solo hay SVG.
fn icon_path(name: &str, theme: &str) -> Option<PathBuf> {
    let path = if Path::new(name).is_absolute() {
        PathBuf::from(name)
    } else {
        freedesktop_icons::lookup(name)
            .with_size(SIZE as u16)
            .with_theme(theme)
            .with_cache()
            .find()?
    };
    let is_png = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
    is_png.then_some(path)
}

/// Decodifica un PNG a `SIZE × SIZE` RGBA (sin premultiplicar).
fn decode_png_icon(path: &Path) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let data = &buf[..info.buffer_size()];
    let (w, h) = (info.width as usize, info.height as usize);
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    if w == 0 || h == 0 || rgba.len() != w * h * 4 {
        return None;
    }
    Some(resize_rgba(&rgba, w, h, SIZE as usize))
}

/// Pesos de un filtro triangular (bilinear con antialias al reducir) para un eje.
fn axis_weights(src: usize, dst: usize) -> Vec<(usize, Vec<f32>)> {
    let scale = src as f32 / dst as f32;
    let support = scale.max(1.0);
    (0..dst)
        .map(|i| {
            let center = (i as f32 + 0.5) * scale;
            let first = ((center - support).floor().max(0.0)) as usize;
            let last = ((center + support).ceil() as usize).min(src);
            let mut weights: Vec<f32> = (first..last)
                .map(|x| (1.0 - ((x as f32 + 0.5 - center) / support).abs()).max(0.0))
                .collect();
            let sum: f32 = weights.iter().sum();
            if sum > 0.0 {
                weights.iter_mut().for_each(|w| *w /= sum);
            }
            (first, weights)
        })
        .collect()
}

/// Reescala RGBA a `dst × dst` promediando en alfa premultiplicado (sin halos).
fn resize_rgba(src: &[u8], w: usize, h: usize, dst: usize) -> Vec<u8> {
    if w == dst && h == dst {
        return src.to_vec();
    }
    let premul: Vec<f32> = src
        .chunks_exact(4)
        .flat_map(|p| {
            let a = p[3] as f32 / 255.0;
            [
                p[0] as f32 * a,
                p[1] as f32 * a,
                p[2] as f32 * a,
                p[3] as f32,
            ]
        })
        .collect();
    let (xw, yw) = (axis_weights(w, dst), axis_weights(h, dst));
    // Horizontal: h filas × dst columnas.
    let mut tmp = vec![0.0f32; h * dst * 4];
    for y in 0..h {
        for (x, (first, weights)) in xw.iter().enumerate() {
            let mut acc = [0.0f32; 4];
            for (k, wt) in weights.iter().enumerate() {
                let i = (y * w + first + k) * 4;
                for c in 0..4 {
                    acc[c] += premul[i + c] * wt;
                }
            }
            tmp[(y * dst + x) * 4..][..4].copy_from_slice(&acc);
        }
    }
    // Vertical y des-premultiplicado.
    let mut out = vec![0u8; dst * dst * 4];
    for (y, (first, weights)) in yw.iter().enumerate() {
        for x in 0..dst {
            let mut acc = [0.0f32; 4];
            for (k, wt) in weights.iter().enumerate() {
                let i = ((first + k) * dst + x) * 4;
                for c in 0..4 {
                    acc[c] += tmp[i + c] * wt;
                }
            }
            let a = acc[3];
            let o = (y * dst + x) * 4;
            if a > 0.5 {
                let inv = 255.0 / a;
                for c in 0..3 {
                    out[o + c] = (acc[c] * inv).round().clamp(0.0, 255.0) as u8;
                }
            }
            out[o + 3] = a.round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_keeps_flat_color_and_alpha() {
        let src: Vec<u8> = (0..(96 * 96)).flat_map(|_| [200, 10, 10, 255]).collect();
        let out = resize_rgba(&src, 96, 96, 48);
        assert_eq!(out.len(), 48 * 48 * 4);
        assert!(out.chunks_exact(4).all(|p| p == [200, 10, 10, 255]));
    }

    #[test]
    fn upscale_transparent_stays_transparent() {
        let src = vec![0u8; 22 * 22 * 4];
        let out = resize_rgba(&src, 22, 22, 48);
        assert!(out.chunks_exact(4).all(|p| p[3] == 0));
    }

    #[test]
    fn svg_is_rejected_for_fallback() {
        assert!(icon_path("/no/existe/icono.svg", "hicolor").is_none());
    }
}
