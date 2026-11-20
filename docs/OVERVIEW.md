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

### Pendiente

- [ ] **Cerrar una ventana desde el overview** (clic central o botón): hay que traducir
      `closewindow` a la API Lua de Hyprland en `ipc.rs::dispatch` y verificarlo; no se añadió sin
      poder probarlo.
- [ ] **Varios monitores**: `ClientInfo` no trae el monitor, así que las ventanas de otro monitor
      se escalan con el tamaño del actual.
- [ ] Arrastrar una ventana a otro workspace.
- [ ] Miniaturas en vivo (Fase 3 de arriba) y hot corner.
- [ ] Verificación visual en el R7 (el fundido, el deslizamiento de las franjas y el atenuado de
      la búsqueda no se han visto en pantalla).

