//! Pestaña Wallpaper: galería, miniaturas y colores sólidos.

use super::*;
use crate::sysinfo::Wallpaper;

/// Estado de la pestaña Wallpaper (galería, miniaturas y selección).
pub struct WallpaperState {
    pub wallpaper_color: Option<String>,
    pub wallpaper_selected: Option<usize>,
    pub wallpaper_image: Option<std::path::PathBuf>,
    pub wallpaper_hover: Option<std::path::PathBuf>,
    pub wallpaper_image_request: Option<std::path::PathBuf>,
    pub wallpaper_grid: bool,
    pub wallpaper_files: Vec<std::path::PathBuf>,
    pub wallpaper_scan: Option<(std::path::PathBuf, std::time::Instant)>,
    pub thumbs: crate::thumbs::Thumbnails,
    pub last_thumb_revision: u64,
    pub last_system_revision: u64,
    pub thumb_textures: std::collections::HashMap<std::path::PathBuf, egui::TextureHandle>,
}

impl Default for WallpaperState {
    fn default() -> Self {
        Self {
            wallpaper_color: None,
            wallpaper_selected: None,
            wallpaper_image: None,
            wallpaper_hover: None,
            wallpaper_image_request: None,
            wallpaper_grid: false,
            wallpaper_files: Vec::new(),
            wallpaper_scan: None,
            thumbs: crate::thumbs::Thumbnails::new(),
            last_thumb_revision: 0,
            last_system_revision: 0,
            thumb_textures: std::collections::HashMap::new(),
        }
    }
}

/// Carpeta de imágenes del usuario (`XDG_PICTURES_DIR` o `~/Pictures`).
pub(super) fn pictures_dir() -> Option<std::path::PathBuf> {
    let home = std::path::PathBuf::from(std::env::var_os("HOME")?);
    let dir = std::env::var_os("XDG_PICTURES_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| home.join("Pictures"));
    Some(if dir.is_dir() { dir } else { home })
}

/// Carpeta de wallpapers: `[wallpapers] dir` de la config (con `~`), o
/// `~/Pictures/Unsplash`, o la carpeta de imágenes del usuario.
pub(super) fn wallpaper_dir() -> Option<std::path::PathBuf> {
    let home = std::path::PathBuf::from(std::env::var_os("HOME")?);
    let configured = config::get()
        .wallpapers
        .dir
        .clone()
        .map(|raw| match raw.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None if raw == "~" => home.clone(),
            None => std::path::PathBuf::from(raw),
        });
    let dir = configured.unwrap_or_else(|| home.join("Pictures/Unsplash"));
    if dir.is_dir() {
        Some(dir)
    } else {
        pictures_dir()
    }
}

const IMAGE_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "webp", "gif", "bmp"];

/// Imágenes (no recursivo) de `dir`, ordenadas por nombre sin distinguir mayúsculas.
pub(super) fn list_images(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        })
        .collect();
    files.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    files
}

/// Título y subtítulo de una imagen. Los nombres de Unsplash
/// (`autor-apellido-ID11-unsplash.jpg`) se muestran como "Autor Apellido" + ID.
pub(super) fn wallpaper_label(path: &std::path::Path) -> (String, String) {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(base) = stem.strip_suffix("-unsplash") {
        let chars: Vec<char> = base.chars().collect();
        if chars.len() > 12 && chars[chars.len() - 12] == '-' {
            let author: String = chars[..chars.len() - 12]
                .iter()
                .collect::<String>()
                .split('-')
                .map(capitalize)
                .collect::<Vec<_>>()
                .join(" ");
            let id: String = chars[chars.len() - 11..].iter().collect();
            return (author, format!("Unsplash · {id}"));
        }
    }
    (stem, "Imagen".to_string())
}

/// Colores sólidos que se aplican con `awww clear`.
const WALLPAPERS: [(&str, &str); 6] = [
    ("Negro", "000000"),
    ("Pizarra", "222222"),
    ("Piedra", "444444"),
    ("Gris", "777777"),
    ("Claro", "dddddd"),
    ("Metal", "dd9999"),
];

