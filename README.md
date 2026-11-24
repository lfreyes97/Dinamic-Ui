# Dinamic UI

UI para Hyprland escrita en Rust desde cero: una **isla** compacta en la parte superior que se expande a una **barra** con panel funcional. Sin launcher (rofi u otros cubren ese papel).

## Stack

| Capa | Crate |
|---|---|
| Layer-shell / eventos Wayland | `smithay-client-toolkit` 0.21 |
| UI (inmediata, con widgets) | `egui` 0.37 |
| Iconos | `egui-pop-phosphor` (Phosphor Regular + Bold, bundleado) |
| Colores | base16 Black Metal / Bathory (config `theme` + `DINAMIC_UI_THEME`) |
| Configuración | `toml`, recarga en caliente con `--reload` |
| Render | `egui-wgpu` 0.37 + `wgpu` 30 |
| Datos de Hyprland | socket IPC (`j/activeworkspace`, `j/activewindow`, `dispatch`) |
| Notificaciones | servidor freedesktop `org.freedesktop.Notifications` (`zbus` 5) |

`eframe` **no** se usa: su backend (winit) no soporta `wlr-layer-shell`. Por eso el surface se crea con smithay y se pasa a wgpu mediante los handles crudos de Wayland (`wl_display` / `wl_surface`).

## Requisitos

- Hyprland (o cualquier compositor wlroots con `zwlr_layer_shell_v1`).
- Rust 1.95+ (egui-wgpu 0.37 lo requiere).
- En Arch: `base-devel wayland libxkbcommon mesa` (gbm/include).

## Compilar y ejecutar

```bash
cargo build --release
./target/release/dinamic-ui
```

Arranca arriba al centro como isla (**260×32** por defecto, píldora negra con borde `#303030`; tamaños y colores configurables) y muestra **solo tres datos** distribuidos con `space-between`: workspace actual a la izquierda (badge 22×22), reloj semibold centrado y batería a la derecha (icono dibujado en verde `#8be09b` + porcentaje, leyendo `/sys/class/power_supply`). Haz clic en la isla para expandirla a barra (**640×32**). En la barra:

- Workspaces `1`–`10` por defecto (`workspaces.count`) como círculos, con el activo invertido: `hyprland dispatch workspace N`.
- Botón de opciones de ventana (icono de cuadrícula): abre el **panel de control** con cinco pestañas (Workspace, Clock, System, Wallpaper y Alerts).
- Reloj con hora y fecha corta en español (`Mié. 18 nov.`).
- Batería (verde), Wi-Fi y audio a la derecha.
- `Esc` o el botón "A isla" del panel: cierra el panel / vuelve a la isla. `Esc` **nunca** cierra el proceso; para salir usa el botón "Salir" del panel o `dinamic-ui --quit`.

Los iconos son Phosphor y se registran como fallback de la fuente proporcional de egui, así que no dependen de las fuentes instaladas en el sistema.

### Control por teclado (sin depender del foco)

El binario expone un socket en `$XDG_RUNTIME_DIR/dinamic-ui.sock` y un modo CLI:

```bash
./target/release/dinamic-ui --toggle     # isla <-> barra
./target/release/dinamic-ui --expand
./target/release/dinamic-ui --collapse
./target/release/dinamic-ui --panel      # abre/cierra el panel
./target/release/dinamic-ui --compact    # fija el modo compacto (reloj)
./target/release/dinamic-ui --notch      # vuelve al notch
./target/release/dinamic-ui --overview   # overview de workspaces
./target/release/dinamic-ui --activities            # lanzador: apps + ventanas
./target/release/dinamic-ui --overview-strip-h     # franja horizontal
./target/release/dinamic-ui --overview-strip-v     # franja vertical
./target/release/dinamic-ui --taskbar     # taskbar Windows-like (4 lados)
./target/release/dinamic-ui --topbar      # topbar GNOME-like
./target/release/dinamic-ui --menu       # command menu (lanzador/archivos)
./target/release/dinamic-ui --quit
./target/release/dinamic-ui --reload    # recarga config.toml sin reiniciar
```

Así puedes enlazarlos en Hyprland (ver `hyprland/dinamic-ui.conf`): `SUPER+I` isla/barra,
`SUPER+T` taskbar, `SUPER+B` topbar, `SUPER+O` overview, `SUPER SHIFT+O` actividades,
`SUPER ALT+O` franja horizontal, `SUPER CTRL+O` franja vertical y `SUPER+M` command menu.

### Barras y overviews

La **taskbar** (`--taskbar`) es una vista sobre el modelo del dock, acoplable a los
cuatro lados con `[taskbar] side = "bottom" | "top" | "left" | "right"`, con
`autohide` opcional. La **topbar** (`--topbar`) muestra Actividades, reloj y estado
con desplegables de ajustes rápidos y calendario; su altura se configura en
`[topbar] height`. Ambas son mutuamente excluyentes y sustituyen a la isla/barra
mientras están activas.

