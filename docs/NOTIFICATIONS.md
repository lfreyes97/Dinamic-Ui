# Notificaciones — arquitectura

## Principio

Dinamic UI **es el servidor estándar** `org.freedesktop.Notifications`. No depende
de Mako, dunst ni de ningún daemon concreto, y no consulta interfaces privadas
que puedan activar a otro proceso por D-Bus.

## Implementación

`src/notifications.rs` registra el objeto `/org/freedesktop/Notifications` con
`zbus` y atiende los métodos de la especificación freedesktop:

| Método | Acción |
|---|---|
| `Notify` | encola la notificación y devuelve un id |
| `CloseNotification` | la quita de la cola |
| `GetCapabilities` | `body`, `actions` |
| `GetServerInformation` | nombre, vendor, versión, spec `1.2` |

Un hilo propio mantiene la conexión D-Bus; el event loop de Wayland solo hace
`poll()`. El nombre se pide con `request_name`: si otro servidor (p. ej.
Quickshell) ya lo tiene, se avisa por log y no se roba el nombre.

## Estado y vistas

- `Shared { queue, next_id, session_count, dnd, revision }` es la única fuente de verdad.
- La **isla** muestra solo la activa, en una **tarjeta propia** de 320×68 bajo la
  barra (`ui::notification_card`), con app, hora, título, cuerpo y botón de cerrar.
  No se comprime dentro de la barra. El badge `×N` es el total de la sesión y se
  reinicia con "Limpiar todo".
- Animación: **slide in down** (~0,5 s, la tarjeta baja desde detrás de la barra con
  fade) y **slide out up** (mismo recorrido en sentido inverso al caducar o cerrar).
- La pestaña **Alerts** del panel consume la misma cola: DND, listado y
  "Limpiar todo". No hay segunda suscripción D-Bus.
- `dnd` suprime el display en la isla pero mantiene el historial del panel.
  Se activa con el toggle de la pestaña Alerts o con `DINAMIC_UI_DND=1` al arrancar.
- La pestaña Alerts **apila** todas las notificaciones de la cola en una lista con
  scroll; con `panel_h = 360` caben unas 3 tarjetas visibles a la vez.

## Pendiente

- [ ] Emitir `NotificationClosed` / `ActionInvoked` al descartar o pulsar una acción.
- [ ] Ejecutar las `actions` (`default`, etc.) del mensaje.
- [ ] Persistir el historial en `$XDG_STATE_HOME/dinamic-ui/`.
- [ ] Retirar `NotificationServer` de Quickshell cuando Dinamic UI lo sustituya.
