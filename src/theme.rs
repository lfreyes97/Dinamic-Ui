//! Tema compatible con base16.
//!
//! Tres capas (ver `docs/DESIGN_TOKENS.md`):
//! 1. **Esquema**: los 16 slots `base00`–`base0F`. Vienen incluidos los de
//!    [base16-black-metal-scheme](https://github.com/metalelf0/base16-black-metal-scheme)
//!    o se carga cualquier esquema base16 desde un archivo.
//! 2. **Tokens semánticos** (`Palette`): derivados del esquema con la guía de estilo de base16.
//! 3. Tokens de componente (radios, espaciados…): viven en `ui/mod.rs`.
//!
//! Las claves antiguas (`surface`, `border`, `badge`, `text`, `muted`, `accent`) siguen
//! funcionando como override de token.

use std::{collections::HashMap, path::PathBuf, sync::Mutex};

use egui::Color32;

use crate::config;

/// Los 16 slots de un esquema base16, de `base00` a `base0F`.
pub type Base16 = [Color32; 16];

/// Tokens semánticos que consumen los componentes.
///
/// Los seis primeros son los históricos; el resto se adopta componente a componente.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// Fondo de barras y tarjetas (`base01`, con el alfa habitual de las superficies).
    pub surface: Color32,
    /// Borde fino: mezcla de `surface` hacia `text`, porque `base02` casi no se ve.
    pub border: Color32,
    /// Fondo de insignias y botones (`base02`).
    pub badge: Color32,
    /// Texto principal (`base05`).
    pub text: Color32,
    /// Texto secundario (`base04`).
    pub muted: Color32,
    /// Color de acento (`base0B` por defecto).
    pub accent: Color32,

    /// Fondo más profundo (`base00`).
    pub bg: Color32,
    /// Superficie elevada (`base02`).
    pub raised: Color32,
    /// Detalles tenues (`base03`): pistas y separadores, **nunca texto** (≈1,5:1).
    pub faint: Color32,
    /// Texto de más énfasis (`base07`).
    pub text_hi: Color32,
    /// Pista de barras y deslizadores.
    pub track: Color32,
    /// Selección (`base02`).
    pub selection: Color32,
    /// Superficie al pasar el cursor.
    pub hover: Color32,
    /// Superficie pulsada.
    pub pressed: Color32,
    /// Acento translúcido para fondos activos.
    pub accent_soft: Color32,
    /// Texto atenuado (placeholders, etiquetas apagadas): `muted` hacia `surface`.
    /// Decorativo: ≈3:1, por debajo del mínimo de texto.
    pub dim: Color32,
    /// Zonas hundidas (barra de pestañas, huecos), más oscuras que `surface`.
    pub inset: Color32,
    /// Relleno de las tarjetas flotantes (panel, menús).
    pub card_fill: Color32,
    /// Estados, según la convención base16 (`base08`, `base09`, `base0B`, `base0D`).
    pub danger: Color32,
    pub warn: Color32,
    pub ok: Color32,
    pub info: Color32,
}

