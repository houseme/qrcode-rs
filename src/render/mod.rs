//! Rendering facade.
//!
//! The pure rendering core (`Pixel`, `Canvas`, `Renderer`, string/unicode/ANSI
//! backends, and color helpers) lives in `qrcode-render` and is re-exported
//! here to preserve the 1.x `qrcode_rs::render::*` API. Feature-gated backends
//! still live in this facade crate until their dedicated crates are split out.

#[cfg(feature = "image")]
pub use qrcode_render::image;
pub use qrcode_render::{Canvas, Pixel, RenderError, RenderTemplate, Renderer, StyledPixel};
pub use qrcode_render::{ansi, colors, string, unicode};

pub mod eps;
pub mod html;
pub mod pdf;
pub mod pic;
pub mod svg;
