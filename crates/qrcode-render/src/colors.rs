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
//! use qrcode_render::colors::hex_to_rgb;
//!
//! // Works under any feature combination (no renderer backend required).
//! assert_eq!(hex_to_rgb("#1a1a2e"), Some((0x1a, 0x1a, 0x2e)));
//! assert_eq!(hex_to_rgb("invalid"), None);
//! ```

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::{
    borrow::ToOwned,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

/// Parses a `#rrggbb` or `#rrggbbaa` hex color string into RGB or RGBA bytes.
///
/// Returns `None` if the format is invalid.
///
/// # Example
///
/// ```
/// use qrcode_render::colors::hex_to_rgb;
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
/// use qrcode_render::colors::hex_to_rgba;
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
/// use qrcode_render::colors::rgb_to_hex;
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
/// use qrcode_render::colors::rgba_to_hex;
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
/// use qrcode_render::colors::rgb_to_css;
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
/// use qrcode_render::colors::rgba_to_css;
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
/// use qrcode_render::colors::Srgba;
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

/// A color space that can convert to and from sRGB.
///
/// Render backends can accept a concrete color space and lower it to their
/// native paint operators. RGB-oriented outputs keep using [`RgbColor`], while
/// print-oriented outputs such as EPS/PDF can preserve [`CmykColor`].
pub trait ColorSpace: Copy + Sized {
    /// Component storage used by this color space.
    type Component: Copy;

    /// Converts this color to sRGB.
    fn to_rgb(self) -> RgbColor;

    /// Converts an sRGB color into this color space.
    fn from_rgb(rgb: RgbColor) -> Self;
}

/// An opaque sRGB color.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RgbColor {
    /// Red component.
    pub r: u8,
    /// Green component.
    pub g: u8,
    /// Blue component.
    pub b: u8,
}

impl RgbColor {
    /// Creates an opaque sRGB color.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Converts this color to normalized RGB components.
    pub fn to_array(self) -> [f64; 3] {
        [self.r as f64 / 255.0, self.g as f64 / 255.0, self.b as f64 / 255.0]
    }

    /// Converts this color to an opaque [`Srgba`].
    pub const fn to_srgba(self) -> Srgba {
        Srgba::rgb(self.r, self.g, self.b)
    }
}

impl ColorSpace for RgbColor {
    type Component = u8;

    fn to_rgb(self) -> RgbColor {
        self
    }

    fn from_rgb(rgb: RgbColor) -> Self {
        rgb
    }
}

impl From<(u8, u8, u8)> for RgbColor {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self::new(r, g, b)
    }
}

impl From<Srgba> for RgbColor {
    fn from(color: Srgba) -> Self {
        Self::new(color.r, color.g, color.b)
    }
}

impl From<RgbColor> for Srgba {
    fn from(color: RgbColor) -> Self {
        color.to_srgba()
    }
}

/// A CMYK color with normalized `0.0..=1.0` components.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct CmykColor {
    /// Cyan component.
    pub c: f64,
    /// Magenta component.
    pub m: f64,
    /// Yellow component.
    pub y: f64,
    /// Key/black component.
    pub k: f64,
}

impl CmykColor {
    /// Creates a CMYK color, clamping all components to `0.0..=1.0`.
    pub fn new(c: f64, m: f64, y: f64, k: f64) -> Self {
        Self { c: clamp_unit(c), m: clamp_unit(m), y: clamp_unit(y), k: clamp_unit(k) }
    }

    /// Converts this color to normalized CMYK components.
    pub fn to_array(self) -> [f64; 4] {
        [self.c, self.m, self.y, self.k]
    }
}

impl ColorSpace for CmykColor {
    type Component = f64;

    fn to_rgb(self) -> RgbColor {
        let r = (1.0 - self.c) * (1.0 - self.k);
        let g = (1.0 - self.m) * (1.0 - self.k);
        let b = (1.0 - self.y) * (1.0 - self.k);
        RgbColor::new(unit_to_u8(r), unit_to_u8(g), unit_to_u8(b))
    }

    fn from_rgb(rgb: RgbColor) -> Self {
        let r = rgb.r as f64 / 255.0;
        let g = rgb.g as f64 / 255.0;
        let b = rgb.b as f64 / 255.0;
        let k = 1.0 - r.max(g).max(b);
        if k >= 1.0 {
            return Self::new(0.0, 0.0, 0.0, 1.0);
        }
        let denom = 1.0 - k;
        Self::new((1.0 - r - k) / denom, (1.0 - g - k) / denom, (1.0 - b - k) / denom, k)
    }
}

