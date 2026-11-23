# Exploración opcional: fakers de otros WCs

**Perspectiva de exploración, no un plan ni un roadmap.** El proyecto se usa con
Hyprland y no hay necesidad actual de otros WCs. Esto es una idea que podría seguirse
si algún día interesa; no está comprometida ni priorizada.

La idea: simular, dentro del banco de pruebas visual, cómo se vería la UI ante las
convenciones de datos/eventos de otros WCs (`HyprlandFake`, `RiverFake`, `WestonFake`,
manifiestos de escenario y recetas de compilación), mediante **fakers de comportamiento**.

**No es soporte.** Emular el comportamiento observable (datos y eventos) no implica
soportar el WC real. El soporte real exigiría diseñar, entre otras cosas, un IPC por
cada WC; eso se describe aparte más abajo y tampoco está comprometido.

**Estado: idea, no implementada y no priorizada.** Se mantiene **fuera** de
[`PREVIEWER.md`](PREVIEWER.md): las fases de ese documento terminan en el previewer de
una ventana. Esto es una extensión del *host* de datos, no de la UI.

## Por qué es posible (la semilla ya está)

- `ui::draw(ui, state, palette, t, panel_t, pill_w)` es puro: no sabe de Wayland ni de
  qué compositor hay debajo.
- `UiState` es solo datos: se puede copiar, guardar, clonar o reconstruir para cualquier
  escenario.
- Los componentes no ejecutan nada: encolan acciones en `pending_commands` /
  `pending_spawns`. Hoy se registran; mañana se pueden reenviar, descartar o comparar.
- `UpdateCtx { dt, screen_w, screen_h, cursor }` ya es la frontera entre «el mundo real»
  y «los componentes». El previewer actual la rellena a mano; un `FakeHost` la produciría.

## Piezas a añadir

### 1. `FakeHost` (trait)

Una interfaz que exponga lo que el shell hoy obtiene de módulos concretos:

| Método | Hoy viene de | En el fake |
|---|---|---|
| `snapshot()` / `clients()` | `ipc::HyprlandIpc` | datos del perfil |
| `cursor()` | IPC `cursorpos` | posición inyectada (o rastrillo automático) |
| `system()` / `battery()` | `sysinfo` + `power::read()` | valores del manifiesto |
| `notifications()` | servidor DBus | cola local (`push_preview` ya existe) |
| `pulse events` | socket de eventos | eventos disparados a mano o por script |
| `sink` de acciones | `ipc::dispatch` + `spawn_detached` | registro, diff o puente opcional |

El previewer actual ya es un `FakeHost` implícito repartido entre `sample_state`,
`sample_system` y `drain_actions`. Formalizarlo es el primer paso.

### 2. Fakers de comportamiento (no soporte)

| Faker | Qué emula | Matiz |
|---|---|---|
| `HyprlandFake` | forma de los datos `j/activeworkspace`, `j/activewindow` y eventos | es el comportamiento actual de `sample_state` |
| `RiverFake` | workspaces/tags con otras convenciones, foco distinto | no cambia la UI, solo la forma de los datos |
| `WestonFake` | ausencia de layer-shell y eventos genéricos | emula el *escenario*, no el protocolo: Weston no implementa `zwlr_layer_shell_v1` |
| (idea, sin compromiso) `SwayFake`, `NiriFake` | variaciones de datos/eventos | mismo patrón, solo si se decide explorar |

Los fakers producen datos con la **forma** que tendrían en otro WC para probar la UI.
No hablan su IPC, no negocian protocolos y no hacen que el shell funcione allí.

**Sabores de UI (esbozo)**: el modo GNOME (`flavor_gnome`) sería topbar + overview,
**sin dock y sin notch/isla**: el topbar sustituye a la barra y el dock es intrusivo
en esa metáfora. La combinación temporal `topbar + dock` que se compiló es solo un
puente; el notch debe quedar fuera de ese modo.

### 2b. Soporte real (fuera de este plan)

Soportar otro WC de verdad sería otro proyecto: habría que diseñar, entre otras cosas,
un **IPC por WC** y una capa de eventos en el shell:

| WC | Qué habría que diseñar |
|---|---|
| Hyprland | socket `j/*`, `dispatch`, eventos (ya existe en `src/ipc.rs`) |
| Sway / i3 | swayipc (`get_workspaces`, `get_tree`, eventos `workspace::`) |
| River | protocolo/ctl propio, convenciones de tags y foco |
| Niri | su socket y modelo de workspaces |
| Weston | sin `zwlr_layer_shell_v1`: haría falta otro tipo de surface o renunciar al layer |

Eso implicaría una abstracción tipo `WcHost` **en el shell real**, adaptadores por WC y
decisiones de producto (qué se pierde sin layer-shell). No está comprometido aquí: este
documento solo cubre los fakers del banco de pruebas.

### 3. Manifiesto de escenario (TOML)

El «formulario»: config, paleta/tema, perfil de WC, estado inicial (isla, barra, panel,
overview), datos del sistema, cola de notificaciones y resolución.

```toml
scenario = "tema-claro-con-muchas-apps"
host = "HyprlandFake"          # o RiverFake / WestonFake
screen = [1366, 768]

[config]
theme = "bathory"
bar_h = 28
workspaces_count = 6

[state]
component = "overview"
query = "term"

[system]
battery_percent = 12
notification_count = 5
```

Se carga con `just previewer --scenario scenarios/foo.toml`. Cambiar de escenario no
recompila nada: es solo datos.

### 4. Compilador de recetas

A partir del manifiesto, emitir de forma reproducible:

- los features exactos (`--no-default-features --features bar,topbar,previewer`),
- los env vars `DINAMIC_UI_*`,
- el comando para regenerar PNG o para aplicar el escenario al shell real,
- y una línea base (`baseline.png`) contra la que hacer diff.

O sea: `scenario.toml` → `recipe.sh` → `baseline.png`.

## Si se decide explorar (fases hipotéticas, sin fecha ni compromiso)

- [ ] **A — Extraer `FakeHost`**: mover `sample_state` / `sample_system` /
      `drain_actions` detrás del trait, sin cambio de comportamiento del previewer.
- [ ] **B — Fakers `HyprlandFake` y `RiverFake`**: mismos componentes, datos distintos.
- [ ] **C — Manifiesto `--scenario`**: cargar TOML, poblar `UiState` y paleta.
- [ ] **D — Receta y línea base**: exportar comandos/env/PNG; diff automatizado.
- [ ] **E — Más fakers (solo si se decide)**: `WestonFake`, y eventualmente `SwayFake` /
      `NiriFake`. Ninguna fase implica soporte real del WC.

## Fuera de alcance

- **Soporte real de otros WCs**: exige IPCs/adaptadores por WC y se trata arriba en
  «2b. Soporte real»; no es una promesa de este plan.
- Soporte real de `zwlr_layer_shell_v1` en compositores que no lo implementan.
- Ejecutar acciones en el WC real desde el previewer. El puente al socket de control
  sería **opt-in** y rompería el aislamiento documentado en `PREVIEWER.md`.
- Escribir `config.toml` del usuario: los manifiestos son archivos propios, no la config real.
