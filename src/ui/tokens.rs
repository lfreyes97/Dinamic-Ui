//! Capa 3 de `docs/DESIGN_TOKENS.md`: radios, espaciados y escala.
//!
//! Las capas 1 y 2 (esquema base16 y `Palette`) viven en `theme.rs`. Aquí solo
//! están las medidas que antes se escribían a mano en cada componente.

/// Escala de la UI según la altura de la pantalla. A 720 px es 1, hasta 1,6.
pub fn scale(screen_h: f32) -> f32 {
    (screen_h / 720.0).clamp(1.0, 1.6)
}

/// Radios canónicos en px a 720p. Sustituyen los `CornerRadius::same(4/7/10/14/18)`
/// que había repartidos por los componentes.
pub struct Radius;

impl Radius {
    pub const XS: u8 = 4;
    pub const SM: u8 = 7;
    pub const MD: u8 = 10;
    pub const LG: u8 = 14;

    /// Radio escalado y acotado a `u8`, que es lo que espera egui.
    pub fn scaled(base: u8, scale: f32) -> u8 {
        ((base as f32 * scale).round() as i32).clamp(0, u8::MAX as i32) as u8
    }
}

/// Espaciados canónicos en px a 720p. Alias de `SP_S/M/L` en `ui/mod.rs`.
pub struct Space;

impl Space {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 6.0;
    pub const MD: f32 = 8.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_is_clamped_between_one_and_one_and_a_half() {
        assert_eq!(scale(720.0), 1.0);
        assert_eq!(scale(360.0), 1.0);
        assert!((scale(1080.0) - 1.5).abs() < 1e-6);
        assert_eq!(scale(4000.0), 1.6);
    }

    #[test]
    fn radii_scale_and_stay_in_u8() {
        assert_eq!(Radius::scaled(Radius::MD, 1.0), 10);
        assert_eq!(Radius::scaled(Radius::MD, 1.5), 15);
        assert_eq!(Radius::scaled(Radius::LG, 100.0), u8::MAX);
    }
}
