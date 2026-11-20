# Tokens de diseño — compatibles con base16

Referencia: [metalelf0/base16-black-metal-scheme](https://github.com/metalelf0/base16-black-metal-scheme).
Implementación en `src/theme.rs`.

## Tres capas

| Capa | Contenido | Dónde |
|---|---|---|
| 1. Esquema | Los 16 slots `base00`–`base0F` | `theme.rs` (`Base16`) |
| 2. Tokens semánticos | `Palette`, derivada del esquema con la guía de estilo de base16 | `theme.rs` (`derive`) |
| 3. Tokens de componente | Radios, espaciados, tipografía, elevación, movimiento | `ui/mod.rs` *(pendiente de centralizar)* |

## Capa 1 — Esquemas

Incluidos los 11 de black-metal: `black-metal`, `bathory`, `burzum`, `dark-funeral`,
`gorgoroth`, `immortal`, `khold`, `marduk`, `mayhem`, `nile`, `venom`. Verificados contra los
YAML oficiales (0 diferencias). **En toda la familia solo cambian `base0A` y `base0B`**; los
otros 14 slots son idénticos.

Cualquier otro esquema base16 se carga desde un archivo (`scheme = "~/ruta/esquema.yaml"`):
YAML plano, YAML con `palette:` o TOML, con los 16 slots. Con slots incompletos se avisa en el
log y se usa `black-metal`.

## Capa 2 — Mapeo a tokens

| Token | Origen | Contraste sobre `surface` (black-metal) |
|---|---|---|
| `bg` | `base00` | — |
| `surface` | `base01` (alfa 238) | — |
| `badge`, `raised`, `selection` | `base02` | — |
| `faint` | `base03` | **1,5:1: solo pistas y separadores, nunca texto** |
| `muted` | `base04` | 6,6:1 |
| `text` | `base05` | 10,4:1 |
| `text_hi` | `base07` | 10,4:1 (en esta familia = `base05`) |
| `accent` | `base0B` (configurable) | 8,1:1 (`black-metal`) |
| `danger` / `warn` / `ok` / `info` | `base08` / `base09` / `base0B` / `base0D` | 4,7 / 8,1 / 8,1 / 5,3 |
| `border` | mezcla de `surface` hacia `text` (15 %) | 1,4:1 *(`base02` solo da 1,18:1)* |
| `track`, `hover`, `pressed` | mezcla de `surface` hacia `text` (12 / 6 / 12 %) | — |
| `accent_soft` | `accent` al 16 % de alfa | — |

**Hover, pulsado y borde se mezclan hacia `text`, no hacia blanco**, para que funcionen
también en esquemas claros.

### Decisiones

- **Estados (A):** slots base16 puros. En black-metal son grises y rosas (la familia es
  monocroma: no hay verde, rojo, azul ni ámbar), así que un indicador como la batería no
  debe depender solo del tono; debe leerse también por relleno o forma. Los verdes/ámbares de
  los mockups quedan fuera del esquema.
- **Corrección de contraste (B):** interruptor `contrast_correction`.
  - Apagado (por defecto): se respeta el esquema y solo se avisa en el log.
  - Encendido: `text`, `muted`, `text_hi` suben a 4,5:1 y `accent`, `danger`, `warn`, `ok`,
    `info` a 3:1, acercándolos al extremo legible (blanco en superficies oscuras, negro en
    claras). `faint` y `border` son decorativos y no se corrigen.
  - Caso real: `venom` y `immortal` con `base0A` como acento (1,9:1 y 3,2:1).
- **Acento (C):** `base0B` por defecto; `accent = "base0A"` (otro slot) o un hex.

## Configuración

```toml
[theme]
scheme = "black-metal"            # o "bathory", "venom"…, o una ruta a un esquema base16
contrast_correction = false
# accent = "base0B"               # slot o hex
# [theme.slots]
# base0B = "ddeecc"               # sustituye un slot del esquema
# surface / border / badge / text / muted = "rrggbb"   # override de token suelto
```

Compatibilidad: `name = "bathory"` sigue funcionando (alias de `scheme`), igual que
`DINAMIC_UI_THEME` y las claves de override históricas.

## Estado de la migración

- [x] Capa 1: esquemas incluidos, carga desde archivo, overrides de slot (`theme.rs`, con tests).
- [x] Capa 2: `Palette` con 22 tokens; los seis históricos se derivan del esquema.
- [x] Guarda de contraste y corrección opcional.
- [x] **`panel` y `wallpaper` migrados**: sin colores fijos. Los 13 `P_*` y los grises sueltos
      pasaron a tokens; `battery_tone` usa `info`/`danger`/`warn`/`ok`.
- [x] `just lint-colors` (parte de `just ci`): impide que `panel.rs`, `wallpaper.rs` y `mod.rs`
      vuelvan a pintar con literales.
- [ ] Pendientes de migrar (literales `Color32::from_*` restantes): `overview_collection` (15), `topbar` (9), `menu` (8), `dock` (7), `taskbar` (5), `bar` (3), `notifications` (2), `overview` (2).
      Al migrar un archivo, añadirlo a `lint-colors`.
- [ ] Capa 3: radios, espaciados, tipografía y movimiento centralizados.

### Cómo se usan los tokens

El render es de un solo hilo, así que la paleta del frame se fija una vez (`theme::set_current`
en `ui::draw`) y los helpers la leen con `pal()` en vez de recibirla por parámetro.

| Antes | Ahora |
|---|---|
| `P_SURFACE` / `P_INK` | `pal().solid()` |
| `P_BORDER`, `P_TRACK`, `P_ICON_BOX` | `pal().border`, `.track`, `.badge` |
| `P_TEXT`, `P_INVERT`, gris `dd` | `pal().text_hi` |
| `P_SOFT`, gris `bb` | `pal().text` |
| `P_MUTED`, `P_HEADING`, grises `88`/`99` | `pal().muted` |
| `P_DIM`, gris `66` | `pal().dim` |
| `P_TABBAR`, gris `16` | `pal().inset` |
| grises `33`/`44` | `pal().faint` |
| velos sobre miniaturas | `Palette::scrim(a)` / `Palette::sheen(a)` |

### Cambios visibles

- Los `P_*` no salían del esquema (`f5` es más brillante que `base07 = c1c1c1`): el texto del
  panel pasa a verse algo más tenue.
- `P_MUTED` (`77`), `P_HEADING` (`aa`) y `P_DIM` (`66`) eran tres niveles de gris; el esquema
  tiene dos (`base04` y `base03`, este último ilegible como texto). Se conservan `muted` y un
  `dim` derivado, así que `P_MUTED` y `P_HEADING` ahora son el mismo color.
- La batería ya no es verde/ámbar/rojo/azul: usa los slots de estado. En black-metal son grises
  y rosas, y el nivel se distingue sobre todo por el relleno del glifo.
- El panel (y `wallpaper`) cambian con el esquema y el acento, como el resto de la UI.
