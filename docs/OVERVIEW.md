# Overview — diseño

## Referencia

Como el overview de GNOME / tu Quickshell: vista de todos los workspaces sobre
el escritorio oscurecido, con las ventanas en su posición real, búsqueda de apps
y clic para enfocar. Es un **modo** más del shell, no una ventana aparte.

## Principios

- El surface ya cubre toda la pantalla: el overview se dibuja como una capa más
  (no requiere otro layer shell).
- Una sola fuente de datos: `ipc.rs` (`j/clients`, `j/workspaces`, `j/monitors`).
- Las vistas (isla, barra, panel, overview) consumen estado; no consultan Hyprland
  directamente.
- Se activa por IPC (`--overview`) y, más adelante, por hot corner superior izquierdo.

## Fases

### Fase 1 — Vista de workspaces ✅ completada

1. `UiState.overview: bool` + comando `overview` en `control.rs`.
2. `ClientInfo` ampliado con workspace, posición y tamaño.
3. Overlay a pantalla completa: fondo oscuro, rejilla de tarjetas de workspace,
   cada una con rectángulos de ventana en su posición relativa (esquemático; sin
   miniaturas en vivo todavía).
4. Clic en una ventana → enfocar y cerrar. Clic en un workspace → cambiar y cerrar.
   `Esc` o clic fuera → cerrar.

### Fase 2 — Búsqueda de apps

- Campo de texto arriba; lista de `.desktop` ya escaneada por `appicons.rs`.
- Escribir filtra; `Enter` lanza la app; flechas navegan.

### Fase 3 — Miniaturas en vivo (opcional)

- `ext-image-copy-capture-v1` / `wlr-screencopy` vía `wayland-protocols`.
- Es la parte más pesada: se evaluará coste antes de añadirla.

### Fase 4 — Pulido

- Hot corner, animación de entrada (escala + fade), indicadores de monitor.

## Estado actual (ronda de mejora de overviews)

Cuatro vistas sobre un mismo estado (`OverviewState`): **Workspaces** (rejilla), **Actividades**,
**franja horizontal** y **franja vertical**. La lógica vive en `ui/overview_model.rs` (pura, con
tests); el dibujo en `overview.rs` y `overview_collection.rs`. Comandos: `overview`,
`overview-activities`, `overview-strip-h`, `overview-strip-v`. **El mismo comando abre y cierra**
(antes solo la rejilla alternaba); con otra vista abierta, cambia a esa.

| | Workspaces | Actividades | Franjas |
|---|---|---|---|
| Modal (teclado en exclusiva) | sí | sí | no (no le quita el teclado a la app de debajo) |
| Búsqueda al escribir | filtra ventanas (atenúa las que no coinciden) | filtra apps instaladas | — |
| `←→↑↓`, `Tab` | mueve la selección | mueve la selección | avanza/retrocede |
| `↵` | cambia de workspace (con búsqueda: enfoca la primera coincidencia) | enfoca o lanza | cambia de workspace |
| `1`–`9` | salta al workspace (sin búsqueda) | — | salta al workspace |
| `Esc` | limpia la búsqueda y, si no hay, cierra | ídem | cierra |

- **Fundido** de entrada y salida (franjas: además se deslizan desde su borde). Mientras se
  desvanece al cerrar no recibe clics.
- **Actividades usa las apps reales** (`.desktop`, leídas al abrir y soltadas al cerrar), no una
  lista fija. Primero las abiertas, luego las ancladas (`[dock] pinned`) y el resto por nombre;
  con búsqueda, por puntuación (prefijo > inicio de palabra > contiene > id > comentario).
  Con varias ventanas de una app, `↵` va ciclando entre ellas. Una ventana sin `.desktop` aparece
  igualmente (se enfoca, no se lanza).
- La ventana activa se pinta encima si se solapan; las tarjetas vacías lo indican.
- El lanzamiento respeta las comillas del `Exec` (`desktop::split_exec`).
- Las entradas `.desktop` se leen en un solo sitio (`src/desktop.rs`: solo el grupo
  `[Desktop Entry]`, códigos de campo fuera) y las comparten iconos, command menu y overview.

