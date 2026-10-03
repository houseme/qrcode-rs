//! HTML rendering support.
//!
//! Generates HTML `<table>` or CSS Grid output for embedding QR codes in web pages.
//!
//! # Example
//!
//! ```
//! use qrcode_core::Color as ModuleColor;
//! use qrcode_html::Color;
//! use qrcode_render::Renderer;
//!
//! let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
//! let html = Renderer::<Color>::new(&modules, 2, 1).build();
//! println!("{}", html);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

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

use core::marker::PhantomData;

use qrcode_core::Color as ModuleColor;
use qrcode_render::{Canvas as RenderCanvas, MAX_BUFFER_BYTES, Pixel, RenderError};

const TABLE_HEADER: &str = r#"<!DOCTYPE html><html><head><meta charset="UTF-8"></head><body><table style="border-collapse:collapse;line-height:0">"#;
const TABLE_FOOTER: &str = "</table></body></html>";
const TABLE_CELL_PREFIX: &str = r#"<td style="width:1px;height:1px;background:"#;
const TABLE_CELL_SUFFIX: &str = r#""></td>"#;
const GRID_HEADER: &str = r#"<!DOCTYPE html><html><head><meta charset="UTF-8"></head><body><div style="display:grid;grid-template-columns:repeat("#;
const GRID_HEADER_SUFFIX: &str = r#",1px);line-height:0">"#;
const GRID_FOOTER: &str = "</div></body></html>";
const GRID_CELL_PREFIX: &str = r#"<div style="width:1px;height:1px;background:"#;
const GRID_CELL_SUFFIX: &str = r#""></div>"#;

/// Rendering mode for HTML output.
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum Mode {
    /// Generate HTML `<table>` (default).
    #[default]
    Table,
    /// Generate `<div>` with CSS Grid layout.
    Grid,
}

/// An HTML color.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Color<'a>(pub &'a str);

impl<'a> Pixel for Color<'a> {
    type Image = String;
    type Canvas = Canvas<'a>;

    fn default_unit_size() -> (u32, u32) {
        (1, 1)
    }

    fn default_color(color: ModuleColor) -> Self {
        Color(color.select("#000", "#fff"))
    }
}

#[doc(hidden)]
pub struct Canvas<'a> {
    dark_pixels: Vec<bool>,
    width: u32,
    height: u32,
    dark_color: &'a str,
    light_color: &'a str,
    mode: Mode,
    marker: PhantomData<Color<'a>>,
    table_capacity: usize,
    grid_capacity: usize,
}

fn escaped_attr_len(value: &str) -> Result<usize, RenderError> {
    value.chars().try_fold(0usize, |length, ch| {
        let bytes = match ch {
            '&' | '\'' => 5,
            '<' | '>' => 4,
            '"' => 6,
            _ => ch.len_utf8(),
        };
        length.checked_add(bytes).ok_or(RenderError::OutputTooLarge)
    })
}

fn injected_attr_capacity(input_len: usize, attrs: &[(&str, &str)]) -> Option<usize> {
    attrs
        .iter()
        .filter(|(key, _)| is_attr_name(key))
        .try_fold(input_len, |capacity, (key, value)| {
            let additional = key.len().checked_add(escaped_attr_len(value).ok()?)?.checked_add(4)?;
            capacity.checked_add(additional)
        })
        .filter(|&capacity| capacity <= isize::MAX as usize)
}