const fn c(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Slots comunes a toda la familia black-metal; solo `base0A` y `base0B` cambian.
const BLACK_METAL_COMMON: [u32; 16] = [
    0x000000, 0x121212, 0x222222, 0x333333, 0x999999, 0xc1c1c1, 0x999999, 0xc1c1c1, 0x5f8787,
    0xaaaaaa, 0x000000, 0x000000, 0xaaaaaa, 0x888888, 0x999999, 0x444444,
];

/// (nombre, base0A, base0B) de cada variante de base16-black-metal-scheme.
const BLACK_METAL_VARIANTS: [(&str, u32, u32); 11] = [
    ("black-metal", 0xa06666, 0xdd9999),
    ("black-metal-bathory", 0xe78a53, 0xfbcb97),
    ("black-metal-burzum", 0x99bbaa, 0xddeecc),
    ("black-metal-dark-funeral", 0x5f81a5, 0xd0dfee),
    ("black-metal-gorgoroth", 0x8c7f70, 0x9b8d7f),
    ("black-metal-immortal", 0x556677, 0x7799bb),
    ("black-metal-khold", 0x974b46, 0xeceee3),
    ("black-metal-marduk", 0x626b67, 0xa5aaa7),
    ("black-metal-mayhem", 0xeecc6c, 0xf3ecd4),
    ("black-metal-nile", 0x777755, 0xaa9988),
    ("black-metal-venom", 0x79241f, 0xf8f7f2),
];

const DEFAULT_SCHEME: &str = "black-metal";

/// Esquemas incluidos en el binario.
pub fn builtin_scheme(name: &str) -> Option<Base16> {
    let wanted = normalize_name(name);
    let (_, a, b) = BLACK_METAL_VARIANTS.iter().find(|(n, _, _)| *n == wanted)?;
    let mut slots = BLACK_METAL_COMMON;
    slots[0x0A] = *a;
    slots[0x0B] = *b;
    Some(slots.map(c))
}

/// "Bathory", "bathory" y "black-metal-bathory" son el mismo esquema.
fn normalize_name(name: &str) -> String {
    let n = name.trim().to_lowercase().replace([' ', '_'], "-");
    if n == "black-metal" || n.starts_with("black-metal-") {
        n
    } else {
        format!("black-metal-{n}")
    }
}

// ── Carga de esquemas desde archivo ──────────────────────────────────────

/// Lee un esquema base16 en texto: YAML plano, YAML con `palette:` o TOML.
/// Hacen falta los 16 slots.
pub fn parse_scheme(text: &str) -> Option<Base16> {
    let mut slots: [Option<Color32>; 16] = [None; 16];
    for line in text.lines() {
        let line = line.trim();
        let Some(split) = line.find([':', '=']) else {
            continue;
        };
        let (key, value) = (line[..split].trim().trim_matches('"'), &line[split + 1..]);
        let Some(index) = slot_index(key) else {
            continue;
        };
        let value = value.trim();
        let token = match value.strip_prefix(['"', '\'']) {
            Some(rest) => rest.split(['"', '\'']).next().unwrap_or(""),
            None => value.split_whitespace().next().unwrap_or(""),
        };
        if let Some(color) = parse_hex(Some(token)) {
            slots[index] = Some(color);
        }
    }
    let mut out = [Color32::BLACK; 16];
    for (i, slot) in slots.iter().enumerate() {
        out[i] = (*slot)?;
    }
    Some(out)
}

/// "base0B", "base0b" o "0B" → 11.
fn slot_index(key: &str) -> Option<usize> {
    let key = key.trim().to_lowercase();
    let digits = key.strip_prefix("base").unwrap_or(&key);
    if digits.len() != 2 || !digits.starts_with('0') {
        return None;
    }
    usize::from_str_radix(digits, 16).ok().filter(|i| *i < 16)
}

fn expand_tilde(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}

fn is_path(spec: &str) -> bool {
    spec.contains('/')
        || spec.ends_with(".yaml")
        || spec.ends_with(".yml")
        || spec.ends_with(".toml")
}

/// Esquema pedido por la config; ante un fallo, avisa y usa el de por defecto.
fn resolve_scheme(spec: Option<&str>) -> Base16 {
    let default = || builtin_scheme(DEFAULT_SCHEME).expect("esquema por defecto");
    let Some(spec) = spec.map(str::trim).filter(|s| !s.is_empty()) else {
        return default();
    };
    if is_path(spec) {
        let path = expand_tilde(spec);
        return match std::fs::read_to_string(&path) {
            Ok(text) => parse_scheme(&text).unwrap_or_else(|| {
                warn_once(format!(
                    "esquema {}: faltan slots base00–base0F; se usa {DEFAULT_SCHEME}",
                    path.display()
                ));
                default()
            }),
            Err(e) => {
                warn_once(format!(
                    "esquema {}: {e}; se usa {DEFAULT_SCHEME}",
                    path.display()
                ));
                default()
            }
        };
    }
    builtin_scheme(spec).unwrap_or_else(|| {
        warn_once(format!(
            "esquema `{spec}` desconocido; se usa {DEFAULT_SCHEME}"
        ));
        default()
    })
}

// ── Color ────────────────────────────────────────────────────────────────

/// Mezcla opaca de `a` hacia `b` (`t` en 0..1).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let ch = |x: u8, y: u8| {
        (x as f32 + (y as f32 - x as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(ch(a.r(), b.r()), ch(a.g(), b.g()), ch(a.b(), b.b()))
}

fn linear(channel: u8) -> f32 {
    let c = channel as f32 / 255.0;
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Luminancia relativa WCAG.
pub fn luminance(color: Color32) -> f32 {
    0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
}

/// Razón de contraste WCAG entre dos colores (1..21).
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (hi, lo) = {
        let (la, lb) = (luminance(a), luminance(b));
        if la >= lb {
            (la, lb)
        } else {
            (lb, la)
        }
    };
    (hi + 0.05) / (lo + 0.05)
}

/// Acerca `color` al extremo legible sobre `surface` (blanco si es oscura, negro si es clara)
/// hasta alcanzar `min`. Si ya cumple, no lo toca.
pub fn correct_contrast(color: Color32, surface: Color32, min: f32) -> Color32 {
    if contrast(color, surface) >= min {
        return color;
    }
    let target = if luminance(surface) < 0.5 {
        Color32::WHITE
    } else {
        Color32::BLACK
    };
    (1..=20)
        .map(|step| mix(color, target, step as f32 * 0.05))
        .find(|candidate| contrast(*candidate, surface) >= min)
        .unwrap_or(target)
}

/// Mínimos WCAG: 4,5:1 para texto y 3:1 para elementos de interfaz.
const MIN_TEXT: f32 = 4.5;
const MIN_UI: f32 = 3.0;

fn warn_once(message: String) {
    static LAST: Mutex<Vec<String>> = Mutex::new(Vec::new());
    if let Ok(mut seen) = LAST.lock() {
        if !seen.contains(&message) {
            eprintln!("[dinamic-ui] tema: {message}");
            seen.push(message);
        }
    }
}

// ── Derivación de tokens ─────────────────────────────────────────────────

/// Deriva los tokens semánticos de un esquema (sin overrides ni corrección).
pub fn derive(scheme: &Base16) -> Palette {
    let s = |i: usize| scheme[i];
    let base01 = s(0x01);
    let mut p = Palette {
        surface: Color32::from_rgba_unmultiplied(base01.r(), base01.g(), base01.b(), 238),
        border: base01,
        badge: s(0x02),
        text: s(0x05),
        muted: s(0x04),
        accent: s(0x0B),
        bg: s(0x00),
        raised: s(0x02),
        faint: s(0x03),
        text_hi: s(0x07),
        track: base01,
        selection: s(0x02),
        hover: base01,
        pressed: base01,
        accent_soft: Color32::TRANSPARENT,
        dim: base01,
        inset: base01,
        card_fill: base01,
        danger: s(0x08),
        warn: s(0x09),
        ok: s(0x0B),
        info: s(0x0D),
    };
    finish(&mut p);
    p
}

/// Recalcula los tokens que se derivan de los primarios (`surface`, `text`, `muted`, `accent`,
/// `bg`). Hover, pulsado y borde se mezclan hacia `text`, no hacia blanco: valen también en
/// esquemas claros.
fn finish(p: &mut Palette) {
    let base = opaque(p.surface);
    p.border = mix(base, p.text, 0.15);
    p.track = mix(base, p.text, 0.12);
    p.hover = mix(base, p.text, 0.06);
    p.pressed = mix(base, p.text, 0.12);
    p.accent_soft = Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), 41);
    p.dim = mix(p.muted, base, 0.4);
    p.inset = mix(p.bg, base, 0.8);
    let card = mix(p.bg, base, 0.45);
    p.card_fill = Color32::from_rgba_unmultiplied(card.r(), card.g(), card.b(), 245);
}

