//! Rendering pipeline for converting QR codes into visual output.
//!
//! This module provides the [`Pixel`] and [`Canvas`] traits that abstract over
//! different output formats, and the [`Renderer`] builder that drives the
//! conversion from QR code data to a final image.
//!
//! # Supported formats
//!
//! | Module  | Feature | Output type |
//! |---------|---------|-------------|
//! | `image` | `image` | PNG/JPEG via the `image` crate |
//! | `svg`   | `svg`   | SVG XML string |
//! | `eps`   | `eps`   | Encapsulated PostScript |
//! | `html`  | `html`  | HTML table or CSS Grid |
//! | `pic`   | `pic`   | PIC (troff) macros |
//! | `string`| —       | Plain text with custom characters |
//! | `unicode`| —      | Unicode block-element rendering |
//!
//! # Custom rendering
//!
//! Implement [`Pixel`] for your own type to render into a custom format.
//! The [`Pixel`] trait defines how to create dark/light pixels and how to
//! finalize a canvas into a concrete image.

use crate::cast::As;
use crate::types::Color;
use core::cmp::max;

pub mod ansi;
pub mod colors;
pub mod eps;
pub mod html;
pub mod image;
pub mod pdf;
pub mod pic;
pub mod string;
pub mod svg;
pub mod unicode;

//------------------------------------------------------------------------------
//{{{ Pixel trait

/// Abstraction of an image pixel.
pub trait Pixel: Copy + Sized {
    /// Type of the finalized image.
    type Image: Sized + 'static;

    /// The type that stores an intermediate buffer before finalizing to a
    /// concrete image
    type Canvas: Canvas<Pixel = Self, Image = Self::Image>;

    /// Obtains the default module size. The result must be at least 1×1.
    fn default_unit_size() -> (u32, u32) {
        (8, 8)
    }

    /// Obtains the default pixel color when a module is dark or light.
    fn default_color(color: Color) -> Self;
}

/// Rendering canvas of a QR code image.
pub trait Canvas: Sized {
    /// The pixel type stored in this canvas.
    type Pixel: Sized;
    /// The finalized image type produced from this canvas.
    type Image: Sized;

    /// Constructs a new canvas of the given dimensions.
    fn new(width: u32, height: u32, dark_pixel: Self::Pixel, light_pixel: Self::Pixel) -> Self;

    /// Draws a single dark pixel at the (x, y) coordinate.
    fn draw_dark_pixel(&mut self, x: u32, y: u32);

    /// Draws a filled dark rectangle covering the given module range. Default
    /// implementation fills it pixel by pixel; override for a faster path.
    fn draw_dark_rect(&mut self, left: u32, top: u32, width: u32, height: u32) {
        for y in top..(top + height) {
            for x in left..(left + width) {
                self.draw_dark_pixel(x, y);
            }
        }
    }

    /// Finalize the canvas to a real image.
    fn into_image(self) -> Self::Image;
}

//}}}
//------------------------------------------------------------------------------
//{{{ Renderer

/// A QR code renderer. This is a builder type which converts a bool-vector into
/// an image.
pub struct Renderer<'a, P: Pixel> {
    content: &'a [Color],
    modules_count: u32, // <- we call it `modules_count` here to avoid ambiguity of `width`.
    quiet_zone: u32,
    module_size: (u32, u32),

    dark_color: P,
    light_color: P,
    has_quiet_zone: bool,
}

