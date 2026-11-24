# Tras archivar quickshell (siguiente hito tras la Fase 2 del previewer)

Cutover aplicado el **23 nov 2026**: quickshell dejó de arrancarse y dinamic-ui pasó a
ser la shell y el servidor de notificaciones. La Fase 2 del previewer (config en vivo)
es el banco de pruebas para afinar lo que viene después.

## Qué se hizo (registro)

- `~/.config/hypr/hyprland.lua`: backup en
  `hyprland.lua.pre-quickshell-20261123-143432`; autostart `quickshell` → `awww-daemon`
  (guarded) + dinamic-ui; `dinamicUi` definido arriba; binds añadidos: `SUPER+I`,
  `SHIFT+I`, `ESC`, `ALT+I`, `CTRL+I`, `SUPER+D`, `SUPER+,`. Sintaxis validada con `luac -p`.
- `target/release/dinamic-ui` compilado (21 MB, `opt-level = 2` + LTO thin).
- `quickshell` detenido; su config archivada en
  `~/.config/quickshell.archived-20261123` (no borrada; se restaura moviéndola de vuelta).
- Verificado en vivo: `org.freedesktop.Notifications` en manos de dinamic-ui, socket de
  control presente, comandos CLI responden, tarjeta de notificación visible, wallpaper
  intacto (`awww query`) y `awww-daemon` en marcha.

Los cambios del `.lua` se aplican del todo al reiniciar Hyprland; en la sesión del
cutover dinamic-ui se arrancó a mano.

## Funciones que quedaron descubiertas

Quickshell cubría estas cosas que dinamic-ui aún no tiene. Orden sugerido de prioridad:

| # | Función perdida | Sustituto propuesto | Prioridad |
|---|---|---|---|
| 1 | ~~OSD de volumen / brillo~~ → implementado: `src/ui/osd.rs`, comandos `--osd-volume`/`--osd-brightness` y binds encadenados; ver `docs/OSD.md` | Componente del shell sobre `wpctl` + `brightnessctl` (o script con `notify-send`); los datos ya están en `sysinfo` | ✅ |
| 2 | Panel de capturas (región/pantalla/ventana, retardo) | `grim` + `slurp` + `wl-copy` enlazados a `SUPER SHIFT S`, con notificación de confirmación | Alta |
| 3 | Alt+Tab (MRU) y `SUPER+Tab` (workspaces) | Componente usando el modelo de clientes que ya tiene `ipc::clients()`; no requiere nuevo IPC | Alta |
| 4 | ~~Quick settings con toggles~~ → implementado parcialmente: wifi, bluetooth, DND, volumen, brillo y lock con confirmación; faltan power mode y night light | `src/actions.rs` + renderer compartido en `ui/quick_settings.rs`, hosts en topbar y panel System | ✅/Media |
| 5 | Tema que sigue `org.gnome.desktop.interface` (`color-scheme`, `accent-color`) | Watch de gsettings → `config::set` + `Palette::load()`; se apoya directamente en la Fase 2 del previewer | Media |
| 6 | Hot corners (superior-izquierdo overview, inferior-izquierdo apps) | El cursor ya llega en `UpdateCtx`; añadir componente con umbral de permanencia (~180 ms) | Baja |
| 7 | System tray | Requiere `com.system76.CosmicStatusNotifierWatcher` (está activable en D-Bus) | Baja |
| 8 | MPRIS (tarjeta de media en el calendario) | `playerctl` / MPRIS vía zbus; encaja en la pestaña Alerts o el calendario del topbar | Baja |

## Por qué va después de la Fase 2 del previewer

La Fase 2 añade recarga en vivo de `config.toml` y sliders de tema/tamaños. Eso convierte
al previewer en el banco de pruebas de estos reemplazos: se puede ajustar el OSD, los
toggles o el tema y ver el resultado sin recompilar ni reiniciar el shell real. Por eso
este backlog se encadena después de esa fase, no antes.

## Notas

- No es una promesa de implementar todo: es el inventario de lo que se dejó atrás y el
  orden en que conviene recuperarlo.
- Parte ya tiene cimientos en el repo: `sysinfo` (datos de sistema), `ipc::clients()`
  (ventanas), `NotificationCard` (confirmaciones) y el panel System (solo lectura).
- `awww` ya gestiona el fondo; no hay que reimplementar el wallpaper picker de quickshell.