fn layout(width: u32, height: u32, dark: Color<'_>, light: Color<'_>) -> Result<(usize, usize, usize), RenderError> {
    let area = (width as usize).checked_mul(height as usize).ok_or(RenderError::OutputTooLarge)?;
    let color_bytes = if area == 0 { 0 } else { escaped_attr_len(dark.0)?.max(escaped_attr_len(light.0)?) };
    let table_cell_bytes = TABLE_CELL_PREFIX
        .len()
        .checked_add(TABLE_CELL_SUFFIX.len())
        .and_then(|bytes| bytes.checked_add(color_bytes))
        .ok_or(RenderError::OutputTooLarge)?;
    let table_capacity = area
        .checked_mul(table_cell_bytes)
        .and_then(|bytes| (height as usize).checked_mul(9).and_then(|rows| bytes.checked_add(rows)))
        .and_then(|bytes| bytes.checked_add(TABLE_HEADER.len() + TABLE_FOOTER.len()))
        .ok_or(RenderError::OutputTooLarge)?;
    let grid_cell_bytes = GRID_CELL_PREFIX
        .len()
        .checked_add(GRID_CELL_SUFFIX.len())
        .and_then(|bytes| bytes.checked_add(color_bytes))
        .ok_or(RenderError::OutputTooLarge)?;
    let width_digits = if width == 0 { 1 } else { width.ilog10() as usize + 1 };
    let grid_capacity = area
        .checked_mul(grid_cell_bytes)
        .and_then(|bytes| {
            bytes.checked_add(GRID_HEADER.len() + width_digits + GRID_HEADER_SUFFIX.len() + GRID_FOOTER.len())
        })
        .ok_or(RenderError::OutputTooLarge)?;
    let total_bytes = area.checked_add(table_capacity.max(grid_capacity)).ok_or(RenderError::OutputTooLarge)?;
    if total_bytes > MAX_BUFFER_BYTES || total_bytes > isize::MAX as usize {
        return Err(RenderError::OutputTooLarge);
    }
    Ok((area, table_capacity, grid_capacity))
}

impl<'a> Canvas<'a> {
    /// Sets the HTML rendering mode (Table or Grid).
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }
}

fn push_escaped_attr_value(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
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
        let (area, table_capacity, grid_capacity) =
            layout(width, height, dark_pixel, light_pixel).unwrap_or_else(|error| panic!("{error}"));
        Canvas {
            dark_pixels: vec![false; area],
            width,
            height,
            dark_color: dark_pixel.0,
            light_color: light_pixel.0,
            mode: Mode::default(),
            marker: PhantomData,
            table_capacity,
            grid_capacity,
        }
    }

    fn validate_dimensions(width: u32, height: u32, dark: &Color<'a>, light: &Color<'a>) -> Result<(), RenderError> {
        layout(width, height, *dark, *light).map(|_| ())
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        let idx = (y as usize).checked_mul(self.width as usize).and_then(|row| row.checked_add(x as usize));
        if let Some(pixel) = idx.and_then(|idx| self.dark_pixels.get_mut(idx)) {
            *pixel = true;
        }
    }

    fn into_image(self) -> String {
        match self.mode {
            Mode::Table => self.into_table(),
            Mode::Grid => self.into_grid(),
        }
    }
}

impl<'a> Canvas<'a> {
    fn into_table(self) -> String {
        let mut html = String::with_capacity(self.table_capacity);
        html.push_str(TABLE_HEADER);

        for y in 0..self.height {
            html.push_str("<tr>");
            for x in 0..self.width {
                let idx = y as usize * self.width as usize + x as usize;
                let color = if self.dark_pixels[idx] { self.dark_color } else { self.light_color };
                html.push_str(TABLE_CELL_PREFIX);
                push_escaped_attr_value(&mut html, color);
                html.push_str(TABLE_CELL_SUFFIX);
            }
            html.push_str("</tr>");
        }

        html.push_str(TABLE_FOOTER);
        html
    }

    fn into_grid(self) -> String {
        let mut html = String::with_capacity(self.grid_capacity);
        html.push_str(GRID_HEADER);
        html.push_str(&self.width.to_string());
        html.push_str(GRID_HEADER_SUFFIX);

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = y as usize * self.width as usize + x as usize;
                let color = if self.dark_pixels[idx] { self.dark_color } else { self.light_color };
                html.push_str(GRID_CELL_PREFIX);
                push_escaped_attr_value(&mut html, color);
                html.push_str(GRID_CELL_SUFFIX);
            }
        }

        html.push_str(GRID_FOOTER);
        html
    }
}

