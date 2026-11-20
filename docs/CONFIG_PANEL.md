# Panel de configuración (system-conf)

Páginas de ajustes con navegación lateral (feature `config-panel`, comando `--config-panel`, alias
`--config`). Código: `src/ui/config_panel.rs`; vistas previas: `src/ui/preview_config.rs`.
Plan: Fase 6.5 de `docs/WIDGETS_PLAN.md`. Guía visual: `docs/config-panel-mockup.html` (líneas de
diseño, no medidas).

**Estado: maqueta de solo lectura terminada.** Muestra los valores reales de la configuración
cargada. Editar y guardar (`toml_edit`) no está hecho: ver «Fuera de alcance».

## Qué hay

- **Ventana** centrada al 80 % de la pantalla, con navegación a la izquierda, cabecera con el
  título y la descripción de la página, cuerpo con scroll y un pie con la nota de solo lectura.
  `Esc` cierra; `↑` y `↓` pasan a la página visible anterior o siguiente. Un clic en la
  navegación cambia de página. Un clic fuera de la ventana cierra el panel.
- **Navegación agrupada** como en el mockup: «Apariencia y vistas» (Tema y colores, Barras,
  Overviews, Dashboard), «Comportamiento» (Workspaces, Notificaciones, Dock, Atajos) y «Sistema»
  (config.toml). Un grupo sin páginas visibles no aparece.
- **Solo lo compilado.** Cada página declara de qué features depende (basta una). Si ninguna está
  compilada, la página no aparece en la navegación (`Page::features`, `visible_pages`):

  | Página | Features |
  |---|---|
  | Tema y colores, Atajos, config.toml | siempre |
  | Barras | `bar`, `topbar`, `taskbar` |
  | Overviews | `overview` |
  | Dashboard | `dashboard` |
  | Workspaces | `bar`, `overview` (lo usan la barra y los overviews) |
  | Notificaciones | `notifications` |
  | Dock | `dock` |

  Dentro de «Barras», cada tarjeta (isla y barra, topbar, taskbar) solo aparece si su feature
  está compilada. Atajos lista solo los comandos cuya feature está compilada.
- **Escala local**: `scale(alto) = (alto / 720).clamp(1.0, 1.6)` para márgenes, filas, iconos y
  textos. Probado a 1366×768 y 1920×1080 (`target/ui-preview/config*.png`).
- **Colores solo de tokens**: `palette.*`, `Palette::scrim`, `sheen` y `with_alpha`.

### Páginas y datos reales

Todos los valores vienen de `state.shared.config` (tipo `Config`), salvo lo indicado.

| Página | Muestra | Clave |
|---|---|---|
| Tema y colores | esquema configurado (o «black-metal (por defecto)»), contraste WCAG, acento, overrides de token | `theme.scheme` / `theme.name`, `theme.contrast_correction`, `theme.accent`, `theme.surface`, `border`, `badge`, `text`, `muted` |
| Tema y colores | cuadrícula con los 22 tokens de `Palette` en uso (nombre, `#RRGGBB`, opacidad si es translúcido) | `theme::current()` |
| Barras · isla y barra | techo de expansión (Notch / Compacto / Barra / Panel) y tamaños | `bar.max_state`, `sizes.bar_h`, `panel_h`, `island_w`, `bar_w` |
| Barras · topbar | activa y altura | `topbar.enabled`, `topbar.height` |
| Barras · taskbar | activa, lado, grosor y autohide | `taskbar.enabled`, `side`, `size`, `autohide` |
| Overviews | lado y tamaño de las dos franjas; el tamaño muestra el porcentaje y los píxeles reales en esta pantalla | `overview.strip_h_side`, `strip_h_height`, `strip_v_side`, `strip_v_width` |
| Workspaces | número y nombre de cada workspace (o «sin nombre») | `workspaces.count`, `workspaces.names` |
| Notificaciones | tiempo en la isla | `notifications.ttl_secs` |
| Dock | activado y apps fijadas | `dock.enabled`, `dock.pinned` |
| config.toml | ruta (con `~`) y el texto del archivo, en monoespaciado y con scroll; comentarios, secciones y líneas en tonos distintos | archivo de `config::path()` |

