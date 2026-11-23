# Previsualizador interactivo (widget factory de Dinamic UI)

Ventana normal para ver y manejar todos los componentes compilados sin Hyprland:
comandos, datos de ejemplo, hover, clics, animaciones y teclado reales (feature
`previewer`, binario `previewer`, receta `just previewer`). El tema y los tamaños en
vivo funcionan desde la Fase 2.
Código: `src/bin/previewer.rs` sobre la lib del crate.

**Estado: Fases 0, 1 y 2 implementadas y verificadas en el R3** (23 nov 2026). Las vistas
previas PNG (`just preview`, `ui/preview.rs`) siguen existiendo para regresiones
visuales; esto las complementa, no las sustituye: cubre justo lo que el PNG no puede
(cursor, fundidos, teclado).

> **Perspectiva opcional (no roadmap)**: los *fakers de comportamiento* de otros WCs
> (`HyprlandFake`, `RiverFake`, `WestonFake`), los manifiestos de escenario y las recetas
> de compilación son una idea de exploración que podría seguirse, descrita en
> [`SUPPORT_OTHER_WCS.md`](SUPPORT_OTHER_WCS.md). El proyecto se usa con Hyprland; no es
> soporte real de esos WCs (exigiría IPCs propios), no está priorizada y no mezcla con
> las fases de este documento.

## Por qué es posible (cimientos ya existentes)

- **`ui::draw(ui, state, palette, t, panel_t, pill_w)` es puro**: recibe estado, paleta
  y progresos como parámetros; no toca Wayland. Los tests `ui_preview` ya lo renderizan
  sin compositor con `egui_kittest`.
- **Registro de componentes** (`ui/component.rs`): `build_components()` (z-order, por
  feature), `dispatch_command`, `dispatch_key`, `update_components`, `wants_keyboard`.
  `UpdateCtx` ya lleva `cursor: Option<(i32, i32)>`: el hover del notch, el autohide del
  dock y el de la taskbar funcionan con solo alimentar esa posición.
- **Los componentes no ejecutan nada**: encolan acciones en `shared.pending_commands`
  (dispatches de Hyprland) y `shared.pending_spawns` (apps, `awww`); solo `main.rs` las
  ejecuta. El previewer las drena a un **registro de acciones** y nunca las ejecuta.
- **Datos de ejemplo**: `sample_state()`, `apps()`, `sample_system()` y
  `fake_desktop()` en `ui/sample.rs` (compartidos por las previews PNG y el
  previewer). El shell ya tiene además un fallback mínimo sin Hyprland
  (`demo_snapshot()` en `ipc.rs`); los datos ricos del previewer son los de `sample_state`.
- **Navegación lateral ya portada y con tests**: el panel de configuración
  (`ui/config_panel.rs`, maqueta de solo lectura) tiene la geometría de ventana
  (`frame_rects`), las entradas de navegación por grupos (`nav_entries`, pura y con
  tests), el dibujo + selección (`sidebar`), el cromo (`window_chrome`) y las filas de
  control (`Control`). Es la referencia de patrón; el MVP usa `egui::Panel` con widgets
  nativos porque es cromo de herramienta, no la UI bajo prueba.
- **eframe no se usa en el shell** porque winit no soporta `wlr-layer-shell`. Para el
  previewer no hace falta layer-shell: una ventana normal basta. eframe 0.37 comparte
  egui 0.37 y wgpu 30 con el shell.

> Las maquetas HTML de `docs/` son referencia histórica: su diseño ya está portado a
> egui en su mayor parte (`config_panel.rs`, `dashboard.rs`, …). La fuente para copiar
> es el código Rust, no el HTML.

## Diseño

Ventana con dos zonas:

| Zona | Contenido |
|---|---|
| Sidebar izquierda | Comandos de los componentes compilados, tema/tamaños en vivo, datos falsos y registro de acciones |
| Centro | Escritorio falso (`fake_desktop`) + `ui::draw` real, al tamaño de pantalla simulado |

- **Mismo camino que el CLI**: los botones llaman `dispatch_command("expand" | "panel" |
  "overview" | "activities" | "dashboard" | "menu" | "topbar" | "taskbar" | …)`, idéntico
  al socket de control. Las entradas están gates por las mismas features que
  `build_components()`: **solo se ofrece lo compilado**, como el panel de configuración.
