//! EPS rendering support.
//!
//! # Example
//!
//! ```
//! use qrcode_core::Color as ModuleColor;
//! use qrcode_eps::Color;
//! use qrcode_render::Renderer;
//!
//! let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
//! let eps = Renderer::<Color>::new(&modules, 2, 1).build();
//! println!("{eps}");
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

use qrcode_core::Color as ModuleColor;
use qrcode_render::{Canvas as RenderCanvas, Pixel, StyledPixel};

/// An EPS color (`[R, G, B]`).
///
/// Each value must be in the range of 0.0 to 1.0.
#[derive(Copy, Clone, Default, PartialEq, PartialOrd)]
pub struct Color(pub [f64; 3]);

impl Pixel for Color {
    type Canvas = Canvas;
    type Image = String;

    fn default_color(color: ModuleColor) -> Self {
        Self(color.select(Default::default(), [1.0; 3]))
    }
}

impl StyledPixel for Color {
    fn from_hex(hex: &str) -> Self {
        let (r, g, b) = qrcode_render::colors::hex_to_rgb(hex).unwrap_or((0, 0, 0));
        Self([r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0])
    }
}

#[doc(hidden)]
pub struct Canvas {
    eps: String,
    height: u32,
}

impl RenderCanvas for Canvas {
    type Pixel = Color;
    type Image = String;

    fn new(width: u32, height: u32, dark_pixel: Color, light_pixel: Color) -> Self {
        let mut eps = format!(
            concat!(
                "%!PS-Adobe-3.0 EPSF-3.0\n",
                "%%BoundingBox: 0 0 {w} {h}\n",
                "%%Pages: 1\n",
                "%%EndComments\n",
                "gsave\n",
                "{bgr} {bgg} {bgb} setrgbcolor\n",
                "0 0 {w} {h} rectfill\n",
                "grestore\n",
                "{fgr} {fgg} {fgb} setrgbcolor\n"
            ),
            w = width,
            h = height,
            fgr = dark_pixel.0[0],
            fgg = dark_pixel.0[1],
            fgb = dark_pixel.0[2],
            bgr = light_pixel.0[0],
            bgg = light_pixel.0[1],
            bgb = light_pixel.0[2],
        );
        // Preallocate for the worst-case dark-module rectfill lines (~20 B each),
        // matching the per-module heuristic used by the HTML renderer.
        eps.reserve((width as usize) * (height as usize) * 20);
        Self { eps, height }
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        self.draw_dark_rect(x, y, 1, 1);
    }

    fn draw_dark_rect(&mut self, left: u32, top: u32, width: u32, height: u32) {
        let bottom = self.height - top;
        writeln!(self.eps, "{left} {bottom} {width} {height} rectfill").unwrap();
    }

    fn into_image(mut self) -> String {
        self.eps.push_str("%%EOF");
        self.eps
    }
}

#[cfg(test)]
mod tests {
    use super::Color;
    use qrcode_render::StyledPixel;

    #[test]
    fn eps_renderer_outputs_bounding_box_and_rects() {
        let modules =
            [qrcode_core::Color::Dark, qrcode_core::Color::Light, qrcode_core::Color::Light, qrcode_core::Color::Dark];

        let eps =
            qrcode_render::Renderer::<Color>::new(&modules, 2, 1).quiet_zone(false).module_dimensions(1, 1).build();

        assert!(eps.starts_with("%!PS-Adobe-3.0 EPSF-3.0"));
        assert!(eps.contains("%%BoundingBox: 0 0 2 2"));
        assert!(eps.contains("0 2 1 1 rectfill"));
        assert!(eps.ends_with("%%EOF"));
    }

    #[test]
    fn eps_color_parses_hex_for_templates() {
        let color = Color::from_hex("#336699");

        assert!((color.0[0] - 0.2).abs() < f64::EPSILON);
        assert!((color.0[1] - 0.4).abs() < f64::EPSILON);
        assert!((color.0[2] - 0.6).abs() < f64::EPSILON);
    }
}
