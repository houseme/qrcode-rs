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

use core::cmp::max;
use qrcode_core::As;
pub use qrcode_core::Color;

pub mod ansi;
pub mod colors;
#[cfg(feature = "image")]
pub mod image;
pub mod string;
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

/// A [`Pixel`] constructible from a CSS-style hex color string (`"#rrggbb"` or
/// `"#rgb"`), used by `Renderer::template` to apply a `QrTemplate`.
///
/// Implemented for the owned, styled backends (image RGB/RGBA, EPS, PDF, ANSI).
/// The borrowing backends (`svg::Color`, `html::Color`) are not `StyledPixel`
/// because their color borrows from the input and can't be stored generically;
/// apply those colors manually instead.
pub trait StyledPixel: Pixel {
    /// Builds a pixel from a hex color string, falling back to black on an
    /// unparseable value.
    fn from_hex(hex: &str) -> Self;
}

/// Styling data that can be applied to a [`Renderer`] with
/// [`Renderer::template`].
///
/// The facade crate implements this for its `QrTemplate`, while downstream
/// crates can provide their own lightweight template types without depending on
/// the facade.
pub trait RenderTemplate {
    /// Dark module color as a CSS hex string.
    fn dark_color(&self) -> &str;

    /// Light module color as a CSS hex string.
    fn light_color(&self) -> &str;

    /// Optional module dimensions `(width, height)`.
    fn module_size(&self) -> Option<(u32, u32)>;

    /// Whether to include the quiet zone.
    fn quiet_zone(&self) -> bool;
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

impl<C, P> qrcode_core::Renderer<C> for Renderer<'_, P>
where
    C: qrcode_core::ModuleStorage + ?Sized,
    P: Pixel,
{
    type Output = P::Image;
    type Error = core::convert::Infallible;

    fn render(&self, _code: &C) -> Result<Self::Output, Self::Error> {
        Ok(self.build())
    }
}

impl<'a, P: StyledPixel> Renderer<'a, P> {
    /// Applies a render template: dark/light colors (via
    /// [`StyledPixel::from_hex`]), optional module size, and the quiet-zone
    /// setting.
    pub fn template<T: RenderTemplate>(mut self, tmpl: &T) -> Self {
        self.dark_color = P::from_hex(tmpl.dark_color());
        self.light_color = P::from_hex(tmpl.light_color());
        if let Some((w, h)) = tmpl.module_size() {
            self.module_size = (w, h);
        }
        self.has_quiet_zone = tmpl.quiet_zone();
        self
    }
}

//}}}
