//! SVG rendering support.
//!
//! # Example
//!
//! ```
//! use qrcode_rs::QrCode;
//! use qrcode_rs::render::svg;
//!
//! let code = QrCode::new(b"Hello").unwrap();
//! let svg_xml = code.render::<svg::Color>().build();
//! println!("{}", svg_xml);

#![cfg(feature = "svg")]

use std::fmt::Write;
use std::marker::PhantomData;

use crate::render::{Canvas as RenderCanvas, Pixel};
use crate::types::Color as ModuleColor;

/// An SVG color.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Color<'a>(pub &'a str);

impl<'a> Pixel for Color<'a> {
    type Image = String;
    type Canvas = Canvas<'a>;

    fn default_color(color: ModuleColor) -> Self {
        Color(color.select("#000", "#fff"))
    }
}

#[doc(hidden)]
pub struct Canvas<'a> {
    svg: String,
    // Pending rect for merging horizontally adjacent modules.
    pending_left: u32,
    pending_top: u32,
    pending_width: u32,
    pending_height: u32,
    has_pending: bool,
    marker: PhantomData<Color<'a>>,
}

impl<'a> Canvas<'a> {
    fn flush_pending(&mut self) {
        if self.has_pending {
            write!(
                self.svg,
                "M{} {}h{}v{}h-{}z",
                self.pending_left, self.pending_top, self.pending_width, self.pending_height, self.pending_width
            )
            .unwrap();
            self.has_pending = false;
        }
    }
}

impl<'a> RenderCanvas for Canvas<'a> {
    type Pixel = Color<'a>;
    type Image = String;

    fn new(width: u32, height: u32, dark_pixel: Color<'a>, light_pixel: Color<'a>) -> Self {
        Canvas {
            svg: format!(
                concat!(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
                    r#"<svg xmlns="http://www.w3.org/2000/svg""#,
                    r#" version="1.1" width="{w}" height="{h}""#,
                    r#" viewBox="0 0 {w} {h}" shape-rendering="crispEdges">"#,
                    r#"<path d="M0 0h{w}v{h}H0z" fill="{bg}"/>"#,
                    r#"<path fill="{fg}" d=""#,
                ),
                w = width,
                h = height,
                fg = dark_pixel.0,
                bg = light_pixel.0
            ),
            pending_left: 0,
            pending_top: 0,
            pending_width: 0,
            pending_height: 0,
            has_pending: false,
            marker: PhantomData,
        }
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        self.draw_dark_rect(x, y, 1, 1);
    }

    fn draw_dark_rect(&mut self, left: u32, top: u32, width: u32, height: u32) {
        if self.has_pending
            && top == self.pending_top
            && height == self.pending_height
            && left == self.pending_left + self.pending_width
        {
            // Merge with the previous rect.
            self.pending_width += width;
        } else {
            self.flush_pending();
            self.pending_left = left;
            self.pending_top = top;
            self.pending_width = width;
            self.pending_height = height;
            self.has_pending = true;
        }
    }