/// Injects custom attributes into the QR container element (`<table>` in
/// [`Mode::Table`], `<div>` in [`Mode::Grid`]). If no container is found the
/// input is returned unchanged. A missing or unclosed opening tag is also
/// returned unchanged. Quoted `>` characters are preserved.
/// Raw-text containers are skipped; `noscript` is treated conservatively as
/// raw text with browser scripting enabled, and `plaintext` consumes to EOF.
///
/// # Example
///
/// ```
/// use qrcode_core::Color as ModuleColor;
/// use qrcode_html::{self as html, Color};
/// use qrcode_render::Renderer;
///
/// let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
/// let html = Renderer::<Color>::new(&modules, 2, 1).build();
/// let html = html::inject_attributes(&html, &[("class", "qr")]);
/// let start = html.find("<table").unwrap();
/// let tag_end = start + html[start..].find('>').unwrap();
/// assert!(html[start..tag_end].contains(r#"class="qr""#));
/// ```
pub fn inject_attributes(html: &str, attrs: &[(&str, &str)]) -> String {
    let Some(start) = opening_tag_start(html, "table").or_else(|| opening_tag_start(html, "div")) else {
        return html.to_owned();
    };
    let Some(close) = opening_tag_end(html, start) else {
        return html.to_owned();
    };
    let close = if html.as_bytes()[close - 1] == b'/' { close - 1 } else { close };
    let capacity = injected_attr_capacity(html.len(), attrs).expect("HTML attribute output exceeds platform limits");
    let mut result = String::with_capacity(capacity);
    result.push_str(&html[..close]);
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
    result.push_str(&html[close..]);
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
            cursor = if let Some(end) = markup[start + 2..].find("?>") {
                start + 2 + end + 2
            } else {
                // HTML treats non-XML processing instructions as bogus comments.
                start + 2 + markup[start + 2..].find('>')? + 1
            };
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
        if tag_name.eq_ignore_ascii_case(target) {
            return Some(start);
        }
        if tag_name.eq_ignore_ascii_case("plaintext") {
            return None;
        }
        cursor = opening_tag_end(markup, start)? + 1;
        if ["script", "style", "title", "textarea", "iframe", "xmp", "noembed", "noframes", "noscript"]
            .iter()
            .any(|name| tag_name.eq_ignore_ascii_case(name))
        {
            cursor = raw_text_end(markup, cursor, tag_name)?;
        }
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

fn raw_text_end(markup: &str, start: usize, name: &str) -> Option<usize> {
    let bytes = markup.as_bytes();
    let mut cursor = start;
    while cursor < bytes.len() {
        let close = cursor + bytes[cursor..].iter().position(|&byte| byte == b'<')?;
        if bytes[close..].starts_with(b"</") {
            let name_start = close + 2;
            let name_end = name_start.checked_add(name.len())?;
            if bytes.get(name_start..name_end).is_some_and(|actual| actual.eq_ignore_ascii_case(name.as_bytes()))
                && bytes.get(name_end).is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b'>' || *byte == b'/')
            {
                return opening_tag_end(markup, close).map(|end| end + 1);
            }
        }
        cursor = close + 1;
    }
    None
}

