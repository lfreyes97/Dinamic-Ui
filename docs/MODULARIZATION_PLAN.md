# Plan de acción — Componentes desacoplados en compilación

Objetivo: que un usuario pueda compilar solo lo que quiere (p. ej. `dock + topbar`,
o `taskbar + overview`) mediante features de Cargo, sin pagar RAM, dependencias ni
hilos de lo demás.

## Decisiones tomadas

- **`bar` es un solo componente con estados de expansión**, no varios: notch → compacto →
  píldora → barra full width → panel. Comparten surface, animación y geometría. El máximo
  al que sube es config en runtime (`bar.max_state`), no compilación.
- **Features de Cargo** solo para componentes independientes: `bar`, `dock`, `taskbar`
  (por confirmar si es un modo de `dock`), `overview`, `notifications`, `wallpaper`.
  Las que arrastran dependencias pesadas (`notifications`, `wallpaper`, `overview`) son
  las prioritarias.
- **Config en runtime** para apariencia y comportamiento (tamaños, `pinned`, autohide, colores).
- **Núcleo siempre compilado** (sin feature): `gfx`, `ipc`, `config`, `theme`, `control`.
- Todos los `#[cfg(feature)]` se concentran en un único registro de componentes y en
  `Cargo.toml`; no se dispersan por `ui.rs`.

## Fase 0 — Línea base (2026-11-21)

- [ ] Cerrar la auditoría de RAM (`docs/RAM_AUDIT.md`) antes de refactorizar, para que
      las mediciones no se mezclen con el refactor.
- [ ] Captura de pantalla de referencia de cada modo (isla, barra, panel, dock, overview).
- [ ] Anotar RSS y tamaño del binario release como línea base.

## Fase 1 — Partir `ui.rs` (3130 líneas), sin cambio de comportamiento

- [ ] Crear `src/ui/mod.rs` con lo compartido (`Metrics`, geometría, `UiState` común).
- [ ] Extraer un módulo por componente: `bar` (notch/isla/píldora/barra/panel como una
      máquina de estados), `dock`, `overview`, `notifications` (tarjeta), `wallpaper`
      (pestaña), `command_menu`.
- [ ] Cada módulo con su propio estado, `draw()` y rects de entrada.
- [ ] Verificar: `cargo build` + capturas idénticas a la Fase 0.
- [ ] Un commit por componente extraído.

## Fase 2 — Estado compartido y trait `Component`

- [ ] Definir `Shared` en el núcleo: clientes/workspaces/monitores (`ipc`), config, paleta,
      iconos. Ningún componente depende de otro; solo de `Shared`.
- [ ] `bar`: enum de estados + interpolación entre rects; `max_state` en config.
- [ ] Trait `Component` con `draw`, `input_rects` y `on_command`.
- [ ] `build_components()` como único lugar de registro.
- [ ] `input_region` = unión de `input_rects` de los componentes activos.
- [ ] Tamaño del layer surface calculado desde los componentes compilados (no `bar_h + panel_h`
      fijo), para que sin topbar no se reserve altura.

## Fase 3 — Features de Cargo

- [ ] Declarar features en `Cargo.toml`; `default` = todas.
- [ ] Dependencias opcionales ligadas a su feature: `zbus` → `notifications`;
      `image` → `wallpaper`/`overview`; `thumbs.rs` y `appicons.rs` bajo `cfg`.
- [ ] `#[cfg]` en los `mod` correspondientes de `main.rs` y en `build_components()`.
- [ ] `control.rs`: comandos de componentes no compilados responden "no compilado".
- [ ] Resolver dependencias cruzadas detectadas al compilar (lo que hoy fuerza a `overview`
      a necesitar `dock`) moviéndolas a `Shared`.

## Fase 4 — Verificación y CI

- [ ] Matriz mínima: `--no-default-features`, cada feature sola, y `--all-features`
      (no todas las combinaciones).
- [ ] Combinaciones nombradas de referencia: `dock+topbar`, `taskbar+overview`, `island` sola.
- [ ] Comparar RSS y tamaño de binario por perfil contra la línea base.
- [ ] `justfile`: recetas `just build-minimal`, `just check-matrix`.

## Fase 5 — Documentación

- [ ] README: tabla de features y perfiles de ejemplo.
- [ ] Actualizar `docs/*.md` indicando la feature de cada componente.
- [ ] `config.example.toml`: marcar qué secciones requieren qué feature.

## Pendiente por revisar

- ¿`taskbar` es un modo de `dock` o un componente aparte? Decidir al revisar su mockup.
- Los mockups `taskbar-mockup.html`, `topbar-mockup.html`, `dashboard-mockup.html`,
  `command-menu-mockup.html` y `config-panel-mockup.html` no se han analizado en detalle:
  confirmar qué dependencias entre componentes asumen antes de cerrar la lista de features.
- `plan-mode-panel.md` convive con este plan; decidir si se mueve a `docs/`.

## Riesgos

| Riesgo | Mitigación |
|---|---|
| Refactor de `ui.rs` rompe animaciones/geometría | Capturas de referencia + un commit por módulo |
| Explosión de combinaciones de features | CI con matriz acotada |
| `cfg` dispersos y difíciles de leer | Un solo registro; núcleo sin features |
| Dependencias ocultas entre componentes | Mover estado a `Shared` al detectarlas |
