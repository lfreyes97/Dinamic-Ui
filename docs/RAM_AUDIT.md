# Auditoría de RAM — línea base cerrada

Medido con `target/release/dinamic-ui`: RSS ~131 MB, de los cuales 22 MB son anónimos
(heap/texturas) y 108 MB son librerías de GPU mapeadas (nvidia gpucomp 30, gallium 26,
LLVM 26, nvidia eglcore 7, vulkan_radeon 5).

| Métrica | Línea base |
|---|---|
| RSS | ~131 MB (22 MB anónimos + 108 MB GPU mapeada) |
| Binario release | 20 MB (`opt-level = 2`, `lto = "thin"`) |
| Capturas visuales | `just snapshots` → `docs/baseline/` (comparación visual manual) |

La línea base queda congelada antes del refactor de `ui.rs`. Las tareas de abajo son
optimizaciones pendientes, no bloquean Fase 1 del plan de modularización.

## Tareas, por prioridad

1. [ ] `src/gfx.rs:41` — `Backends::all()` → `Backends::VULKAN`, y `PowerPreference::LowPower`
       en `RequestAdapterOptions` (hoy `Default`). Verificar qué adaptador se elige. (~40–80 MB)
2. [ ] `Cargo.toml` perfil release: `codegen-units = 1`, `strip = true`, `panic = "abort"`. (~3–5 MB)
3. [ ] `MALLOC_ARENA_MAX=2` (servicio o `hyprland/dinamic-ui.conf`) o allocator `mimalloc`. (~2–5 MB)
4. [ ] `src/gfx.rs:99` — `PresentMode::Fifo` y `desired_maximum_frame_latency: 1`. (pocos MB)
5. [ ] LRU con tope (~64) para `thumb_textures` (`ui.rs:151`) y `app_icon_textures` (`ui.rs:189`);
       soltar miniaturas al cerrar la pestaña Wallpaper.
6. [ ] `src/thumbs.rs:121` — decodificar JPEG reducido (`JpegDecoder::scale`) y pedir solo las
       miniaturas visibles (`ui.rs:2665` las pide todas).
7. [ ] Opcional: quitar `default_fonts` de egui (~1–3 MB), con fallback si falta Inter.
8. [ ] Opcional: `appicons::scan_desktop_files` lazy.

## Después

Volver a medir RSS (`/proc/PID/status`, `smaps`) y comparar con la línea base de arriba.
Nota: el servidor de notificaciones no se probó (otro daemon tenía el nombre D-Bus).

## Idea: componentes residentes vs. bajo demanda

Hoy todos los componentes (barra, topbar, taskbar, dock, los cuatro overviews, menú, panel)
permanecen en memoria, y por eso alternar entre ellos es casi instantáneo. Se puede descargar
lo que no está visible (texturas de iconos y miniaturas, estado de componentes ocultos) para
ahorrar RAM, a costa de unos milisegundos al volver a abrirlos. Mejor como interruptor de
configuración (p. ej. `[memory] residency = "resident" | "on-demand"`) que como decisión fija.
Se combina con el punto 5 (LRU de texturas). Medir en el R7 antes de decidir el valor por defecto.

