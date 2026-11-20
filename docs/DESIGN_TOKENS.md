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
- [x] Capa 2: `Palette` ampliada de 6 a 19 tokens; los seis históricos se derivan del esquema.
- [x] Guarda de contraste y corrección opcional.
- [ ] Capa 3: radios, espaciados, tipografía y movimiento centralizados.
- [ ] **Migrar componentes a tokens.** Hoy `panel` usa la paleta 1 vez y `wallpaper` 0 veces: pintan
      con grises fijos (`P_*`, `Color32::from_*`). Ahí el tema aún no llega.
- [ ] Regla de revisión: sin `Color32::from_*` fuera de `theme.rs` (grep en `just lint`).

### Cambio visible que traerá la migración del panel

Los `P_*` actuales no salen del esquema (`f5` es más brillante que `base07 = c1c1c1`). Al
adoptar tokens, el panel pasará a verse algo más tenue y a teñirse con el acento, como el
resto de la UI.
