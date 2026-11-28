//! Pegamento gráfico: wgpu sobre un `wl_surface` de Wayland + renderer de egui.

use std::{ptr::NonNull, time::Instant};

use egui::{Context, RawInput, Rect};
use egui_wgpu::{
    wgpu::{
        Backends, CompositeAlphaMode, CreateSurfaceError, CurrentSurfaceTexture, Device, Instance,
        InstanceDescriptor, PresentMode, Queue, RequestAdapterOptions, RequestDeviceError, Surface,
        SurfaceColorSpace, SurfaceConfiguration, SurfaceTargetUnsafe, TextureFormat, TextureUsages,
    },
    Renderer, RendererOptions, ScreenDescriptor,
};
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use wayland_backend::client::Backend;
use wayland_client::{protocol::wl_surface::WlSurface, Proxy};

use crate::{
    theme::Palette,
    ui::{self, UiState},
};

pub struct Gfx {
    pub ctx: Context,
    input: RawInput,
    device: Device,
    queue: Queue,
    surface: Surface<'static>,
    config: SurfaceConfiguration,
    renderer: Renderer,
    start: Instant,
    needs_repaint: bool,
    /// Icono de cursor que pidió egui este frame; `main` lo aplica al wl_pointer.
    pub cursor_icon: egui::CursorIcon,
}

