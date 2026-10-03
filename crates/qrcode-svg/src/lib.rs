//! SVG rendering support.
//!
//! # Example
//!
//! ```
//! use qrcode_core::Color as ModuleColor;
//! use qrcode_render::Renderer;
//! use qrcode_svg::Color;
//!
//! let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
//! let svg_xml = Renderer::<Color>::new(&modules, 2, 1).build();
//! println!("{}", svg_xml);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::{
    borrow::ToOwned,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use core::fmt::Write;
use core::marker::PhantomData;

use qrcode_core::Color as ModuleColor;
use qrcode_render::{Canvas as RenderCanvas, Pixel};

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

fn push_escaped_attr_value(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
}

fn escaped_attr_len(value: &str) -> Option<usize> {
    value.chars().try_fold(0usize, |length, ch| {
        let bytes = match ch {
            '&' => 5,
            '<' | '>' => 4,
            '"' | '\'' => 6,
            _ => ch.len_utf8(),
        };
        length.checked_add(bytes)
    })
}

fn injected_attr_capacity(input_len: usize, attrs: &[(&str, &str)]) -> Option<usize> {
    attrs
        .iter()
        .filter(|(key, _)| is_attr_name(key))
        .try_fold(input_len, |capacity, (key, value)| {
            let additional = key.len().checked_add(escaped_attr_len(value)?)?.checked_add(4)?;
            capacity.checked_add(additional)
        })
        .filter(|&capacity| capacity <= isize::MAX as usize)
}

fn is_attr_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first == ':' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch == ':' || ch == '-' || ch == '.' || ch.is_ascii_alphanumeric())
}

impl<'a> RenderCanvas for Canvas<'a> {
    type Pixel = Color<'a>;
    type Image = String;