thread_local! {
    static CURRENT: std::cell::Cell<Option<Palette>> = const { std::cell::Cell::new(None) };
}

/// Fija la paleta del frame en curso. El render es de un solo hilo: los helpers de dibujo la
/// leen con `current()` en vez de recibirla por parámetro en cada llamada.
pub fn set_current(palette: Palette) {
    CURRENT.with(|c| c.set(Some(palette)));
}

/// Paleta del frame en curso (el esquema por defecto si aún no se fijó, p. ej. en tests).
pub fn current() -> Palette {
    CURRENT
        .with(|c| c.get())
        .unwrap_or_else(|| derive(&builtin_scheme(DEFAULT_SCHEME).expect("esquema por defecto")))
}

impl Palette {
    /// `surface` sin transparencia (para rellenos que no deben dejar ver el fondo).
    pub fn solid(&self) -> Color32 {
        opaque(self.surface)
    }

    /// Un color con otro alfa (para brillos que dependen de una animación).
    pub fn with_alpha(color: Color32, alpha: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
    }

    /// Velo oscuro sobre una imagen (miniaturas, rótulos): no es color de tema.
    pub fn scrim(alpha: u8) -> Color32 {
        Color32::from_black_alpha(alpha)
    }

    /// Velo claro sobre una imagen (brillos y bordes sobre miniaturas).
    pub fn sheen(alpha: u8) -> Color32 {
        Color32::from_white_alpha(alpha)
    }
}

