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

## Estado actual

Tres vistas de **workspaces** sobre un mismo estado (`OverviewState`): la **rejilla**, la
**franja horizontal** y la **franja vertical**. La lógica vive en `ui/overview_model.rs` (pura,
con tests); el dibujo en `overview.rs` (rejilla) y `overview_strips.rs` (franjas); las piezas
visuales comunes (buscador, barra de teclas, sombra) en `overlay.rs` y la navegación en rejilla en
`nav.rs`. Comandos: `overview`, `overview-strip-h`, `overview-strip-v`. **El mismo comando abre y
cierra**; con otra vista abierta, cambia a esa.

> **Actividades ya no es un overview.** Es un lanzador (apps instaladas + ventanas) con su propia
> feature `activities`, comando `--activities` y documento: ver `docs/ACTIVITIES.md`. Se abre con
> `overview`/`activities` de forma excluyente (solo un overlay a pantalla completa a la vez).

| | Rejilla | Franjas |
|---|---|---|
| Modal (teclado en exclusiva) | sí | no (no le quita el teclado a la app de debajo) |
| Búsqueda al escribir | filtra ventanas (atenúa las que no coinciden) | — |
| `←→↑↓`, `Tab` | mueve la selección | avanza/retrocede |
| `↵` | cambia de workspace (con búsqueda: enfoca la primera coincidencia) | cambia de workspace |
| `1`–`9` | salta al workspace (sin búsqueda) | salta al workspace |
| `Esc` | limpia la búsqueda y, si no hay, cierra | cierra |

- **Fundido** de entrada y salida (franjas: además se deslizan desde su borde). Mientras se
  desvanece al cerrar no recibe clics.
- La ventana activa se pinta encima si se solapan; las tarjetas vacías lo indican.

### Diseño visual (alineado con `overviews-mockup.html`)

- **Buscador** con chip `ESC` en la rejilla: se tiñe de acento al escribir y cuenta las
  coincidencias.
- **Rejilla 3×2** que incluye también los workspaces vacíos (se rellenan con los de menor
  número libre): etiqueta `01` arriba, nombre abajo (`[workspaces] names`, o «Vacío» /
  «N ventanas» si no hay nombre), tarjeta activa teñida con halo de acento, sombra y ligero
  levantado al pasar el cursor. Las ventanas son bloques translúcidos y la activa va teñida.
- **Franjas:** botones con borde y miniaturas con número arriba a la derecha y nombre abajo. **Sin
  botón hacia otra vista**: cada franja es un overview completo por sí misma y no enlaza con la
  rejilla de pantalla completa (así no depende de que esté compilada ni activada).
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
- **Controles y textos** (flechas, número y nombre) crecen con el grosor.

### Vista previa sin compositor

`just preview` (`cargo test ui_preview -- --ignored`) renderiza la **UI real** a PNG en
`target/ui-preview/` usando la GPU y `egui_kittest` (dependencia solo de desarrollo), con datos
de ejemplo y la config por defecto: `overview-workspaces`, `…-search`, `overview-activities`
(y `…-search`, de Actividades), `strip-h-top`, `strip-h-bottom`, `strip-v-left`, `strip-v-right`, y las versiones
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

