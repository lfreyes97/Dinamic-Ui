# Actividades

Lanzador a pantalla completa: las **apps instaladas** y las **ventanas abiertas**, con búsqueda.
Comando `--activities` (alias antiguo `--overview-activities`), atajo `SUPER SHIFT, O` en
`hyprland/dinamic-ui.conf`, y es lo que abre el botón «Actividades» del topbar (con el overview de
workspaces como respaldo si Actividades no está compilada).

## Por qué no es un overview

Un overview muestra **workspaces** (la rejilla y las franjas). Actividades muestra **apps**: lee
los `.desktop`, busca y lanza o enfoca, igual que el modo «Apps» del command menu. Por eso es su
propia feature, `activities`, que **no depende de `overview`**: se puede compilar una sin la otra.
Comparten solo piezas comunes (`overlay.rs`: buscador, barra de teclas; `nav.rs`: navegación en
rejilla). Código: `ui/activities.rs` (estado, teclado y dibujo) y `ui/activities_model.rs` (puro,
con tests).

## Qué hace

- **Apps reales** (`desktop::scan`, leídas al abrir y soltadas al cerrar). Sin búsqueda: primero las
  abiertas, luego las ancladas (`[dock] pinned`) y el resto por nombre. Con búsqueda, por
  puntuación (prefijo > inicio de palabra > contiene > id > comentario), con el criterio de
  `src/search.rs` que comparte el command menu. Máximo 24.
- **Activar** (`↵` o clic): si la app tiene ventanas abiertas las enfoca (con varias, cicla entre
  ellas); si no, la lanza. Una ventana sin `.desktop` aparece igualmente (se enfoca, no se lanza).
- **Tira de ventanas abiertas** abajo, con una vista esquemática de cada una y su título. Como las
  franjas del overview, **no es una barra de estado**: su alto es una fracción de la pantalla
  (`[activities] strip_height`, 1/5 por defecto = 154 px en 1366×768; también acepta píxeles). Las
  tarjetas se derivan de ese alto y de la proporción del monitor (la vista esquemática la
  conserva), y cuántas caben sale del ancho. La ventana activa va primero; si hay más de las que
  caben aparece «+N ventanas más». Código: `activities_model::film_layout` (puro, con tests).
- **Teclado exclusivo**: escribir filtra, `←→↑↓`/`Tab` mueven la selección, `↵` activa, `Esc`
  limpia la búsqueda y, si ya está vacía, cierra.
- **Fundido** de entrada y salida.
- Un punto bajo cada tile por ventana abierta (hasta tres).

## Diseño y dimensiones

Cuadrícula de 6 columnas centrada, tiles con borde y el buscador con chip `ESC`, como el mockup.
Las medidas son una guía: tiles, separación, iconos y tira de ventanas **escalan entre 1× y 1,6×**
respecto a 720p, y los textos con la raíz de esa escala.

## Pendiente

- [ ] Verificación en el R7: el fundido y el teclado exclusivo no se pueden ver con `just preview`.
- [ ] Iconos SVG (hoy solo PNG, ver `docs/MODULARIZATION_PLAN.md`): las apps con icono solo SVG
      muestran un glifo de reserva.
