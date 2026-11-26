# Dock inferior (autohide)

## Comportamiento

- El layer surface cubre toda la pantalla; el dock se dibuja abajo centrado.
- Aparece (`slide in` desde abajo, ~0,2 s) cuando el cursor entra en una **franja
  inferior acotada**: los **24 px finales**, pero solo en el ancho del dock más
  32 px a cada lado. Ya visible, se mantiene mientras el cursor esté sobre el dock
  más un margen de 12 px (histéresis). Antes ambas zonas ocupaban todo el borde
  inferior y cualquier paso por abajo lo desplegaba. La geometría la calcula
  `dock.rs` (`dock_rect`, `activate_zone`, `persist_zone`) y la usan el dibujo,
  `input_rects` y la detección de cursor: no pueden desincronizarse. La posición
  se consulta por IPC (`cursorpos`) cada 120 ms.
- **Zona reservada**: el dock es siempre autohide (`DockConfig` no tiene aún
  `autohide`), así que **no reserva** hueco: la franja aparece y desaparece con
  el dock. Las reservas reales (`exclusive_zone`) viven en surfaces auxiliares
  y hoy solo las usan topbar y taskbar. Si se añade `dock.autohide`, con
  `false` se reservaría de forma permanente el alto del dock, como una barra fija.
- Elementos: apps fijadas (`[dock] pinned`) + ventanas mapeadas (`j/clients`),
  sin duplicar por `class`.
- **Moverse entre ventanas**: una sola ventana → enfocar por dirección; varias →
  clic abre un menú con los títulos y "Nueva ventana"; si la app ya está activa,
  el clic **cicla** a la siguiente ventana. Clic derecho abre el menú, clic medio
  lanza otra instancia. Puntos bajo el icono indican cuántas ventanas hay.
- El icono activo usa el color de acento.
- **Iconos reales de app**: `appicons.rs` mapea `class` → `Icon=` del `.desktop`
  y carga el PNG del tema (hicolor/Adwaita) en un hilo propio. En la captura se
  ven Firefox, Kitty y Chrome reales. Para SVG se usa el `magick` del sistema
  (sin añadir `resvg`); se buscan todos los tamaños del tema. Si no hay `.desktop`,
  se cae a un icono Phosphor por clase (globo, código, terminal…), nunca a letras.
- Las órdenes a Hyprland se traducen a la API Lua (`hl.dsp.focus({workspace=N})`,
  `hl.dsp.focus({window=...})`) porque la config Lua no acepta `dispatch` clásico.
- `input_region` incluye la franja inferior solo mientras el dock está visible.

## Configuración

```toml
[dock]
enabled = true
pinned = ["firefox", "code", "kitty"]
```

## Pendiente

- [ ] Agrupar varias ventanas de la misma app en un menú.
- [ ] Indicador de ventanas abiertas (punto) y animación de "salto" al lanzar.
- [ ] Soportar `.desktop` ids (ahora se usa `Command::new` directo).
