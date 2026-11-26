# AGENTS.md

Instrucciones para agentes que trabajen en este repositorio.

## Idioma, commits y alcance

- Respuestas y documentación: **español**.
- Mensajes de commit: **inglés**, estilo convencional (`feat:`, `fix:`, `docs:`,
  `chore:`, `style:`). Separar código, ejemplos y documentación en commits
  distintos cuando sea razonable.
- **Todo commit es una propuesta** hasta que el usuario lo confirme: no
  commitear sin confirmación explícita. Presentar antes una tabla con un renglón
  por archivo (`ruta`, `+`, `−`, `neto`) y una fila final de resumen con el total
  de líneas modificadas y el neto.
- **No hacer push** salvo petición explícita.
- Este repo **no es un dotfiles**. `hyprland/hyprland.example.lua` es un ejemplo;
  sus rutas son relativas vía `DINAMIC_UI_REPO` con fallback a `$HOME`.

## Arquitectura

- `src/lib.rs` contiene el shell; `src/main.rs` es el binario delgado del bucle
  Wayland. El previewer es `src/bin/previewer.rs` tras la feature `previewer`.
- Hay **un único layer surface** a pantalla completa anclado a los cuatro bordes.
  El `input_region` limita los clics. No hay `exclusive_zone` real todavía.
- Los componentes se registran en `ui::component::build_components()`; el orden
  del vector es el z-order. Compilado lo decide Cargo; activado lo decide config.
- `ui::draw()` es puro: recibe estado, paleta y progresos. **Los componentes
  nunca ejecutan nada**: encolan acciones en `Shared` y `main.rs` las ejecuta.
- Los datos viven en `Shared` y se piden por `Needs` al hilo `sysinfo`; la UI solo
  lee `Shared.system` / `Shared.battery` y reacciona a `system_revision`.

## QuickSettings

- `src/actions.rs`: `SettingAction` tipado y `SettingsState` con semáforo:
  `AwaitingConfirmation` (Lock, ámbar hasta que el usuario confirma) e
  `InFlight` (ámbar hasta que `sysinfo` confirma o expira en 3 s).
- `src/ui/quick_settings.rs` es el **único renderer**, con dos hosts: el overlay
  que abre el caret del estado y la pestaña System del panel (para usuarios sin
  topbar completo). No dibujar dos veces la misma lógica.
- Opciones elegibles en `[quick_settings] items`.
- Power mode implementado (`powerprofilesctl`, item `power-mode`); capturas en
  `screenshots.rs`. Pendiente: night light (no hay esquema/servicio en esta máquina).

## Previewer

- Es un **faker**: reutiliza los componentes reales con `sample_state` y
  `fake_desktop`. Sirve para diseñar y testear, no para decidir la arquitectura
  del shell. Sus acciones solo se registran en el log.
- Sidebar colapsable con `F1`; `DINAMIC_UI_HIDE_SIDEBAR=1` arranca oculto;
  `DINAMIC_UI_DESIGN_PLACEMENT=<nombre>` fija una variante del sandbox.
- Para capturas del previewer, **parar primero el shell real**: su layer surface
  se dibuja encima de la ventana del previewer. Arrancarlo de nuevo después.

## Verificación

- Iteración diaria: `just ci-fast` (fmt + check-fast + test-fast, perfil sin debuginfo y
  dependencias sin codegen). Es lo rápido; no incluye clippy ni la matriz.
- Antes de un commit: `just ci`. La matriz **no** va en el gate diario: `just lint-matrix no`
  se corre solo antes de push (compila ~21 variantes, tarda ~1 min y usa ~900 MB de target).
- `just preview` corre **en serie** (`--test-threads=1`): varios harness wgpu en
  paralelo segfaultean en la GPU híbrida del R3.
- Cambiar features rompe la matriz fácil: usar `#[cfg(...)]` bloques, nunca
  `cfg!(...)` como atajo para acceder a campos condicionados por feature.
- El release tarda ~4 min. Recompilarlo y relanzarlo si se quiere ver un cambio
  en vivo; al detener el shell, `mako` puede robar `org.freedesktop.Notifications`
  (servicio deshabilitado en arranque: pararlo y relanzar dinamic-ui).

## Estilo visual

- Iconos: **Phosphor del crate** (`egui_phosphor::regular as ic`), nunca texto
  Unicode como `▲`/`▼`.
- Botones clicables de la topbar: chip siempre dibujado con `sheen(10)`, borde y
  esquinas redondeadas (`tool_chip`); hover sube a `sheen(22)`.
- Controles de modo: `▲` en el notch activo, `▲▼` en el hueco reloj↔sistema,
  `▼` en el topbar. El caret del estado ya abre el resumen: no añadir entradas
  redundantes.

## Límites y exploración

- Otros WCs (`Sugar_*`, `fake_hypr`, `fake_river`, `fake_weston`) son una
  **idea de exploración**, no roadmap. Soportarlos de verdad exige diseñar IPCs
  propios. Ver `docs/SUPPORT_OTHER_WCS.md`.
- La reserva real de zonas se hace con surfaces auxiliares (`Reservations` en
  `src/main.rs`); ya no hay parche de gaps en la config de Hyprland.
- Los sliders de config nunca escriben `config.toml` del usuario.

## Relación con el usuario

- Explicar con ejemplos concretos antes de codear cuando haya una decisión de
  arquitectura. Evitar ráfagas de preguntas abstractas: si hay duda, proponer un
  camino concreto y pedir confirmación.
- No inventar funcionalidad "porque sí". Si algo es una maqueta incompleta,
  decirlo y no tratarlo como decisión tomada.
