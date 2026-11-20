# Plan de acción — Componentes desacoplados en compilación

Objetivo: que un usuario pueda compilar solo lo que quiere (p. ej. `dock + bar`,
o `taskbar + overview`) mediante features de Cargo, sin pagar RAM, dependencias ni
hilos de lo demás.

## Decisiones tomadas

- **`bar` es un solo componente con estados de expansión**: notch → compacto → píldora →
  barra full width (la "topbar") → panel. Comparten surface, animación y geometría. El
  máximo al que sube es config en runtime (`bar.max_state`), no compilación.
- **Un único layer surface a pantalla completa** (como hoy: `main.rs:134-136`, anclado a
  los 4 lados con `set_size(0, 0)`). No se reserva altura; la exclusión de clics la hace
  el `input_region`. Un surface por componente exigiría un `Gfx` por `wl_surface`
  (`gfx.rs` está atado a uno): descartado por caro.
- **Features de Cargo** (componentes independientes o con dependencias pesadas):

  | Feature | Implica | Notas |
  |---|---|---|
  | `bar` | — | notch/isla/píldora/barra/panel |
  | `dock` | `icons` | modelo + vista autohide |
  | `taskbar` | `dock` | otra vista sobre el modelo del dock |
  | `overview` | `icons` | consume clientes del núcleo, no del dock |
  | `command-menu` | `icons` | `launcher.rs` |
  | `notifications` | `dep:zbus` | servidor DBus en hilo propio |
  | `wallpaper` | `dep:image` | pestaña + `thumbs.rs` |
  | `svg-icons` (opcional, futura) | `icons`, `dep:resvg` (sin texto) | solo si la medición de fallbacks lo justifica |
  | `icons` (interna) | `dep:freedesktop-icons`, `dep:png` | `appicons.rs`; la activan sus consumidores. Solo PNG, sin SVG ni `magick` |

  `dashboard` y `config-panel` (mockups) son **futuros**: se añaden como feature cuando se
  implementen, no entran en las fases actuales.
- **"Compilado" ≠ "activado".** Compilado lo decide Cargo; activado es `enabled = true`
  en config. El panel de ajustes solo ofrece interruptores de lo compilado, y `control.rs`
  responde "no compilado" para el resto.
- **Iconos: solo PNG, sin SVG.** Se elimina `magick` (proceso externo, dependencia de runtime
  no controlada) y no se añade `resvg`. La búsqueda pasa de `icon_path()` con `hicolor`/`Adwaita`
  fijos al crate `freedesktop-icons`, que respeta el tema y su herencia. Las apps con solo
  icono SVG caen al glifo Phosphor (ya es el fallback). Si al medir faltan demasiados,
  se añade la feature opcional `svg-icons` con `resvg` sin texto (medido: +0,9 MB de binario,
  +31 crates, ~18 s de compilación; con features por defecto +2,2 MB y +57 crates).
- **Config en runtime** para apariencia y comportamiento (tamaños, `pinned`, autohide, colores).
- **Núcleo siempre compilado** (sin feature): `gfx`, `ipc`, `config`, `theme`, `control`.
- Los `#[cfg(feature)]` se concentran en el registro de componentes y en `Cargo.toml`.

## Fase 0 — Línea base (tras la auditoría de RAM del 2026-11-21)

- [x] Cerrar `docs/RAM_AUDIT.md` antes de refactorizar, para no mezclar mediciones.
- [x] Receta `just snapshots`: lanza cada modo (isla, barra, panel, dock, overview) y guarda
      capturas en `docs/baseline/` para comparar a ojo (revisión visual manual, sin diff
      automático: el reloj y la batería cambian). Pendiente ejecutar `just snapshots` en Hyprland
      para generar las imágenes (aquí no hay compositor).
- [x] Anotar RSS y tamaño del binario release como línea base (~131 MB y 20 MB).

## Fase 1 — Partir `ui.rs` (3130 líneas), sin cambio de comportamiento

- [x] Crear `src/ui/mod.rs` con lo compartido (`Metrics`, geometría, `UiState` común).
- [x] Extraer un módulo por componente: `bar` (máquina de estados), `dock` (modelo: pinned +
      ventanas, agrupación, `DockAction`, iconos; y vista autohide), `taskbar`, `overview`,
      `notifications` (tarjeta), `wallpaper` (pestaña), `command_menu`.
      Hecho: `bar`, `dock`, `overview`, `notifications`, `wallpaper` y `menu` (command menu).
      `taskbar` sigue solo en mockup: se extrae cuando se implemente, junto a su feature.
