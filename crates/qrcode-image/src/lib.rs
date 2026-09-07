//! Image rendering support for `qrcode-rs`.
//!
//! This crate is the format-specific home for the image backend. The actual
//! renderer implementation is shared with [`qrcode-render`] and re-exported
//! here so applications can depend on a narrow, image-focused crate without
//! changing the renderer API.
//!
//! The crate has no default features. Enable `image` for image pixels and
//! helpers; enable only `std` when an application needs the shared renderer
//! traits without pulling in the image library.
//!
//! # Example
//!
//! ```
//! # #[cfg(feature = "image")]
//! # {
//! use qrcode_core::Color;
//! use qrcode_image::{Luma, Renderer};
//!
//! let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
//! let image = Renderer::<Luma<u8>>::new(&modules, 2, 1).build();
//! assert_eq!(image.dimensions(), (32, 32));
//! # }
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

pub use qrcode_render::{Canvas, Pixel, RenderError, RenderTemplate, Renderer, StyledPixel};

#[cfg(feature = "image")]
use qrcode_core::ModuleSource;

#[cfg(feature = "image")]
pub mod image {
    //! Image crate types and image-specific QR helpers.

    pub use ::image::*;
    pub use qrcode_render::image::{
        Gradient, GradientDirection, ImageFormat, apply_gradient_background, encode_to_format, overlay_logo,
    };
}

#[cfg(feature = "image")]
pub use ::image::{DynamicImage, GrayImage, ImageBuffer, Luma, LumaA, Rgb, RgbImage, Rgba, RgbaImage};

#[cfg(feature = "image")]
pub use qrcode_render::image::{
    Gradient, GradientDirection, ImageFormat, apply_gradient_background, encode_to_format, overlay_logo,
};

#[cfg(feature = "image")]
/// Rendering options shared by the high-level image helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderOptions {
    /// Quiet-zone width in modules when [`Self::include_quiet_zone`] is true.
    pub quiet_zone: u32,
    /// Width of each QR module in output pixels.
    pub module_width: u32,
    /// Height of each QR module in output pixels.
    pub module_height: u32,
    /// Whether to include the quiet zone around the QR modules.
    pub include_quiet_zone: bool,
}

#[cfg(feature = "image")]
impl Default for RenderOptions {
    fn default() -> Self {
        Self { quiet_zone: 4, module_width: 8, module_height: 8, include_quiet_zone: true }
    }
}

#[cfg(feature = "image")]
impl RenderOptions {
    /// Sets the quiet-zone width in modules.
    #[must_use]
    pub const fn quiet_zone(mut self, quiet_zone: u32) -> Self {
        self.quiet_zone = quiet_zone;
        self
    }

    /// Sets the output size of each QR module in pixels.
    #[must_use]
    pub const fn module_size(mut self, width: u32, height: u32) -> Self {
        self.module_width = width;
        self.module_height = height;
        self
    }

    /// Disables the quiet zone around the QR modules.
    #[must_use]
    pub const fn without_quiet_zone(mut self) -> Self {
        self.include_quiet_zone = false;
        self
    }
}

#[cfg(feature = "image")]
/// Errors returned by the high-level image rendering helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageRenderError {
    /// The source module grid is invalid.
    Render(RenderError),
    /// The requested output dimensions exceed the image crate's `u32` limits.
    OutputTooLarge {
        /// Requested output width in pixels.
        width: u32,
        /// Requested output height in pixels.
        height: u32,
    },
}

#[cfg(feature = "image")]
impl core::fmt::Display for ImageRenderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Render(error) => error.fmt(f),
            Self::OutputTooLarge { width, height } => {
                write!(f, "requested image dimensions are too large: {width}x{height}")
            }
        }
    }
}

#[cfg(feature = "image")]
impl std::error::Error for ImageRenderError {}

#[cfg(feature = "image")]
impl From<RenderError> for ImageRenderError {
    fn from(error: RenderError) -> Self {
        Self::Render(error)
    }
}

#[cfg(feature = "image")]
/// Errors returned by direct source-to-image encoding helpers.
#[derive(Debug)]
pub enum EncodeError {
    /// Rendering the source failed.
    Render(ImageRenderError),
    /// Encoding the rendered image failed.
    Image(::image::ImageError),
}

#[cfg(feature = "image")]
impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Render(error) => error.fmt(f),
            Self::Image(error) => error.fmt(f),
        }
    }
}

#[cfg(feature = "image")]
impl std::error::Error for EncodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Render(error) => Some(error),
            Self::Image(error) => Some(error),
        }
    }
}

