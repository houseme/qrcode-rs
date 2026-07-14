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
use core::fmt;
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

/// Errors returned by fallible render construction or rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderError {
    /// The module source is not a non-empty square row-major QR module grid.
    InvalidModuleSource {
        /// Source width in modules.
        width: usize,
        /// Source height in modules.
        height: usize,
        /// Number of row-major modules exposed by the source.
        len: usize,
    },

    /// The module source is wider than this renderer can represent internally.
    ModuleSourceTooWide {
        /// Source width in modules.
        width: usize,
    },
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::InvalidModuleSource { width, height, len } => {
                write!(f, "invalid module source dimensions: width={width}, height={height}, len={len}")
            }
            RenderError::ModuleSourceTooWide { width } => write!(f, "module source width {width} exceeds u32::MAX"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RenderError {}

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

    /// Creates a new renderer from a module-grid source.
    ///
    /// This is the read-only-source counterpart to [`Renderer::new`]. It is
    /// useful when rendering a borrowed view that implements
    /// [`qrcode_core::ModuleSource`] but does not expose facade-specific QR code
    /// methods.
    ///
    /// # Panics
    ///
    /// Panics if `source` is not square or if its row-major module slice length
    /// does not match `width() * height()`.
    pub fn from_source<C>(source: &'a C, quiet_zone: u32) -> Renderer<'a, P>
    where
        C: qrcode_core::ModuleSource + ?Sized,
    {
        match Self::try_from_source(source, quiet_zone) {
            Ok(renderer) => renderer,
            Err(err) => panic!("{err}"),
        }
    }

    /// Creates a new renderer from a QR symbol.
    ///
    /// This is the metadata-aware counterpart to [`Renderer::from_source`].
    /// The quiet zone is inferred from [`qrcode_core::QrSymbol::quiet_zone`].
    ///
    /// # Panics
    ///
    /// Panics if `symbol` exposes an invalid module grid.
    pub fn from_symbol<S>(symbol: &'a S) -> Renderer<'a, P>
    where
        S: qrcode_core::QrSymbol + ?Sized,
    {
        Self::from_source(symbol, symbol.quiet_zone())
    }

    /// Tries to create a new renderer from a module-grid source.
    ///
    /// Unlike [`Renderer::from_source`], this constructor reports malformed
    /// sources as [`RenderError`] instead of panicking.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::InvalidModuleSource`] when `source` is empty,
    /// non-square, or its row-major module slice length does not match
    /// `width() * height()`. Returns [`RenderError::ModuleSourceTooWide`] when
    /// the width cannot be represented by this renderer.
    pub fn try_from_source<C>(source: &'a C, quiet_zone: u32) -> Result<Renderer<'a, P>, RenderError>
    where
        C: qrcode_core::ModuleSource + ?Sized,
    {
        let width = source.width();
        let height = source.height();
        let len = source.modules().len();
        let Some(expected_len) = width.checked_mul(height) else {
            return Err(RenderError::InvalidModuleSource { width, height, len });
        };
        if width == 0 || width != height || len != expected_len {
            return Err(RenderError::InvalidModuleSource { width, height, len });
        }
        if width > u32::MAX as usize {
            return Err(RenderError::ModuleSourceTooWide { width });
        }
        Ok(Self::new(source.modules(), width, quiet_zone))
    }

    /// Tries to create a new renderer from a QR symbol.
    ///
    /// This is the fallible, metadata-aware counterpart to
    /// [`Renderer::try_from_source`]. The quiet zone is inferred from
    /// [`qrcode_core::QrSymbol::quiet_zone`].
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Renderer::try_from_source`] when the symbol
    /// exposes an invalid module grid.
    pub fn try_from_symbol<S>(symbol: &'a S) -> Result<Renderer<'a, P>, RenderError>
    where
        S: qrcode_core::QrSymbol + ?Sized,
    {
        Self::try_from_source(symbol, symbol.quiet_zone())
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
    C: qrcode_core::ModuleSource + ?Sized,
    P: Pixel,
{
    type Output = P::Image;
    type Error = RenderError;

    fn render(&self, code: &C) -> Result<Self::Output, Self::Error> {
        let mut renderer = Renderer::try_from_source(code, self.quiet_zone)?;
        renderer.module_size = self.module_size;
        renderer.dark_color = self.dark_color;
        renderer.light_color = self.light_color;
        renderer.has_quiet_zone = self.has_quiet_zone;
        Ok(renderer.build())
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

#[cfg(test)]
mod tests {
    use super::{RenderError, Renderer};
    use qrcode_core::{Color, EcLevel, ModuleSource, QrSymbol, Renderer as CoreRenderer, Version};

    struct BadSource {
        modules: [Color; 4],
    }

    impl ModuleSource for BadSource {
        fn get(&self, x: usize, y: usize) -> Color {
            self.modules[y * self.width() + x]
        }

        fn width(&self) -> usize {
            3
        }

        fn height(&self) -> usize {
            2
        }

        fn modules(&self) -> &[Color] {
            &self.modules
        }
    }

    struct SymbolSource {
        version: Version,
        modules: [Color; 1],
    }

    impl ModuleSource for SymbolSource {
        fn get(&self, _x: usize, _y: usize) -> Color {
            self.modules[0]
        }

        fn width(&self) -> usize {
            1
        }

        fn height(&self) -> usize {
            1
        }

        fn modules(&self) -> &[Color] {
            &self.modules
        }
    }

    impl QrSymbol for SymbolSource {
        fn version(&self) -> Version {
            self.version
        }

        fn error_correction_level(&self) -> EcLevel {
            EcLevel::M
        }
    }

    #[test]
    fn try_from_source_returns_error_for_invalid_dimensions() {
        let source = BadSource { modules: [Color::Dark; 4] };

        let result = Renderer::<char>::try_from_source(&source, 0);
        assert!(matches!(result, Err(RenderError::InvalidModuleSource { width: 3, height: 2, len: 4 })));
    }

    #[test]
    fn core_renderer_returns_error_for_invalid_source() {
        let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
        let renderer = Renderer::<char>::new(&modules, 2, 0);
        let source = BadSource { modules: [Color::Dark; 4] };

        assert_eq!(
            CoreRenderer::render(&renderer, &source).unwrap_err(),
            RenderError::InvalidModuleSource { width: 3, height: 2, len: 4 }
        );
    }

    #[test]
    fn from_symbol_uses_normal_qr_quiet_zone() {
        let source = SymbolSource { version: Version::Normal(1), modules: [Color::Dark] };

        let output = Renderer::<char>::from_symbol(&source).dark_color('#').light_color('.').build();

        assert_eq!(output.lines().next().map(str::len), Some(9));
    }

    #[test]
    fn from_symbol_uses_micro_qr_quiet_zone() {
        let source = SymbolSource { version: Version::Micro(1), modules: [Color::Dark] };

        let output = Renderer::<char>::from_symbol(&source).dark_color('#').light_color('.').build();

        assert_eq!(output.lines().next().map(str::len), Some(5));
    }
}
