# Command menu — implementación

Basado en `command-menu-mockup.html`. Se abre con `dinamic-ui --menu` /
`just menu` / **SUPER + SPACE**.

## Modos

| Modo | Datos | Acción |
|---|---|---|
| Apps (1) | `.desktop` del sistema + usuario (`src/desktop.rs`) | `Exec=` sin códigos de campo, respetando comillas → spawn |
| Archivos (2) | `~/Pictures`, `~/Documents`, `~/Downloads` (config `[menu] file_dirs`) | `xdg-open` |
| Workspaces (3) | `j/workspaces` occupados | `dispatch hl.dsp.focus({ workspace })` |
| Comandos (4) | acciones del shell | panel, overview, reload, compact, quit |

## Interacción

- Escribir filtra (substring + subsecuencia). `↑↓` navegan, `↵` ejecuta,
  `Tab` cambia de modo, `1..4` saltan a un modo, `Esc` cierra.
- Clic en un resultado ejecuta; clic en una pestaña cambia de modo.
- Mientras está abierto el layer pide `KeyboardInteractivity::Exclusive`.
- El `input_region` pasa a toda la pantalla.

## Pendiente

- [x] Iconos reales en los resultados: se resuelven por el `id` del `.desktop` (siempre presente
      en el mapa de iconos). *Sin verificar en pantalla.*
- [ ] Destacar solo coincidencias estrictas (el score de subsecuencia es muy laxo).
- [ ] Archivos recientes de `recently-used.xbel` y orden por uso.
- [ ] Animación de entrada del palette.
