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
  | `icons` (interna) | `dep:image` | `appicons.rs`; la activan sus consumidores |

  `dashboard` y `config-panel` (mockups) son **futuros**: se añaden como feature cuando se
  implementen, no entran en las fases actuales.
- **"Compilado" ≠ "activado".** Compilado lo decide Cargo; activado es `enabled = true`
  en config. El panel de ajustes solo ofrece interruptores de lo compilado, y `control.rs`
  responde "no compilado" para el resto.
- **Config en runtime** para apariencia y comportamiento (tamaños, `pinned`, autohide, colores).
- **Núcleo siempre compilado** (sin feature): `gfx`, `ipc`, `config`, `theme`, `control`.
- Los `#[cfg(feature)]` se concentran en el registro de componentes y en `Cargo.toml`.

## Fase 0 — Línea base (tras la auditoría de RAM del 2026-11-21)

- [ ] Cerrar `docs/RAM_AUDIT.md` antes de refactorizar, para no mezclar mediciones.
- [ ] Receta `just snapshots`: lanza cada modo (isla, barra, panel, dock, overview) y guarda
      capturas en `docs/baseline/`; `just snapshots-diff` compara contra ellas. Sin esto,
      "capturas idénticas" es manual y frágil.
- [ ] Anotar RSS y tamaño del binario release como línea base.

## Fase 1 — Partir `ui.rs` (3130 líneas), sin cambio de comportamiento

- [ ] Crear `src/ui/mod.rs` con lo compartido (`Metrics`, geometría, `UiState` común).
- [ ] Extraer un módulo por componente: `bar` (máquina de estados), `dock` (modelo: pinned +
      ventanas, agrupación, `DockAction`, iconos; y vista autohide), `taskbar`, `overview`,
      `notifications` (tarjeta), `wallpaper` (pestaña), `command_menu`.
- [ ] Cada módulo con su propio estado, `draw()` y rects de entrada.
- [ ] Verificar por módulo: `cargo build`, `cargo clippy`, `cargo test` y `just snapshots-diff`.
- [ ] Un commit por componente extraído.

## Fase 2 — Estado compartido y trait `Component`

- [ ] Definir `Shared` en el núcleo con:
  - datos: clientes/workspaces/monitores (`ipc`), config, paleta, texturas de iconos
    (`app_icon_textures`);
  - **colas diferidas** que hoy viven en `UiState` (`ui.rs:159-174`): `pending_commands`,
    `pending_spawns`, `dock_action`, `overview_focus`, `overview_workspace`, `menu_action`.
  Sin las colas, "ningún componente depende de otro" se incumple por debajo.
- [ ] **Mover el polling de `ipc.clients()` al núcleo** (hoy en `main.rs:546-566`, colgado del
      temporizador del dock). Hoy `overview` lee `state.dock_clients`; con `overview` sin
      `dock` no habría datos. `dock_clients` pasa a `Shared.clients`.
- [ ] `bar`: enum de estados + interpolación entre rects; `max_state` en config.
- [ ] Trait `Component` con `draw`, `input_rects` y `on_command`.
- [ ] `build_components()` como único lugar de registro.
- [ ] `input_region` = unión de `input_rects` de los componentes activos.

## Fase 3 — Features de Cargo

- [ ] Declarar features según la tabla; `default` = todas. `taskbar = ["dock"]`; `dock`,
      `overview`, `command-menu` → `icons`.
- [ ] Dependencias opcionales: `zbus` → `notifications`; `image` → `icons` y `wallpaper`
      (lo usan `appicons.rs` y `thumbs.rs`, no solo wallpaper).
- [ ] `#[cfg]` en los `mod` de `main.rs` y en `build_components()`.
- [ ] `control.rs`: comandos de componentes no compilados responden "no compilado".
- [ ] El panel de ajustes oculta/deshabilita lo no compilado.
- [ ] Resolver dependencias ocultas que aparezcan al compilar cada feature sola.

## Fase 4 — Verificación y CI

- [ ] Matriz mínima: `--no-default-features`, cada feature sola, y `--all-features`.
- [ ] Combinaciones de referencia: `dock+bar`, `taskbar+overview`, `bar` sola.
- [ ] Comparar RSS y tamaño de binario por perfil contra la línea base.
- [ ] `justfile`: `just build-minimal`, `just check-matrix`.

## Fase 5 — Documentación

- [ ] README: tabla de features y perfiles de ejemplo.
- [ ] Actualizar `docs/*.md` indicando la feature de cada componente.
- [ ] `config.example.toml`: marcar qué secciones requieren qué feature.

## Pendiente por revisar

- Mockups `taskbar`, `topbar`, `dashboard`, `command-menu` y `config-panel`: no analizados en
  detalle; confirmar qué dependencias asumen antes de implementar `dashboard`/`config-panel`.

## Riesgos

| Riesgo | Mitigación |
|---|---|
| Refactor de `ui.rs` rompe animaciones/geometría | `just snapshots-diff` + un commit por módulo |
| Explosión de combinaciones de features | CI con matriz acotada |
| `cfg` dispersos y difíciles de leer | Un solo registro; núcleo sin features |
| Dependencias ocultas entre componentes | `Shared` con datos y colas; compilar cada feature sola |
| `image` opcional rompe un build solo-`taskbar` | feature interna `icons` |
