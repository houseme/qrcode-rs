//! PDF rendering support.
//!
//! The implementation lives in the `qrcode-pdf` crate and is re-exported here
//! to preserve the 1.x `qrcode_rs::render::pdf` module path.

#![cfg(feature = "pdf")]

pub use qrcode_pdf::*;
