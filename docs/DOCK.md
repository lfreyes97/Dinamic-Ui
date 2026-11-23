# Dock inferior (autohide)

## Comportamiento

- El layer surface cubre toda la pantalla; el dock se dibuja abajo centrado.
- Aparece (`slide in` desde abajo, ~0,2 s) cuando el cursor llega a los **24 px
  inferiores** y se oculta al salir de los **72 px** que reclama el dock (histéresis):
  así el cursor puede estar sobre la parte alta del dock sin que se esconda. La
  posición se consulta por IPC (`cursorpos`) cada 120 ms.
- **Zona reservada (diseño pendiente)**: la reserva real (`exclusive_zone`) debe ir
  ligada a `autohide`. Con autohide activo **no** se reserva (el hueco aparece y
  desaparece con el dock); con `autohide = false` se reservan los 72 px de forma
  permanente, como una barra fija. El parche temporal `hyprland/patch.lua` cubre
  **solo el topbar** con gaps globales de Hyprland y no toca el dock.
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
