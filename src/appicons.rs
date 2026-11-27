//! Iconos reales de apps (feature `icons`).
//!
//! Resuelve `class` de Hyprland → `Icon=` del `.desktop` → PNG o SVG del tema
//! activo (con herencia, vía `freedesktop-icons`). Los SVG se rasterizan con
//! `resvg` en el hilo de iconos; si no hay textura, la UI cae al glifo Phosphor.
//! El RGBA reescalado se guarda en `$XDG_CACHE_HOME/dinamic-ui/icons` con clave
//! tema+ruta+mtime, así que no se decodifica de nuevo al reiniciar.

use std::{
    collections::hash_map::DefaultHasher,
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread,
};

pub const SIZE: u32 = 48;
/// Sube de versión cuando cambia el formato del caché en disco.
const CACHE_VERSION: u8 = 2;

type ReadyIcons = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

pub struct AppIcons {
    names: HashMap<String, String>,
    theme: String,
    ready: ReadyIcons,
    tx: Option<Sender<(String, String)>>,
    /// Incluye los fallos: una clase que no resolvió no se reintenta por frame.
    requested: HashSet<String>,
}

impl Default for AppIcons {
    fn default() -> Self {
        Self::new()
    }
}

impl AppIcons {
    pub fn new() -> Self {
        let mut this = Self {
            names: HashMap::new(),
            theme: resolve_theme(),
            ready: Arc::new(Mutex::new(Vec::new())),
            tx: None,
            requested: HashSet::new(),
        };
        this.scan_desktop_files();
        this
    }

    /// Tema vigente, para detectar cambios con `--reload` desde `Shared`.
    pub fn theme() -> String {
        resolve_theme()
    }

    fn scan_desktop_files(&mut self) {
        // También las entradas ocultas: una ventana abierta puede venir de una.
        for app in crate::desktop::scan() {
            if app.icon.is_empty() {
                continue;
            }
            let classes = std::iter::once(app.id.clone()).chain(app.wm_class.clone());
            for class in classes.filter(|c| !c.is_empty()) {
                self.names.entry(class).or_insert_with(|| app.icon.clone());
            }
        }
    }

    pub fn request(&mut self, class: &str) {
        let key = class.to_lowercase();
        if key.is_empty() || !self.requested.insert(key.clone()) {
            return;
        }
        let Some(icon_name) = self.names.get(&key).cloned() else {
            return;
        };
        let theme = self.theme.clone();
        let ready = Arc::clone(&self.ready);
        let tx = self.tx.get_or_insert_with(|| {
            let (tx, rx) = mpsc::channel::<(String, String)>();
            thread::Builder::new()
                .name("dinamic-ui-appicons".into())
                .spawn(move || {
                    for (class, name) in rx {
                        if let Some(rgba) = load_or_decode(&name, &theme) {
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

/// Caché en disco de los RGBA ya reescalados, como la de miniaturas.
fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("dinamic-ui").join("icons"))
}

/// La clave incluye tema, ruta real, mtime y tamaño: un cambio de icono o de
/// tema invalida el archivo sin tocar los demás.
fn cache_file(name: &str, theme: &str, path: &Path) -> Option<PathBuf> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut hasher = DefaultHasher::new();
    (CACHE_VERSION, theme, name, path, meta.len(), mtime, SIZE).hash(&mut hasher);
    Some(cache_dir()?.join(format!("{:016x}.rgba", hasher.finish())))
}

/// Sirve el RGBA cacheado o lo decodifica/rasteriza y lo guarda. Se llama en el
/// hilo de iconos, nunca en el de render.
fn load_or_decode(name: &str, theme: &str) -> Option<Vec<u8>> {
    let path = icon_path(name, theme)?;
    if let Some(file) = cache_file(name, theme, &path) {
        if let Ok(bytes) = std::fs::read(&file) {
            if bytes.len() == SIZE as usize * SIZE as usize * 4 {
                return Some(bytes);
            }
        }
    }
    let is_svg = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    let rgba = if is_svg {
        decode_svg_icon(&path)
    } else {
        decode_png_icon(&path)
    }?;
    if let Some(file) = cache_file(name, theme, &path) {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&file, &rgba);
    }
    Some(rgba)
}

/// Ruta del icono (PNG o SVG) en el tema activo, con herencia.
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
    let valid = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png") || e.eq_ignore_ascii_case("svg"));
    valid.then_some(path)
}

/// Rasteriza un SVG a `SIZE × SIZE` con `resvg`. La salida de tiny-skia está
/// premultiplicada; la convertimos a RGBA normal para egui.
fn decode_svg_icon(path: &Path) -> Option<Vec<u8>> {
    let svg = std::fs::read(path).ok()?;
    let mut opt = resvg::usvg::Options {
        resources_dir: path.parent().map(Path::to_path_buf),
        ..Default::default()
    };
    opt.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_data(&svg, &opt).ok()?;
    let size = tree.size();
    if size.width() <= 0.0 || size.height() <= 0.0 {
        return None;
    }
    let scale = (SIZE as f32 / size.width()).min(SIZE as f32 / size.height());
    let tx = (SIZE as f32 - size.width() * scale) / 2.0;
    let ty = (SIZE as f32 - size.height() * scale) / 2.0;
    let transform = resvg::tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, tx, ty);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(SIZE, SIZE)?;
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some(
        pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect(),
    )
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
    fn svg_is_accepted_for_rasterization() {
        assert!(icon_path("/no/existe/icono.svg", "hicolor").is_some());
        assert!(icon_path("/no/existe/icono.txt", "hicolor").is_none());
    }

    #[test]
    fn svg_se_rasteriza_a_rgba_de_48px() {
        let dir = std::env::temp_dir().join(format!("dinamic-ui-appicons-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.svg");
        std::fs::write(
            &path,
            "<svg xmlns='http://www.w3.org/2000/svg' width='48' height='48'><rect width='48' height='48' fill='#ff0000'/></svg>",
        )
        .unwrap();
        let rgba = decode_svg_icon(&path).expect("rasteriza el SVG");
        assert_eq!(rgba.len(), SIZE as usize * SIZE as usize * 4);
        assert_eq!(&rgba[..4], &[255, 0, 0, 255]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn cache_file_cambia_con_el_tema_y_es_estable() {
        let path = Path::new(file!());
        let a = cache_file("firefox", "tema-a", path).unwrap();
        let b = cache_file("firefox", "tema-b", path).unwrap();
        assert_ne!(a, b);
        assert_eq!(a, cache_file("firefox", "tema-a", path).unwrap());
        assert_eq!(a.extension().unwrap(), "rgba");
    }
}
