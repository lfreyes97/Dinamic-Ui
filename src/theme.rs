//! Paletas base16 (metalelf0/base16-black-metal-scheme) con overrides de config.

use egui::Color32;

use crate::config;

#[derive(Clone, Copy)]
pub struct Palette {
    pub surface: Color32,
    pub border: Color32,
    pub badge: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
}

const fn c(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn black_metal() -> Palette {
    Palette {
        surface: Color32::from_rgba_unmultiplied(0x12, 0x12, 0x12, 238),
        border: c(0x303030),
        badge: c(0x242424),
        text: c(0xc1c1c1),
        muted: c(0x999999),
        accent: c(0xdd9999),
    }
}

fn bathory() -> Palette {
    Palette {
        surface: Color32::from_rgba_unmultiplied(0x12, 0x12, 0x12, 238),
        border: c(0x303030),
        badge: c(0x242424),
        text: c(0xc1c1c1),
        muted: c(0x999999),
        accent: c(0xfbcb97),
    }
}

impl Palette {
    pub fn load() -> Self {
        let config = config::get();
        let mut palette = match config.theme.name.as_deref() {
            Some("bathory") => bathory(),
            _ => black_metal(),
        };
        palette.border = override_color(&config.theme.border, palette.border);
        palette.badge = override_color(&config.theme.badge, palette.badge);
        palette.text = override_color(&config.theme.text, palette.text);
        palette.muted = override_color(&config.theme.muted, palette.muted);
        palette.accent = override_color(&config.theme.accent, palette.accent);
        // El surface se especifica sin alfa; se lo devolvemos.
        if let Some(color) = parse_hex(config.theme.surface.as_deref()) {
            palette.surface = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 238);
        }
        palette
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
