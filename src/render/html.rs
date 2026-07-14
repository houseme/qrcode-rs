//! HTML rendering support.
//!
//! The implementation lives in the `qrcode-html` crate and is re-exported here
//! to preserve the 1.x `qrcode_rs::render::html` module path.

#![cfg(feature = "html")]

pub use qrcode_html::*;
