//! EPS rendering support.
//!
//! The implementation lives in the `qrcode-eps` crate and is re-exported here
//! to preserve the 1.x `qrcode_rs::render::eps` module path.

#![cfg(feature = "eps")]

pub use qrcode_eps::*;