    fn into_image(mut self) -> String {
        self.flush_pending();
        self.svg.push_str(r#""/></svg>"#);
        self.svg
    }
}

/// Injects custom attributes into the root `<svg>` element of an SVG string.
///
/// # Example
///
/// ```
/// use qrcode_rs::QrCode;
/// use qrcode_rs::render::svg::{self, Color};
///
/// let code = QrCode::new(b"Hello").unwrap();
/// let svg = code.render::<Color>().build();
/// let svg = svg::inject_attributes(&svg, &[("class", "qr-code"), ("id", "main")]);
/// assert!(svg.contains(r#"class="qr-code""#));
/// ```
pub fn inject_attributes(svg: &str, attrs: &[(&str, &str)]) -> String {
    let insert_pos = svg.find('>').expect("invalid SVG: no closing '>' found");
    let mut result = String::with_capacity(svg.len() + attrs.iter().map(|(k, v)| k.len() + v.len() + 5).sum::<usize>());
    result.push_str(&svg[..insert_pos]);
    for (key, value) in attrs {
        result.push(' ');
        result.push_str(key);
        result.push_str(r#"=""#);
        result.push_str(value);
        result.push('"');
    }
    result.push_str(&svg[insert_pos..]);
    result
}

/// Rounds the corners of rectangular path segments in an SVG string.
///
/// This post-processes the SVG output from `render::<svg::Color>().build()`,
/// replacing sharp rectangular paths (`M...h...v...h-...z`) with rounded-corner
/// equivalents using SVG arc commands.
///
/// # Example
///
/// ```
/// use qrcode_rs::QrCode;
/// use qrcode_rs::render::svg::{self, Color};
///
/// let code = QrCode::new(b"Hello").unwrap();
/// let svg = code.render::<Color>().build();
/// let svg = svg::round_corners(&svg, 2);
/// assert!(svg.contains("A2"));
/// ```
pub fn round_corners(svg: &str, radius: u32) -> String {
    if radius == 0 {
        return svg.to_owned();
    }

    // Locate the last d="..." attribute (the foreground path).
    // The SVG has two paths: background and foreground. We want the foreground (last) one.
    let last_d = svg.rfind(" d=\"").or_else(|| svg.rfind("\td=\"")).or_else(|| svg.rfind("\nd=\""));
    let Some(d_attr_pos) = last_d else { return svg.to_owned() };
    let d_val_start = d_attr_pos + 4; // skip ` d="`
    let d_val_end = svg[d_val_start..].find('"').map(|p| d_val_start + p).unwrap_or(svg.len());
    let head = &svg[..d_val_start];
    let tail = &svg[d_val_end..];
    let path_data = &svg[d_val_start..d_val_end];
    let r = radius as f64;

    // Scan path_data for M...h...v...h...z rect patterns and replace them.
    let bytes = path_data.as_bytes();
    let len = bytes.len();
    let mut new_path = String::with_capacity(path_data.len() * 2);
    let mut pos = 0;

    while pos < len {
        // Find next 'M'.
        let m = match bytes[pos..].iter().position(|&b| b == b'M') {
            Some(p) => pos + p,
            None => break,
        };
        // Copy text before M.
        if m > pos {
            new_path.push_str(&path_data[pos..m]);
        }

        // Try to parse M<left> <top>h<width>v<height>h-<width>z starting at m.
        if let Some(end) = try_parse_rect(path_data, m, r, &mut new_path) {
            pos = end;
        } else {
            // Not a rect pattern, keep the M and advance.
            new_path.push('M');
            pos = m + 1;
        }
    }

    // Copy any remaining text after the last M.
    if pos < len {
        new_path.push_str(&path_data[pos..]);
    }

    let mut result = String::with_capacity(svg.len() + new_path.len());
    result.push_str(head);
    result.push_str(&new_path);
    result.push_str(tail);
    result
}

/// Tries to parse `M<left> <top>h<width>v<height>h-<width>z` at position `m`.
/// On success, writes the rounded version to `out` and returns the position after `z`.
/// On failure, returns None.
fn try_parse_rect(path: &str, m: usize, r: f64, out: &mut String) -> Option<usize> {
    let bytes = path.as_bytes();
    let len = bytes.len();

    let mut p = m + 1; // skip 'M'
    let left = parse_number(path, &mut p)?;
    skip_comma_space(path, &mut p);
    let top = parse_number(path, &mut p)?;

    if p >= len || bytes[p] != b'h' {
        return None;
    }
    p += 1; // skip 'h'
    let width = parse_number(path, &mut p)?;

    if p >= len || bytes[p] != b'v' {
        return None;
    }
    p += 1; // skip 'v'
    let height = parse_number(path, &mut p)?;

    if p >= len || bytes[p] != b'h' {
        return None;
    }
    p += 1; // skip 'h'
    let neg_width = parse_number(path, &mut p)?;

    // Must be negative and match -width.
    if neg_width >= 0.0 || (neg_width + width).abs() > 0.01 {
        return None;
    }

    if p >= len || bytes[p] != b'z' {
        return None;
    }
    p += 1; // skip 'z'

    // Emit rounded (or sharp) rect.
    let r = r.min(width / 2.0).min(height / 2.0);
    if r < 0.5 {
        write!(out, "M{left} {top}h{width}v{height}h-{width}z").unwrap();
    } else {
        let xr = left + r;
        let yr = top + r;
        let wr = width - 2.0 * r;
        let hr = height - 2.0 * r;
        let xpw = left + width;
        let yph = top + height;
        write!(
            out,
            "M{left} {yr}A{r} {r} 0 0 1 {xr} {top}h{wr}A{r} {r} 0 0 1 {xpw} {yr}v{hr}A{r} {r} 0 0 1 {xr} {yph}h-{wr}A{r} {r} 0 0 1 {left} {yr}z",
        )
        .unwrap();
    }

    Some(p)
}

/// Parses a floating-point number at position `p`, advancing `p` past it.
fn parse_number(s: &str, p: &mut usize) -> Option<f64> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let start = *p;

    // Optional sign.
    if *p < len && (bytes[*p] == b'-' || bytes[*p] == b'+') {
        *p += 1;
    }

    // Integer part.
    while *p < len && bytes[*p].is_ascii_digit() {
        *p += 1;
    }

    // Fractional part.
    if *p < len && bytes[*p] == b'.' {
        *p += 1;
        while *p < len && bytes[*p].is_ascii_digit() {
            *p += 1;
        }
    }

    if *p == start {
        return None;
    }

    s[start..*p].parse::<f64>().ok()
}

/// Skips optional comma and whitespace at position `p`.
fn skip_comma_space(s: &str, p: &mut usize) {
    let bytes = s.as_bytes();
    let len = bytes.len();
    while *p < len
        && (bytes[*p] == b' ' || bytes[*p] == b'\t' || bytes[*p] == b'\n' || bytes[*p] == b'\r' || bytes[*p] == b',')
    {
        *p += 1;
    }
}

/// Animation presets for SVG QR codes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Animation {
    /// A horizontal scan line sweeps across the QR code.
    ScanLine,
    /// The QR code fades in from transparent.
    FadeIn,
    /// The QR code pulses between full and reduced opacity.
    Pulse,
}