impl Palette {
    pub fn load() -> Self {
        let theme = config::get().theme.clone();
        let mut scheme = resolve_scheme(theme.scheme.as_deref().or(theme.name.as_deref()));
        apply_slot_overrides(&mut scheme, &theme.slots);

        let mut p = derive(&scheme);
        // Acento: un slot (`base0A`) o un hex. Por defecto `base0B`.
        if let Some(spec) = theme.accent.as_deref() {
            p.accent = match slot_index(spec) {
                Some(i) if spec.trim().to_lowercase().starts_with("base") => scheme[i],
                _ => parse_hex(Some(spec)).unwrap_or(p.accent),
            };
        }
        // Overrides de token (claves históricas).
        let surface_opaque = parse_hex(theme.surface.as_deref());
        if let Some(color) = surface_opaque {
            p.surface = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 238);
        }
        p.text = override_color(&theme.text, p.text);
        p.muted = override_color(&theme.muted, p.muted);
        p.badge = override_color(&theme.badge, p.badge);
        finish(&mut p);
        // Un override explícito de borde gana sobre el derivado.
        p.border = override_color(&theme.border, p.border);

        check_contrast(&mut p, theme.contrast_correction);
        p
    }
}

fn opaque(color: Color32) -> Color32 {
    Color32::from_rgb(color.r(), color.g(), color.b())
}

fn apply_slot_overrides(scheme: &mut Base16, slots: &HashMap<String, String>) {
    for (key, value) in slots {
        match (slot_index(key), parse_hex(Some(value))) {
            (Some(i), Some(color)) => scheme[i] = color,
            _ => warn_once(format!(
                "[theme.slots] `{key} = {value}` no es un slot válido"
            )),
        }
    }
}