    fn new(width: u32, height: u32, dark_pixel: Color<'a>, light_pixel: Color<'a>) -> Self {
        Canvas {
            svg: {
                let mut svg = format!(
                    concat!(
                        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
                        r#"<svg xmlns="http://www.w3.org/2000/svg""#,
                        r#" version="1.1" width="{w}" height="{h}""#,
                        r#" viewBox="0 0 {w} {h}" shape-rendering="crispEdges">"#,
                        r#"<path d="M0 0h{w}v{h}H0z" fill=""#,
                    ),
                    w = width,
                    h = height
                );
                push_escaped_attr_value(&mut svg, light_pixel.0);
                svg.push_str(r#""/><path fill=""#);
                push_escaped_attr_value(&mut svg, dark_pixel.0);
                svg.push_str(r#"" d=""#);
                svg
            },
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
/// Supports self-closing roots and `>` inside quoted attribute values.
///
/// # Example
///
/// ```
/// use qrcode_core::Color as ModuleColor;
/// use qrcode_render::Renderer;
/// use qrcode_svg::{self as svg, Color};
///
/// let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
/// let svg = Renderer::<Color>::new(&modules, 2, 1).build();
/// let svg = svg::inject_attributes(&svg, &[("class", "qr-code"), ("id", "main")]);
/// // The attribute lands *inside* the <svg ...> opening tag.
/// let start = svg.find("<svg").unwrap();
/// let tag_end = start + svg[start..].find('>').unwrap();
/// assert!(svg[start..tag_end].contains(r#"class="qr-code""#));
/// ```
pub fn inject_attributes(svg: &str, attrs: &[(&str, &str)]) -> String {
    // Target the root <svg …> opening tag (skipping any leading <?xml ?> declaration).
    let tag_start = opening_tag_start(svg, "svg").expect("invalid SVG: no <svg> element");
    let tag_end = opening_tag_end(svg, tag_start).expect("invalid SVG: no closing '>' in <svg>");
    let insert_pos = if svg.as_bytes()[tag_end - 1] == b'/' { tag_end - 1 } else { tag_end };
    let capacity = injected_attr_capacity(svg.len(), attrs).expect("SVG attribute output exceeds platform limits");
    let mut result = String::with_capacity(capacity);
    result.push_str(&svg[..insert_pos]);
    for (key, value) in attrs {
        if !is_attr_name(key) {
            continue;
        }
        result.push(' ');
        result.push_str(key);
        result.push_str(r#"=""#);
        push_escaped_attr_value(&mut result, value);
        result.push('"');
    }
    result.push_str(&svg[insert_pos..]);
    result
}

fn opening_tag_start(markup: &str, target: &str) -> Option<usize> {
    let bytes = markup.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let start = cursor + bytes[cursor..].iter().position(|&byte| byte == b'<')?;
        let tail = &markup[start..];
        if tail.starts_with("<!--") {
            cursor = start + 4 + markup[start + 4..].find("-->")? + 3;
            continue;
        }
        if tail.starts_with("<?") {
            cursor = start + 2 + markup[start + 2..].find("?>")? + 2;
            continue;
        }
        if tail.starts_with("<![CDATA[") {
            cursor = start + 9 + markup[start + 9..].find("]]>")? + 3;
            continue;
        }
        if tail.starts_with("<!") {
            cursor = declaration_end(markup, start)?;
            continue;
        }
        if tail.starts_with("</") {
            cursor = opening_tag_end(markup, start)? + 1;
            continue;
        }
        let name_start = start + 1;
        let name_end = name_start
            + bytes[name_start..]
                .iter()
                .position(|&byte| byte.is_ascii_whitespace() || byte == b'/' || byte == b'>')
                .unwrap_or(bytes.len() - name_start);
        let tag_name = &markup[name_start..name_end];
        if tag_name.rsplit(':').next() == Some(target) {
            return Some(start);
        }
        cursor = opening_tag_end(markup, start)? + 1;
    }
    None
}

// Ignore quoted identifiers and internal DTD subsets, including their comments
// and processing instructions. No entity expansion or external reads occur.
fn declaration_end(markup: &str, start: usize) -> Option<usize> {
    let bytes = markup.as_bytes();
    let mut cursor = start + 2;
    let mut quote = None;
    let mut subset_depth = 0usize;
    while cursor < bytes.len() {
        if quote.is_none() {
            if bytes[cursor..].starts_with(b"<!--") {
                cursor += 4 + markup[cursor + 4..].find("-->")? + 3;
                continue;
            }
            if bytes[cursor..].starts_with(b"<?") {
                cursor += 2 + markup[cursor + 2..].find("?>")? + 2;
                continue;
            }
        }
        let byte = bytes[cursor];
        if let Some(delimiter) = quote {
            if byte == delimiter {
                quote = None;
            }
        } else {
            match byte {
                b'\'' | b'"' => quote = Some(byte),
                b'[' => subset_depth += 1,
                b']' => subset_depth = subset_depth.saturating_sub(1),
                b'>' if subset_depth == 0 => return Some(cursor + 1),
                _ => {}
            }
        }
        cursor += 1;
    }
    None
}

fn opening_tag_end(svg: &str, tag_start: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, &byte) in svg.as_bytes()[tag_start..].iter().enumerate() {
        if let Some(delimiter) = quote {
            if byte == delimiter {
                quote = None;
            }
        } else {
            match byte {
                b'\'' | b'"' => quote = Some(byte),
                b'>' => return Some(tag_start + offset),
                _ => {}
            }
        }
    }
    None
}

/// Adds screen-reader accessibility attributes to the root `<svg>` element:
/// `role="img"` and `aria-label="<label>"`.
///
/// # Example
///
/// ```
/// use qrcode_core::Color as ModuleColor;
/// use qrcode_render::Renderer;
/// use qrcode_svg::{self as svg, Color};
///
/// let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
/// let svg = Renderer::<Color>::new(&modules, 2, 1).build();
/// let svg = svg::aria_label(&svg, "QR code saying Hello");
/// assert!(svg.contains(r#"role="img""#));
/// assert!(svg.contains(r#"aria-label="QR code saying Hello""#));
/// ```
pub fn aria_label(svg: &str, label: &str) -> String {
    inject_attributes(svg, &[("role", "img"), ("aria-label", label)])
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
/// use qrcode_core::Color as ModuleColor;
/// use qrcode_render::Renderer;
/// use qrcode_svg::{self as svg, Color};
///
/// let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
/// let svg = Renderer::<Color>::new(&modules, 2, 1).build();
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
/// use qrcode_core::Color as ModuleColor;
/// use qrcode_render::Renderer;
/// use qrcode_svg::{self as svg, Animation, Color};
///
/// let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
/// let svg = Renderer::<Color>::new(&modules, 2, 1).build();
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

    // Insert the style after the opening <svg ...> tag, not after a leading
    // XML declaration.
    let tag_start = opening_tag_start(svg, "svg").expect("invalid SVG: no <svg> element");
    let tag_end = opening_tag_end(svg, tag_start).expect("invalid SVG: no closing '>' found");
    let self_closing = svg.as_bytes()[tag_end - 1] == b'/';
    let tag_name = svg[tag_start + 1..tag_end]
        .split(|ch: char| ch.is_ascii_whitespace() || ch == '/')
        .next()
        .expect("SVG opening tag has a name");
    let namespace_prefix = tag_name.rsplit_once(':').map(|(prefix, _)| prefix);
    let style_extra = namespace_prefix
        .map(|prefix| prefix.len().checked_add(1).and_then(|bytes| bytes.checked_mul(2)))
        .unwrap_or(Some(0));
    let root_extra = if self_closing { tag_name.len().checked_add(2) } else { Some(0) };
    let capacity = svg
        .len()
        .checked_add(css.len())
        .and_then(|bytes| bytes.checked_add(style_extra?))
        .and_then(|bytes| bytes.checked_add(root_extra?))
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .expect("SVG animation output exceeds platform limits");
    let mut result = String::with_capacity(capacity);
    if self_closing {
        result.push_str(&svg[..tag_end - 1]);
        result.push('>');
        push_animation_style(&mut result, css, namespace_prefix);
        result.push_str("</");
        result.push_str(tag_name);
        result.push('>');
    } else {
        result.push_str(&svg[..tag_end + 1]);
        push_animation_style(&mut result, css, namespace_prefix);
    }
    result.push_str(&svg[tag_end + 1..]);
    result
}

fn push_animation_style(out: &mut String, css: &str, namespace_prefix: Option<&str>) {
    if let Some(prefix) = namespace_prefix {
        out.push('<');
        out.push_str(prefix);
        out.push_str(":style>");
        out.push_str(&css["<style>".len()..css.len() - "</style>".len()]);
        out.push_str("</");
        out.push_str(prefix);
        out.push_str(":style>");
    } else {
        out.push_str(css);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_svg() -> String {
        let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
        qrcode_render::Renderer::<Color>::new(&modules, 2, 1).build()
    }

    #[test]
    fn test_inject_attributes() {
        let svg = sample_svg();
        let svg = inject_attributes(&svg, &[("class", "qr-code"), ("id", "main")]);
        assert!(svg.contains(r#"class="qr-code""#));
        assert!(svg.contains(r#"id="main""#));
        assert!(svg.starts_with(r#"<?xml version="1.0""#));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn test_inject_empty_attrs() {
        let svg = sample_svg();
        let original = svg.clone();
        let svg = inject_attributes(&svg, &[]);
        assert_eq!(svg, original);
    }

    #[test]
    fn attribute_injection_ignores_tag_delimiters_inside_quoted_values() {
        let cases = [
            (r#"<svg data-note="a>b"><path/></svg>"#, r#"<svg data-note="a>b" class="qr"><path/></svg>"#),
            (
                r#"<svg data-note='二维码 > "说明"'><path/></svg>"#,
                r#"<svg data-note='二维码 > "说明"' class="qr"><path/></svg>"#,
            ),
            (
                r#"<?xml version="1.0"?><svg data-first="1>0" data-second='3>2'><path/></svg>"#,
                r#"<?xml version="1.0"?><svg data-first="1>0" data-second='3>2' class="qr"><path/></svg>"#,
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(inject_attributes(input, &[("class", "qr")]), expected);
        }
    }

    #[test]
    fn attribute_injection_preserves_self_closing_roots() {
        let cases = [
            (r#"<svg/>"#, r#"<svg class="qr"/>"#),
            (r#"<svg />"#, r#"<svg  class="qr"/>"#),
            (r#"<svg data-note="/>"/>"#, r#"<svg data-note="/>" class="qr"/>"#),
        ];
        for (input, expected) in cases {
            assert_eq!(inject_attributes(input, &[("class", "qr")]), expected);
            assert_eq!(inject_attributes(input, &[]), input);
            assert_eq!(inject_attributes(input, &[("invalid name", "ignored")]), input);
        }
    }

    #[test]
    fn aria_labels_preserve_quoted_and_self_closing_root_syntax() {
        let input = r#"<svg data-note='a>b'/>"#;
        let expected = r#"<svg data-note='a>b' role="img" aria-label="二维码 &quot;A&quot; &amp; B"/>"#;
        assert_eq!(aria_label(input, "二维码 \"A\" & B"), expected);
    }

    #[test]
    fn generated_svg_attribute_injection_keeps_legacy_bytes() {
        let input = sample_svg();
        let start = input.find("<svg").unwrap();
        let position = start + input[start..].find('>').unwrap();
        let expected = format!("{} class=\"qr\"{}", &input[..position], &input[position..]);
        assert_eq!(inject_attributes(&input, &[("class", "qr")]), expected);
    }

    #[test]
    fn injected_attributes_reserve_exact_escaped_bytes_and_skip_invalid_names() {
        let ignored = "\"".repeat(4096);
        let attrs = [("invalid name", ignored.as_str()), ("data-note", "&\"'<>💖")];
        let expected = r#"<svg data-note="&amp;&quot;&apos;&lt;&gt;💖"/>"#;
        assert_eq!(injected_attr_capacity(6, &attrs), Some(expected.len()));
        assert_eq!(inject_attributes("<svg/>", &attrs), expected);
        let invalid_attrs = [("invalid name", ignored.as_str())];
        assert_eq!(injected_attr_capacity(6, &invalid_attrs), Some(6));
        assert_eq!(inject_attributes("<svg/>", &invalid_attrs), "<svg/>");
        assert_eq!(injected_attr_capacity(isize::MAX as usize, &[("class", "qr")]), None);
        assert_eq!(injected_attr_capacity(usize::MAX, &[]), None);
    }

    #[test]
    fn attributes_target_real_svg_after_comments_processing_instructions_and_doctype() {
        let cases = [
            (r#"<!-- example <svg> --><svg/>"#, r#"<!-- example <svg> --><svg class="qr"/>"#),
            (r#"<?example fake="<svg>"?><svg/>"#, r#"<?example fake="<svg>"?><svg class="qr"/>"#),
            (
                r#"<!DOCTYPE svg [<!ELEMENT svg ANY><!ENTITY example "<svg>">]><svg/>"#,
                r#"<!DOCTYPE svg [<!ELEMENT svg ANY><!ENTITY example "<svg>">]><svg class="qr"/>"#,
            ),
            (
                r#"<!DOCTYPE svg [<!-- ]> <svg> --><?example " <svg> ?><!ELEMENT svg ANY>]><svg/>"#,
                r#"<!DOCTYPE svg [<!-- ]> <svg> --><?example " <svg> ?><!ELEMENT svg ANY>]><svg class="qr"/>"#,
            ),
            (
                r#"<wrapper><![CDATA[<svg/>]]><svg data-note="a>b"/></wrapper>"#,
                r#"<wrapper><![CDATA[<svg/>]]><svg data-note="a>b" class="qr"/></wrapper>"#,
            ),
            (
                r#"<wrapper><svg-shadow/><svg data-note="a>b"/></wrapper>"#,
                r#"<wrapper><svg-shadow/><svg data-note="a>b" class="qr"/></wrapper>"#,
            ),
            (
                r#"<s:svg xmlns:s="http://www.w3.org/2000/svg"/>"#,
                r#"<s:svg xmlns:s="http://www.w3.org/2000/svg" class="qr"/>"#,
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(inject_attributes(input, &[("class", "qr")]), expected);
        }
    }

    #[test]
    fn animation_skips_fake_svg_roots_without_changing_the_preamble() {
        let input = r#"<!-- example <svg> --><?example <svg> ?><svg/>"#;
        let style = animate("<svg/>", Animation::Pulse);
        assert_eq!(animate(input, Animation::Pulse), format!("<!-- example <svg> --><?example <svg> ?>{style}"));
    }

    #[test]
    #[should_panic(expected = "invalid SVG: no <svg> element")]
    fn fake_svg_tag_names_do_not_satisfy_the_root_contract() {
        let _ = inject_attributes("<svg-shadow/>", &[("class", "qr")]);
    }

    #[test]
    fn colors_are_xml_escaped() {
        let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
        let svg = qrcode_render::Renderer::<Color>::new(&modules, 2, 1)
            .dark_color(Color(r##"black"/><script>alert(1)</script><path fill="red"##))
            .light_color(Color(r#"" onload="alert(1)"#))
            .build();

        assert!(!svg.contains("<script>"));
        assert!(!svg.contains(" onload=\""));
        assert!(svg.contains("&lt;script&gt;"));
        assert!(svg.contains("&quot;"));
    }

    #[test]
    fn injected_attribute_values_are_xml_escaped() {
        let svg = aria_label(&sample_svg(), r#"QR "><script>alert(1)</script>"#);

        assert!(!svg.contains("<script>"));
        assert!(svg.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(svg.contains("&quot;&gt;"));
    }

    #[test]
    fn invalid_attribute_names_are_skipped() {
        let svg = inject_attributes(&sample_svg(), &[(r#"x" onload="alert(1)"#, "bad"), ("data-ok", "yes")]);

        assert!(!svg.contains("onload"));
        assert!(svg.contains(r#"data-ok="yes""#));
    }

    #[test]
    fn test_round_corners_produces_arcs() {
        let svg = sample_svg();
        let rounded = round_corners(&svg, 2);
        assert!(rounded.contains("A2 2 0 0 1"));
        assert!(rounded.starts_with(r#"<?xml version="1.0""#));
        assert!(rounded.ends_with("</svg>"));
    }

    #[test]
    fn test_round_corners_zero_radius_noop() {
        let svg = sample_svg();
        let rounded = round_corners(&svg, 0);
        assert_eq!(svg, rounded);
    }

    #[test]
    fn test_round_corners_preserves_background() {
        let svg = sample_svg();
        let rounded = round_corners(&svg, 3);
        // Background path should still be sharp (M0 0h...v...H0z).
        assert!(rounded.contains("M0 0h"));
    }

    #[test]
    fn test_round_corners_with_inject_attributes() {
        let svg = sample_svg();
        let svg = inject_attributes(&svg, &[("class", "qr")]);
        let svg = round_corners(&svg, 2);
        assert!(svg.contains(r#"class="qr""#));
        assert!(svg.contains("A2 2"));
    }

    #[test]
    fn test_animate_scanline() {
        let svg = sample_svg();
        let animated = animate(&svg, Animation::ScanLine);
        assert!(animated.contains("@keyframes qr-scan"));
        assert!(animated.contains("<style>"));
        assert!(animated.contains("</style>"));
        assert!(animated.starts_with(r#"<?xml version="1.0""#));
        assert!(animated.ends_with("</svg>"));
    }

    #[test]
    fn test_animate_fade_in() {
        let svg = sample_svg();
        let animated = animate(&svg, Animation::FadeIn);
        assert!(animated.contains("@keyframes qr-fade"));
    }

    #[test]
    fn test_animate_pulse() {
        let svg = sample_svg();
        let animated = animate(&svg, Animation::Pulse);
        assert!(animated.contains("@keyframes qr-pulse"));
    }

    #[test]
    fn test_animate_preserves_svg_structure() {
        let svg = sample_svg();
        let animated = animate(&svg, Animation::FadeIn);
        // Style is inserted after the opening <svg> tag, before the paths.
        let style_pos = animated.find("<style>").unwrap();
        let svg_start = animated.find("<svg").unwrap();
        let svg_tag_end = svg_start + animated[svg_start..].find('>').unwrap();
        assert!(style_pos > svg_tag_end);
        assert!(animated.contains("<path"));
    }

    #[test]
    fn animations_preserve_generated_svg_bytes_for_every_style() {
        let cases = [
            (
                Animation::ScanLine,
                concat!(
                    "<style>",
                    "@keyframes qr-scan{0%{clip-path:inset(0 100% 0 0)}100%{clip-path:inset(0 0 0 0)}}",
                    "path:last-of-type{animation:qr-scan 2s ease-in-out infinite alternate}",
                    "</style>",
                ),
            ),
            (
                Animation::FadeIn,
                concat!(
                    "<style>",
                    "@keyframes qr-fade{0%{opacity:0}100%{opacity:1}}",
                    "path:last-of-type{animation:qr-fade 1.5s ease-out forwards}",
                    "</style>",
                ),
            ),
            (
                Animation::Pulse,
                concat!(
                    "<style>",
                    "@keyframes qr-pulse{0%,100%{opacity:1}50%{opacity:0.3}}",
                    "path:last-of-type{animation:qr-pulse 2s ease-in-out infinite}",
                    "</style>",
                ),
            ),
        ];
        let input = sample_svg();
        let start = input.find("<svg").unwrap();
        let position = start + input[start..].find('>').unwrap() + 1;
        for (animation, css) in cases {
            let expected = format!("{}{css}{}", &input[..position], &input[position..]);
            assert_eq!(animate(&input, animation), expected);
        }
    }

    #[test]
    fn animation_styles_are_inserted_inside_quoted_or_self_closing_roots() {
        let css = concat!(
            "<style>",
            "@keyframes qr-fade{0%{opacity:0}100%{opacity:1}}",
            "path:last-of-type{animation:qr-fade 1.5s ease-out forwards}",
            "</style>",
        );
        let cases = [
            (r#"<svg data-note="a>b"><path/></svg>"#, r#"<svg data-note="a>b">"#, "<path/></svg>"),
            (r#"<svg data-note='二维码 > "说明"'/>"#, r#"<svg data-note='二维码 > "说明"'>"#, "</svg>"),
            ("<svg />", "<svg >", "</svg>"),
            (
                r#"<?xml version="1.0"?><svg data-note="/>"/><!-- tail -->"#,
                r#"<?xml version="1.0"?><svg data-note="/>">"#,
                "</svg><!-- tail -->",
            ),
        ];
        for (input, prefix, suffix) in cases {
            assert_eq!(animate(input, Animation::FadeIn), format!("{prefix}{css}{suffix}"));
        }
    }

    #[test]
    fn prefixed_svg_animations_put_styles_in_the_svg_namespace() {
        let css_body = concat!(
            "@keyframes qr-fade{0%{opacity:0}100%{opacity:1}}",
            "path:last-of-type{animation:qr-fade 1.5s ease-out forwards}",
        );
        for prefix in ["s", "svg", "二维码"] {
            let input = format!("<{prefix}:svg xmlns:{prefix}=\"http://www.w3.org/2000/svg\"/>");
            let expected = format!(
                "<{prefix}:svg xmlns:{prefix}=\"http://www.w3.org/2000/svg\"><{prefix}:style>{css_body}</{prefix}:style></{prefix}:svg>"
            );
            assert_eq!(animate(&input, Animation::FadeIn), expected);
        }
        let input = r#"<s:svg xmlns="urn:other" xmlns:s="http://www.w3.org/2000/svg"/>"#;
        let expected = format!(
            "<s:svg xmlns=\"urn:other\" xmlns:s=\"http://www.w3.org/2000/svg\"><s:style>{css_body}</s:style></s:svg>"
        );
        assert_eq!(animate(input, Animation::FadeIn), expected);
        let input = r#"<s:svg xmlns:s="http://www.w3.org/2000/svg"><s:path/></s:svg>"#;
        let expected =
            format!("<s:svg xmlns:s=\"http://www.w3.org/2000/svg\"><s:style>{css_body}</s:style><s:path/></s:svg>");
        assert_eq!(animate(input, Animation::FadeIn), expected);
    }
}