/// Injects CSS animation into an SVG QR code.
///
/// Adds a `<style>` element with CSS `@keyframes` that animate the foreground
/// path. The animation loops infinitely and does not affect static display
/// (the QR code is fully visible at rest for `ScanLine` and `FadeIn`).
///
/// # Example
///
/// ```
/// use qrcode_rs::QrCode;
/// use qrcode_rs::render::svg::{self, Animation, Color};
///
/// let code = QrCode::new(b"Hello").unwrap();
/// let svg = code.render::<Color>().build();
/// let svg = svg::animate(&svg, Animation::FadeIn);
/// assert!(svg.contains("@keyframes"));
/// ```
pub fn animate(svg: &str, animation: Animation) -> String {
    let css = match animation {
        Animation::ScanLine => {
            concat!(
                "<style>",
                "@keyframes qr-scan{0%{clip-path:inset(0 100% 0 0)}100%{clip-path:inset(0 0 0 0)}}",
                "path:last-of-type{animation:qr-scan 2s ease-in-out infinite alternate}",
                "</style>",
            )
        }
        Animation::FadeIn => {
            concat!(
                "<style>",
                "@keyframes qr-fade{0%{opacity:0}100%{opacity:1}}",
                "path:last-of-type{animation:qr-fade 1.5s ease-out forwards}",
                "</style>",
            )
        }
        Animation::Pulse => {
            concat!(
                "<style>",
                "@keyframes qr-pulse{0%,100%{opacity:1}50%{opacity:0.3}}",
                "path:last-of-type{animation:qr-pulse 2s ease-in-out infinite}",
                "</style>",
            )
        }
    };

    // Insert the style after the opening <svg ...> tag.
    let tag_end = svg.find('>').expect("invalid SVG: no closing '>' found") + 1;
    let mut result = String::with_capacity(svg.len() + css.len());
    result.push_str(&svg[..tag_end]);
    result.push_str(css);
    result.push_str(&svg[tag_end..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::QrCode;

    #[test]
    fn test_inject_attributes() {
        let code = QrCode::new(b"Hello").unwrap();
        let svg = code.render::<Color>().build();
        let svg = inject_attributes(&svg, &[("class", "qr-code"), ("id", "main")]);
        assert!(svg.contains(r#"class="qr-code""#));
        assert!(svg.contains(r#"id="main""#));
        assert!(svg.starts_with(r#"<?xml version="1.0""#));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn test_inject_empty_attrs() {
        let code = QrCode::new(b"Hello").unwrap();
        let svg = code.render::<Color>().build();
        let original = svg.clone();
        let svg = inject_attributes(&svg, &[]);
        assert_eq!(svg, original);
    }

    #[test]
    fn test_round_corners_produces_arcs() {
        let code = QrCode::new(b"Hello").unwrap();
        let svg = code.render::<Color>().build();
        let rounded = round_corners(&svg, 2);
        assert!(rounded.contains("A2 2 0 0 1"));
        assert!(rounded.starts_with(r#"<?xml version="1.0""#));
        assert!(rounded.ends_with("</svg>"));
    }

    #[test]
    fn test_round_corners_zero_radius_noop() {
        let code = QrCode::new(b"Hello").unwrap();
        let svg = code.render::<Color>().build();
        let rounded = round_corners(&svg, 0);
        assert_eq!(svg, rounded);
    }

    #[test]
    fn test_round_corners_preserves_background() {
        let code = QrCode::new(b"Hi").unwrap();
        let svg = code.render::<Color>().build();
        let rounded = round_corners(&svg, 3);
        // Background path should still be sharp (M0 0h...v...H0z).
        assert!(rounded.contains("M0 0h"));
    }

    #[test]
    fn test_round_corners_with_inject_attributes() {
        let code = QrCode::new(b"Hello").unwrap();
        let svg = code.render::<Color>().build();
        let svg = inject_attributes(&svg, &[("class", "qr")]);
        let svg = round_corners(&svg, 2);
        assert!(svg.contains(r#"class="qr""#));
        assert!(svg.contains("A2 2"));
    }

    #[test]
    fn test_animate_scanline() {
        let code = QrCode::new(b"Hello").unwrap();
        let svg = code.render::<Color>().build();
        let animated = animate(&svg, Animation::ScanLine);
        assert!(animated.contains("@keyframes qr-scan"));
        assert!(animated.contains("<style>"));
        assert!(animated.contains("</style>"));
        assert!(animated.starts_with(r#"<?xml version="1.0""#));
        assert!(animated.ends_with("</svg>"));
    }

    #[test]
    fn test_animate_fade_in() {
        let code = QrCode::new(b"Hello").unwrap();
        let svg = code.render::<Color>().build();
        let animated = animate(&svg, Animation::FadeIn);
        assert!(animated.contains("@keyframes qr-fade"));
    }

    #[test]
    fn test_animate_pulse() {
        let code = QrCode::new(b"Hello").unwrap();
        let svg = code.render::<Color>().build();
        let animated = animate(&svg, Animation::Pulse);
        assert!(animated.contains("@keyframes qr-pulse"));
    }

    #[test]
    fn test_animate_preserves_svg_structure() {
        let code = QrCode::new(b"Hi").unwrap();
        let svg = code.render::<Color>().build();
        let animated = animate(&svg, Animation::FadeIn);
        // Style is inserted after the opening <svg> tag, before the paths.
        let style_pos = animated.find("<style>").unwrap();
        let svg_tag_end = animated.find('>').unwrap();
        assert!(style_pos > svg_tag_end);
        assert!(animated.contains("<path"));
    }
}