#[cfg(feature = "image")]
/// Renders any borrowed module source with the image backend.
///
/// The source remains borrowed and the options are applied consistently to
/// any image pixel type supported by `qrcode-render`.
///
/// # Errors
///
/// Returns an error for malformed module grids or when the requested image
/// dimensions cannot be represented by the `image` crate.
pub fn render<P, S>(source: &S, options: RenderOptions) -> Result<P::Image, ImageRenderError>
where
    P: Pixel,
    S: ModuleSource + ?Sized,
{
    let mut renderer = Renderer::<P>::try_from_source(source, options.quiet_zone)?;
    let module_width = options.module_width.max(1);
    let module_height = options.module_height.max(1);
    let quiet_zone = if options.include_quiet_zone { options.quiet_zone } else { 0 };
    let quiet_zone_pixels =
        quiet_zone.checked_mul(2).ok_or(ImageRenderError::OutputTooLarge { width: u32::MAX, height: u32::MAX })?;
    let total_modules = (source.width() as u32)
        .checked_add(quiet_zone_pixels)
        .ok_or(ImageRenderError::OutputTooLarge { width: u32::MAX, height: u32::MAX })?;
    let width = total_modules
        .checked_mul(module_width)
        .ok_or(ImageRenderError::OutputTooLarge { width: u32::MAX, height: u32::MAX })?;
    let height =
        total_modules.checked_mul(module_height).ok_or(ImageRenderError::OutputTooLarge { width, height: u32::MAX })?;
    let _ = (width, height);
    renderer.module_dimensions(module_width, module_height).quiet_zone(options.include_quiet_zone);
    Ok(renderer.build())
}

#[cfg(feature = "image")]
/// Renders a module source as an RGBA8 image.
pub fn render_rgba<S>(source: &S, options: RenderOptions) -> Result<RgbaImage, ImageRenderError>
where
    S: ModuleSource + ?Sized,
{
    render::<Rgba<u8>, _>(source, options)
}

#[cfg(feature = "image")]
/// Renders a module source as a grayscale 8-bit image.
pub fn render_luma<S>(source: &S, options: RenderOptions) -> Result<GrayImage, ImageRenderError>
where
    S: ModuleSource + ?Sized,
{
    render::<Luma<u8>, _>(source, options)
}

#[cfg(feature = "image")]
/// Renders a module source as a dynamic RGBA8 image.
pub fn render_dynamic<S>(source: &S, options: RenderOptions) -> Result<DynamicImage, ImageRenderError>
where
    S: ModuleSource + ?Sized,
{
    Ok(DynamicImage::ImageRgba8(render_rgba(source, options)?))
}

#[cfg(feature = "image")]
fn encode_source<S>(source: &S, options: RenderOptions, format: ImageFormat) -> Result<Vec<u8>, EncodeError>
where
    S: ModuleSource + ?Sized,
{
    let image = render_dynamic(source, options).map_err(EncodeError::Render)?;
    encode_to_format(&image, format).map_err(EncodeError::Image)
}

#[cfg(feature = "image")]
/// Renders and encodes a module source as PNG bytes.
pub fn encode_png<S>(source: &S, options: RenderOptions) -> Result<Vec<u8>, EncodeError>
where
    S: ModuleSource + ?Sized,
{
    encode_source(source, options, ImageFormat::Png)
}

#[cfg(feature = "image")]
/// Renders and encodes a module source as JPEG bytes.
pub fn encode_jpeg<S>(source: &S, options: RenderOptions) -> Result<Vec<u8>, EncodeError>
where
    S: ModuleSource + ?Sized,
{
    encode_source(source, options, ImageFormat::Jpeg)
}

#[cfg(all(test, feature = "image"))]
mod tests {
    use super::{Luma, RenderOptions, Renderer, encode_jpeg, encode_png, render_dynamic, render_luma};
    use qrcode_core::{Color, ModuleView};

    #[test]
    fn image_backend_reexports_rendered_pixels() {
        let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
        let image = Renderer::<Luma<u8>>::new(&modules, 2, 1).build();

        assert_eq!(image.dimensions(), (32, 32));
        assert_eq!(image.get_pixel(8, 8).0, [0]);
        assert_eq!(image.get_pixel(16, 8).0, [255]);
    }

    #[test]
    fn high_level_render_honors_options_without_quiet_zone() {
        let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
        let source = ModuleView::new(&modules, 2).expect("valid test grid");
        let image = render_luma(&source, RenderOptions::default().module_size(3, 2).without_quiet_zone())
            .expect("source is valid");

        assert_eq!(image.dimensions(), (6, 4));
    }

    #[test]
    fn source_helpers_encode_png_and_jpeg() {
        let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
        let source = ModuleView::new(&modules, 2).expect("valid test grid");
        let options = RenderOptions::default().module_size(2, 2);

        let png = encode_png(&source, options).expect("PNG encoding should succeed");
        let jpeg = encode_jpeg(&source, options).expect("JPEG encoding should succeed");

        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(jpeg.starts_with(&[0xff, 0xd8, 0xff]));
    }

    #[test]
    fn dynamic_render_returns_rgba_image() {
        let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
        let source = ModuleView::new(&modules, 2).expect("valid test grid");
        let image = render_dynamic(&source, RenderOptions::default().without_quiet_zone()).expect("source is valid");

        assert_eq!(image.color(), ::image::ColorType::Rgba8);
    }

    #[test]
    fn oversized_output_is_rejected_before_building() {
        let modules = [Color::Dark];
        let source = ModuleView::new(&modules, 1).expect("valid test grid");
        let error = render_luma(&source, RenderOptions::default().module_size(u32::MAX, 1)).unwrap_err();

        assert!(matches!(error, super::ImageRenderError::OutputTooLarge { .. }));
    }
}
