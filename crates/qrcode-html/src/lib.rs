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

use qrcode_core::{As, Color as ModuleColor};
use qrcode_render::{Canvas as RenderCanvas, Pixel};

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
}

impl<'a> Canvas<'a> {
    /// Sets the HTML rendering mode (Table or Grid).
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }
}

impl<'a> RenderCanvas for Canvas<'a> {
    type Pixel = Color<'a>;
    type Image = String;

    fn new(width: u32, height: u32, dark_pixel: Color<'a>, light_pixel: Color<'a>) -> Self {
        Canvas {
            dark_pixels: vec![false; (width * height).as_usize()],
            width,
            height,
            dark_color: dark_pixel.0,
            light_color: light_pixel.0,
            mode: Mode::default(),
            marker: PhantomData,
        }
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        let idx = (y * self.width + x).as_usize();
        if idx < self.dark_pixels.len() {
            self.dark_pixels[idx] = true;
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
        let cap = 512 + (self.width * self.height * 20) as usize;
        let mut html = String::with_capacity(cap);
        html.push_str(r#"<!DOCTYPE html><html><head><meta charset="UTF-8"></head><body><table style="border-collapse:collapse;line-height:0">"#);

        for y in 0..self.height {
            html.push_str("<tr>");
            for x in 0..self.width {
                let idx = (y * self.width + x).as_usize();
                let color = if self.dark_pixels[idx] { self.dark_color } else { self.light_color };
                html.push_str(r#"<td style="width:1px;height:1px;background:"#);
                html.push_str(color);
                html.push_str(r#""></td>"#);
            }
            html.push_str("</tr>");
        }

        html.push_str("</table></body></html>");
        html
    }

    fn into_grid(self) -> String {
        let cap = 512 + (self.width * self.height * 10) as usize;
        let mut html = String::with_capacity(cap);
        html.push_str(r#"<!DOCTYPE html><html><head><meta charset="UTF-8"></head><body><div style="display:grid;grid-template-columns:repeat("#);
        html.push_str(&self.width.to_string());
        html.push_str(r#",1px);line-height:0">"#);

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = (y * self.width + x).as_usize();
                let color = if self.dark_pixels[idx] { self.dark_color } else { self.light_color };
                html.push_str(r#"<div style="width:1px;height:1px;background:"#);
                html.push_str(color);
                html.push_str(r#""></div>"#);
            }
        }

        html.push_str("</div></body></html>");
        html
    }
}

/// Injects custom attributes into the QR container element (`<table>` in
/// [`Mode::Table`], `<div>` in [`Mode::Grid`]). If no container is found the
/// input is returned unchanged.
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
    let Some(start) = html.find("<table").or_else(|| html.find("<div")) else {
        return html.to_owned();
    };
    let Some(close) = html[start..].find('>').map(|p| start + p) else {
        return html.to_owned();
    };
    let mut result = String::with_capacity(html.len() + attrs.len() * 16);
    result.push_str(&html[..close]);
    for (key, value) in attrs {
        result.push(' ');
        result.push_str(key);
        result.push_str(r#"=""#);
        result.push_str(value);
        result.push('"');
    }
    result.push_str(&html[close..]);
    result
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
    use super::Color;
    use alloc::string::String;

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
}