impl From<RgbColor> for CmykColor {
    fn from(color: RgbColor) -> Self {
        Self::from_rgb(color)
    }
}

impl From<CmykColor> for RgbColor {
    fn from(color: CmykColor) -> Self {
        color.to_rgb()
    }
}

/// A CIE L*a*b* color using a D65 white point approximation.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct LabColor {
    /// Lightness.
    pub l: f64,
    /// Green-red axis.
    pub a: f64,
    /// Blue-yellow axis.
    pub b: f64,
}

impl LabColor {
    /// Creates a Lab color.
    pub const fn new(l: f64, a: f64, b: f64) -> Self {
        Self { l, a, b }
    }
}

impl ColorSpace for LabColor {
    type Component = f64;

    fn to_rgb(self) -> RgbColor {
        let fy = (self.l + 16.0) / 116.0;
        let fx = fy + self.a / 500.0;
        let fz = fy - self.b / 200.0;
        let x = lab_inv(fx) * 0.95047;
        let y = lab_inv(fy);
        let z = lab_inv(fz) * 1.08883;
        let r = 3.2404542 * x - 1.5371385 * y - 0.4985314 * z;
        let g = -0.9692660 * x + 1.8760108 * y + 0.0415560 * z;
        let b = 0.0556434 * x - 0.2040259 * y + 1.0572252 * z;
        RgbColor::new(unit_to_u8(r), unit_to_u8(g), unit_to_u8(b))
    }

    fn from_rgb(rgb: RgbColor) -> Self {
        let r = rgb.r as f64 / 255.0;
        let g = rgb.g as f64 / 255.0;
        let b = rgb.b as f64 / 255.0;
        let x = (0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047;
        let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
        let z = (0.0193339 * r + 0.1191920 * g + 0.9503041 * b) / 1.08883;
        let fx = lab_f(x);
        let fy = lab_f(y);
        let fz = lab_f(z);
        Self::new(116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
    }
}

impl From<RgbColor> for LabColor {
    fn from(color: RgbColor) -> Self {
        Self::from_rgb(color)
    }
}

impl From<LabColor> for RgbColor {
    fn from(color: LabColor) -> Self {
        color.to_rgb()
    }
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

fn clamp_unit(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn unit_to_u8(value: f64) -> u8 {
    (clamp_unit(value) * 255.0 + 0.5) as u8
}

fn lab_f(value: f64) -> f64 {
    if value > 0.008856 { cube_root(value) } else { 7.787 * value + 16.0 / 116.0 }
}

fn lab_inv(value: f64) -> f64 {
    let cube = value * value * value;
    if cube > 0.008856 { cube } else { (value - 16.0 / 116.0) / 7.787 }
}

fn cube_root(value: f64) -> f64 {
    if value <= 0.0 {
        return 0.0;
    }
    let mut x = value.max(1.0);
    for _ in 0..12 {
        x = (2.0 * x + value / (x * x)) / 3.0;
    }
    x
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

    #[test]
    fn rgb_color_space_round_trips() {
        let rgb = RgbColor::new(51, 102, 153);

        assert_eq!(RgbColor::from_rgb(rgb), rgb);
        assert_eq!(rgb.to_rgb(), rgb);
        assert_eq!(rgb.to_srgba(), Srgba::rgb(51, 102, 153));
        assert_eq!(rgb.to_array(), [0.2, 0.4, 0.6]);
    }

    #[test]
    fn cmyk_color_space_converts_to_rgb() {
        assert_eq!(CmykColor::new(0.0, 0.0, 0.0, 1.0).to_rgb(), RgbColor::new(0, 0, 0));
        assert_eq!(CmykColor::new(0.0, 1.0, 1.0, 0.0).to_rgb(), RgbColor::new(255, 0, 0));

        let cmyk = CmykColor::from_rgb(RgbColor::new(51, 102, 153));
        assert_eq!(cmyk.to_rgb(), RgbColor::new(51, 102, 153));
    }

    #[test]
    fn lab_color_space_handles_neutral_extremes() {
        let white = LabColor::from_rgb(RgbColor::new(255, 255, 255));
        assert!((white.l - 100.0).abs() < 0.01);
        assert!(white.a.abs() < 0.01);
        assert!(white.b.abs() < 0.01);

        let black = LabColor::from_rgb(RgbColor::new(0, 0, 0));
        assert!(black.l.abs() < 0.01);
        assert_eq!(black.to_rgb(), RgbColor::new(0, 0, 0));
    }
}