- **Animaciones reales**: `update_components(state, UpdateCtx { dt: stable_dt, … })` por
  frame; repintado bajo demanda cuando hay cambios (como el shell). Los parámetros
  `t / panel_t / pill_w` se calculan con un helper compartido con `main.rs`
  (geometría de una sola fuente).
- **Cursor real**: la posición del puntero de egui, mapeada a coordenadas de la pantalla
  simulada, alimenta `UpdateCtx.cursor` (solo `Some` cuando está dentro). Así se pueden
  probar el notch (franja superior centrada), el dock (24 px inferiores) y el autohide
  de la taskbar, que el PNG no cubre.
- **Teclado (básico en Fase 1)**: mapa `egui::Key`/`Event::Text` → `Keysym` y
  `dispatch_key`. El foco exclusivo de los modales (`wants_keyboard`) y el mapa completo
  quedan para la Fase 3.
- **Config en vivo (Fase 2)**: watcher del mtime de `config.toml` (poll por frame,
  sin dependencias nuevas) → `config::reload()` + `Palette::load()` + `state.shared` +
  repaint. El sidebar tiene selector de esquema/acento y sliders de tamaños, workspaces,
  taskbar, topbar y franjas que escriben con `config::set(Config)` (**solo en memoria**).
  Como `metrics()` y `Palette::load()` leen la config global, el efecto es inmediato:
  tema, `bar_h` (24–48), `panel_h` (80–480), `island_w`, `bar_w`, `workspaces.count`,
  lado/autohide de taskbar, lado de franjas y pestaña inicial del panel.
- **Datos falsos honestos**: `sample_state` + `sample_system` fijan batería, wifi,
  audio y brillo; sin dato se muestra «Sin datos», nunca un valor inventado. Ya está
  el botón «inyectar notificación» (sin DBus) para ver la tarjeta, la pestaña Alerts y
  la lista del calendario. Los interruptores de datos falsos (batería/wifi por escenario)
  no son configuración y quedan para el backlog de escenarios.
- **Registro de acciones**: cada `pending_commands`/`pending_spawns` drenado se muestra
  («esto es lo que haría la instancia real»). `should_exit` y `reload_requested` se
  registran y se resetean: el previewer no termina ni recarga el archivo real.
- **Pantalla simulada**: por defecto el tamaño del centro de la ventana (resize en
  vivo, como un monitor más); presets 1280×720, 1366×768 (R3) y 1920×1080 para
  comparar. Escala «ajustar a la ventana» con transformación de capa de egui 0.37
  (verificar en Fase 4; si no alcanza, scroll y presets que quepan).

## Reutilización directa (el MVP fue conectar estas piezas)

| Pieza existente | Dónde | Uso en el previewer |
|---|---|---|
| Geometría de ventana con sidebar | `config_panel::frame_rects`, `nav_entries`, `sidebar` | Referencia de patrón (grupos + entradas por feature). El MVP usa `egui::Panel` con widgets nativos: es cromo de herramienta, no la UI bajo prueba |
| Datos de ejemplo ricos | `sample::sample_state`, `sample::apps`, `sample::sample_system` | Tal cual, extendidos con interruptores de sistema |
| Escritorio falso | `sample::fake_desktop` | Tal cual (exento de `lint-colors`) |
| Dibujo real de todos los componentes | `ui::draw` + `component::*` | Tal cual: comandos, update, teclas |
| Fallback sin Hyprland | `ipc::demo_snapshot` | No se usa en el previewer (datos más ricos arriba); queda como está |

Lo que faltaba no era UI sino **conexión**: el harness de preview vivía en tests
ignorados, la navegación dentro del panel de configuración y el registro de componentes
solo lo manejaba el shell. El MVP (Fase 1) los unió en una ventana viva.

## Garantías de aislamiento (no toca la instancia real)

El aviso de `MODULARIZATION_PLAN.md` («una segunda instancia sustituiría el socket de
control») no aplica: el previewer es otro binario y además:

1. **No liga el socket de control** (`control.rs` no se arranca): `dinamic-ui --toggle`
   sigue hablando con la instancia real.
2. **No reclama `org.freedesktop.Notifications`**: `Notifications::new()` ya evita el
   hilo servidor bajo `cfg!(test)`; se extiende la guarda al modo preview (constructor
   sin servidor). Verificable con `busctl --user list`.
