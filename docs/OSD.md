# OSD de volumen y brillo

Barra efímera bajo el borde superior que confirma un cambio de volumen o brillo. Sustituye
al OSD de Quickshell (ítem 1 de `docs/POST_QUICKSHELL.md`).

## Cómo se dispara

Hay dos caminos, ambos acaban en `DinamicUi::show_osd`:

1. **Teclas multimedia**: los binds de `hyprland/hyprland.example.lua` encadenan el cambio
   con `dinamic-ui --osd-volume` o `--osd-brightness`. El `&&` garantiza que el valor ya
   esté aplicado antes de leerlo.
2. **Sliders de QuickSettings**: al ejecutarse una acción `Volume` o `Brightness`
   (`src/ui/quick_settings.rs`), `main.rs` muestra el OSD correspondiente.

`show_osd` enciende el estado y llama a `sysinfo::force_refresh`: el worker de `sysinfo`
sondea cada 5 s, pero con esa señal lee de inmediato el grupo pedido (`Audio` o `Display`).
Sin ese paso el OSD mostraría el valor anterior hasta 5 s después.

## Comportamiento

- Aparece de golpe (sin fundido de entrada), se mantiene **1,2 s** y se desvanece en **0,18 s**.
- Un disparo nuevo reinicia el temporizador y cambia el tipo si hace falta.
- Mientras está activo pide solo su dato (`Need::Audio` o `Need::Display`); al soltarlo deja
  de pedirlo, aunque `NeedTracker` mantiene el dato vivo unos segundos.
- No reclama entrada (`input_rects` vacío): no intercepta clics.
- Se dibuja **al final** del z-order, así que se ve también con un overview o el menú abiertos.
- Si el dato aún no ha llegado, muestra «sin datos» con la barra vacía en vez de inventar un valor.

## Límites

- No hay configuración de posición, duración ni tema: usa los tokens de `theme.rs` y se
  coloca bajo el topbar (o bajo la barra si el topbar está apagado).
- El valor de los sliders se lee del sistema, no del action: si `wpctl` tarda más que el
  sondeo, el OSD puede mostrar el valor previo durante un frame. En las teclas multimedia
  no pasa porque el `&&` serialize ambos pasos.

## Archivos

- `src/ui/osd.rs`: estado, temporizador, `needs` y dibujo (con tests del ciclo de vida).
- `src/ui/component.rs`: registro del componente como última capa.
- `src/control.rs`: comandos `--osd-volume` y `--osd-brightness` (feature `osd`).
- `src/sysinfo.rs`: `SystemMonitor::force_refresh`.
