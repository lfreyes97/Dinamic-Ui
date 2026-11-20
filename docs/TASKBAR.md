# Taskbar (estilo Windows)

Barra de tareas acoplable a los cuatro lados (feature `taskbar`, que implica `dock`, comando
`taskbar`, `[taskbar] enabled/side/size/autohide`). Código: `src/ui/taskbar.rs`.

**Estado: revisada con el mismo método que el topbar** (vista previa de los cuatro lados, datos
reales de `Shared`, geometría de una sola fuente). Lo que queda depende de probar en el R7 y se
lista en «Fuera de alcance».

## Qué hay

| Zona | Contenido | Al hacer clic |
|---|---|---|
| Inicio | icono de la barra (azul) | «Actividades» (o el overview si no está compilada) |
| Buscar | lupa | el menú de comandos (`menu`), si está compilado |
| Ver tareas | icono de ventana | el overview de workspaces, si está compilado |
| Apps | fijadas (`[dock] pinned`) y las que tienen ventanas abiertas | lanza si no tiene ventanas; si ya tiene el foco, pasa a su siguiente ventana |
| Bandeja | notificaciones, batería (si hay), volumen, Wi-Fi y hora con fecha | solo descripción al pasar el cursor |

- **Indicadores en el borde de pantalla** (como el mockup): una marca de 16 px para la app activa y
  un punto para las que solo tienen ventanas.
- **Bandeja con datos reales**: Wi-Fi y volumen usan `wifi_status` / `audio_status` (los mismos
  helpers del topbar); la batería sale de `Shared.battery` con el icono por nivel y se oculta si
  no hay batería; la campana muestra «no molestar» y un punto si hay notificaciones sin leer.
- **Geometría de una sola fuente**: `taskbar_rect`, `hot_rect` y `near_edge` (la franja de
  activación del autohide) son las mismas funciones para el dibujo, el `input_region` y el
  `update`. El tamaño de las celdas sale de `[taskbar] size` (48 px → celdas de 40, como en el
  mockup), así que los iconos escalan con el grosor (40–72).
- **Modelo de apps**: el mismo que el dock (`make_item`/`items`), con el mismo icono de respaldo
  que el dock (`dock_icon`).

## Hallazgos y corrección

| # | Qué se vio | Evidencia | Estado |
|---|---|---|---|
| 1 | Iconos de la bandeja **fijos** (Wi-Fi alto, volumen alto, batería «alta» aunque el nivel fuera otro) y batería dibujada **aunque no hubiera batería** (sobremesa mostraría un nivel inventado) | `taskbar.rs` antiguo, líneas 304–341 | Corregido: estado real (`wifi_status`, `audio_status`, `battery.present`, icono por nivel) |
| 2 | Inicio, Buscar y Ver tareas **no hacían nada** pero tenían cursor de mano y sin resaltado | `taskbar.rs` antiguo, `slot` sin acción | Corregido: van a `activities`/`overview`/`menu` (vistas internas, no comandos del sistema) con resaltado al pasar el cursor |
| 3 | La app activa se quedaba **«pegada»**: `state.taskbar.active` sobrescribía `window_class` tras el primer clic | `taskbar.rs` antiguo, `active_override` | Corregido: la activa es solo la de `Shared.snapshot.window_class` |
| 4 | Con muchas apps el bloque **no quedaba centrado**: el total se calculaba con todas las apps y se dibujaban menos | `taskbar-bottom-many.png`: el bloque empezaba en x≈265 cuando el centro era 683 | Corregido: el total cuenta solo las apps visibles |
| 5 | Un clic en una app con varias ventanas **siempre** iba a la primera: si ya tenía el foco no pasaba a ninguna otra (el dock sí cicla) | `taskbar.rs` antiguo, `windows[0]` | Corregido: cicla a la siguiente ventana |
| 6 | Marca y punto de la app activa en el **borde interior** en los lados superior e izquierdo (el mockup los pone en el borde de pantalla) | `taskbar-top.png`, `taskbar-left.png` | Corregido (`active_mark`, `edge_point`) |
| 7 | **Fecha en inglés** («20 Nov») | `taskbar-bottom.png` | Corregido: `short_date()` en español |
| 8 | **Geometría fija**: celdas de 38 px e iconos de 20/22 px no dependían de `size`; la franja de activación (24 px) estaba escrita dos veces (dibujo y `update`) | `taskbar.rs` antiguo, `ITEM`, `HOT`; `component.rs` `update` | Corregido: `cell_size`, iconos y fuentes proporcionales; `near_edge` compartido |
| 9 | **Dock reclama entrada con la taskbar activa**: `Dock::input_rects` devuelve 72 px inferiores aunque el dock no se dibuje, y su `update` sigue animando | `component.rs` `Dock::input_rects` / `update` | Corregido en código (no verificado con clics: ver pendientes) |
| 10 | **Acoplamiento con `#[cfg]` dispersos**: el topbar apagaba `state.taskbar` directamente; la taskbar tocaba `state.dock` y `state.bar`; `taskbar.menu` era un campo que nadie activaba (y su Escape nunca se disparaba) | `component.rs` `Topbar::on_command`, `Taskbar::on_command`, `on_key` | Corregido: comandos del registro (`taskbar-off`, `collapse`), `taskbar::set_enabled`, campo muerto eliminado |
| 11 | Iconos de respaldo duplicados (`glyph_for` distinto del `dock_icon` del dock) | `taskbar.rs` antiguo, `glyph_for` | Corregido: se usa `dock_icon` |
| 12 | La bandeja no tenía descripción al pasar el cursor | `taskbar.rs` antiguo | Corregido: descripción real (Wi-Fi, volumen, batería, notificaciones) |

Lo que **no** se encontró: textos en inglés aparte de la fecha (las etiquetas ya eran español), colores
fijos (`just lint-colors` limpio: todo sale de `palette`/`pal()`).

## Fuera de alcance (pendiente)

- [ ] **Botones de sistema sin menú propio**: «Inicio» abre Actividades y «Buscar» el menú de
      comandos; no hay menú de inicio ni búsqueda de aplicaciones propios. Hacerlos sería una
      función nueva.
- [ ] **Apps que no caben**: con muchas apps abiertas (a 1366 px caben unas 16 con el grosor por defecto) las sobrantes no se
      dibujan y no hay forma de llegar a ellas. Falta un indicador «+N» o desplazamiento; es UI nueva.
- [ ] **Acciones de la bandeja** (activar/desactivar Wi-Fi, subir volumen, abrir la campana): solo
      lectura, igual que el resumen del topbar, porque ejecutarlas cambia el sistema real.
- [ ] **Autohide y clics**: el cierre al alejarse, la animación de 0,28 s y que el dock ya no
      reclame entrada (hallazgo 9) se comprueban solo en el R7; la vista previa no tiene cursor.
- [ ] **Duplicación con el dock**: `taskbar::items` repite la lista de apps de `dock::draw_dock`.
      Extraerla a un modelo común toca `dock.rs`, que no es de este trabajo.
- [ ] **Franja de 24 px del dock** escrita aparte (`component.rs`): no se importa `taskbar::HOT`
      porque el dock se compila sin la feature `taskbar`.
- [ ] **Cambiar el lado en caliente y `[taskbar] size`**: los rangos (40–72) están fijados en
      `config.rs`; no se han probado tamaños extremos en pantalla real.

## Vista previa

`just preview` genera `target/ui-preview/taskbar-{bottom,top,left,right}.png` (1366×768, con
ventanas de ejemplo y apps fijadas) y `taskbar-bottom-many.png` (18 apps, para el recorte y el
centrado).
