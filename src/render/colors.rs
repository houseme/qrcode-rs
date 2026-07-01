//! Common color types and conversions for rendering.
//!
//! This module provides convenience types and conversion functions that work
//! across multiple render backends.
//!
//! # Supported formats
//!
//! | Format    | Type        | Example                    |
//! |-----------|-------------|----------------------------|
//! | RGB       | `(u8,u8,u8)`| `(255, 0, 128)`           |
//! | RGBA      | `(u8,u8,u8,u8)` | `(255, 0, 128, 200)` |
//! | Hex       | `&str`      | `"#ff0080"`               |
//! | Named CSS | `&str`      | `"red"`, `"transparent"`  |
//!
//! # Example
//!
//! ```
//! use qrcode_rs::render::colors::hex_to_rgb;
//!
//! // Works under any feature combination (no renderer backend required).
//! assert_eq!(hex_to_rgb("#1a1a2e"), Some((0x1a, 0x1a, 0x2e)));
//! assert_eq!(hex_to_rgb("invalid"), None);
//! ```

/// Parses a `#rrggbb` or `#rrggbbaa` hex color string into RGB or RGBA bytes.
///
/// Returns `None` if the format is invalid.
///
/// # Example
///
/// ```
/// use qrcode_rs::render::colors::hex_to_rgb;
/// assert_eq!(hex_to_rgb("#ff0080"), Some((255, 0, 128)));
/// assert_eq!(hex_to_rgb("#000"), Some((0, 0, 0)));
/// assert_eq!(hex_to_rgb("invalid"), None);
/// ```
pub fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some((r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((r, g, b))
        }
        _ => None,
    }
}

/// Parses a `#rrggbb` or `#rrggbbaa` hex color string into RGBA bytes.
///
/// For `#rrggbb` format, alpha defaults to 255 (fully opaque).
///
/// # Example
///
/// ```
/// use qrcode_rs::render::colors::hex_to_rgba;
/// assert_eq!(hex_to_rgba("#ff0080"), Some((255, 0, 128, 255)));
/// assert_eq!(hex_to_rgba("#ff008080"), Some((255, 0, 128, 128)));
/// assert_eq!(hex_to_rgba("invalid"), None);
/// ```
pub fn hex_to_rgba(hex: &str) -> Option<(u8, u8, u8, u8)> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    match hex.len() {
        3 | 6 => hex_to_rgb(hex).map(|(r, g, b)| (r, g, b, 255)),
        4 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            let a = u8::from_str_radix(&hex[3..4], 16).ok()? * 17;
            Some((r, g, b, a))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some((r, g, b, a))
        }
        _ => None,
    }
}

/// Converts RGB bytes to a CSS hex string (e.g., `"#ff0080"`).
///
/// # Example
///
/// ```
/// use qrcode_rs::render::colors::rgb_to_hex;
/// assert_eq!(rgb_to_hex(255, 0, 128), "#ff0080");
/// ```
pub fn rgb_to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Converts RGBA bytes to a CSS hex string (e.g., `"#ff0080c0"`).
///
/// # Example
///
/// ```
/// use qrcode_rs::render::colors::rgba_to_hex;
/// assert_eq!(rgba_to_hex(255, 0, 128, 192), "#ff0080c0");
/// ```
pub fn rgba_to_hex(r: u8, g: u8, b: u8, a: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
}

/// Converts RGB bytes to a `rgb()` CSS function string.
///
/// # Example
///
/// ```
/// use qrcode_rs::render::colors::rgb_to_css;
/// assert_eq!(rgb_to_css(255, 0, 128), "rgb(255,0,128)");
/// ```
pub fn rgb_to_css(r: u8, g: u8, b: u8) -> String {
    format!("rgb({r},{g},{b})")
}

/// Converts RGBA bytes to a `rgba()` CSS function string.
///
/// # Example
///
/// ```
/// use qrcode_rs::render::colors::rgba_to_css;
/// assert_eq!(rgba_to_css(255, 0, 128, 128), "rgba(255,0,128,128)");
/// ```
pub fn rgba_to_css(r: u8, g: u8, b: u8, a: u8) -> String {
    format!("rgba({r},{g},{b},{a})")
}

