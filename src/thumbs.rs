//! Miniaturas de los wallpapers para la pestaña Wallpaper.
//!
//! Decodificar un JPEG de varios MB en el hilo de render congelaría la UI, así
//! que un hilo propio genera las miniaturas (192×108, recortadas al aspecto
//! 16:9) y las guarda en `$XDG_CACHE_HOME/dinamic-ui/thumbs` (o
//! `~/.cache/dinamic-ui/thumbs`). La clave de caché incluye ruta, tamaño y
//! fecha de modificación: cambiar la imagen regenera la miniatura.

use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread,
};

use image::{imageops::FilterType, ImageReader};

pub const THUMB_W: u32 = 192;
pub const THUMB_H: u32 = 108;

/// Miniatura lista: RGBA de `THUMB_W × THUMB_H`.
pub struct Thumb {
    pub path: PathBuf,
    pub rgba: Vec<u8>,
}

pub struct Thumbnails {
    tx: Option<Sender<PathBuf>>,
    ready: Arc<Mutex<Vec<Thumb>>>,
    revision: Arc<AtomicU64>,
    requested: HashSet<PathBuf>,
}

impl Thumbnails {
    pub fn new() -> Self {
        Self {
            tx: None,
            ready: Arc::new(Mutex::new(Vec::new())),
            revision: Arc::new(AtomicU64::new(0)),
            requested: HashSet::new(),
        }
    }

    /// Pide la miniatura de `path` (una sola vez por ruta). El hilo se crea al
    /// primer pedido.
    pub fn request(&mut self, path: &Path) {
        if !self.requested.insert(path.to_path_buf()) {
            return;
        }
        let tx = self.tx.get_or_insert_with(|| {
            let (tx, rx) = mpsc::channel::<PathBuf>();
            let (ready, revision) = (Arc::clone(&self.ready), Arc::clone(&self.revision));
            let spawned = thread::Builder::new()
                .name("dinamic-ui-thumbs".into())
                .spawn(move || {
                    for path in rx {
                        if let Some(rgba) = load_or_make(&path) {
                            if let Ok(mut ready) = ready.lock() {
                                ready.push(Thumb { path, rgba });
                                revision.fetch_add(1, Ordering::Release);
                            }
                        }
                    }
                });
            if let Err(e) = spawned {
                eprintln!("[dinamic-ui] hilo de miniaturas: {e}");
            }
            tx
        });
        let _ = tx.send(path.to_path_buf());
    }

    /// Miniaturas terminadas desde la última llamada.
    pub fn drain(&self) -> Vec<Thumb> {
        self.ready
            .lock()
            .map(|mut ready| std::mem::take(&mut *ready))
            .unwrap_or_default()
    }

    /// Cambia cada vez que termina una miniatura (para repintar).
    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }
}

fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("dinamic-ui").join("thumbs"))
}

/// Archivo de caché de `path`, según su ruta, tamaño y fecha de modificación.
fn cache_file(path: &Path) -> Option<PathBuf> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (path, meta.len(), mtime, THUMB_W, THUMB_H).hash(&mut hasher);
    Some(cache_dir()?.join(format!("{:016x}.png", hasher.finish())))
}

fn load_or_make(path: &Path) -> Option<Vec<u8>> {
    let cache = cache_file(path);
    if let Some(cached) = cache.as_deref().and_then(|c| image::open(c).ok()) {
        let rgba = cached.to_rgba8();
        if rgba.dimensions() == (THUMB_W, THUMB_H) {
            return Some(rgba.into_raw());
        }
    }
    let decoded = ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .map_err(|e| eprintln!("[dinamic-ui] miniatura de {}: {e}", path.display()))
        .ok()?;
    let thumb = decoded
        .resize_to_fill(THUMB_W, THUMB_H, FilterType::Triangle)
        .to_rgba8();
    if let Some(cache) = cache {
        if let Some(dir) = cache.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = thumb.save(&cache);
    }
    Some(thumb.into_raw())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_cropped_thumbnail_and_caches_it() {
        let dir = std::env::temp_dir().join(format!("dinamic-ui-thumbs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("XDG_CACHE_HOME", dir.join("cache"));
        let src = dir.join("a.png");
        image::RgbaImage::from_pixel(400, 400, image::Rgba([200, 10, 10, 255]))
            .save(&src)
            .unwrap();

        let first = load_or_make(&src).expect("miniatura");
        assert_eq!(first.len(), (THUMB_W * THUMB_H * 4) as usize);
        assert_eq!(&first[..4], &[200, 10, 10, 255]);
        let cached = cache_file(&src).unwrap();
        assert!(cached.is_file(), "debe quedar en caché");
        // Segunda lectura: sale de la caché con el mismo contenido.
        assert_eq!(load_or_make(&src).unwrap(), first);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