3. **No ejecuta spawns ni dispatches**: solo los registra (ni apps, ni `awww`, ni
   `hyprctl`).
4. **No arranca `SystemMonitor` ni lee sysfs**: batería y sistema son datos de ejemplo.
5. **No escribe `config.toml`**: vigila su mtime y lo relee, pero nunca lo modifica;
   los sliders de la Fase 2 viven solo en memoria.

## Cambios realizados (Fase 0, sin cambio de comportamiento)

- **Target lib**: `src/lib.rs` con los módulos que antes declaraba `main.rs`; `main.rs`
  queda como binario fino que usa la lib. `lerp`, `smootherstep` y `approach` pasaron a
  la lib. Las rutas `crate::` de los módulos no cambiaron.
- **APIs públicas mínimas**: `gfx::install_fonts` (antes `pub(crate)`), `config::set`,
  constructor de `UiState`/`NotificationState` en modo preview (sin hilo DBus),
  helper compartido para `t / panel_t / pill_w`, y un `push` de notificación de
  ejemplo en la cola.
- **Feature `previewer = ["dep:eframe"]`** y `[[bin]] previewer` con
  `required-features = ["previewer"]`. eframe 0.37 con `default-features = false`,
  features `wgpu`, `wayland`, `x11`. El build normal y la matriz actual no pagan nada.
- **justfile**: receta `previewer` (flags del perfil rápido + `--features previewer
  --bin previewer`); `previewer` añadido a `check-matrix`/`lint-matrix` (sola y con
  `bar,previewer` para ver el sidebar mínimo).
- **Coste en el R3 (medido)**: eframe+winit frío 1 min 05 s (una vez); rebuild del shell
  tras la lib 24 s (una vez); iteración en caliente ~1,5–2 s. El previewer comparte la
  caché de dependencias del perfil rápido.

## Fases

- [x] **Fase 0 — lib y APIs**: refactor a lib, setters y constructores preview, helper
      de geometría. Sin cambio de comportamiento del shell.
      Verificación: `just ci`, `just check-matrix`, `just lint-matrix`.
- [x] **Fase 1 — MVP**: ventana eframe; sidebar con los comandos de los componentes
      compilados (mismo `dispatch_command` que el CLI); centro con `fake_desktop` +
      `draw`; bucle `update_components` con cursor real; registro de acciones;
      notificación de ejemplo inyectable; teclado básico (Esc, flechas, Enter, Tab,
      Backspace, texto). Verificación: humo en el R3 + `just ci` + 43 PNG intactos.
- [x] **Fase 2 — config en vivo**:
      - **2a**: watcher del mtime de `config.toml` → `config::reload()` +
        `Palette::load()` + refresco de `state.shared` + repaint. Sin dependencias nuevas.
        Los `reload_requested` de los componentes también recargan desde disco.
      - **2b**: selector de esquema/acento y sliders en el sidebar con `config::set`
        (solo memoria): tamaños, `workspaces.count`, taskbar, topbar y franjas.
      Verificación en el R3: editar `config.toml` con el previewer abierto cambió el
      esquema y `bar_h` sin reiniciar; restaurar el archivo lo recargó de vuelta. Un
      cambio desde el sidebar cambió la UI mientras el archivo seguía intacto.
- [ ] **Fase 3 — teclado completo**: mapa Key/Text → Keysym, foco exclusivo de los
      modales (`wants_keyboard`), búsqueda en overview/actividades/menú, `Esc` capa a capa.
- [ ] **Fase 4 — opcional**: presets de pantalla con escala y exportar PNG desde la
      ventana (reutilizando `egui_kittest`). La comparación de dos temas lado a lado se
      evalúa junto con [`SUPPORT_OTHER_WCS.md`](SUPPORT_OTHER_WCS.md) para no duplicar
      el mecanismo de escenarios.

## Después de la Fase 2

El siguiente hito **no es del previewer sino del shell**: cubrir lo que hacía quickshell
y quedó descubierto tras el cutover del 23 nov 2026 (OSD de volumen/brillo, capturas,
Alt+Tab, quick settings, tema vía gsettings…). Está inventariado y priorizado en
[`POST_QUICKSHELL.md`](POST_QUICKSHELL.md). La Fase 2 ya es su banco de pruebas: con
config en vivo se pueden afinar esos reemplazos sin recompilar ni reiniciar el shell.

