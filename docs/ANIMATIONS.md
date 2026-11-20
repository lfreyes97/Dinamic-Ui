# Animaciones: diseño y referencias

Todo lo que se mueve está implementado en este repo, no en una librería:

| Qué | Dónde |
|---|---|
| Progreso, `approach`, `smootherstep`, pulso | `src/main.rs` |
| Crossfade, `smoothstep`, panel con opacidad | `src/ui.rs` |
| Métricas (`bar_h`, `panel_h`, anchos) | `src/config.rs` → `ui::metrics()` |
| Superficie wgpu / repintado | `src/gfx.rs` |
| Event loop calloop / timer / fuente de control | `src/main.rs` |

Los tamaños viven en `config.toml` y se aplican con `dinamic-ui --reload`; durante la
animación no cambian.

## 1. Curva de movimiento

`smootherstep(t) = t³(t(6t−15)+10)` (Ken Perlin). Velocidad cero en ambos extremos,
así el morph no arranca ni frena de golpe. Es matemática estándar:
<https://en.wikipedia.org/wiki/Smoothstep>.

## 2. Crossfade isla ↔ barra

Se dibujan **las dos** filas sobre el mismo rectángulo (`UiBuilder::max_rect`) con
opacidades complementarias calculadas con `smoothstep`:

- isla: `1 − smoothstep(0.15, 0.75, t)`
- barra: `smoothstep(0.25, 0.85, t)`

La primitiva es `egui::Ui::set_opacity` (docs.rs/egui/0.37.0/egui/struct.Ui.html).
La interacción de la isla se desactiva cuando su opacidad baja de 0.9 para que no
bloquee los botones de la barra.

## 3. Panel

`panel_progress` es independiente de `expansion`, por eso se cierra animado. La altura
del surface y la opacidad del contenido usan el mismo valor, suavizado con `smootherstep`.

## 4. Repintado bajo demanda

`FullOutput::viewport_output[ROOT].repaint_delay` (docs.rs/egui-wgpu/0.37.0) indica si
egui pide otro frame. `main.rs` solo presentationa cuando hay cambio, animación,
entrada de ratón, evento de Hyprland o cada 15 ticks (reloj/IPC). En los frames
vacíos se salta también `gfx.draw()`, es decir, no se ejecuta `run_ui` ni el
tesselado de egui.

### Event loop con calloop

Ya no se piden frame callbacks de Wayland (antes 60 despertars/s en reposo). Un único
`calloop::EventLoop` agrupa tres fuentes:

| Fuente | Para qué |
|---|---|
| `WaylandSource` | Entrada (ratón/teclado/configure de layer shell) |
| `calloop::Timer` | Tick de UI: 16 ms con animación/repaint, 250 ms en reposo |
| `Generic<UnixListener>` | Socket de control de los keybinds (`--menu`, `--overview`, …) |

El socket de control es **level-triggered**: en cuanto Hyprland escribe un comando,
el callback lo aceita, lo encola en `pending_control` y marca `tick = true`, así que
el keybind se atiende en el mismo ciclo, sin esperar al timer de 250 ms. El ratón y
el teclado también marcan `tick = true` en sus handlers para que dock e isla
reaccionen de inmediato.

**Mediciones (release, reposo):** ~0,25 % de un core, 130 MB RSS, ~31 wakeups/s.
Antes del salto de `run_ui`: 20,7 % (debug) / 2,9 % (release). Con frame callbacks
y sin calloop: 5 % (debug) / 1 % (release).

## 5. Superficie fija (clave para que la animación sea nuestra)

Esto **no está documentado en ningún crate**: es comportamiento del compositor.

El protocolo `zwlr_layer_shell_unstable_v1` dice que con un solo ancla (`top`) el punto
de anclaje se centra en ese borde, y que `set_margin` solo afecta a bordes anclados:

> "If two orthogonal edges are specified ... otherwise the anchor point will be
> centered on that edge" — `set_anchor`
> "Setting this value for edges you are not anchored to has no effect" — `set_margin`

Pero **no define qué pasa con la posición al cambiar el tamaño**. En Hyprland, con
solo `top`, al crecer recentra pero al encoger deja fijo el borde izquierdo. Y aunque
se compensara con márgen, llamar a `set_size`/`set_margin` cada frame genera un
`configure` por cambio y se producía un desfase de un frame entre tamaño y posición.

Solución definitiva: **el layer surface nunca cambia de geometría**. Se ancla
`TOP | LEFT | RIGHT` con `set_size(0, surface_h)` (de la config) y ocupa todo el ancho siempre.
La píldora se dibuja centrada dentro con egui y el `input_region` se actualiza para
que solo el área visible reciba clics (`wl_compositor.create_region` + `set_input_region`).
Cero `set_size` durante la animación, cero `configure`, cero jitter.

Además, en la config de Hyprland se añadió `no_anim` para el namespace `dinamic-ui`,
para que el compositor no interpole encima de nuestros propios valores.

## Referencias de API usadas

- egui 0.37 — `Context::run_ui`, `Ui::set_opacity`, `UiBuilder`, `FullOutput`, `text::LayoutJob`
- egui-pop-phosphor 0.15 — `add_to_fonts` (Regular) + `add_font_bytes_as_family` (Bold)
- egui-wgpu 0.37 — `ScreenDescriptor`, `Renderer::render`
- smithay-client-toolkit 0.21 — `LayerSurface::{set_anchor,set_margin,set_size}`, `OutputInfo`
- wlr-layer-shell — `wlr-layer-shell-unstable-v1.xml` (en `wayland-protocols-wlr`)
