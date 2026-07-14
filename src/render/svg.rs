//! SVG rendering support.
//!
//! The implementation lives in the `qrcode-svg` crate and is re-exported here
//! to preserve the 1.x `qrcode_rs::render::svg` module path.

#![cfg(feature = "svg")]

pub use qrcode_svg::*;
