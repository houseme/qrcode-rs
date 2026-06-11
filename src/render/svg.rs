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
}