/// Texto e indicadores frente a la superficie: corrige si el interruptor está activo,
/// y si no, solo avisa. `faint` y `border` son decorativos y quedan fuera.
fn check_contrast(p: &mut Palette, correct: bool) {
    let surface = opaque(p.surface);
    let check = |name: &str, color: &mut Color32, min: f32| {
        let ratio = contrast(*color, surface);
        if ratio >= min {
            return;
        }
        if correct {
            *color = correct_contrast(*color, surface, min);
        } else {
            warn_once(format!(
                "`{name}` tiene contraste {ratio:.1}:1 (< {min}:1) sobre la superficie; \
                 activa `contrast_correction` para corregirlo"
            ));
        }
    };
    check("text", &mut p.text, MIN_TEXT);
    check("muted", &mut p.muted, MIN_TEXT);
    check("text_hi", &mut p.text_hi, MIN_TEXT);
    check("accent", &mut p.accent, MIN_UI);
    check("danger", &mut p.danger, MIN_UI);
    check("warn", &mut p.warn, MIN_UI);
    check("ok", &mut p.ok, MIN_UI);
    check("info", &mut p.info, MIN_UI);
    if correct {
        let border = p.border;
        finish(p);
        p.border = border;
    }
}

fn override_color(value: &Option<String>, base: Color32) -> Color32 {
    parse_hex(value.as_deref()).unwrap_or(base)
}

