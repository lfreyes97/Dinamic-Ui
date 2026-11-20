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