- [x] Cada módulo con su propio estado y `draw()`. Los `input_rects` llegan con el trait de Fase 2.
- [x] **Criterio de aceptación**: el struct compartido no conserva ningún campo específico de
      componente (`wallpaper_*`, `menu_*`, `dock_*`, `overview_*`, `notification_list`,
      `thumbs`, `app_icon_textures`…). Si no se eliminan ahora, Fase 3 acabará con `cfg`
      dispersos en el struct justo donde queríamos concentrarlos.
      `UiState` conserva solo datos compartidos (snapshot, clientes, iconos, colas y sistema).
- [x] Verificar por módulo: `cargo build`, `cargo clippy`, `cargo test` y comparación visual manual
      con `just snapshots`. Los tres primeros en verde; la visual pendiente de la línea base.
- [x] Commits por bloque: estructura, estado y línea base (agrupados, no uno por componente).

## Fase 2 — Estado compartido y trait `Component`

- [x] Definir `Shared` en el núcleo (`src/shared.rs`) con:
  - datos: clientes/workspaces/monitores (`ipc`), config, paleta, texturas de iconos
    (`app_icon_textures`);
  - **colas diferidas** que hoy viven en `UiState` (`src/ui/mod.rs`): `pending_commands`,
    `pending_spawns`, `dock_action`, `overview_focus`, `overview_workspace`, `menu_action`.
  Sin las colas, "ningún componente depende de otro" se incumple por debajo.
- [x] **Mover el polling de `ipc.clients()` al núcleo** (antes colgado del temporizador del
      dock). Ahora corre cada 400 ms con su propio temporizador y alimenta
      `Shared.clients`, que leen dock, overview y command menu.
- [x] `bar`: enum `BarPhase` + `max_state` en config (`[bar]`). La interpolación entre rects
      sigue en `ui::component::{bar_rect, card_rect}` con `smootherstep`.
- [x] Trait `Component` con `draw`, `input_rects` y `on_command` (`src/ui/component.rs`).
- [x] `build_components()` como único lugar de registro; el orden del vector es el z-order.
- [x] `input_region` = unión de `input_rects` de los componentes activos. `main.rs` ya no
      calcula rects: llama a `ui::input_rects()`.

## Fase 3 — Features de Cargo ✅ (pendientes de ejecución marcados)

- [x] Features según la tabla; `default` = todas. `taskbar = ["dock"]` (reservada: aún sin
      código propio); `dock`, `overview`, `command-menu` → `icons`; `wallpaper` → `bar` + `image`.
- [x] Dependencias opcionales: `zbus` → `notifications`; `image` → `wallpaper` (`thumbs.rs`);
      `freedesktop-icons` y `png` → `icons` (`appicons.rs`).
- [x] `appicons.rs`: `icon_path()` sustituido por `freedesktop-icons` (con herencia de tema y
      caché); sin SVG ni `magick`; PNG con `png` y reescalado propio a 48 px (filtro triangular
      en alfa premultiplicado, con tests). Los `.svg` devuelven `None` → glifo Phosphor.
      No se usó `with_scale`: las texturas son de 48 px fijos, así que pedir más resolución
      solo añadiría un reescalado.
- [x] Clave `[icons] theme`; si falta, `default_theme_gtk()` (ejecuta `gsettings` una vez, en el
      hilo de carga); si no, `hicolor`. Ojo: ese helper devuelve el `Name=` de `index.theme`,
      que normalmente coincide con el nombre del directorio, pero no siempre.
- [ ] Medir qué porcentaje de apps reales cae al fallback Phosphor por ser solo SVG; si es
      alto, implementar `svg-icons` (`resvg` con `default-features = false`) y medir RSS.
      *Necesita ejecutar el shell.*
- [x] `#[cfg]` en los `mod`, en los campos de `UiState`/`Shared` y en `build_components()`.
      Para no esparcir `cfg` por `main.rs`, el trait `Component` ganó `update`, `on_key` y
      `wants_keyboard`: cada componente lleva su lógica de host (servicio DBus, cursor del dock,
      acciones diferidas, teclado del menú). `main.rs` solo recorre el registro.
- [x] `control.rs`: el CLI rechaza (exit 1) comandos de componentes no compilados con un mensaje
      que nombra la feature; el servidor los registra en el log.