impl Gfx {
    pub fn new(backend: &Backend, wl_surface: &WlSurface, scale: f32) -> Result<Self, GfxError> {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            ..InstanceDescriptor::new_without_display_handle()
        });

        // wgpu no puede crear una superficie desde WlSurface directamente:
        // necesitamos los punteros crudos del display y del surface.
        let raw_display = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
            NonNull::new(backend.display_ptr() as *mut _)
                .ok_or_else(|| GfxError::NullPointer("wl_display".into()))?,
        ));
        let raw_window = RawWindowHandle::Wayland(WaylandWindowHandle::new(
            NonNull::new(wl_surface.id().as_ptr() as *mut _)
                .ok_or_else(|| GfxError::NullPointer("wl_surface".into()))?,
        ));

        let surface = unsafe {
            instance.create_surface_unsafe(SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_display),
                raw_window_handle: raw_window,
            })?
        };

        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|_| GfxError::NoAdapter)?;

        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;

        let caps = surface.get_capabilities(&adapter);
        // egui entrega colores en espacio gamma: un target sRGB aplicaría la
        // conversión dos veces (texto apagado). preferimos formatos lineales.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| *f == TextureFormat::Bgra8Unorm || *f == TextureFormat::Rgba8Unorm)
            .or_else(|| caps.formats.first().copied())
            .ok_or(GfxError::NoFormat)?;

        let alpha_mode = if caps
            .alpha_modes
            .contains(&CompositeAlphaMode::PreMultiplied)
        {
            CompositeAlphaMode::PreMultiplied
        } else {
            caps.alpha_modes
                .first()
                .copied()
                .unwrap_or(CompositeAlphaMode::Auto)
        };

        let present_mode = if caps.present_modes.contains(&PresentMode::Mailbox) {
            PresentMode::Mailbox
        } else {
            PresentMode::Fifo
        };

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            color_space: SurfaceColorSpace::Auto,
            format,
            width: 1,
            height: 1,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![format],
        };
        surface.configure(&device, &config);

        let ctx = Context::default();
        install_fonts(&ctx);
        // En debug, egui activa por defecto el overlay "Unaligned", que parpadea
        // durante la animación por las coordenadas fraccionarias.
        // `Style::debug` solo existe en builds de depuración (egui 0.37).
        #[cfg(debug_assertions)]
        {
            let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
            style.debug.show_unaligned = false;
            ctx.set_style_of(egui::Theme::Dark, style);
        }
        ctx.set_pixels_per_point(scale);

        let input = RawInput {
            focused: true,
            viewport_id: egui::ViewportId::ROOT,
            ..Default::default()
        };

        let renderer = Renderer::new(&device, format, RendererOptions::default());

        Ok(Self {
            ctx,
            input,
            device,
            queue,
            surface,
            config,
            renderer,
            start: Instant::now(),
            needs_repaint: false,
            cursor_icon: egui::CursorIcon::Default,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.ctx.set_pixels_per_point(scale);
    }

    /// Tamaño lógico (puntos), no físico.
    pub fn set_logical_size(&mut self, width: f32, height: f32) {
        self.input.screen_rect = Some(Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(width, height),
        ));
    }

    pub fn push_event(&mut self, event: egui::Event) {
        self.input.events.push(event);
    }

    pub fn needs_repaint(&self) -> bool {
        self.needs_repaint
    }

    /// `render=false` procesa la UI y las texturas pero no presentationa:
    /// útil para ahorrar GPU cuando no hay cambios.
    pub fn draw(
        &mut self,
        state: &mut UiState,
        palette: Palette,
        t: f32,
        panel_t: f32,
        pill_w: f32,
        render: bool,
    ) -> Result<(), GfxError> {
        self.input.time = Some(self.start.elapsed().as_secs_f64());
        let raw_input = self.input.take();
        let mut full_output = self.ctx.run_ui(raw_input, |ui| {
            ui::draw(ui, state, palette, t, panel_t, pill_w)
        });
        self.needs_repaint = full_output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .is_some_and(|v| v.repaint_delay <= std::time::Duration::from_millis(16));
        self.cursor_icon = full_output.platform_output.cursor_icon;

        // Aplicar primero las texturas: `drain` vacía el delta, así que un
        // retorno anticipado más abajo no dispara el panic de `TexturesDelta`.
        for (id, image_deltas) in full_output.textures_delta.set.drain() {
            for image_delta in image_deltas {
                self.renderer
                    .update_texture(&self.device, &self.queue, id, &image_delta);
            }
        }
        for id in full_output.textures_delta.free.drain() {
            self.renderer.free_texture(&id);
        }

        let triangles = self
            .ctx
            .tessellate(full_output.shapes, self.ctx.pixels_per_point());

        if !render {
            return Ok(());
        }

        let texture = match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(texture)
            | CurrentSurfaceTexture::Suboptimal(texture) => texture,
            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => return Ok(()),
            CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            CurrentSurfaceTexture::Lost => return Err(GfxError::SurfaceLost),
            CurrentSurfaceTexture::Validation => return Err(GfxError::Other("validación".into())),
        };

        let view = texture.texture.create_view(&Default::default());
        let screen = ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: self.ctx.pixels_per_point(),
        };

        let mut encoder = self.device.create_command_encoder(&Default::default());

        self.renderer
            .update_buffers(&self.device, &self.queue, &mut encoder, &triangles, &screen);

        {
            let pass = encoder.begin_render_pass(&egui_wgpu::wgpu::RenderPassDescriptor {
                label: Some("dinamic-ui"),
                multiview_mask: None,
                color_attachments: &[Some(egui_wgpu::wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: egui_wgpu::wgpu::Operations {
                        load: egui_wgpu::wgpu::LoadOp::Clear(egui_wgpu::wgpu::Color::TRANSPARENT),
                        store: egui_wgpu::wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.renderer
                .render(&mut pass.forget_lifetime(), &triangles, &screen);
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(texture);
        Ok(())
    }
}

#[derive(Debug)]
pub enum GfxError {
    NullPointer(String),
    Surface(CreateSurfaceError),
    NoAdapter,
    Device(RequestDeviceError),
    NoFormat,
    SurfaceLost,
    Other(String),
}

impl std::fmt::Display for GfxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NullPointer(s) => write!(f, "puntero nulo: {s}"),
            Self::Surface(e) => write!(f, "error creando superficie: {e}"),
            Self::NoAdapter => write!(f, "no se encontró un adaptador compatible"),
            Self::Device(e) => write!(f, "error creando dispositivo: {e}"),
            Self::NoFormat => write!(f, "sin formato de textura adecuado"),
            Self::SurfaceLost => write!(f, "superficie perdida"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for GfxError {}

impl From<CreateSurfaceError> for GfxError {
    fn from(e: CreateSurfaceError) -> Self {
        Self::Surface(e)
    }
}

impl From<RequestDeviceError> for GfxError {
    fn from(e: RequestDeviceError) -> Self {
        Self::Device(e)
    }
}

/// Inter como fuente principal (Regular) y semibold como familia aparte.
/// egui 0.37 no tiene peso en `FontId`: `.strong()` solo cambia el color, así
/// que los textos destacados usan explícitamente `inter-semibold`.
/// Expande `~/` en las rutas de configuración.
fn expand_home(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

/// TTF (regular, semibold) según la config. Sin rutas configuradas, autodetecta
/// Inter en el directorio de datos; se prueban ambas capitalizaciones porque
/// Google Fonts instala `instatypst` y la documentación histórica decía `Instatypst`.
/// La existencia de los archivos la comprueba `install_fonts`.
fn resolve_font_files(
    font: &crate::config::FontConfig,
    data_home: &std::path::Path,
) -> (Option<std::path::PathBuf>, Option<std::path::PathBuf>) {
    if font.regular.is_some() || font.semibold.is_some() {
        return (
            font.regular.as_deref().map(expand_home),
            font.semibold.as_deref().map(expand_home),
        );
    }
    let fonts = data_home.join("fonts");
    let dir = ["instatypst", "Instatypst"]
        .iter()
        .map(|name| fonts.join(name).join("Inter").join("static"))
        .find(|dir| dir.join("Inter_18pt-Regular.ttf").is_file());
    match dir {
        Some(dir) => (
            Some(dir.join("Inter_18pt-Regular.ttf")),
            Some(dir.join("Inter_18pt-SemiBold.ttf")),
        ),
        None => (None, None),
    }
}

pub fn install_fonts(ctx: &Context) {
    use egui::{text::FontData, FontDefinitions, FontFamily};

    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    egui_phosphor::add_font_bytes_as_family(
        &mut fonts,
        "phosphor-bold",
        egui_phosphor::Variant::Bold.font_bytes(),
    );

    let cfg = crate::config::get();
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))
        })
        .unwrap_or_default();
    let (regular, semibold) = resolve_font_files(&cfg.font, &data_home);

    if let Some(path) = regular {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("inter".into(), FontData::from_owned(bytes).into());
            if let Some(keys) = fonts.families.get_mut(&FontFamily::Proportional) {
                keys.insert(0, "inter".into());
            }
        }
    }
    if let Some(path) = semibold {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("inter-semibold".into(), FontData::from_owned(bytes).into());
            fonts.families.insert(
                FontFamily::Name("inter-semibold".into()),
                vec!["inter-semibold".into(), "inter".into()],
            );
        }
    }

    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FontConfig;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("dinamic-ui-font-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn autodetect_acepta_minusculas_y_mayusculas() {
        for name in ["instatypst", "Instatypst"] {
            let root = temp_dir(name);
            let dir = root.join("fonts").join(name).join("Inter").join("static");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("Inter_18pt-Regular.ttf"), b"").unwrap();
            let (regular, semibold) = resolve_font_files(&FontConfig::default(), &root);
            assert_eq!(regular, Some(dir.join("Inter_18pt-Regular.ttf")));
            assert_eq!(semibold, Some(dir.join("Inter_18pt-SemiBold.ttf")));
            std::fs::remove_dir_all(&root).unwrap();
        }
    }

    #[test]
    fn sin_archivos_no_hay_fuentes() {
        let root = temp_dir("missing");
        let (regular, semibold) = resolve_font_files(&FontConfig::default(), &root);
        assert_eq!((regular, semibold), (None, None));
    }

    #[test]
    fn config_explicita_tiene_prioridad() {
        let root = temp_dir("explicit");
        let font = FontConfig {
            regular: Some("/tmp/regular.ttf".into()),
            semibold: Some("/tmp/semibold.ttf".into()),
        };
        let (regular, semibold) = resolve_font_files(&font, &root);
        assert_eq!(regular, Some(PathBuf::from("/tmp/regular.ttf")));
        assert_eq!(semibold, Some(PathBuf::from("/tmp/semibold.ttf")));
    }
}