Los interruptores y los selectores son **de solo lectura**: no responden al clic.

### config.toml

Se lee **una vez al abrir** el panel (`on_command`), no en cada frame. Si no existe, la página
dice «No hay config.toml: se usan los valores por defecto». Si no se puede leer, muestra el
error. Si el archivo cambia mientras el panel está abierto, no se ve hasta volver a abrirlo.

### Atajos

Lista de `dinamic-ui --<comando>` con su descripción. **Espejo a mano** de `Command` en
`src/control.rs` (`COMMANDS` en `config_panel.rs`, con su feature). Habría que derivarla de allí
(hoy no hay una lista de comandos exportada). Un test comprueba que los nombres son únicos, pero
no que coincidan con `control.rs`. No se inventan atajos de teclado: los keybinds de Hyprland no
están en este repositorio.

## Qué no es real (y por qué)

El mockup tiene controles que la configuración actual no tiene. No se muestran como si
funcionaran, y tampoco se inventa su valor:

- **Dashboard**: `config.toml` no tiene sección de dashboard. La página dice que su diseño y sus
  widgets se decidirán con `layout.toml` (Fase 6.3).
- **Overviews**: «estilo por defecto» y «animación de entrada» no existen en `OverviewConfig`.
- **Tema**: las tarjetas de «Paleta base16» del mockup se sustituyen por el esquema como texto;
  la lista de esquemas incluidos no está en la configuración.
- **Workspaces**: el interruptor «Nombres» del mockup no existe; los nombres se listan.
- **Notificaciones**: «No molestar», «Posición» y el estado del servidor freedesktop no están en
  la configuración.
- **Dock**: la zona de detección del autohide no está en la configuración.
- **Atajos de Hyprland** (`hyprland/dinamic-ui.conf`): no se leen.
- **Topbar**: los elementos visibles (`topbar.elements`) no existen en la configuración.

## Pendiente

- [ ] **Editar y guardar `config.toml` conservando comentarios** con `toml_edit`. Es el trabajo
      grande: tocar el archivo del usuario sin perder sus comentarios, validar antes de escribir,
      y recargar con `--reload`. Hoy la página es de solo lectura a propósito.
- [ ] **Validar el archivo** antes de mostrarlo como «válido»: hoy se muestra el texto tal cual,
      sin comprobar que el TOML parsea (el shell conserva la configuración anterior si falla).
- [ ] **Derivar `COMMANDS` de `src/control.rs`** en lugar de copiarla.
- [ ] **Añadir a la configuración** lo que el mockup prevé y hoy no existe (sección de dashboard,
      estilo por defecto de overview, elementos de la topbar, «No molestar» y posición de
      notificaciones), cada cosa con su consumidor. Sin consumidor, no entra en la página.
- [ ] **Recarga en vivo** al cambiar el archivo mientras el panel está abierto.
- [ ] **Acciones del sistema** (wifi, volumen, brillo): fuera del panel de ajustes.
- [ ] **Verificación en el R7** de lo que la vista previa no cubre: clics, el cierre al pulsar
      fuera y el desplazamiento de las páginas con la rueda.

## Verificación

- `cargo test config_panel`: 19 tests de la lógica pura (visibilidad por features, páginas,
  comandos, rects a 1366×768 y 1920×1080, navegación, formato de valores, lectura del archivo).
- `just preview`: `target/ui-preview/config.png` (1366×768), `config-1080p.png`, `config-paleta.png`
  (una pantalla más alta, para ver la paleta completa) y una imagen por página
  (`config-bars`, `config-overviews`, `config-dashboard`, `config-workspaces`,
  `config-notifications`, `config-dock`, `config-keys`, `config-toml`). Los datos son los de
  `config.example.toml` parseado con `Config`.
- `just lint-matrix` y `just lint-colors`: ver el informe de la sesión.
