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
pub mod image {
    //! Image crate types and image-specific QR helpers.

    pub use ::image::*;
    pub use qrcode_render::image::{
        Gradient, GradientDirection, ImageFormat, apply_gradient_background, encode_to_format, overlay_logo,
    };
}

#[cfg(feature = "image")]
pub use ::image::{DynamicImage, GrayImage, ImageBuffer, Luma, LumaA, Rgb, Rgba};

#[cfg(feature = "image")]
pub use qrcode_render::image::{
    Gradient, GradientDirection, ImageFormat, apply_gradient_background, encode_to_format, overlay_logo,
};

#[cfg(all(test, feature = "image"))]
mod tests {
    use super::{Luma, Renderer};
    use qrcode_core::Color;

    #[test]
    fn image_backend_reexports_rendered_pixels() {
        let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
        let image = Renderer::<Luma<u8>>::new(&modules, 2, 1).build();

        assert_eq!(image.dimensions(), (32, 32));
        assert_eq!(image.get_pixel(8, 8).0, [0]);
        assert_eq!(image.get_pixel(16, 8).0, [255]);
    }
}
