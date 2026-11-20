# Módulo de wallpaper — diseño

## Principio

**Una fuente de datos, varias vistas.** La pestaña Wallpaper no posee el estado:
lo consume. Las vistas (lista y grid) son intercambiables y comparten exactamente
la misma colección de imágenes y miniaturas.

## Fuentes de datos

| Dato | Origen | Módulo |
|---|---|---|
| Carpeta de imágenes | `[wallpapers] dir` en `config.toml` | `config.rs` |
| Lista de archivos | `list_images(dir)`, reescaneo cada 5 s | `ui.rs` → `UiState.wallpaper_files` |
| Miniaturas 192×108 | hilo propio + caché en `$XDG_CACHE_HOME/dinamic-ui/thumbs` | `thumbs.rs` |
| Texturas egui | `load_texture` al terminar cada miniatura | `UiState.thumb_textures` |
| Fondo real | `awww query` | `sysinfo.rs` |

## Reglas

1. La UI **nunca** ejecuta `awww` directamente: publica una intención en
   `UiState` (`wallpaper_image_request`, `wallpaper_color`) y `main.rs` lanza
   el comando. Así la vista no depende del daemon.
2. Las miniaturas se decodifican fuera del hilo de render; la UI solo pinta
   `TextureHandle`s ya listas. Mientras tanto, muestra un placeholder.
3. El fondo "actual" se obtiene del sistema (`awww query`), no de lo que la
   UI marcó: la selección optimista se corrige en el siguiente poll.
4. Lista y grid son la misma datos con distinto layout. El botón del header
   de `IMAGES` alterna `UiState.wallpaper_grid`.

## Pendiente para completar el módulo

- [ ] Preview grande con hover (la de Quickshell: `hovered || current`).
- [ ] Shuffle y "abrir carpeta" (`xdg-open`).
- [ ] Pestaña de transición (efecto/dirección/curva/duración + probar).
- [ ] Persistir fondo y transición en `~/.local/state/dinamic-ui/`.
- [ ] Atajo IPC/CLI para cambiar de wallpaper sin abrir el panel.

## Nota de arquitectura general

Los mismos principios deberían aplicarse a notificaciones: hoy `notifications.rs`
lee la interfaz de Mako, lo que acopla el módulo a un daemon concreto (y puede
llegar a activarlo por D-Bus). El siguiente paso es convertirlo en una fuente
de datos agnóstica, con la vista de la isla y la pestaña Alerts como consumidoras.
