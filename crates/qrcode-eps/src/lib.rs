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
use qrcode_render::colors::{CmykColor as SharedCmykColor, ColorSpace, RgbColor};
use qrcode_render::{Canvas as RenderCanvas, Pixel, StyledPixel};

const MAX_STREAM_PREALLOC: usize = 8 * 1024 * 1024;

fn stream_capacity(width: u32, height: u32) -> usize {
    // Scaled vector output can cover many pixels with a single rectangle.
    (width as usize).saturating_mul(height as usize).saturating_mul(20).min(MAX_STREAM_PREALLOC)
}

/// An EPS color (`[R, G, B]`).
///
/// Each value must be in the range of 0.0 to 1.0.
#[derive(Copy, Clone, Default, PartialEq, PartialOrd)]
pub struct Color(pub [f64; 3]);

/// An EPS CMYK color (`[C, M, Y, K]`).
///
/// Each value is clamped into the range of 0.0 to 1.0 when constructed from the
/// shared render color space.
#[derive(Copy, Clone, Default, PartialEq, PartialOrd)]
pub struct CmykColor(pub [f64; 4]);

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

impl From<RgbColor> for Color {
    fn from(color: RgbColor) -> Self {
        Self(color.to_array())
    }
}

impl Pixel for CmykColor {
    type Canvas = CmykCanvas;
    type Image = String;

    fn default_color(color: ModuleColor) -> Self {
        Self(color.select([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 0.0]))
    }
}

impl StyledPixel for CmykColor {
    fn from_hex(hex: &str) -> Self {
        let (r, g, b) = qrcode_render::colors::hex_to_rgb(hex).unwrap_or((0, 0, 0));
        SharedCmykColor::from_rgb(RgbColor::new(r, g, b)).into()
    }
}

impl From<SharedCmykColor> for CmykColor {
    fn from(color: SharedCmykColor) -> Self {
        Self(color.to_array())
    }
}

#[doc(hidden)]
pub struct Canvas {
    eps: String,
    height: u32,
}

#[doc(hidden)]
pub struct CmykCanvas {
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
        eps.reserve(stream_capacity(width, height));
        Self { eps, height }
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        self.draw_dark_rect(x, y, 1, 1);
    }

    fn draw_dark_rect(&mut self, left: u32, top: u32, width: u32, height: u32) {
        let bottom = self.height - top - height;
        writeln!(self.eps, "{left} {bottom} {width} {height} rectfill").unwrap();
    }

    fn into_image(mut self) -> String {
        self.eps.push_str("%%EOF");
        self.eps
    }
}

impl RenderCanvas for CmykCanvas {
    type Pixel = CmykColor;
    type Image = String;

    fn new(width: u32, height: u32, dark_pixel: CmykColor, light_pixel: CmykColor) -> Self {
        let mut eps = format!(
            concat!(
                "%!PS-Adobe-3.0 EPSF-3.0\n",
                "%%BoundingBox: 0 0 {w} {h}\n",
                "%%Pages: 1\n",
                "%%EndComments\n",
                "gsave\n",
                "{bc} {bm} {by} {bk} setcmykcolor\n",
                "0 0 {w} {h} rectfill\n",
                "grestore\n",
                "{fc} {fm} {fy} {fk} setcmykcolor\n"
            ),
            w = width,
            h = height,
            fc = dark_pixel.0[0],
            fm = dark_pixel.0[1],
            fy = dark_pixel.0[2],
            fk = dark_pixel.0[3],
            bc = light_pixel.0[0],
            bm = light_pixel.0[1],
            by = light_pixel.0[2],
            bk = light_pixel.0[3],
        );
        eps.reserve(stream_capacity(width, height));
        Self { eps, height }
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        self.draw_dark_rect(x, y, 1, 1);
    }

    fn draw_dark_rect(&mut self, left: u32, top: u32, width: u32, height: u32) {
        let bottom = self.height - top - height;
        writeln!(self.eps, "{left} {bottom} {width} {height} rectfill").unwrap();
    }