// ── Wallpaper ───────────────────────────────────────────────────────────

pub(super) fn is_light(color: Color32) -> bool {
    (color.r() as u32 * 299 + color.g() as u32 * 587 + color.b() as u32 * 114) / 1000 > 150
}

/// Índice del swatch cuyo color es `hex` (`rrggbb`).
pub fn wallpaper_swatch_index(hex: &str) -> Option<usize> {
    WALLPAPERS
        .iter()
        .position(|(_, h)| h.eq_ignore_ascii_case(hex))
}

pub(super) fn panel_wallpaper(ui: &mut egui::Ui, state: &mut UiState) {
    let width = ui.available_width();
    let selected = state
        .wallpaper
        .wallpaper_selected
        .and_then(|i| WALLPAPERS.get(i));
    let real = state.shared.system.wallpaper.clone();
    // Miniaturas terminadas: subirlas como texturas.
    for thumb in state.wallpaper.thumbs.drain() {
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [
                crate::thumbs::THUMB_W as usize,
                crate::thumbs::THUMB_H as usize,
            ],
            &thumb.rgba,
        );
        let name = format!("wallpaper-{}", thumb.path.display());
        let handle = ui
            .ctx()
            .load_texture(name, image, egui::TextureOptions::LINEAR);
        state.wallpaper.thumb_textures.insert(thumb.path, handle);
    }
    let color_of = |hex: &str| crate::theme::parse_hex(Some(hex)).unwrap_or(Color32::BLACK);
    // El hover manda: al pasar por una miniatura se ve ahí, sin aplicarla.
    let preview_path = state
        .wallpaper
        .wallpaper_hover
        .as_ref()
        .or(state.wallpaper.wallpaper_image.as_ref());

    section_heading(
        ui,
        if state.wallpaper.wallpaper_hover.is_some() {
            "PREVIEW"
        } else {
            "CURRENT WALLPAPER"
        },
        "Desktop",
    );
    let (preview, _) = ui.allocate_exact_size(egui::Vec2::new(width, 130.0), egui::Sense::hover());
    let base = match (selected, &real) {
        (Some((_, hex)), _) => color_of(hex),
        (None, Some(Wallpaper::Color(hex))) => color_of(hex),
        _ => pal().inset,
    };
    ui.painter().rect(
        preview,
        CornerRadius::same(12),
        base,
        Stroke::new(1.0, pal().faint),
        StrokeKind::Inside,
    );
    // Si el fondo es una imagen con miniatura, se usa de fondo de la vista previa
    // (recortada al aspecto del recuadro) bajo un velo oscuro para leer la hora.
    let has_image = match (
        selected,
        preview_path.and_then(|p| state.wallpaper.thumb_textures.get(p)),
    ) {
        (None, Some(texture)) => {
            let target = preview.width() / preview.height();
            let source = crate::thumbs::THUMB_W as f32 / crate::thumbs::THUMB_H as f32;
            let uv = if target < source {
                let w = target / source;
                egui::Rect::from_min_max(
                    egui::pos2((1.0 - w) / 2.0, 0.0),
                    egui::pos2((1.0 + w) / 2.0, 1.0),
                )
            } else {
                let h = source / target;
                egui::Rect::from_min_max(
                    egui::pos2(0.0, (1.0 - h) / 2.0),
                    egui::pos2(1.0, (1.0 + h) / 2.0),
                )
            };
            egui::Image::new(egui::load::SizedTexture::new(texture.id(), preview.size()))
                .uv(uv)
                .corner_radius(12)
                .paint_at(ui, preview);
            ui.painter()
                .rect_filled(preview, CornerRadius::same(12), Palette::scrim(110));
            true
        }
        _ => false,
    };
    // Aproximación de los degradados CSS: un círculo claro y otro oscuro.
    if !has_image {
        ui.painter().circle_filled(
            egui::Pos2::new(preview.max.x - 70.0, preview.min.y + 62.0),
            50.0,
            Palette::sheen(14),
        );
        ui.painter().circle_filled(
            egui::Pos2::new(preview.min.x + 80.0, preview.max.y - 30.0),
            26.0,
            Palette::scrim(40),
        );
    }
    let ink = if is_light(base) {
        pal().solid()
    } else {
        pal().text_hi
    };
    let now = Local::now();
    text_at(
        ui,
        preview.center() - egui::Vec2::new(0.0, 10.0),
        egui::Align2::CENTER_CENTER,
        now.format("%H:%M"),
        TEXT_DISPLAY,
        ink,
    );
    text_at(
        ui,
        preview.center() + egui::Vec2::new(0.0, 12.0),
        egui::Align2::CENTER_CENTER,
        format!(
            "{}, {} de {}",
            capitalize(DAYS_LONG[now.weekday().num_days_from_monday() as usize]),
            now.day(),
            MONTHS_LONG[(now.month() - 1) as usize]
        ),
        TEXT_CAPTION,
        ink.gamma_multiply(0.8),
    );
    let caption = match (selected, &real) {
        (Some((name, hex)), _) => format!("{name} · #{hex}"),
        (None, _) if preview_path.is_some() => {
            let (title, _) = wallpaper_label(preview_path.unwrap());
            let label = truncate(&title, 34);
            if state.wallpaper.wallpaper_hover.is_some() {
                format!("{label} · vista previa")
            } else {
                label
            }
        }
        (None, Some(Wallpaper::Color(hex))) => format!("Color · #{hex}"),
        (None, Some(Wallpaper::Image(path))) => std::path::Path::new(path)
            .file_name()
            .map(|n| truncate(&n.to_string_lossy(), 34))
            .unwrap_or_else(|| "Imagen".to_string()),
        (None, None) => "Fondo actual · sin datos".to_string(),
    };
    text_at(
        ui,
        egui::Pos2::new(preview.min.x + 12.0, preview.max.y - 12.0),
        egui::Align2::LEFT_CENTER,
        caption,
        TEXT_CAPTION,
        ink.gamma_multiply(0.75),
    );
    // Placeholder: "Preview ↗" no abre nada (la vista previa no renderiza imágenes).
    let menu = egui::Rect::from_min_max(
        egui::Pos2::new(preview.max.x - 72.0, preview.max.y - 26.0),
        egui::Pos2::new(preview.max.x - 9.0, preview.max.y - 8.0),
    );
    ui.painter().rect(
        menu,
        CornerRadius::same(6),
        Palette::scrim(80),
        Stroke::new(1.0, Palette::sheen(48)),
        StrokeKind::Inside,
    );
    text_at(
        ui,
        menu.center(),
        egui::Align2::CENTER_CENTER,
        "Preview ↗",
        TEXT_CAPTION,
        pal().text_hi,
    );
    ui.add_space(15.0);

    // Imágenes de la carpeta configurada (`[wallpapers] dir`); clic → `awww img`
    // (vía main). La carpeta se reescanea cada 5 s o al cambiarla en la config.
    let dir = wallpaper_dir();
    let stale = match (&dir, &state.wallpaper.wallpaper_scan) {
        (Some(d), Some((scanned, at))) => {
            scanned != d || at.elapsed() > std::time::Duration::from_secs(5)
        }
        (Some(_), None) => true,
        (None, _) => false,
    };
    if let (true, Some(d)) = (stale, &dir) {
        state.wallpaper.wallpaper_files = list_images(d);
        state.wallpaper.wallpaper_scan = Some((d.clone(), std::time::Instant::now()));
        for path in &state.wallpaper.wallpaper_files {
            state.wallpaper.thumbs.request(path);
        }
    }
    let dir_name = dir
        .as_ref()
        .and_then(|d| d.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "—".to_string());
    let files = std::mem::take(&mut state.wallpaper.wallpaper_files);
    let view_icon = if state.wallpaper.wallpaper_grid {
        ic::LIST
    } else {
        ic::SQUARES_FOUR
    };
    if section_heading_action(
        ui,
        &format!("IMAGES · {} en {}", files.len(), truncate(&dir_name, 16)),
        view_icon,
    )
    .clicked()
    {
        state.wallpaper.wallpaper_grid = !state.wallpaper.wallpaper_grid;
    }
    if files.is_empty() {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, 44.0), egui::Sense::hover());
        card_at(ui, rect, 10);
        text_at(
            ui,
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Sin imágenes en esta carpeta",
            TEXT_BODY,
            pal().muted,
        );
    }
    let mut chosen: Option<std::path::PathBuf> = None;
    let mut hover_candidate: Option<std::path::PathBuf> = None;
    if !state.wallpaper.wallpaper_grid {
        for (i, path) in files.iter().enumerate() {
            let (rect, _) =
                ui.allocate_exact_size(egui::Vec2::new(width, 48.0), egui::Sense::hover());
            let response = ui
                .interact(
                    rect,
                    ui.scope_id().with(("panel-wp-image", i)),
                    egui::Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(path.to_string_lossy());
            if response.hovered() {
                hover_candidate = Some(path.clone());
            }
            let current = state.wallpaper.wallpaper_image.as_deref() == Some(path.as_path());
            card_at(ui, rect, 10);
            if current {
                ui.painter().rect_stroke(
                    rect,
                    CornerRadius::same(10),
                    Stroke::new(1.5, pal().text_hi),
                    StrokeKind::Inside,
                );
            } else if response.hovered() {
                ui.painter().rect_stroke(
                    rect,
                    CornerRadius::same(10),
                    Stroke::new(1.0, pal().faint),
                    StrokeKind::Inside,
                );
            }
            let (title, subtitle) = wallpaper_label(path);
            // Miniatura 16:9 a la izquierda; mientras se genera, un icono de imagen.
            let thumb = egui::Rect::from_min_size(
                egui::Pos2::new(rect.min.x + 6.0, rect.center().y - 18.0),
                egui::Vec2::new(64.0, 36.0),
            );
            match state.wallpaper.thumb_textures.get(path) {
                Some(texture) => {
                    egui::Image::new(egui::load::SizedTexture::new(texture.id(), thumb.size()))
                        .corner_radius(6)
                        .paint_at(ui, thumb);
                }
                None => {
                    ui.painter()
                        .rect_filled(thumb, CornerRadius::same(6), pal().badge);
                    icon_at(ui, thumb.center(), ic::IMAGE, ICON_MD, pal().muted);
                }
            }
            let text_x = thumb.max.x + 10.0;
            text_at(
                ui,
                egui::Pos2::new(text_x, rect.center().y - 7.0),
                egui::Align2::LEFT_CENTER,
                truncate(&title, 28),
                TEXT_TITLE,
                pal().text_hi,
            );
            text_at(
                ui,
                egui::Pos2::new(text_x, rect.center().y + 8.0),
                egui::Align2::LEFT_CENTER,
                &subtitle,
                TEXT_CAPTION,
                pal().muted,
            );
            if current {
                let check = egui::Pos2::new(rect.max.x - 18.0, rect.center().y);
                ui.painter().circle_filled(check, 8.5, pal().text_hi);
                icon_at(ui, check, ic::CHECK, ICON_XS - 2.0, pal().solid());
            }
            if response.clicked() {
                chosen = Some(path.clone());
            }
            ui.add_space(SP_S);
        }
    } else {
        // Misma fuente de datos, vista de grid: 3 columnas de miniaturas 16:9.
        let gap = 8.0;
        let columns = 3usize;
        let tile_w = (width - gap * (columns - 1) as f32) / columns as f32;
        let tile_h = tile_w * 9.0 / 16.0;
        let row_h = tile_h + 16.0 + gap;
        let rows = files.len().div_ceil(columns);
        let (area, _) = ui.allocate_exact_size(
            egui::Vec2::new(width, rows as f32 * row_h),
            egui::Sense::hover(),
        );
        for (i, path) in files.iter().enumerate() {
            let x = area.min.x + (i % columns) as f32 * (tile_w + gap);
            let y = area.min.y + (i / columns) as f32 * row_h;
            let image_rect =
                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(tile_w, tile_h));
            let response = ui
                .interact(
                    image_rect,
                    ui.scope_id().with(("panel-wp-grid", i)),
                    egui::Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(path.to_string_lossy());
            if response.hovered() {
                hover_candidate = Some(path.clone());
            }
            let current = state.wallpaper.wallpaper_image.as_deref() == Some(path.as_path());
            card_at(ui, image_rect, 8);
            match state.wallpaper.thumb_textures.get(path) {
                Some(texture) => {
                    egui::Image::new(egui::load::SizedTexture::new(
                        texture.id(),
                        image_rect.size(),
                    ))
                    .corner_radius(8)
                    .paint_at(ui, image_rect);
                }
                None => {
                    ui.painter()
                        .rect_filled(image_rect, CornerRadius::same(8), pal().badge);
                    icon_at(ui, image_rect.center(), ic::IMAGE, ICON_MD, pal().muted);
                }
            }
            if current {
                ui.painter().rect_stroke(
                    image_rect,
                    CornerRadius::same(8),
                    Stroke::new(2.0, pal().text_hi),
                    StrokeKind::Inside,
                );
                let check = egui::pos2(image_rect.max.x - 12.0, image_rect.min.y + 12.0);
                ui.painter().circle_filled(check, 8.0, pal().text_hi);
                icon_at(ui, check, ic::CHECK, ICON_XS - 2.0, pal().solid());
            } else if response.hovered() {
                ui.painter().rect_stroke(
                    image_rect,
                    CornerRadius::same(8),
                    Stroke::new(1.0, pal().dim),
                    StrokeKind::Inside,
                );
            }
            let (title, _) = wallpaper_label(path);
            text_at(
                ui,
                egui::pos2(x + 2.0, y + tile_h + 7.0),
                egui::Align2::LEFT_TOP,
                truncate(&title, 20),
                TEXT_CAPTION,
                pal().text_hi,
            );
            if response.clicked() {
                chosen = Some(path.clone());
            }
        }
    }
    state.wallpaper.wallpaper_files = files;
    state.wallpaper.wallpaper_hover = hover_candidate;
    if let Some(path) = chosen {
        state.wallpaper.wallpaper_image = Some(path.clone());
        state.wallpaper.wallpaper_selected = None;
        state.wallpaper.wallpaper_image_request = Some(path);
    }
    ui.add_space(11.0);

    // Colores sólidos: 3×2; clic → `awww clear` (vía main).
    section_heading(ui, "COLORS", "Solid fill");
    let gap = 8.0;
    let tile_h = 62.0;
    let (grid, _) = ui.allocate_exact_size(
        egui::Vec2::new(width, tile_h * 2.0 + gap + 6.0),
        egui::Sense::hover(),
    );
    // Margen de 3 px para que el contorno del seleccionado no se recorte.
    let grid = grid.shrink(3.0);
    let tile_w = (grid.width() - gap * 2.0) / 3.0;
    for (i, (name, hex)) in WALLPAPERS.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            grid.min
                + egui::Vec2::new(
                    (i % 3) as f32 * (tile_w + gap),
                    (i / 3) as f32 * (tile_h + gap),
                ),
            egui::Vec2::new(tile_w, tile_h),
        );
        let response = ui
            .interact(
                rect,
                ui.scope_id().with(("panel-wallpaper", i)),
                egui::Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(format!("{name} · #{hex}"));
        let color = color_of(hex);
        ui.painter().rect(
            rect,
            CornerRadius::same(9),
            color,
            Stroke::new(1.0, pal().faint),
            StrokeKind::Inside,
        );
        ui.painter().circle_filled(
            egui::Pos2::new(rect.min.x + rect.width() * 0.68, rect.min.y + 20.0),
            12.0,
            Palette::sheen(16),
        );
        if state.wallpaper.wallpaper_selected == Some(i) {
            ui.painter().rect_stroke(
                rect.expand(3.0),
                CornerRadius::same(11),
                Stroke::new(2.0, pal().text_hi),
                StrokeKind::Middle,
            );
            let check = egui::Pos2::new(rect.max.x - 14.5, rect.min.y + 14.5);
            ui.painter().circle_filled(check, 8.5, pal().text_hi);
            icon_at(ui, check, ic::CHECK, ICON_XS - 2.0, pal().solid());
        }
        if response.clicked() {
            state.wallpaper.wallpaper_selected = Some(i);
            state.wallpaper.wallpaper_image = None;
            state.wallpaper.wallpaper_color = Some(hex.to_string());
        }
    }
    ui.add_space(15.0);

    // Valores de solo lectura: tipo de fondo según `awww query` (sin ajuste ni
    // rotación) y esquema de color del tema de config.
    section_heading(ui, "APPEARANCE", "Display options");
    let scheme = capitalize(
        &config::get()
            .theme
            .name
            .clone()
            .unwrap_or_else(|| "black-metal".to_string())
            .replace('-', " "),
    );
    for (i, (name, value)) in [
        (
            "Fit mode",
            match &real {
                Some(Wallpaper::Image(_)) => "Imagen",
                Some(Wallpaper::Color(_)) => "Color sólido",
                None => "Sin datos",
            }
            .to_string(),
        ),
        ("Color scheme", scheme),
        ("Wallpaper rotation", "No disponible".to_string()),
    ]
    .into_iter()
    .enumerate()
    {
        if i > 0 {
            ui.add_space(7.0);
        }
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, 36.0), egui::Sense::hover());
        card_at(ui, rect, 10);
        let y = rect.center().y;
        text_at(
            ui,
            egui::Pos2::new(rect.min.x + 12.0, y),
            egui::Align2::LEFT_CENTER,
            name,
            TEXT_BODY,
            pal().text_hi,
        );
        icon_at(
            ui,
            egui::Pos2::new(rect.max.x - 16.0, y),
            ic::CARET_RIGHT,
            ICON_XS,
            pal().muted,
        );
        text_at(
            ui,
            egui::Pos2::new(rect.max.x - 26.0, y),
            egui::Align2::RIGHT_CENTER,
            value,
            TEXT_CAPTION,
            pal().muted,
        );
    }

    let library = wallpaper_dir();
    let browse = panel_footer(
        ui,
        "Wallpaper library",
        "Browse files ↗",
        false,
        library.is_some(),
    );
    if let (true, Some(dir)) = (browse.clicked(), library) {
        open_path(state, &dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsplash_labels() {
        let p = std::path::Path::new("/x/steve-a-johnson-rji55XQsGkw-unsplash.jpg");
        assert_eq!(
            wallpaper_label(p),
            (
                "Steve A Johnson".to_string(),
                "Unsplash · rji55XQsGkw".to_string()
            )
        );
        // ID con guion y guion bajo (alfabeto base64url).
        let p = std::path::Path::new("/x/marek-pavlik-_MoM-kvI_a1-unsplash.jpg");
        assert_eq!(wallpaper_label(p).0, "Marek Pavlik");
        let p = std::path::Path::new("/x/wallhaven-ogjgyl.png");
        assert_eq!(
            wallpaper_label(p),
            ("wallhaven-ogjgyl".to_string(), "Imagen".to_string())
        );
    }

    #[test]
    fn lists_only_images_sorted() {
        let dir = std::env::temp_dir().join(format!("dinamic-ui-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["b.JPG", "a.png", "notas.txt", "c.webp"] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        let names: Vec<String> = list_images(&dir)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(names, vec!["a.png", "b.JPG", "c.webp"]);
    }
}
