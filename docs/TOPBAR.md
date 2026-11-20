# Topbar (estilo GNOME)

Franja superior a todo el ancho (feature `topbar`, comando `--topbar`, `[topbar] enabled/height`).
Código: `src/ui/topbar.rs`.

## Qué hay

| Zona | Contenido | Al hacer clic |
|---|---|---|
| Izquierda | «Actividades» y el título de la app enfocada | abre Actividades (o el overview si no está compilada) |
| Centro | fecha corta en español y hora | calendario, anclado bajo el reloj |
| Derecha | icono de wifi, de audio y batería con su % | resumen del sistema, anclado a la derecha |

- **Datos reales** de `Shared`: wifi, bluetooth, volumen, brillo y salida de audio (`sysinfo`) y
  batería (`power`). Comparte con la barra `wifi_status` / `audio_status`, así los iconos y su
  descripción son los mismos en las dos.
- **Resumen del sistema: solo lectura.** Muestra el estado de Wi-Fi (red) y Bluetooth
  (apagado / activado / N conectados), las barras de volumen y brillo, la salida de audio y la
  batería con el tiempo restante. Sin dato aparece «Sin datos» o «—», nunca un valor inventado.
- **Calendario:** en español, con botones de mes anterior y siguiente (un clic en el título vuelve a
  hoy), hoy resaltado y día seleccionable. Su alto depende de las semanas que ocupa el mes.
- **Geometría de una sola fuente**: `quick_rect` y `calendar_rect` los usan tanto el dibujo como el
  `input_region`, así no pueden desincronizarse.
- Los botones se resaltan al pasar el cursor y mientras su desplegable está abierto.
- **Datos bajo demanda:** el hilo de `sysinfo` solo consulta si algún componente lo pide
  (`Component::wants_system`): la barra expandida, el panel abierto o el topbar visible.

## Pendiente

- [ ] **Acciones** (activar/desactivar wifi y bluetooth, subir volumen y brillo): hay que lanzar
      comandos del sistema (`nmcli`, `bluetoothctl`, `wpctl`, `brightnessctl`…). Se dejó en solo
      lectura hasta poder comprobar en el R7 qué herramientas hay y cómo responden.
- [ ] **Alinearlo con lo acordado** (`docs/WIDGETS_PLAN.md`): hoy el topbar es un segundo
      componente; el notch de la barra sigue dibujándose y reclamando entrada por debajo. Lo
      previsto es que sea un estado de `bar` (reemplaza al notch), con **workspaces a la
      izquierda** y un popover de **calendario + notificaciones** (hoy el calendario no las incluye).
- [ ] `Avión` y `Nocturno` se quitaron del resumen porque no había dato real que mostrar.
- [ ] Verificación en el R7 de la interacción (clics, hover) y de la altura frente a otras
      pantallas: `[topbar] height` es un valor fijo (24–48 px), no una fracción de la pantalla.
