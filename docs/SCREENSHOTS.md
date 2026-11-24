# Panel de capturas

Sustituye al panel de capturas de Quickshell (ítem 2 de `docs/POST_QUICKSHELL.md`).
Se abre con `--screenshots` (bind `PRINT` en el ejemplo de Hyprland) o con el comando
`screenshots` del socket de control.

## Modos

| Modo | Acción |
|---|---|
| Región | `grim -g "$(slurp)" - \| wl-copy` — arrastras el área |
| Pantalla completa | `grim - \| wl-copy` |
| Ventana activa | geometría de `hyprctl activewindow -j` (vía `jq`) → `grim -g … \| wl-copy` |
| Pantalla en 5 s | `sleep 5` y luego pantalla completa |

Tras copiar, `notify-send` vuelve al servidor de notificaciones de dinamic-ui, así que
la confirmación aparece como tarjeta del shell (no depende de otro daemon).

## Arquitectura

- `src/ui/screenshots.rs`: estado del panel, dibujo y manejo de teclado.
- `src/ui/component.rs`: registro del componente; cierra los otros overlays al abrirse.
- `src/control.rs`: comando `screenshots` (feature `screenshots`).
- El componente **no ejecuta nada**: encola `sh -c` en `Shared.pending_spawns` y
  `main.rs` lo lanza fuera del render, igual que el resto de acciones.
- Teclado: `↑`/`↓` eligen, `↵` captura, `Esc` cierra. Clic en una fila también captura.
- No hay configuración: los cuatro modos y el retardo de 5 s son fijos.

## Requisitos

`grim`, `slurp`, `wl-copy`, `jq` y `notify-send` (presentes en la máquina de desarrollo).
Hyprland debe permitir `screencopy` a `grim` (`hl.permission` en la config).