Hay tres overviews de workspaces (`--overview`, `--overview-strip-h`,
`--overview-strip-v`). Las franjas se acoplan con `[overview] strip_h_side` y
`strip_v_side` y su tamaño (`strip_h_height` / `strip_v_width`: fracción de la pantalla o
píxeles). Actividades (`--activities`) es aparte: un lanzador con las apps instaladas y las
ventanas abiertas, con su propia feature. Las maquetas de referencia están en `docs/*-mockup.html`.

### Archivo de configuración

Se lee de `$XDG_CONFIG_HOME/dinamic-ui/config.toml` (o `~/.config/dinamic-ui/config.toml`). Copia [`config.example.toml`](config.example.toml) como punto de partida:

```toml
[theme]
name = "black-metal"   # o "bathory"
accent = "dd9999"      # opcional

[sizes]
bar_h = 32
panel_h = 160
island_w = 260
bar_w = 640

[workspaces]
count = 10

[notifications]
ttl_secs = 6
```

Tras editarlo: `dinamic-ui --reload`. Los campos ausentes usan los defaults y los env vars `DINAMIC_UI_*` tienen prioridad sobre el archivo.

### Variables

```bash
DINAMIC_UI_EXPANDED=1 ./target/debug/dinamic-ui   # arranca como barra
DINAMIC_UI_PANEL=1 ./target/debug/dinamic-ui      # arranca con el panel abierto
DINAMIC_UI_THEME=bathory ./target/debug/dinamic-ui # paleta alternativa
DINAMIC_UI_BAR_H=28 ./target/debug/dinamic-ui      # alto de isla/barra (24..48, default 32)
DINAMIC_UI_PANEL_H=140 ./target/debug/dinamic-ui   # alto del panel (80..320, default 160)
DINAMIC_UI_PANEL_TAB=4 ./target/debug/dinamic-ui   # pestaña inicial del panel (0..4)
```

El surface fijo mide `bar_h + 8 + panel_h + 2` según `config.toml` (342 px con los defaults: 32 de barra + 8 de separación + 300 de panel + 2 de margen para el borde). La barra es una píldora de 640×32 (se amplía sola si los workspaces configurados no caben) y el panel es una tarjeta independiente de 300 px de alto centrada debajo, que mide 390 px con la isla y el ancho de la barra con ella expandida. Con 300 px la pestaña Workspace cabe entera; Clock, System, Wallpaper y Alertas hacen scroll (barra fina en el padding derecho).

## Configuración de Hyprland recomendada

Ver [`hyprland/dinamic-ui.conf`](hyprland/dinamic-ui.conf). Lo mínimo es arrancarlo al inicio:

```lua
hl.on("hyprland.start", function()
    hl.exec_cmd("/ruta/a/Dinamic-Ui/target/release/dinamic-ui")
end)
```

## Estructura

```
src/
├── main.rs          # event loop de Wayland, layer surface, input, animación
├── gfx.rs           # wgpu sobre wl_surface + renderer de egui
├── ui.rs            # isla / barra / panel
├── ipc.rs           # consultas y dispatch al socket de Hyprland
├── power.rs         # batería desde sysfs
├── theme.rs         # paletas base16 + overrides
├── config.rs        # config.toml + recarga
├── sysinfo.rs       # Wi-Fi, audio, brillo, layout y fondo (hilo propio)
├── thumbs.rs        # miniaturas de wallpapers (hilo propio + caché en disco)
├── notifications.rs # servidor freedesktop + cola de notificaciones
└── control.rs       # socket de control + CLI

docs/ANIMATIONS.md      # diseño de las animaciones
docs/WALLPAPER.md       # diseño del módulo de wallpaper
docs/NOTIFICATIONS.md   # servidor freedesktop y tarjeta de notificación
docs/DOCK.md            # dock inferior con autohide
docs/ISLAND.md          # isla tipo notch y batería
docs/OVERVIEW.md        # overview de workspaces
docs/COMMAND_MENU.md    # command menu (maqueta + implementación)
docs/PREVIEWER.md       # previsualizador interactivo (widget factory)
docs/SUPPORT_OTHER_WCS.md # fakers de otros WCs (idea opcional, no roadmap)
docs/POST_QUICKSHELL.md # cutover y backlog tras archivar quickshell
plan-mode-panel.md      # plan de modos y panel
config.example.toml     # ejemplo de configuración
```

## Estado actual y próximos pasos

Funciona: layer-shell, render transparente con wgpu, isla → barra animada, workspace/reloj/batería, panel con botones iconados, paleta base16, config TOML con recarga, keybinds por socket/CLI, destello al cambiar de workspace/ventana y repintado bajo demanda (se dibuja solo cuando hay cambios o cada ~15 frames).

### Animaciones