- [ ] El panel de ajustes oculta/deshabilita lo no compilado. *Aplaza a `config-panel`
      (futuro). Mientras tanto, las pestañas del panel (`Wallpaper`, `Alertas`) ya se generan
      según lo compilado.*
- [x] Dependencias ocultas resueltas al compilar cada feature sola:
  - `overview` y `command-menu` **no pedían** iconos (solo los leía el dock): la carga y
    subida de texturas pasó a `Shared::icon()`.
  - `overview` importaba `dock_icon` de `dock.rs`: movido a `ui/mod.rs`.
  - `BarState` (y el clamp de pestañas) dependían de `panel`: `BarState` vive ahora en el núcleo.
  - Los colores `P_*` del panel eran tokens de diseño de `ui/mod.rs`: movidos allí.
  - `command-menu` cerraba el overview tocando su estado: ahora con `cfg` puntual.
- Decisiones: `BarState` es núcleo; `notifications` es independiente de `bar`; `wallpaper`
  implica `bar` (es una pestaña del panel).
- Cambio de comportamiento menor: los lanzamientos del dock y del menú pasan por
  `pending_spawns` (`spawn_detached`: stdio a `/dev/null`, hilo que espera al proceso) en vez
  de `Command::spawn` directo.
- Verificado: clippy `-D warnings` en 9 combinaciones (`just lint-matrix`), `cargo check` de
  la matriz (`just check-matrix`), `cargo test`, `cargo fmt --check`. **No se ejecutó el
  shell**: una segunda instancia sustituiría el socket de control de la que esté en marcha.
  Falta una pasada manual por modo (isla, barra, panel, dock, overview, menú, notificaciones).

### Medición (release, `opt-level = 2`, `lto = "thin"`, sin `strip`)

| Build | Binario | Crates |
|---|---|---|
| `--no-default-features` | — | 173 |
| solo `bar` | **15,2 MB** | 173 |
| `dock` / `overview` / `command-menu` | — | 193 (+iconos) |
| `wallpaper` | — | 190 (+`image`) |
| `notifications` | — | 232 (+`zbus`) |
| completo (default) | **19,4 MB** | 258 |

> Continúa en `docs/WIDGETS_PLAN.md` (widgets, panel desacoplado, dashboard y bento editable).

## Fase 4 — Verificación y CI

- [x] Matriz mínima: `--no-default-features`, cada feature sola, y `--all-features`
      (`just check-matrix`, `just lint-matrix`). Falta llevarla a CI.
- [x] Combinaciones de referencia: `dock+bar`, `taskbar+overview`, `bar` sola.
- [ ] Comparar RSS por perfil contra la línea base (tamaño de binario ya medido arriba).
      *Necesita ejecutar el shell.*
- [x] `justfile`: `just build-minimal`, `just check-matrix`, `just lint-matrix`.

## Fase 5 — Documentación

- [ ] README: tabla de features y perfiles de ejemplo.
- [ ] Actualizar `docs/*.md` indicando la feature de cada componente.
- [ ] `config.example.toml`: marcar qué secciones requieren qué feature.

## Dependencias de los mockups (revisadas)

- `taskbar` y `topbar`: no introducen datos nuevos. La taskbar es otra vista sobre el modelo
  del dock (pinned + clientes del núcleo); la topbar se resuelve como estado de `bar`
  (`bar.max_state`), con reloj y datos del sistema de `Shared`.
- `dashboard`: solo `sysinfo` + paleta + `Shared.clients`; no necesita `dock` ni iconos propios
  (usa las texturas ya compartidas). Para implementarlo se añade como feature.
- `config-panel`: necesita el registro de componentes compilados para ocultar lo no compilado,
  y el `config` compartido. No añade dependencias de render. Para implementarlo se añade como feature.
- `command-menu`: comparte las texturas de iconos con `dock`/`overview` (ya cubierto por `icons`).

## Riesgos

| Riesgo | Mitigación |
|---|---|
| Refactor de `ui.rs` rompe animaciones/geometría | capturas `just snapshots` revisadas a ojo + un commit por módulo |
| Explosión de combinaciones de features | CI con matriz acotada |
| `cfg` dispersos y difíciles de leer | Un solo registro; núcleo sin features |
| Dependencias ocultas entre componentes | `Shared` con datos y colas; compilar cada feature sola |
| `image` opcional rompe un build solo-`taskbar` | `icons` ya no usa `image` (solo `png`) |
| Apps solo-SVG sin icono real | Fallback Phosphor; medir y, si hace falta, feature `svg-icons` |