impl<'a, P: Pixel> Renderer<'a, P> {
    /// Creates a new renderer.
    ///
    /// # Panics
    /// panics if content is not `modules_count` squared big
    pub fn new(content: &'a [Color], modules_count: usize, quiet_zone: u32) -> Renderer<'a, P> {
        assert_eq!(modules_count * modules_count, content.len());
        Renderer {
            content,
            modules_count: modules_count.as_u32(),
            quiet_zone,
            module_size: P::default_unit_size(),
            dark_color: P::default_color(Color::Dark),
            light_color: P::default_color(Color::Light),
            has_quiet_zone: true,
        }
    }

    /// Sets color of a dark module. Default is opaque black.
    pub fn dark_color(&mut self, color: P) -> &mut Self {
        self.dark_color = color;
        self
    }

    /// Sets color of a light module. Default is opaque white.
    pub fn light_color(&mut self, color: P) -> &mut Self {
        self.light_color = color;
        self
    }

    /// Whether to include the quiet zone in the generated image.
    pub fn quiet_zone(&mut self, has_quiet_zone: bool) -> &mut Self {
        self.has_quiet_zone = has_quiet_zone;
        self
    }

    /// Sets the size of each module in pixels. Default is 8×8.
    pub fn module_dimensions(&mut self, width: u32, height: u32) -> &mut Self {
        self.module_size = (max(width, 1), max(height, 1));
        self
    }

    /// Sets the minimum total image size in pixels, including the quiet zone if
    /// applicable. The renderer will try to find the dimension as small as
    /// possible, such that each module in the QR code has uniform size (no
    /// distortion).
    ///
    /// For instance, a version 1 QR code has 19 modules across including the
    /// quiet zone. If we request an image of size ≥200×200, we get that each
    /// module's size should be 11×11, so the actual image size will be 209×209.
    pub fn min_dimensions(&mut self, width: u32, height: u32) -> &mut Self {
        let quiet_zone = if self.has_quiet_zone { 2 } else { 0 } * self.quiet_zone;
        let width_in_modules = self.modules_count + quiet_zone;
        let unit_width = width.div_ceil(width_in_modules);
        let unit_height = height.div_ceil(width_in_modules);
        self.module_dimensions(unit_width, unit_height)
    }

    /// Sets the maximum total image size in pixels, including the quiet zone if
    /// applicable. The renderer will try to find the dimension as large as
    /// possible, such that each module in the QR code has uniform size (no
    /// distortion).
    ///
    /// For instance, a version 1 QR code has 19 modules across including the
    /// quiet zone. If we request an image of size ≤200×200, we get that each
    /// module's size should be 10×10, so the actual image size will be 190×190.
    ///
    /// The module size is at least 1×1, so if the restriction is too small, the
    /// final image *can* be larger than the input.
    pub fn max_dimensions(&mut self, width: u32, height: u32) -> &mut Self {
        let quiet_zone = if self.has_quiet_zone { 2 } else { 0 } * self.quiet_zone;
        let width_in_modules = self.modules_count + quiet_zone;
        let unit_width = width / width_in_modules;
        let unit_height = height / width_in_modules;
        self.module_dimensions(unit_width, unit_height)
    }

    /// Sets dimensions suitable for web display (200×200 pixels minimum).
    ///
    /// This is a convenience preset for embedding QR codes in web pages.
    /// The actual size may be slightly larger to maintain uniform module sizing.
    pub fn for_web(&mut self) -> &mut Self {
        self.min_dimensions(200, 200)
    }

    /// Sets dimensions suitable for printing at the specified DPI.
    ///
    /// Targets a 1-inch × 1-inch physical size. For example, at 300 DPI
    /// the image will be at least 300×300 pixels.
    ///
    /// # Arguments
    ///
    /// * `dpi` - Dots per inch (common values: 150 for draft, 300 for standard, 600 for high quality)
    pub fn for_print(&mut self, dpi: u32) -> &mut Self {
        self.min_dimensions(dpi.max(72), dpi.max(72))
    }

    /// Sets dimensions suitable for social media platform sharing.
    ///
    /// Targets platform-recommended sizes:
    ///
    /// | Platform       | Size (px) |
    /// |----------------|-----------|
    /// | `"twitter"`    | 400×400   |
    /// | `"facebook"`   | 600×600   |
    /// | `"instagram"`  | 1080×1080 |
    /// | `"wechat"`     | 600×600   |
    /// | Any other      | 400×400   |
    pub fn for_social(&mut self, platform: &str) -> &mut Self {
        let size = match platform {
            "twitter" | "x" => 400,
            "facebook" | "fb" => 600,
            "instagram" | "ig" => 1080,
            "wechat" | "weixin" => 600,
            _ => 400,
        };
        self.min_dimensions(size, size)
    }

    /// Renders the QR code into an image.
    pub fn build(&self) -> P::Image {
        let w = self.modules_count;
        let qz = if self.has_quiet_zone { self.quiet_zone } else { 0 };
        let width = w + 2 * qz;

        let (mw, mh) = self.module_size;
        let real_width = width * mw;
        let real_height = width * mh;

        let mut canvas = P::Canvas::new(real_width, real_height, self.dark_color, self.light_color);
        let mut i = 0;
        for y in 0..width {
            for x in 0..width {
                if qz <= x && x < w + qz && qz <= y && y < w + qz {
                    if self.content[i] != Color::Light {
                        canvas.draw_dark_rect(x * mw, y * mh, mw, mh);
                    }
                    i += 1;
                }
            }
        }

        canvas.into_image()
    }
}

//}}}