    fn into_image(mut self) -> String {
        self.eps.push_str("%%EOF");
        self.eps
    }
}

#[cfg(test)]
mod tests {
    use super::{Canvas, CmykCanvas, CmykColor, Color, MAX_STREAM_PREALLOC, stream_capacity};
    use qrcode_render::{Canvas as RenderCanvas, Renderer, StyledPixel};

    #[test]
    fn eps_renderer_outputs_bounding_box_and_rects() {
        let modules =
            [qrcode_core::Color::Dark, qrcode_core::Color::Light, qrcode_core::Color::Light, qrcode_core::Color::Dark];

        let eps =
            qrcode_render::Renderer::<Color>::new(&modules, 2, 1).quiet_zone(false).module_dimensions(1, 1).build();

        assert!(eps.starts_with("%!PS-Adobe-3.0 EPSF-3.0"));
        assert!(eps.contains("%%BoundingBox: 0 0 2 2"));
        assert!(eps.contains("0 1 1 1 rectfill"));
        assert!(eps.contains("1 0 1 1 rectfill"));
        assert!(eps.ends_with("%%EOF"));
    }

    #[test]
    fn eps_color_parses_hex_for_templates() {
        let color = Color::from_hex("#336699");

        assert!((color.0[0] - 0.2).abs() < f64::EPSILON);
        assert!((color.0[1] - 0.4).abs() < f64::EPSILON);
        assert!((color.0[2] - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn eps_cmyk_renderer_outputs_native_cmyk_commands() {
        let modules =
            [qrcode_core::Color::Dark, qrcode_core::Color::Light, qrcode_core::Color::Light, qrcode_core::Color::Dark];

        let eps = Renderer::<CmykColor>::new(&modules, 2, 1)
            .dark_color(CmykColor([1.0, 0.0, 0.0, 0.25]))
            .light_color(CmykColor([0.0, 0.0, 0.0, 0.0]))
            .quiet_zone(false)
            .module_dimensions(1, 1)
            .build();

        assert!(eps.contains("0 0 0 0 setcmykcolor"));
        assert!(eps.contains("1 0 0 0.25 setcmykcolor"));
        assert!(!eps.contains("setrgbcolor"));
    }

    #[test]
    fn eps_rgb_rectangles_stay_inside_vertical_bounds() {
        let mut canvas = Canvas::new(8, 12, Color([0.0; 3]), Color([1.0; 3]));
        canvas.draw_dark_rect(1, 0, 4, 3);
        canvas.draw_dark_rect(2, 10, 3, 2);
        canvas.draw_dark_pixel(0, 11);

        assert!(canvas.into_image().ends_with("1 9 4 3 rectfill\n2 0 3 2 rectfill\n0 0 1 1 rectfill\n%%EOF"));
    }

    #[test]
    fn eps_cmyk_rectangles_stay_inside_vertical_bounds() {
        let mut canvas = CmykCanvas::new(8, 12, CmykColor([0.0, 0.0, 0.0, 1.0]), CmykColor([0.0; 4]));
        canvas.draw_dark_rect(1, 0, 4, 3);
        canvas.draw_dark_rect(2, 10, 3, 2);
        canvas.draw_dark_pixel(0, 11);

        assert!(canvas.into_image().ends_with("1 9 4 3 rectfill\n2 0 3 2 rectfill\n0 0 1 1 rectfill\n%%EOF"));
    }

    #[test]
    fn eps_stream_preallocation_is_bounded() {
        assert_eq!(stream_capacity(u32::MAX, u32::MAX), MAX_STREAM_PREALLOC);
        assert_eq!(stream_capacity(10_000, 10_000), MAX_STREAM_PREALLOC);
        assert_eq!(stream_capacity(2, 3), 120);
        assert_eq!(stream_capacity(0, u32::MAX), 0);
    }
}
