# Isla tipo notch

## Comportamiento

- En reposo es una franja de **56×5 px** centrada arriba (solo lo que asoma del
  notch), con un punto de acento. Las esquinas superiores son rectas: la forma
  cuelga del borde de la pantalla y solo redondea las inferiores.
- Al acercar el cursor a la franja superior central (y ≤ 18 px, |x − centro| ≤ 100)
  se despliega a una píldora de 128×32 que muestra **solo el reloj**.
- La transición usa `island_progress` con `approach` y caída acelerada (`p²`,
  tipo gravedad): la píldora negra lisa "cae" desde el notch y se posa, como el
  Dynamic Island de iPhone (~0,17 s).
- El `input_region` sigue el rect real (notch o píldora), así que el resto del borde
  superior no captura clics.
- El modo compacto muestra **icono de reloj + hora** (15 px semibold) en acento/texto.
- **Interacción**: clic en el notch/compacto lo engancha en modo compacto; un
  segundo clic expande a la barra completa. El hover solo lo despliega temporalmente.
- `DINAMIC_UI_ISLAND=1` fuerza el estado desplegado (pruebas).
- Comandos de control: `dinamic-ui --compact` fija el compacto y `--notch` vuelve
  al notch (también `just compact` / `just notch`).

## Batería con vida

`battery_tone()` colorea el indicador según nivel: verde (>60 %), ámbar (21–60 %),
rojo (≤20 %) y azul si está cargando. Se aplica en la barra y en la pestaña System,
y el glifo lleva un halo suave del mismo color.