/// A unified sRGBA color type for use across all render backends.
///
/// `Srgba` provides a single type that can create colors for any renderer:
/// SVG (CSS hex), ANSI (escape codes), EPS/PDF (0.0–1.0 arrays), image (`Rgba<u8>`).
///
/// # Example
///
/// ```
/// use qrcode_rs::render::colors::Srgba;
///
/// let c = Srgba::from_hex("#ff0080").unwrap();
/// assert_eq!(c.to_hex(), "#ff0080");
/// assert_eq!(c.to_css(), "rgba(255,0,128,255)");
/// let arr = c.to_array();
/// assert!((arr[0] - 1.0).abs() < 0.001);
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Srgba {
    /// Red component.
    pub r: u8,
    /// Green component.
    pub g: u8,
    /// Blue component.
    pub b: u8,
    /// Alpha component (0 = transparent, 255 = opaque).
    pub a: u8,
}

impl Srgba {
    /// Creates a new sRGBA color.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Creates a fully opaque sRGBA color.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Parses a hex color string (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`).
    pub fn from_hex(hex: &str) -> Option<Self> {
        let (r, g, b, a) = hex_to_rgba(hex)?;
        Some(Self { r, g, b, a })
    }

    /// Converts to a CSS hex string (e.g., `"#ff0080"` or `"#ff0080c0"` if alpha < 255).
    pub fn to_hex(self) -> String {
        if self.a == 255 { rgb_to_hex(self.r, self.g, self.b) } else { rgba_to_hex(self.r, self.g, self.b, self.a) }
    }

    /// Converts to a CSS `rgba()` function string.
    pub fn to_css(self) -> String {
        rgba_to_css(self.r, self.g, self.b, self.a)
    }

    /// Converts to an ANSI foreground escape sequence (`\x1b[38;2;R;G;Bm`).
    pub fn to_ansi_fg(self) -> String {
        format!("\x1b[38;2;{};{};{}m", self.r, self.g, self.b)
    }

    /// Converts to an ANSI background escape sequence (`\x1b[48;2;R;G;Bm`).
    pub fn to_ansi_bg(self) -> String {
        format!("\x1b[48;2;{};{};{}m", self.r, self.g, self.b)
    }

    /// Converts to a `[f64; 3]` array with values in 0.0–1.0, suitable for EPS/PDF renderers.
    pub fn to_array(self) -> [f64; 3] {
        [self.r as f64 / 255.0, self.g as f64 / 255.0, self.b as f64 / 255.0]
    }

    /// Linearly interpolates between `self` and `other`.
    ///
    /// `t = 0.0` returns `self`, `t = 1.0` returns `other`.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let inv = 1.0 - t;
        Self {
            r: (self.r as f32 * inv + other.r as f32 * t) as u8,
            g: (self.g as f32 * inv + other.g as f32 * t) as u8,
            b: (self.b as f32 * inv + other.b as f32 * t) as u8,
            a: (self.a as f32 * inv + other.a as f32 * t) as u8,
        }
    }
}

impl From<(u8, u8, u8)> for Srgba {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self::rgb(r, g, b)
    }
}

impl From<(u8, u8, u8, u8)> for Srgba {
    fn from((r, g, b, a): (u8, u8, u8, u8)) -> Self {
        Self::new(r, g, b, a)
    }
}

impl From<Srgba> for (u8, u8, u8, u8) {
    fn from(c: Srgba) -> Self {
        (c.r, c.g, c.b, c.a)
    }
}

#[cfg(feature = "image")]
impl From<Srgba> for image::Rgba<u8> {
    fn from(c: Srgba) -> Self {
        image::Rgba([c.r, c.g, c.b, c.a])
    }
}