pub(crate) fn parse_hex(value: Option<&str>) -> Option<Color32> {
    let hex = value?.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color32::from_rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(c: Color32) -> String {
        format!("{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
    }

    #[test]
    fn builtin_black_metal_matches_the_published_scheme() {
        let s = builtin_scheme("black-metal").unwrap();
        let got: Vec<String> = s.iter().map(|c| hex(*c)).collect();
        assert_eq!(
            got,
            [
                "000000", "121212", "222222", "333333", "999999", "c1c1c1", "999999", "c1c1c1",
                "5f8787", "aaaaaa", "a06666", "dd9999", "aaaaaa", "888888", "999999", "444444"
            ]
        );
    }

    #[test]
    fn only_base0a_and_base0b_vary_across_variants() {
        let reference = builtin_scheme("black-metal").unwrap();
        for (name, a, b) in BLACK_METAL_VARIANTS {
            let s = builtin_scheme(name).unwrap();
            for i in (0..16).filter(|i| *i != 0x0A && *i != 0x0B) {
                assert_eq!(s[i], reference[i], "{name} slot {i}");
            }
            assert_eq!(hex(s[0x0A]), format!("{a:06x}"));
            assert_eq!(hex(s[0x0B]), format!("{b:06x}"));
        }
    }

    #[test]
    fn short_names_resolve() {
        assert_eq!(
            builtin_scheme("Bathory"),
            builtin_scheme("black-metal-bathory")
        );
        assert_eq!(
            builtin_scheme("dark funeral"),
            builtin_scheme("black-metal-dark-funeral")
        );
        assert!(builtin_scheme("no-existe").is_none());
    }

    const FLAT: &str = r##"scheme: "Test"
author: "x"
base00: "000000"
base01: "#121212"
base02: '222222'
base03: 333333 # comentario
base04: "999999"
base05: "c1c1c1"
base06: "999999"
base07: "c1c1c1"
base08: "5f8787"
base09: "aaaaaa"
base0A: "a06666"
base0B: "dd9999"
base0C: "aaaaaa"
base0D: "888888"
base0E: "999999"
base0F: "444444"
"##;

    #[test]
    fn parses_flat_nested_and_toml_formats() {
        let flat = parse_scheme(FLAT).unwrap();
        assert_eq!(flat, builtin_scheme("black-metal").unwrap());

        let nested = FLAT
            .replace(
                "scheme: \"Test\"\nauthor: \"x\"\n",
                "system: \"base16\"\npalette:\n",
            )
            .replace("base", "  base");
        assert_eq!(parse_scheme(&nested).unwrap(), flat);

        let toml = FLAT.replace(": ", " = ");
        assert_eq!(parse_scheme(&toml).unwrap(), flat);
    }

    #[test]
    fn incomplete_scheme_is_rejected() {
        assert!(parse_scheme("base00: \"000000\"\nbase01: \"121212\"\n").is_none());
    }

    #[test]
    fn derives_semantic_tokens_from_the_guide() {
        let p = derive(&builtin_scheme("black-metal").unwrap());
        assert_eq!(hex(p.text), "c1c1c1"); // base05
        assert_eq!(hex(p.muted), "999999"); // base04
        assert_eq!(hex(p.accent), "dd9999"); // base0B
        assert_eq!(hex(p.badge), "222222"); // base02
        assert_eq!(hex(p.bg), "000000"); // base00
        assert_eq!(hex(p.danger), "5f8787"); // base08
                                             // El borde se mezcla hacia el texto: más visible que base02 (1,18:1).
        assert!(contrast(p.border, opaque(p.surface)) > 1.3);
    }

    #[test]
    fn contrast_matches_known_values() {
        let surface = c(0x121212);
        assert!((contrast(c(0xc1c1c1), surface) - 10.4).abs() < 0.1);
        assert!((contrast(c(0x333333), surface) - 1.48).abs() < 0.02);
    }

    #[test]
    fn correction_lifts_failing_colors_and_leaves_passing_ones() {
        let surface = c(0x121212);
        let venom_0a = c(0x79241f); // 1,9:1
        let fixed = correct_contrast(venom_0a, surface, MIN_UI);
        assert!(contrast(fixed, surface) >= MIN_UI);
        assert_ne!(fixed, venom_0a);
        assert_eq!(
            correct_contrast(c(0xc1c1c1), surface, MIN_TEXT),
            c(0xc1c1c1)
        );
    }

    #[test]
    fn correction_goes_toward_black_on_light_surfaces() {
        let light = c(0xf0f0f0);
        let fixed = correct_contrast(c(0xcccccc), light, MIN_TEXT);
        assert!(contrast(fixed, light) >= MIN_TEXT);
        assert!(luminance(fixed) < luminance(c(0xcccccc)));
    }

    #[test]
    fn hover_mixes_toward_text_so_light_schemes_work() {
        // Esquema claro: base00 claro, base05 oscuro.
        let mut light = builtin_scheme("black-metal").unwrap();
        light[0x01] = c(0xeeeeee);
        light[0x05] = c(0x222222);
        let p = derive(&light);
        assert!(luminance(p.hover) < luminance(c(0xeeeeee)));
    }

    #[test]
    fn panel_tokens_keep_their_visual_hierarchy() {
        let p = derive(&builtin_scheme("black-metal").unwrap());
        let surface = p.solid();
        // texto > muted > dim, y dim sigue siendo legible como etiqueta apagada (~3:1).
        assert!(contrast(p.text_hi, surface) >= contrast(p.muted, surface));
        assert!(contrast(p.muted, surface) > contrast(p.dim, surface));
        assert!(contrast(p.dim, surface) > 2.5);
        // `faint` queda por debajo de cualquier texto.
        assert!(contrast(p.faint, surface) < 2.0);
        // Zonas hundidas más oscuras que la superficie; tarjeta casi opaca.
        assert!(luminance(p.inset) <= luminance(surface));
        assert_eq!(p.card_fill.a(), 245);
    }

    #[test]
    fn correction_keeps_derived_tokens_in_sync() {
        let mut scheme = builtin_scheme("black-metal-venom").unwrap();
        scheme[0x0B] = c(0x79241f); // acento que falla (1,9:1)
        let mut p = derive(&scheme);
        check_contrast(&mut p, true);
        assert!(contrast(p.accent, p.solid()) >= MIN_UI);
        let expected =
            Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), 41);
        assert_eq!(p.accent_soft, expected);
    }

    #[test]
    fn current_palette_roundtrip() {
        let mut p = derive(&builtin_scheme("black-metal-bathory").unwrap());
        p.accent = c(0x123456);
        set_current(p);
        assert_eq!(hex(current().accent), "123456");
    }

    #[test]
    fn slot_index_accepts_common_spellings() {
        assert_eq!(slot_index("base0B"), Some(11));
        assert_eq!(slot_index("base0b"), Some(11));
        assert_eq!(slot_index("0B"), Some(11));
        assert_eq!(slot_index("base10"), None);
        assert_eq!(slot_index("accent"), None);
    }
}
