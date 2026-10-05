//! Private QR-only engine derived from rxing 0.9.3.
//! Source algorithms retain their original copyright and license notices.
#![allow(non_snake_case, non_camel_case_types)]
// Preserve QR terminology, arithmetic from the reviewed port, and Rust 1.88 compatibility.
#![allow(clippy::upper_case_acronyms, clippy::manual_is_multiple_of, clippy::chunks_exact_to_as_chunks)]

pub mod common;
mod exceptions;
pub mod qrcode;
pub use exceptions::Exceptions;
mod luminance_source;
pub use luminance_source::LuminanceSource;
mod luma_luma_source;
pub use luma_luma_source::Luma8Source;
mod binarizer;
pub use binarizer::Binarizer;
mod rxing_result_point;
pub use rxing_result_point::*;

#[cfg(test)]
mod result_point;
#[cfg(test)]
pub use result_point::*;