- Morph isla ↔ barra con `smootherstep` (arranque y parada sin salto de velocidad).
- Crossfade del contenido: la isla y la barra se dibujan superpuestas con opacidad complementaria, en vez de cambiar de golpe a mitad de camino.
- El panel tiene progreso propio: se abre **y se cierra** animado (altura + opacidad).
- El layer surface tiene geometría fija (todo el ancho, `bar_h + 8 + panel_h + 2` de la config): la píldora se dibuja dentro y el `input_region` la sigue. No hay ni un `set_size` durante la animación, así que no hay desfase con los `configure` del compositor. Ver `docs/ANIMATIONS.md`.
- La config de Hyprland lleva `no_anim` para el namespace `dinamic-ui`, para evitar doble interpolación.
- El destello de eventos usa decaimiento cuadrático en vez de lineal.

Pendiente:

- [x] Notificaciones DBus dentro de la isla (Fase B).
- [ ] Múltiples monitores (ahora se ancla al monitor por defecto).
- [ ] Entrada de texto por teclado en egui (Escape y los keybinds cubren el control básico).
- [x] Configurable vía archivo (colores, tamaños, workspaces) con recarga en caliente.
- [ ] Cubrir el backlog de quickshell archivado (OSD, capturas, Alt+Tab, quick settings…): [`docs/POST_QUICKSHELL.md`](docs/POST_QUICKSHELL.md).

> **Idea opcional de exploración (no es roadmap ni está priorizada)**: simular en el
> previewer el comportamiento de otros WCs mediante *fakers*. El proyecto se usa con
> Hyprland y no hay necesidad actual de otros compositores; el soporte real exigiría
> diseñar IPCs propios. La idea, por si algún día interesa, está en
> [`docs/SUPPORT_OTHER_WCS.md`](docs/SUPPORT_OTHER_WCS.md).

Ver [`plan-mode-panel.md`](plan-mode-panel.md) para el plan completo de modos y la siguiente fase (notificaciones DBus).

### Panel de control

El panel se abre con el botón de cuadrícula de la barra (o `--panel`) y tiene cinco pestañas:

- **Workspace**: rejilla de workspaces (clic → `dispatch workspace N`) y presets de layout.
- **Clock**: hora con segundos, fecha larga en español, formato y semana ISO.
- **System**: batería (con tiempo restante), Wi-Fi, Bluetooth, brillo, volumen y dispositivos, leídos cada 5 s con el panel abierto vía `nmcli`, `bluetoothctl`, `brightnessctl`, `pactl` y `hyprctl` ("Sin datos" si falta alguno).
- **Wallpaper**: muestra el fondo actual (imagen o color, leído con `awww query`), lista las imágenes de la carpeta `[wallpapers] dir` (por defecto `~/Pictures/Unsplash`, o `~/Pictures` si no existe) y las aplica con `awww img`, y ofrece seis colores sólidos con `awww clear`. Cada imagen lleva miniatura: se genera en un hilo aparte (192×108) y se guarda en `~/.cache/dinamic-ui/thumbs`, así que la primera vez tarda unos segundos y después es instantánea; con una imagen de fondo, la vista previa la usa de fondo. Requiere [`awww`](https://codeberg.org/LGFae/awww) con su demonio (`awww-daemon`) en marcha; el fondo persiste aunque se cierre la UI.
- **Alerts**: DND, "Limpiar todo" y tarjetas de notificación con descartar individual.

La pestaña Alerts usa la misma cola DBus que la isla: no hay dos suscripciones.

### Tipografía

Se usa **Inter** (Regular) como fuente principal y **Inter SemiBold** para los textos destacados. Por defecto se autodetectan en `$XDG_DATA_HOME/fonts/{instatypst,Instatypst}/Inter/static` (ambas capitalizaciones, porque Google Fonts instala la carpeta en minúsculas); con `[font] regular/semibold` en `config.toml` se elige cualquier par de TTF. La fuente se aplica en vivo desde el previewer y con `dinamic-ui --reload`. Egui 0.37 no tiene peso en `FontId` (`.strong()` solo cambia el color), así que la negrita se selecciona por familia. El surface usa un formato **no sRGB** (`Bgra8Unorm`/`Rgba8Unorm`) porque egui entrega colores en espacio gamma: con sRGB el texto salía apagado.

### Dock

Dock inferior con autohide: aparece al llevar el cursor a los 24 px inferiores (IPC `cursorpos`), con apps fijadas + ventanas mapeadas de Hyprland. Clic enfoca o lanza. Ver [`docs/DOCK.md`](docs/DOCK.md).

### Notificaciones

La isla muestra temporalmente la notificación activa (icono, título y cuerpo), se ensancha para ella y vuelve a los tres datos al caducar o al hacer clic para descartar. Se usa el backend de Mako: `Notify` es un método DBus, no una señal, así que se escucha `PropertiesChanged` y se consulta `ListNotifications`. El TTL se configura en `[notifications] ttl_secs` (default 6 s).

La batería se consulta cada 2 s. Si el equipo no expone batería (desktop), se muestra solo el icono.