fn opening_tag_end(html: &str, tag_start: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, &byte) in html.as_bytes()[tag_start..].iter().enumerate() {
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

/// Adds screen-reader accessibility attributes (`role="img"` and
/// `aria-label="<label>"`) to the QR container element.
///
/// # Example
///
/// ```
/// use qrcode_core::Color as ModuleColor;
/// use qrcode_html::{self as html, Color};
/// use qrcode_render::Renderer;
///
/// let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
/// let html = Renderer::<Color>::new(&modules, 2, 1).build();
/// let html = html::aria_label(&html, "QR code saying Hello");
/// assert!(html.contains(r#"aria-label="QR code saying Hello""#));
/// ```
pub fn aria_label(html: &str, label: &str) -> String {
    inject_attributes(html, &[("role", "img"), ("aria-label", label)])
}

#[cfg(test)]
mod tests {
    use super::{Canvas, Color, Mode, layout};
    use alloc::string::String;
    use qrcode_render::{Canvas as RenderCanvas, RenderError, Renderer};

    #[test]
    fn html_large_dimensions_fail_before_allocation() {
        let dark = Color("#000");
        let light = Color("#fff");
        for (width, height) in [(65_536, 65_536), (u32::MAX, u32::MAX), (0, u32::MAX)] {
            assert_eq!(Canvas::validate_dimensions(width, height, &dark, &light), Err(RenderError::OutputTooLarge));
        }
        let modules = [qrcode_core::Color::Light];
        assert_eq!(
            Renderer::<Color>::new(&modules, 1, 0).module_dimensions(65_536, 65_536).try_build(),
            Err(RenderError::OutputTooLarge)
        );
    }

    #[test]
    fn html_escaped_color_bytes_are_included_in_budget() {
        let color = "\"".repeat(4096);
        assert_eq!(
            Canvas::validate_dimensions(256, 256, &Color(&color), &Color("#fff")),
            Err(RenderError::OutputTooLarge)
        );
    }

    #[test]
    fn html_capacity_covers_both_modes_and_utf8_escaping() {
        let color = Color("\"'&<>💖");
        let (_, table_capacity, grid_capacity) = layout(2, 3, color, color).unwrap();
        for (mode, capacity) in [(Mode::Table, table_capacity), (Mode::Grid, grid_capacity)] {
            let mut canvas = Canvas::new(2, 3, color, color);
            canvas.set_mode(mode);
            canvas.draw_dark_pixel(0, 1);
            let output = canvas.into_image();
            assert_eq!(output.len(), capacity);
            assert_eq!(output.matches("&quot;&#39;&amp;&lt;&gt;💖").count(), 6);
        }
    }

    #[test]
    fn html_empty_canvases_keep_valid_containers() {
        for (width, height) in [(0, 0), (0, 3), (3, 0)] {
            for mode in [Mode::Table, Mode::Grid] {
                let mut canvas = Canvas::new(width, height, Color("#000"), Color("#fff"));
                canvas.set_mode(mode);
                let output = canvas.into_image();
                assert!(output.starts_with("<!DOCTYPE html>"));
                assert!(output.ends_with("</body></html>"));
                assert!(!output.contains("background:"));
            }
        }
    }

    #[test]
    #[should_panic(expected = "rendered output exceeds coordinate or backend resource limits")]
    fn direct_html_constructor_rejects_excessive_area() {
        let _ = Canvas::new(65_536, 65_536, Color("#000"), Color("#fff"));
    }

    fn sample_html() -> String {
        let modules =
            [qrcode_core::Color::Dark, qrcode_core::Color::Light, qrcode_core::Color::Light, qrcode_core::Color::Dark];
        qrcode_render::Renderer::<Color>::new(&modules, 2, 1).build()
    }

    #[test]
    fn test_html_table_render() {
        let html = sample_html();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<table"));
        assert!(html.contains("</table>"));
        assert!(html.contains("#000"));
        assert!(html.contains("#fff"));
    }

    #[test]
    fn test_html_custom_colors() {
        let modules =
            [qrcode_core::Color::Dark, qrcode_core::Color::Light, qrcode_core::Color::Light, qrcode_core::Color::Dark];
        let html = qrcode_render::Renderer::<Color>::new(&modules, 2, 1)
            .dark_color(Color("#333"))
            .light_color(Color("#eee"))
            .build();
        assert!(html.contains("#333"));
        assert!(html.contains("#eee"));
    }

    #[test]
    fn test_aria_label_injected_into_container() {
        let html = sample_html();
        let html = super::aria_label(&html, "a QR code");
        // both attributes land inside the <table …> opening tag
        let start = html.find("<table").unwrap();
        let tag_end = start + html[start..].find('>').unwrap();
        assert!(html[start..tag_end].contains(r#"role="img""#));
        assert!(html[start..tag_end].contains(r#"aria-label="a QR code""#));
    }

    #[test]
    fn attribute_injection_preserves_quoted_delimiters_and_self_closing_tags() {
        let cases = [
            (r#"<table data-note="a>b"><tr></tr></table>"#, r#"<table data-note="a>b" class="qr"><tr></tr></table>"#),
            (
                r#"<div data-note='二维码 > "说明"'>text</div>"#,
                r#"<div data-note='二维码 > "说明"' class="qr">text</div>"#,
            ),
            (r#"<div data-note="/>"/>"#, r#"<div data-note="/>" class="qr"/>"#),
            (r#"<div />"#, r#"<div  class="qr"/>"#),
        ];
        for (input, expected) in cases {
            assert_eq!(super::inject_attributes(input, &[("class", "qr")]), expected);
            assert_eq!(super::inject_attributes(input, &[]), input);
            assert_eq!(super::inject_attributes(input, &[("invalid name", "ignored")]), input);
        }
    }

    #[test]
    fn attribute_injection_keeps_missing_or_unclosed_html_unchanged() {
        for input in
            ["", "<span>text</span>", "<table", r#"<table data-note="unfinished>"#, "<div data-note='unfinished>"]
        {
            assert_eq!(super::inject_attributes(input, &[("class", "qr")]), input);
        }
    }

    #[test]
    fn attribute_injection_keeps_generated_table_and_grid_legacy_bytes() {
        let mut grid = Canvas::new(2, 2, Color("#000"), Color("#fff"));
        grid.set_mode(Mode::Grid);
        grid.draw_dark_pixel(1, 1);
        for input in [sample_html(), grid.into_image()] {
            let start = input.find("<table").or_else(|| input.find("<div")).unwrap();
            let position = start + input[start..].find('>').unwrap();
            let mut expected = String::from(&input[..position]);
            expected.push_str(" class=\"qr\"");
            expected.push_str(&input[position..]);
            assert_eq!(super::inject_attributes(&input, &[("class", "qr")]), expected);
        }
    }

    #[test]
    fn injected_attributes_reserve_exact_escaped_bytes_and_skip_invalid_names() {
        let ignored = "\"".repeat(4096);
        let attrs = [("invalid name", ignored.as_str()), ("data-note", "&\"'<>💖")];
        let expected = r#"<div data-note="&amp;&quot;&#39;&lt;&gt;💖"/>"#;
        assert_eq!(super::injected_attr_capacity(6, &attrs), Some(expected.len()));
        assert_eq!(super::inject_attributes("<div/>", &attrs), expected);
        let invalid_attrs = [("invalid name", ignored.as_str())];
        assert_eq!(super::injected_attr_capacity(6, &invalid_attrs), Some(6));
        assert_eq!(super::inject_attributes("<div/>", &invalid_attrs), "<div/>");
        assert_eq!(super::injected_attr_capacity(isize::MAX as usize, &[("class", "qr")]), None);
        assert_eq!(super::injected_attr_capacity(usize::MAX, &[]), None);
    }

    #[test]
    fn attributes_target_real_containers_after_non_markup_sections() {
        let cases = [
            (r#"<!-- example <table> --><table/>"#, r#"<!-- example <table> --><table class="qr"/>"#),
            (r#"<?example fake="<table>"?><table/>"#, r#"<?example fake="<table>"?><table class="qr"/>"#),
            (r#"<?example><table/>"#, r#"<?example><table class="qr"/>"#),
            (
                r#"<!DOCTYPE html [<!ENTITY example "<table>">]><table/>"#,
                r#"<!DOCTYPE html [<!ENTITY example "<table>">]><table class="qr"/>"#,
            ),
            (r#"<![CDATA[<table/>]]><table/>"#, r#"<![CDATA[<table/>]]><table class="qr"/>"#),
            (r#"<table-shadow/><table/>"#, r#"<table-shadow/><table class="qr"/>"#),
            (r#"<div-shadow/><div/>"#, r#"<div-shadow/><div class="qr"/>"#),
            (r#"<TABLE data-note='a>b'/>"#, r#"<TABLE data-note='a>b' class="qr"/>"#),
            (
                r#"<section data-example='<table>'><table/></section>"#,
                r#"<section data-example='<table>'><table class="qr"/></section>"#,
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(super::inject_attributes(input, &[("class", "qr")]), expected);
        }
    }

    #[test]
    fn raw_text_elements_do_not_supply_fake_containers_and_table_priority_is_preserved() {
        for element in ["script", "STYLE", "title", "textarea", "iframe", "XMP", "noembed", "noframes", "noscript"] {
            let input = alloc::format!("<{element}>'<table>'; '<div>'</{element}><div/><table/>");
            let expected = alloc::format!("<{element}>'<table>'; '<div>'</{element}><div/><table class=\"qr\"/>");
            assert_eq!(super::inject_attributes(&input, &[("class", "qr")]), expected);
            let input = alloc::format!("<{element}>'<table>'; '<div>'</{element}><div/>");
            let expected = alloc::format!("<{element}>'<table>'; '<div>'</{element}><div class=\"qr\"/>");
            assert_eq!(super::inject_attributes(&input, &[("class", "qr")]), expected);
        }
    }

    #[test]
    fn plaintext_consumes_fake_containers_through_eof() {
        for input in [
            "<plaintext><table/><div/>",
            "<PLAINTEXT>example </plaintext><table/><div/>",
            "<plaintext/>example<table/><div/>",
        ] {
            assert_eq!(super::inject_attributes(input, &[("class", "qr")]), input);
        }
        let input = "<div/><plaintext><table/>";
        assert_eq!(super::inject_attributes(input, &[("class", "qr")]), "<div class=\"qr\"/><plaintext><table/>");
    }

    #[test]
    fn malformed_or_fake_only_markup_keeps_html_unchanged() {
        for input in [
            "<!-- <table>",
            "<?fake <table>",
            "<![CDATA[<table>",
            "<!DOCTYPE html [<table>",
            "<table-shadow/>",
            "<script>'<table>'",
        ] {
            assert_eq!(super::inject_attributes(input, &[("class", "qr")]), input);
        }
    }

    #[test]
    fn colors_are_html_escaped() {
        let modules =
            [qrcode_core::Color::Dark, qrcode_core::Color::Light, qrcode_core::Color::Light, qrcode_core::Color::Dark];
        let html = qrcode_render::Renderer::<Color>::new(&modules, 2, 1)
            .dark_color(Color(r##"red"><script>alert(1)</script><span style="color:red"##))
            .light_color(Color(r#"" onload="alert(1)"#))
            .build();

        assert!(!html.contains("<script>"));
        assert!(!html.contains(" onload=\""));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&quot;"));
    }

    #[test]
    fn injected_attribute_values_are_html_escaped() {
        let html = super::aria_label(&sample_html(), r#"QR "><script>alert(1)</script>"#);

        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(html.contains("&quot;&gt;"));
    }

    #[test]
    fn invalid_attribute_names_are_skipped() {
        let html = super::inject_attributes(&sample_html(), &[(r#"x" onload="alert(1)"#, "bad"), ("data-ok", "yes")]);

        assert!(!html.contains("onload"));
        assert!(html.contains(r#"data-ok="yes""#));
    }
}
