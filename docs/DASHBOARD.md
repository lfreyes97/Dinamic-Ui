# Dashboard (rejilla bento)

Vista a pantalla completa con widgets en rejilla (feature `dashboard`, comando `--dashboard`).
Código: `src/ui/dashboard.rs`. Vista previa: `src/ui/preview_dashboard.rs`.
Guía visual: variante B de `docs/dashboard-mockup.html` (líneas de diseño, no medidas).

**Estado: maqueta de solo lectura.** Dibuja todos los widgets del preset con datos reales cuando
existen y con estado vacío cuando no hay fuente. No ejecuta acciones del sistema: los accesos
rápidos solo disparan comandos internos.

## Qué hay

Cabecera: saludo según la hora con el nombre de la variable `USER`, la fecha larga en español y la
hora. Debajo, una rejilla de 4 × 3 celdas que rellena el espacio disponible.

| Widget | Celdas | Fuente | Contenido |
|---|---|---|---|
| Calendario | 1 × 2 | `chrono` (propio, no `panel.rs`) | Mes actual de lunes a domingo, 6 semanas; hoy resaltado; días de los meses vecinos atenuados |
| Sistema | 2 × 1 | `shared.battery`, `shared.system` | Batería (% y «cargando»), Wi-Fi, volumen y brillo, con barra cuando hay porcentaje |
| Clima | 1 × 1 | ninguna | **Estado vacío** («Sin datos», fuente pendiente) |
| Workspaces | 2 × 1 | `shared.snapshot`, `shared.clients` | Miniatura de cada workspace ocupado con sus ventanas en la posición relativa real; el activo resaltado |
| Reproduciendo | 1 × 1 | ninguna | **Estado vacío** (fuente pendiente) |
| Red | 1 × 1 | `shared.system.wifi` | Estado del Wi-Fi (`wifi_status`) |
| Procesos | 1 × 1 | ninguna | **Estado vacío** (fuente pendiente) |
| Ventanas abiertas | 1 × 1 | `shared.clients` | Clase y título de cada ventana mapeada; «+N más» si no caben |
| Accesos rápidos | 1 × 1 | comandos internos | Menú, Vista general, Actividades, Configuración, Barra superior y Barra de tareas |

- **Datos reales:** Sistema, Workspaces, Calendario, Red y Ventanas abiertas. Sin dato del
  sistema aparece «No detectada» o «sin datos», nunca un valor inventado.
- **Estados vacíos honestos:** Clima, Reproduciendo y Procesos muestran «Sin datos» y «Fuente de
  datos pendiente». Una lista vacía de ventanas o workspaces dice «Sin ventanas» o «Sin
  workspaces», porque ahí sí hay dato (que no hay nada).
- **Accesos rápidos:** cada botón llama a `dispatch_command` con el comando interno. Si el
  componente no está compilado, el comando no hace nada y el dashboard sigue abierto. Si se
  atiende, el dashboard se cierra.
- **Iconos de app:** con la feature `icons` se usan las texturas reales (`shared.icon`); sin ella,
  el glifo de reserva de `dock_icon`. El dashboard no activa `icons` por sí mismo.
- **Interacción:** resaltado al pasar el cursor en los accesos rápidos (`palette.hover`) y cursor
  de mano en lo pulsable. Un clic en el fondo cierra; un clic dentro de una tarjeta no cierra.
  `Esc` cierra.

## Disposición como datos

- `PRESET`: nueve widgets (`Tile { id, col, row, w, h }`) que cubren las 12 celdas sin huecos ni
  solapes.
- `layout(area, gap, cols, rows, tiles)`: función pura que calcula el rect de cada widget. Descarta
  los que no caben en la rejilla. Es la base para una rejilla editable: cambiar el preset no toca
  el dibujo.
- `Rejilla::rect`: el mismo cálculo para subcuadrículas (sistema, accesos).

## Escala

`escala(alto) = (alto / 720).clamp(1.0, 1.6)`, la misma regla que `nav.rs` (no se usa `nav.rs`, que
solo existe con `overview` o `activities`). Márgenes, huecos, iconos y textos se multiplican por
esa escala. Revisado a 1366 × 768 y 1920 × 1080 (`target/ui-preview/dashboard*.png`).

## Pruebas

- Unitarias (`cargo test dashboard`): sin solapes y dentro del área; las 12 celdas cubiertas y
  los huecos exactos; descarte de widgets fuera de la rejilla; escala de la miniatura de ventanas;
  escala; fecha larga en español; saludo; calendario de 42 celdas desde lunes; capitalización del
  nombre.
- Vistas previas (`cargo test ui_preview_dashboard -- --ignored --nocapture`): 1366 × 768, 1080p,
  sin ventanas, muchas ventanas (resúmenes «+N más») y sin datos de sistema.

## Pendiente

- **Fuentes de datos** para Clima (servicio meteorológico), Reproduciendo (MPRIS o similar) y
  Procesos (`/proc`). Hasta entonces, sus tarjetas siguen vacías.
- **Rejilla editable:** `PRESET` es el único layout. Guardarlo en la configuración (pendiente de
  decisión; `config.toml` y el panel de configuración son de otro trabajo).
- **Acciones:** la maqueta es de solo lectura. Un clic en una ventana o un workspace no hace nada
  todavía.
- **Red:** solo el estado del Wi-Fi. Velocidades y Bluetooth quedan fuera hasta que haya datos.
