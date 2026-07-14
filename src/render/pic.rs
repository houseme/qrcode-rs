//! PIC rendering support.
//!
//! The implementation lives in the `qrcode-pic` crate and is re-exported here
//! to preserve the 1.x `qrcode_rs::render::pic` module path.

#![cfg(feature = "pic")]

pub use qrcode_pic::*;
