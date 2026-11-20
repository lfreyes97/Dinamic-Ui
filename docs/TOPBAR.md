# Topbar (estilo GNOME)

Franja superior a todo el ancho (feature `topbar`, comando `--topbar`, `[topbar] enabled/height`).
Código: `src/ui/topbar.rs`.

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
- **La barra cede el borde superior:** mientras el topbar está activo, la barra (notch/isla) no se
  dibuja ni reclama entrada (`BarState::suppressed`); el panel sigue disponible. Antes seguían
  dibujándose y animándose por debajo las dos a la vez.
- **Geometría de una sola fuente**: `quick_rect` y `calendar_rect` los usan tanto el dibujo como el
  `input_region`, así no pueden desincronizarse.
- Los botones se resaltan al pasar el cursor y mientras su desplegable está abierto.
- **Datos bajo demanda:** el hilo de `sysinfo` solo consulta si algún componente lo pide
  (`Component::wants_system`): la barra expandida, el panel abierto o el topbar visible.

## Pendiente

- [ ] **Acciones** (activar/desactivar wifi y bluetooth, subir volumen y brillo): hay que lanzar
      comandos del sistema (`nmcli`, `bluetoothctl`, `wpctl`, `brightnessctl`…). Se dejó en solo
      lectura hasta poder comprobar en el R7 qué herramientas hay y cómo responden.
- [ ] **Topbar como estado de `bar`** (versión completa de lo acordado en
      `docs/WIDGETS_PLAN.md`): hoy sigue siendo un componente aparte que suprime a la barra. Falta
      que sea un estado más de la misma máquina (con `bar.max_state`) y que el calendario y las
      notificaciones estén también en los estados compacto y notch.
- [ ] El aviso emergente de una notificación nueva (`NotificationCard`) se dibuja bajo el centro de
      la barra y se solapa con el calendario si está abierto. Habría que ocultarlo mientras tanto.
- [ ] Las notificaciones solo se pueden cerrar o vaciar: no hay acción de «abrir» (el servicio no
      guarda a qué app activar).
- [ ] `Avión` y `Nocturno` se quitaron del resumen porque no había dato real que mostrar.
- [ ] Verificación en el R7 de la interacción (clics, hover) y de la altura frente a otras
      pantallas: `[topbar] height` es un valor fijo (24–48 px), no una fracción de la pantalla.