### Diseño visual (alineado con `overviews-mockup.html`)

- **Buscador** con chip `ESC` en las dos vistas modales (rejilla y Actividades): se tiñe de acento
  al escribir y la rejilla cuenta las coincidencias.
- **Rejilla 3×2** que incluye también los workspaces vacíos (se rellenan con los de menor
  número libre): etiqueta `01` arriba, nombre abajo (`[workspaces] names`, o «Vacío» /
  «N ventanas» si no hay nombre), tarjeta activa teñida con halo de acento, sombra y ligero
  levantado al pasar el cursor. Las ventanas son bloques translúcidos y la activa va teñida.
- **Actividades:** cuadrícula de 6 columnas centrada, tiles con borde, puntos por ventana abierta
  y tira inferior con una vista esquemática de cada ventana y su título.
- **Franjas:** botones con borde, miniaturas con número arriba a la derecha y nombre abajo,
  y un bloque «Overview» (icono + etiqueta) tras un separador.
- **Barra de ayuda** inferior con las teclas dibujadas como chips.

### Dimensiones: el mockup es una guía, no una medida

Las franjas del mockup (68/78 px) eran una barra de estado, no un panel de escritorios. Ahora son
**paneles proporcionales a la pantalla** (`config::StripSize`: fracción o píxeles):

| Franja | Por defecto | En 1366×768 | En 1920×1080 |
|---|---|---|---|
| Horizontal (alto) | 1/5 del alto | **154 px** | 216 px |
| Vertical (ancho) | 1/6 del ancho | **228 px** | 320 px |

- **Configuración:** `[overview] strip_h_height = 0.25` (fracción, de 0,05 a 0,5) o `= 200`
  (píxeles enteros); igual con `strip_v_width`. Siempre entre 40 px y la mitad de la pantalla.
- **Miniaturas:** conservan la proporción del monitor y envuelven su imagen, así las ventanas se
  leen (en 1366×768 miden ~220×130 px). Cuántas caben sale del espacio libre (3–8) y el
  teclado lo respeta.
- **Controles y textos** (flechas, bloque «Overview», número y nombre) crecen con el grosor.
- **Actividades:** tiles, separación, iconos y tira de ventanas escalan entre 1× y 1,6× respecto
  a 720p; los textos crecen con la raíz de esa escala.

### Vista previa sin compositor

`just preview` (`cargo test ui_preview -- --ignored`) renderiza la **UI real** a PNG en
`target/ui-preview/` usando la GPU y `egui_kittest` (dependencia solo de desarrollo), con datos
de ejemplo y la config por defecto: `overview-workspaces`, `…-search`, `overview-activities`,
`…-search`, `strip-h-top`, `strip-h-bottom`, `strip-v-left`, `strip-v-right`, y las versiones
`-1080p` de rejilla, Actividades y franjas, y `strip-h-top-1366` / `strip-v-right-1366` con la
resolución del R3 (1366×768). Permite revisar el
diseño sin Hyprland. No es una prueba automática (no compara imágenes): sirve para mirar.

### Pendiente

- [ ] **Cerrar una ventana desde el overview** (clic central o botón): hay que traducir
      `closewindow` a la API Lua de Hyprland en `ipc.rs::dispatch` y verificarlo; no se añadió sin
      poder probarlo.
- [ ] **Varios monitores**: `ClientInfo` no trae el monitor, así que las ventanas de otro monitor
      se escalan con el tamaño del actual.
- [ ] Arrastrar una ventana a otro workspace.
- [ ] Miniaturas en vivo (Fase 3 de arriba) y hot corner.
- [ ] Verificación en el R7 de lo que la vista previa no cubre: el fundido y el deslizamiento
      (son animaciones), el teclado exclusivo y el comportamiento con varios monitores. El
      aspecto estático ya se revisó con `just preview`.
- [ ] El mockup tiene ventanas de color (variante azul) y desenfoque del fondo
      (`backdrop-filter`): no se hicieron; el desenfoque necesitaría captura del escritorio.

