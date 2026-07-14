//! QR code decoding (scan-to-data).
//!
//! This module defines the [`QrDecoder`] trait and the value types a decoder
//! returns ([`DecodedQrCode`]). The trait takes a borrowed grayscale view
//! ([`GrayPixels`]) so it stays decoupled from the `image` crate: a decoder
//! working from a camera frame or an embedded framebuffer can implement
//! [`QrDecoder`] without pulling in `image`.
//!
//! Encoding is the primary mission of this crate; decoding is bridged via an
//! opt-in adapter (see the `rqrr` feature). Implementing [`QrDecoder`]
//! for your own decoder is always available.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::vec::Vec;

use qrcode_core::{EcLevel, Version};

#[cfg(feature = "rqrr")]
pub mod rqrr;
pub mod sa_parse;

/// A borrowed grayscale (luma) pixel view: the universal input to a
/// [`QrDecoder`].
///
/// One byte per pixel, row-major, `0` = black / `255` = white. Decoupled from
/// the `image` crate so custom decoders need no image dependency.
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct GrayPixels<'a> {
    /// Image width in pixels.
    width: u32,
    /// Image height in pixels.
    height: u32,
    /// Row-major luma bytes (`len == width * height`).
    data: &'a [u8],
}

impl<'a> GrayPixels<'a> {
    /// Creates a view over `data` (which must hold `width * height` bytes).
    #[must_use]
    pub fn new(width: u32, height: u32, data: &'a [u8]) -> Self {
        Self { width, height, data }
    }

    /// The image width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// The image height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// The luma byte at `(x, y)` (`0` = black, `255` = white).
    ///
    /// # Panics
    ///
    /// Panics if `(x, y)` is out of bounds.
    #[must_use]
    pub fn get(&self, x: u32, y: u32) -> u8 {
        self.data[(y as usize) * (self.width as usize) + (x as usize)]
    }
}

#[cfg(feature = "image")]
impl<'a> From<&'a image::GrayImage> for GrayPixels<'a> {
    fn from(img: &'a image::GrayImage) -> Self {
        Self::new(img.width(), img.height(), img.as_raw())
    }
}

/// A QR code recovered from an image by a [`QrDecoder`].
///
/// `#[non_exhaustive]`: fields may grow in 1.x (e.g. encoding mode, mask,
/// position) without a breaking change; construct via [`DecodedQrCode::new`]
/// and read via the accessors.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedQrCode {
    /// The decoded payload bytes.
    data: Vec<u8>,
    /// The QR version.
    version: Version,
    /// The error-correction level.
    ec_level: EcLevel,
}

impl DecodedQrCode {
    /// Creates a decoded QR code from its recovered fields.
    #[must_use]
    pub fn new(data: Vec<u8>, version: Version, ec_level: EcLevel) -> Self {
        Self { data, version, ec_level }
    }

    /// The decoded payload bytes.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// The QR version.
    #[must_use]
    pub fn version(&self) -> Version {
        self.version
    }

    /// The error-correction level.
    #[must_use]
    pub fn ec_level(&self) -> EcLevel {
        self.ec_level
    }
}

/// A QR code decoder: turns a grayscale image back into data.
///
/// Implement this for your own decoder (e.g. wrapping a native binding); use
/// the bundled `RqrrDecoder` (behind the `rqrr` feature) for the
/// `rqrr` crate.
///
/// `decode` returns a [`Vec`] because an image may contain more than one QR
/// code; the order is decoder-defined.
pub trait QrDecoder {
    /// The error type returned on failure.
    type Error;

    /// Decodes all QR codes found in `image`.
    ///
    /// # Errors
    ///
    /// Returns `Self::Error` if decoding fails.
    fn decode(&self, image: GrayPixels<'_>) -> Result<Vec<DecodedQrCode>, Self::Error>;
}
