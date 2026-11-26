# Isla tipo notch

## Comportamiento

- En reposo es una franja de **56×5 px** centrada arriba (solo lo que asoma del
  notch), con un punto de acento. Las esquinas superiores son rectas: la forma
  cuelga del borde de la pantalla y solo redondea las inferiores.
- Al acercar el cursor a la franja superior central (y ≤ 18 px, |x − centro| ≤ 100)
  se despliega a una píldora de 128×32 que muestra **icono de reloj + hora**.
  Esa zona de entrada no cambia. La histéresis actúa solo a la salida: se añade
  el rect animado más 12 px (`src/ui/bar.rs::notch_hover_zones`), así que bajar
  el cursor por dentro de la píldora no la retrae. Si el IPC de cursor falla, se
  conserva el último estado en vez de cerrarla.
- La transición usa `island_progress` con `approach` y caída acelerada (`p²`,
  tipo gravedad): la píldora negra lisa "cae" desde el notch y se posa, como el
  Dynamic Island de iPhone (~0,17 s). El contenido va por delante del
  contenedor: icono, hora y chip comparten opacidad y escala propias y se encogen
  con fundido mientras la píldora todavía conserva parte de su recorrido
  (`island_content_alpha` / `island_content_scale`). La escala del contenido es
  `p²(2 − p)`, siempre menor que el progreso `p` del rect: a mitad de viaje el
  contenido va al 38 % y la píldora al 50 %. El punto de reposo se apaga con la
  curva complementaria, sin solaparse con la hora.
- El `input_region` sigue el rect real (notch o píldora), así que el resto del borde
  superior no captura clics.
- El modo compacto muestra **icono de reloj + hora** (15 px Inter Regular) en acento/texto.
- **Interacción**: clic en el notch/compacto lo engancha en modo compacto; un
  segundo clic expande a la barra completa. Ambos pasan por los comandos
  `compact`/`expand`, así que `[bar] max_state` limita también el ratón.
  `Esc` / `--collapse` vuelven al notch, no al compacto enganchado.
- Con `[bar] notch_control = true` (default) y `max_state ≥ compact`, el notch
  activo muestra un `▲` anclado al borde derecho del rect animado: es la pista
  visual del clic que continúa el ascenso.
- `DINAMIC_UI_ISLAND=1` fuerza el estado desplegado (pruebas).
- Comandos de control: `dinamic-ui --compact` fija el compacto y `--notch` vuelve
  al notch (también `just compact` / `just notch`).

## Batería con vida

`battery_tone()` colorea el indicador según nivel: verde (>60 %), ámbar (21–60 %),
rojo (≤20 %) y azul si está cargando. Se aplica en la barra y en la pestaña System,
y el glifo lleva un halo suave del mismo color.
