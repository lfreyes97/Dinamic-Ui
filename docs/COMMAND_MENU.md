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

- Escribir filtra. `↑↓` navegan, `↵` ejecuta, `Tab` cambia de modo, `1..4` saltan a un
  modo, `Esc` cierra.
- Criterio de búsqueda (`src/search.rs`, el mismo que Actividades): cada palabra de la consulta
  debe coincidir con algún campo de la entrada (título, `id`, subtítulo). La mejor coincidencia
  de cada palabra puntúa: prefijo del título (100) > inicio de palabra del título (80) > contiene
  en el título (60) > `id` (40) > subtítulo (20). Sin subsecuencia: «frfx» no encuentra
  «Firefox». Se ordena por puntuación (a igualdad, el orden de la lista) y la consulta vacía
  muestra todo.
- `id` por modo: el `id` del `.desktop` en Apps, la clave en Comandos (`quit`, `reload`…), vacío
  en Archivos y Workspaces. En Archivos el subtítulo es la carpeta, así que buscar una carpeta
  encuentra sus archivos.
- Clic en un resultado ejecuta; clic en una pestaña cambia de modo.
- Mientras está abierto el layer pide `KeyboardInteractivity::Exclusive`.
- El `input_region` pasa a toda la pantalla.

## Pendiente

- [x] Iconos reales en los resultados: se resuelven por el `id` del `.desktop` (siempre presente
      en el mapa de iconos). *Sin verificar en pantalla.*
- [x] Destacar solo coincidencias estrictas: la subsecuencia se eliminó y el criterio es el
      compartido con Actividades (`src/search.rs`, ver «Interacción»). *Sin verificar en
      pantalla.*
- [ ] Archivos recientes de `recently-used.xbel` y orden por uso.
- [ ] Animación de entrada del palette.
