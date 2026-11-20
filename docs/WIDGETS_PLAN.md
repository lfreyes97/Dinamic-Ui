# Plan — Widgets, panel desacoplado, dashboard y bento editable

Continúa `docs/MODULARIZATION_PLAN.md` (Fases 0–3 hechas). Reúne tres ideas que son una sola:

1. Desacoplar las funciones del panel (calendario/reloj, workspaces, sistema, wallpaper…).
2. Usar el sistema del panel como base de un **dashboard** y de un **system-conf**.
3. Hacerlo **editable, estilo bento**.

## Idea central: el widget como unidad

Hoy el panel es una tarjeta con 5 pestañas, cada una una función que dibuja una lista
vertical de ancho fijo. Si cada pestaña pasa a ser un **widget** que sabe dibujarse en un rect
cualquiera, el resto sale solo:

| Host | Qué hace con los widgets |
|---|---|
| **Panel** (hoy) | una pestaña = un widget a ancho completo |
| **Dashboard** | una rejilla de widgets a pantalla completa (la variante B del mockup: 4×3, tiles de 2×2, 2×1, 1×1) |
| **Dashboard editable** | la misma rejilla, con la posición y el tamaño de cada widget guardados como datos |
| **config-panel** (system-conf) | páginas de ajustes; reutiliza el mismo contenedor y los mismos controles |

Así, "desacoplar el panel", "dashboard" y "bento editable" no son tres trabajos: son el
mismo cambio con tres anfitriones.

## Decisiones tomadas

- **Sistema es núcleo.** Hora, batería, wifi, audio, pantalla y layout son un servicio de
  `Shared` que consumen notch, topbar, taskbar, dashboard y panel. La página/widget *Sistema*
  es permanente (siempre compilada); solo pinta esos datos.
- **Widgets opcionales por feature:** workspaces, calendario/reloj, wallpaper (ya existe),
  alertas (ya existe). Nombres de feature por decidir (`widget-workspaces`, `widget-clock`…).
- **Los datos se piden por demanda y dependen del estado.** `needs(&self, state) -> Needs` se
  evalúa cada frame (no es fijo por componente):

  | Fase de la barra | Datos que necesita |
  |---|---|
  | Notch en reposo | nada |
  | Compact / píldora | hora |
  | Bar (topbar) | hora, batería, wifi, audio |
  | Panel / dashboard | lo que declaren sus widgets visibles |

  El servicio consulta la unión de lo declarado. Respeta `bar.max_state` (no pide lo que
  nunca se mostrará) y mantiene los datos vivos unos segundos tras dejar de pedirlos
  (histéresis, para que el notch no arranque y pare el servicio al entrar y salir el cursor).
  Sustituye a `set_active(show_panel || expanded)` de `main.rs`.
- **El layout es dato, no código.** Vive en un archivo aparte (`layout.toml`), no en
  `config.toml`, para no reescribir un archivo que el usuario edita a mano y comenta.

## Fases

### Fase 6.0 — Datos por demanda

- [ ] `Needs` (conjunto de banderas: `TIME`, `BATTERY`, `NETWORK`, `AUDIO`, `DISPLAY`, `LAYOUT`).
- [ ] `Component::needs(&self, state) -> Needs` y `Shared`/`main.rs` toman la unión.
- [ ] `sysinfo` y `power` consultan solo lo pedido, con histéresis.
- [ ] `bar` mapea `BarPhase` → `Needs`, acotado por `bar.max_state`.
- [ ] Tests: unión, histéresis, `max_state`.

### Fase 6.1 — Widget y contenedor del panel

- [ ] Trait `Widget`: `id()` estable, `title()`, tamaño mínimo/preferido en celdas, `needs()`,
      `update()` y `draw(ui, rect, state, palette)`.
- [ ] Registro `build_widgets()` con `#[cfg]`, igual que `build_components()`.
- [ ] `ui/panel.rs` → `ui/panel/` (contenedor: cabecera, pestañas, tarjeta) y una página por
      archivo en `ui/widgets/`: `system` (permanente), `workspaces`, `clock`, y los ya
      separados `wallpaper` y `alerts`.
- [ ] **Dibujo adaptable.** Hoy cada página asume ~356 px de ancho y hace scroll vertical; el
      widget debe dibujarse en el rect que reciba. Es el trabajo grande de esta fase.
- [ ] Mover el estado propio de cada widget fuera de `BarState` (p. ej. `layout_preset` →
      widget de workspaces) y la sincronización de `BarShell::update` a su widget.
- [ ] El panel sigue viéndose igual (una pestaña = un widget a ancho completo).

### Fase 6.2 — Dashboard (host de rejilla)

- [ ] Feature `dashboard`; comando `--dashboard`/`just dashboard`; rect de entrada a pantalla
      completa; `Esc` lo cierra.
- [ ] Rejilla N×M con huecos y spans; **preset estático** copiado del mockup (variante B).
- [ ] Widgets nuevos que pide el mockup y no existen aún: procesos, reproduciendo (MPRIS),
      clima, accesos rápidos. Decidir cuáles entran en la primera versión.

### Fase 6.3 — Layout como datos

- [ ] `layout.toml` versionado: `[[dashboard.widget]] id, col, row, w, h`.
- [ ] Un widget cuyo `id` no esté compilado se **omite** (con aviso en el log), así un mismo
      layout sirve en builds distintos.
- [ ] Presets incluidos: las variantes A–D del mockup.
- [ ] Validación: solapes, fuera de rejilla, tamaños por debajo del mínimo del widget.

### Fase 6.4 — Edición bento

- [ ] Modo edición: arrastrar y redimensionar con ajuste a la rejilla (egui permite esto
      directamente con `ui.interact` sobre cada rect).
- [ ] Resolución de colisiones (rechazar o empujar; decidir).
- [ ] Añadir/quitar widgets desde una paleta de los compilados.
- [ ] Guardar, cancelar y restaurar el preset.

### Fase 6.5 — config-panel (system-conf)

- [ ] Feature `config-panel`: navegación lateral como en el mockup (Tema, Barras, Overviews,
      Dashboard, Workspaces, Notificaciones, Dock, Atajos, `config.toml`).
- [ ] Mostrar solo lo compilado (cierra el pendiente de "el panel de ajustes oculta lo no
      compilado" de la Fase 3).
- [ ] Escritura de configuración conservando comentarios (`toml_edit`).
- [ ] El engranaje del panel lleva aquí (hoy lleva a Alertas por falta de vista de ajustes).

## Riesgos y preguntas abiertas

| Riesgo / duda | Nota |
|---|---|
| Hacer los widgets adaptables es trabajo grande (≈1100 líneas de `panel.rs`) | Hacerlo página a página, con el panel igual de funcional en cada commit |
| Edición con ratón sobre un layer-shell | El `input_region` ya cubre pantalla completa en overview/menú; el dashboard sigue el mismo patrón |
| ¿Dashboard como overlay (como overview) o como región fija? | El mockup apunta a overlay; confirmar |
| ¿Cuántas variantes de layout? | El mockup tiene A–D; ¿son presets o alternativas de diseño que se descartan? |
| Escribir config sin perder comentarios | `toml_edit`, o limitarse a `layout.toml` al principio |
| Widgets que no existen (procesos, MPRIS, clima) | Cada uno añade datos y posiblemente dependencias; decidir alcance de v1 |
| Prueba manual | Todo esto es visual: se verifica en el R7 |
