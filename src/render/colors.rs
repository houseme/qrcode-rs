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
//! use qrcode_rs::QrCode;
//! use qrcode_rs::render::svg;
//!
//! let code = QrCode::new(b"Hello").unwrap();
//! // SVG/HTML accept any CSS color string directly.
//! let svg = code.render::<svg::Color>()
//!     .dark_color(svg::Color("#1a1a2e"))
//!     .light_color(svg::Color("#e0e0e0"))
//!     .build();
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
}