#[cfg(feature = "image")]
impl From<image::Rgba<u8>> for Srgba {
    fn from(p: image::Rgba<u8>) -> Self {
        Self::new(p.0[0], p.0[1], p.0[2], p.0[3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_rgb_6_digit() {
        assert_eq!(hex_to_rgb("#ff0080"), Some((255, 0, 128)));
        assert_eq!(hex_to_rgb("#000000"), Some((0, 0, 0)));
        assert_eq!(hex_to_rgb("#ffffff"), Some((255, 255, 255)));
    }

    #[test]
    fn test_hex_to_rgb_3_digit() {
        assert_eq!(hex_to_rgb("#f08"), Some((255, 0, 136)));
        assert_eq!(hex_to_rgb("#000"), Some((0, 0, 0)));
        assert_eq!(hex_to_rgb("#fff"), Some((255, 255, 255)));
    }

    #[test]
    fn test_hex_to_rgb_no_hash() {
        assert_eq!(hex_to_rgb("ff0080"), Some((255, 0, 128)));
    }

    #[test]
    fn test_hex_to_rgb_invalid() {
        assert_eq!(hex_to_rgb("invalid"), None);
        assert_eq!(hex_to_rgb("#gg0000"), None);
        assert_eq!(hex_to_rgb("#12345"), None);
    }

    #[test]
    fn test_hex_to_rgba_6_digit() {
        assert_eq!(hex_to_rgba("#ff0080"), Some((255, 0, 128, 255)));
    }

    #[test]
    fn test_hex_to_rgba_8_digit() {
        assert_eq!(hex_to_rgba("#ff008080"), Some((255, 0, 128, 128)));
    }

    #[test]
    fn test_hex_to_rgba_4_digit() {
        assert_eq!(hex_to_rgba("#f08c"), Some((255, 0, 136, 204)));
    }

    #[test]
    fn test_rgb_to_hex() {
        assert_eq!(rgb_to_hex(255, 0, 128), "#ff0080");
        assert_eq!(rgb_to_hex(0, 0, 0), "#000000");
    }

    #[test]
    fn test_rgba_to_hex() {
        assert_eq!(rgba_to_hex(255, 0, 128, 192), "#ff0080c0");
    }

    #[test]
    fn test_rgb_to_css() {
        assert_eq!(rgb_to_css(255, 0, 128), "rgb(255,0,128)");
    }

    #[test]
    fn test_rgba_to_css() {
        assert_eq!(rgba_to_css(255, 0, 128, 128), "rgba(255,0,128,128)");
    }

    #[test]
    fn test_roundtrip() {
        let (r, g, b) = (42, 128, 255);
        let hex = rgb_to_hex(r, g, b);
        assert_eq!(hex_to_rgb(&hex), Some((r, g, b)));
    }

    #[test]
    fn test_srgba_from_hex() {
        let c = Srgba::from_hex("#ff0080").unwrap();
        assert_eq!(c, Srgba::new(255, 0, 128, 255));
    }

    #[test]
    fn test_srgba_from_hex_with_alpha() {
        let c = Srgba::from_hex("#ff0080c0").unwrap();
        assert_eq!(c, Srgba::new(255, 0, 128, 192));
    }

    #[test]
    fn test_srgba_to_hex() {
        assert_eq!(Srgba::rgb(255, 0, 128).to_hex(), "#ff0080");
        assert_eq!(Srgba::new(255, 0, 128, 192).to_hex(), "#ff0080c0");
    }

    #[test]
    fn test_srgba_to_css() {
        assert_eq!(Srgba::rgb(255, 0, 128).to_css(), "rgba(255,0,128,255)");
    }

    #[test]
    fn test_srgba_to_ansi() {
        let c = Srgba::rgb(0, 51, 102);
        assert_eq!(c.to_ansi_fg(), "\x1b[38;2;0;51;102m");
        assert_eq!(c.to_ansi_bg(), "\x1b[48;2;0;51;102m");
    }

    #[test]
    fn test_srgba_to_array() {
        let c = Srgba::rgb(255, 0, 128);
        let arr = c.to_array();
        assert!((arr[0] - 1.0).abs() < 0.001);
        assert!((arr[1] - 0.0).abs() < 0.001);
        assert!((arr[2] - 0.502).abs() < 0.01);
    }

    #[test]
    fn test_srgba_lerp() {
        let a = Srgba::rgb(0, 0, 0);
        let b = Srgba::rgb(255, 255, 255);
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
        let mid = a.lerp(b, 0.5);
        assert_eq!(mid.r, 127);
    }

    #[test]
    fn test_srgba_from_tuple() {
        let c: Srgba = (255, 0, 128).into();
        assert_eq!(c, Srgba::rgb(255, 0, 128));
    }
}