## Verificación (lo que el PNG no cubre)

- [x] `just ci` y matrices con `previewer` (sola y `bar,previewer`). `check-matrix` y
      `lint-matrix` pasan; con `--no-default-features --features previewer` no hay
      componentes y el centro queda vacío sin panic.
- [ ] Manual en el R3 (parcial): hover del notch, autohide de dock y taskbar,
      desplegables del topbar que cierran al pulsar fuera, fundidos de
      overview/actividades, `Esc`, búsqueda con texto, pestañas del panel, morph isla ↔ barra.
- [x] Convivencia: con quickshell dueño de `org.freedesktop.Notifications`, el
      previewer no lo reclama ni liga el socket de control (verificado con
      `busctl --user list` y `$XDG_RUNTIME_DIR`); el registro drena dispatches y
      spawns sin ejecutarlos.
- [x] `lint-colors`: el sidebar usa los visuales por defecto de egui o tokens de
      `theme.rs`, sin literales de color; `fake_desktop` vive en `ui/sample.rs`
      (exento, como `theme.rs`).

## Notas de implementación (R3, 23 nov 2026)

- **API de egui/eframe 0.37**: `SidePanel` ya no existe (`egui::Panel::left`), y
  `eframe::App` pide `fn ui(&mut self, &mut Ui, &mut Frame)`. Los `Ui` hijos heredan
  el estilo del padre, y `ui::draw` deja el ctx con visuales transparentes (el shell
  es un layer): el previewer resetea el root a `Visuals::dark()` cada frame. Así el
  sidebar tiene fondo opaco y el centro queda en las mismas condiciones que kittest.
- **Fase 2**: `config_mtime` guarda el `modified()` del archivo; si cambia,
  `reload_from_disk()` + `sync_config()` actualizan la `Arc<Config>`, la paleta y los
  flags de taskbar/topbar. Los controles del sidebar clonan la config, llaman a
  `config::set` y vuelven a `sync_config()`: el archivo nunca se escribe. Un `reload`
  pedido por un componente sigue el mismo camino.
- **`just preview` en serie** (`--test-threads=1`): varios harness wgpu en paralelo
  segfaultean en la GPU híbrida del R3. En serie: 6 tests, 43 PNG, ~52–82 s.
- **Costes medidos**: eframe frío 1 min 05 s (una vez, feature-gated); rebuild del
  shell tras el refactor a lib 24 s (una vez); iteración en caliente ~2 s.
- **A/B visual contra `HEAD`**: las 18 imágenes que difieren son las mismas que
  difieren `HEAD vs HEAD` (reloj + hilo de iconos). Regeneradas en la misma franja
  horaria, las diferencias bajan a ~5 px (taskbar/topbar) y ~35 px (dashboard): no hay
  regresión visual.
- El sidebar del MVP es cromo de herramienta con los visuales por defecto de egui;
  la UI bajo prueba (el centro) usa los tokens del tema como siempre.
- **Sandbox de diseño** (temporal, solo en el previewer): el sidebar tiene un bloque
  «Modo diseño» con 5 posiciones candidatas para los controles de modo (derecha,
  izquierda, reloj, notch y hueco reloj↔sistema). Se dibujan sobre la barra/topbar
  real con el tema actual, sin enviar comandos. El sidebar se oculta con `F1` (o el
  botón «Ocultar sidebar» / el `☰` flotante); `DINAMIC_UI_HIDE_SIDEBAR=1` arranca
  oculto y `DINAMIC_UI_DESIGN_PLACEMENT=<nombre>` fija la variante para capturas.

## Fuera de alcance

- Acciones reales del sistema (wifi, volumen, lanzar apps): el previewer registra, no
  ejecuta. Lo mismo que el resumen de solo lectura del topbar.
- Múltiples monitores y layer-shell: no los necesita una ventana normal.
- Escribir `config.toml` desde los sliders (se puede añadir como export manual después).
- Fakers de otros WCs, escenarios y recetas: idea de exploración opcional en
  [`SUPPORT_OTHER_WCS.md`](SUPPORT_OTHER_WCS.md); no son soporte real, no están
  priorizados y no entran en estas fases.
- Sustituir a `just preview`: los PNG siguen siendo la regresión visual automatizable.
