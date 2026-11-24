# AGENTS.md

Instrucciones para agentes que trabajen en este repositorio.

## Idioma, commits y alcance

- Respuestas y documentación: **español**.
- Mensajes de commit: **inglés**, estilo convencional (`feat:`, `fix:`, `docs:`,
  `chore:`, `style:`). Separar código, ejemplos y documentación en commits
  distintos cuando sea razonable.
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
- Acciones pendientes: power mode, night light y capturas.

## Previewer

- Es un **faker**: reutiliza los componentes reales con `sample_state` y
  `fake_desktop`. Sirve para diseñar y testear, no para decidir la arquitectura
  del shell. Sus acciones solo se registran en el log.
- Sidebar colapsable con `F1`; `DINAMIC_UI_HIDE_SIDEBAR=1` arranca oculto;
  `DINAMIC_UI_DESIGN_PLACEMENT=<nombre>` fija una variante del sandbox.
- Para capturas del previewer, **parar primero el shell real**: su layer surface
  se dibuja encima de la ventana del previewer. Arrancarlo de nuevo después.

## Verificación

- `just ci` y `just lint-matrix no` antes de considerar cerrado un cambio.
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
- La reserva real de zonas requiere surfaces auxiliares. `hyprland/patch.lua` es
  un parche temporal de gaps de Hyprland, no una solución de protocolo.
- Los sliders de config nunca escriben `config.toml` del usuario.

## Relación con el usuario

- Explicar con ejemplos concretos antes de codear cuando haya una decisión de
  arquitectura. Evitar ráfagas de preguntas abstractas: si hay duda, proponer un
  camino concreto y pedir confirmación.
- No inventar funcionalidad "porque sí". Si algo es una maqueta incompleta,
  decirlo y no tratarlo como decisión tomada.
