# Plan: sistema de modos y panel

Estado actual del shell y plan de trabajo para la próxima fase grande.

## 1. Modos existentes

El shell tiene tres estados visuales controlados por `UiState` (`src/ui.rs`):

Tamaños = valores por defecto de `config.toml`.

| Modo | Tamaño (default) | Contenido |
|---|---|---|
| **Isla** | 260×32 | workspace · reloj · batería (`space-between`) |
| **Barra** | 640×32 | workspaces 1–10 · opciones · reloj+fecha · sistema |
| **Panel** | 390×300 | tarjeta redondeada centrada bajo la barra, con 5 pestañas |

Transiciones:

- Isla → Barra: clic en la isla o `dinamic-ui --expand/--toggle`.
- Barra → Isla: botón "A isla" del panel, `Esc` o `--collapse`.
- Barra → Panel: botón de opciones (cuadrícula) o `--panel`.

## 2. Geometría (ya estabilizada)

- El layer surface es **fijo** a todo el ancho y `bar_h + panel_h` (config) de alto.
- La píldora se dibuja dentro y el `input_region` sigue su rect.
- No hay `set_size`/`set_margin` durante la animación → sin `configure` events ni jitter.
- Animación: `smootherstep` + crossfade de opacidad + panel con progreso propio.
- `hyprland.lua` lleva `no_anim` para el namespace `dinamic-ui`.

## 3. Problema resuelto en Fase A

Antes los parámetros estaban repartidos en constantes y variables de entorno, sin una
única fuente de verdad ni forma de recargar. Ahora viven en `config.toml` y se aplican
con `dinamic-ui --reload` (los env vars siguen como override).

## 4. Plan de implementación

### Fase A — Configuración por archivo ✅ completada

1. Añadir crate `toml` + `serde`.
2. Nuevo `src/config.rs`:
   - `Config { theme, sizes, workspaces }` con `Default`.
   - Carga desde `$XDG_CONFIG_HOME/dinamic-ui/config.toml` (o `~/.config/dinamic-ui/config.toml`).
   - Valores ausentes → defaults; valores inválidos → warning + default.
3. `Palette::load()` lee la config (env vars siguen como override).
4. `ui::metrics()` lee la config en cada acceso; `--reload` reemplaza el `Arc<Config>`.
   - Recargar también la paleta (`self.palette = Palette::load()`) y el tamaño del surface.
5. Control socket: nuevo comando `reload` + CLI `--reload`.
6. Keybinds de ejemplo en `hyprland/dinamic-ui.conf`.
7. Ejemplo en `config.example.toml` y documentación revisada en `README.md` / `docs/ANIMATIONS.md`.

### Fase B — Notificaciones DBus ✅ completada

1. `zbus` 5 implementa el **servidor estándar** `org.freedesktop.Notifications`
   (`Notify`, `CloseNotification`, `GetCapabilities`, `GetServerInformation`) en un hilo propio.
2. Sin backend de Mako: ni consultas privadas ni activación por D-Bus de otro daemon.
3. Cola con TTL (`notifications.ttl_secs`), máximo 16, revisión para detectar cambios.
4. La isla mantiene sus tres datos y muestra la notificación en una **tarjeta propia**
   de 320×68 bajo la barra, con app, hora, título, cuerpo y botón de cerrar.
5. La pestaña Alerts consume la misma cola (DND, listado, limpiar todo).

### Fase C — Revisión ✅ completada

1. `README.md`: estados, comandos, variables y archivo de config.
2. `docs/ANIMATIONS.md`: geometría fija, crossfade, fuentes.
3. Este plan: marcar lo completado.

## 5. Criterios de aceptación

- `cargo check` sin errores.
- Arranca sin archivo de config con los valores actuales.
- `--reload` aplica cambios sin reiniciar.
- La isla sigue mostrando solo workspace/reloj/batería.
- Documentación coherente con el código.

## Fase D — Modo Panel ✅ completada

El panel de control tiene cinco pestañas:
**Workspace** (workspaces + presets de layout), **Clock** (reloj, calendario, alarmas,
reloj mundial), **System** (batería, conectividad, niveles, dispositivos), **Wallpaper**
(colector + apariencia) y **Notifications** (DND, lista agrupada, descartar/clear-all,
estado vacío). Esta última reutiliza la cola de la Fase B: no necesita otro módulo
DBus, solo una vista y acciones sobre la misma cola.

### Implementación ✅ completada

- Panel con cabecera "Control Center" y cinco pestañas (icono + etiqueta, activa resaltada).
- Workspace: rejilla de workspaces configurables (clic → `dispatch workspace N`) + presets.
- Clock: hora con segundos, fecha larga en español, formato y semana ISO.
- System: batería con barra de progreso y estado, Wi-Fi y audio.
- Wallpaper: vista previa con la hora y seis swatches que pintan un color sólido con `awww clear`.
- Alerts: DND (suprime la isla), limpiar todo, tarjetas de notificación con descartar por id y estado vacío. Reutiliza la cola de la Fase B.
- `UiState.panel_tab` controla la pestaña; `DINAMIC_UI_PANEL_TAB=0..4` permite arrancar en una concreta.
