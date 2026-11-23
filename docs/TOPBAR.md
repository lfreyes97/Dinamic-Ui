# Topbar (estilo GNOME)

Franja superior a todo el ancho (feature `topbar`, comando `--topbar`, `[topbar] enabled/height`).
Código: `src/ui/topbar.rs`.

**Estado: terminado en lo que se puede cerrar sin ejecutar el shell.** Lo que queda (acciones del
sistema y fusión con `bar`) está en «Fuera de alcance» y depende de pruebas en el R7.

## Qué hay

| Zona | Contenido | Al hacer clic |
|---|---|---|
| Izquierda | «Actividades», los workspaces y el título de la app enfocada | Actividades (o el overview si no está compilada); un punto cambia de workspace |
| Centro | fecha corta en español y hora | calendario con las notificaciones debajo, anclado bajo el reloj |
| Derecha | icono de wifi, de audio y batería con su % | resumen del sistema, anclado a la derecha |

- **Datos reales** de `Shared`: wifi, bluetooth, volumen, brillo y salida de audio (`sysinfo`) y
  batería (`power`). Comparte con la barra `wifi_status` / `audio_status`, así los iconos y su
  descripción son los mismos en las dos.
- **Resumen del sistema: solo lectura.** Muestra el estado de Wi-Fi (red) y Bluetooth
  (apagado / activado / N conectados), las barras de volumen y brillo, la salida de audio y la
  batería con el tiempo restante. Sin dato aparece «Sin datos» o «—», nunca un valor inventado.
- **Workspaces:** un punto por cada workspace ocupado más el actual (hasta 8), con el actual
  alargado y teñido de acento. Un clic cambia de workspace.
- **Calendario:** en español, con botones de mes anterior y siguiente (un clic en el título vuelve a
  hoy), hoy resaltado y día seleccionable. Su alto depende de las semanas que ocupa el mes.
- **Notificaciones en el calendario** (feature `notifications`): bajo el calendario, con el
  interruptor «No molestar», las tres más recientes con su botón de cerrar y, si hay más, «+N más»
  con «Borrar todo». Sin notificaciones muestra «Sin notificaciones». Si la feature no está
  compilada, el calendario es solo calendario.
- **Controles de modo**: con `[bar] hueco_control = true` (default), la barra
  expandida muestra `▲▼` en el hueco entre el reloj y el estado: `▲` entra al
  topbar y `▼` vuelve a la isla (equivalente a `collapse`). El topbar muestra un
  `▼` equivalente si `[topbar] mode_control = true`.
- **La barra cede el borde superior:** mientras el topbar está activo, la barra (notch/isla) no se
  dibuja ni reclama entrada (`BarState::suppressed`); el panel sigue disponible. Antes seguían
  dibujándose y animándose por debajo las dos a la vez.
- **Geometría de una sola fuente**: `quick_rect` y `calendar_rect` los usan tanto el dibujo como el
  `input_region`, así no pueden desincronizarse.
- Los botones se resaltan al pasar el cursor y mientras su desplegable está abierto.
- **Los desplegables se cierran al pulsar fuera** (o con `Esc`): mientras hay uno abierto, toda la
  pantalla recibe el clic, como en un menú; las tarjetas absorben el clic en sus zonas sin widget
  para que no cuente como «fuera». El primer clic fuera solo cierra, no llega a la ventana de debajo.
- **El aviso emergente de una notificación nueva espera** mientras hay un desplegable abierto
  (`BarState::popover_open`); no se pierde: sigue en la lista del calendario.
- **Datos bajo demanda:** el hilo de `sysinfo` solo consulta si algún componente lo pide
  (`Component::wants_system`): la barra expandida, el panel abierto o el topbar visible.

## Fuera de alcance (depende de probar en el R7)

- [ ] **Acciones del resumen del sistema** (activar/desactivar wifi y bluetooth, subir volumen y
      brillo): hay que lanzar comandos del sistema (`nmcli`, `bluetoothctl`, `wpctl`,
      `brightnessctl`…). Se dejó en solo lectura hasta comprobar qué herramientas hay y cómo
      responden; ejecutarlas desde aquí cambiaría el wifi o el volumen de la máquina real.
- [ ] **Topbar como estado de `bar`** (versión completa de lo acordado en
      `docs/WIDGETS_PLAN.md`): hoy es un componente aparte que suprime a la barra. Falta que sea un
      estado más de la misma máquina (con `bar.max_state`) y que el calendario y las notificaciones
      estén también en los estados compacto y notch.
- [ ] **Abrir una notificación:** solo se pueden cerrar o vaciar; el servicio no guarda a qué app
      activar.
- [ ] `[topbar] height` es un valor fijo (24–48 px, 34 por defecto), no una fracción de la
      pantalla: una barra de estado tiene un grosor «natural» y no se escala como las franjas.
- [ ] Verificación en el R7 de lo que la vista previa no cubre: clics, hover, el cierre al pulsar
      fuera y el primer clic que solo cierra.
