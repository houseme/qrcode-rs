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

extern crate alloc;

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::vec::Vec;

use core::fmt;
use qrcode_core::{EcLevel, Version};

#[cfg(feature = "rqrr")]
pub mod rqrr;
#[cfg(feature = "rxing")]
pub mod rxing;
#[cfg(feature = "rxing")]
pub use rxing::{ScanSymbol, StructuredAppendHeader};
pub mod sa_parse;

/// Errors returned when validating a [`GrayPixels`] buffer.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrayPixelsError {
    /// The image dimensions exceed the byte-slice size supported by this target.
    DimensionsOverflow,
    /// The buffer does not contain exactly one byte per pixel.
    BufferLengthMismatch {
        /// Required byte count for the supplied dimensions.
        expected: usize,
        /// Actual byte count in the supplied buffer.
        actual: usize,
    },
}

impl fmt::Display for GrayPixelsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionsOverflow => f.write_str("grayscale image dimensions exceed the supported buffer size"),
            Self::BufferLengthMismatch { expected, actual } => {
                write!(f, "grayscale buffer length mismatch: expected {expected} bytes, got {actual}")
            }
        }
    }
}

impl ::core::error::Error for GrayPixelsError {}

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

    /// Creates a view after validating its dimensions and buffer length.
    ///
    /// Zero-area dimensions are accepted when the buffer is empty. A decoder
    /// may require a non-empty image even when this view is valid.
    ///
    /// # Errors
    ///
    /// Returns [`GrayPixelsError::DimensionsOverflow`] if the pixel count cannot
    /// be represented by a byte slice on this target, or
    /// [`GrayPixelsError::BufferLengthMismatch`] if `data.len()` differs from
    /// `width * height`.
    pub fn try_new(width: u32, height: u32, data: &'a [u8]) -> Result<Self, GrayPixelsError> {
        let expected = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|length| usize::try_from(length).ok())
            .filter(|&length| length <= isize::MAX as usize)
            .ok_or(GrayPixelsError::DimensionsOverflow)?;
        if data.len() != expected {
            return Err(GrayPixelsError::BufferLengthMismatch { expected, actual: data.len() });
        }
        Ok(Self::new(width, height, data))
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
        assert!(x < self.width && y < self.height, "grayscale pixel coordinates are out of bounds");
        self.data[(y as usize) * (self.width as usize) + (x as usize)]
    }

    /// Returns the luma byte at `(x, y)` without panicking.
    ///
    /// Returns `None` when the coordinates are out of bounds, their index cannot
    /// be represented, or a view constructed with [`Self::new`] has no byte for
    /// that pixel.
    #[must_use]
    pub fn try_get(&self, x: u32, y: u32) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let width = usize::try_from(self.width).ok()?;
        let x = usize::try_from(x).ok()?;
        let y = usize::try_from(y).ok()?;
        let index = y.checked_mul(width)?.checked_add(x)?;
        self.data.get(index).copied()
    }
}

#[cfg(feature = "image")]
impl<'a> From<&'a image::GrayImage> for GrayPixels<'a> {
    fn from(img: &'a image::GrayImage) -> Self {
        let (width, height) = (img.width(), img.height());
        // ImageBuffer validates the logical area but permits excess backing bytes.
        let logical_len = width as usize * height as usize;
        Self::new(width, height, &img.as_raw()[..logical_len])
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

    /// Consumes the decoded QR code and transfers its payload buffer without
    /// copying or allocating. The returned bytes may contain arbitrary binary
    /// data, including non-UTF-8 values.
    #[must_use]
    pub fn into_data(self) -> Vec<u8> {
        self.data
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

#[cfg(test)]
mod tests {
    use super::{DecodedQrCode, GrayPixels, GrayPixelsError};
    use alloc::vec::Vec;
    use qrcode_core::{EcLevel, Version};

    #[test]
    fn consuming_decoded_payload_transfers_the_binary_buffer_and_capacity() {
        let mut payload = Vec::with_capacity(32);
        payload.extend_from_slice(&[0x00, 0xff, 0x80, 0x11, 0x00]);
        let pointer = payload.as_ptr();
        let capacity = payload.capacity();
        let decoded = DecodedQrCode::new(payload, Version::Normal(7), EcLevel::Q);

        let bytes = decoded.into_data();
        assert_eq!(bytes, [0x00, 0xff, 0x80, 0x11, 0x00]);
        assert_eq!(bytes.as_ptr(), pointer);
        assert_eq!(bytes.capacity(), capacity);
    }

    #[cfg(feature = "image")]
    #[test]
    fn gray_image_conversion_borrows_only_logical_pixels_from_padded_storage() {
        let image = image::GrayImage::from_raw(2, 2, Vec::from([1, 2, 3, 4, 99, 100, 101])).unwrap();
        let pixels = GrayPixels::from(&image);

        assert_eq!((pixels.width(), pixels.height()), (2, 2));
        assert_eq!(pixels.data, [1, 2, 3, 4]);
        assert_eq!(pixels.data.as_ptr(), image.as_raw().as_ptr());
        assert!(GrayPixels::try_new(pixels.width(), pixels.height(), pixels.data).is_ok());
        assert_eq!(pixels.try_get(1, 1), Some(4));
    }

    #[cfg(feature = "image")]
    #[test]
    fn gray_image_conversion_borrows_an_empty_prefix_for_zero_area_padded_storage() {
        for (width, height) in [(0, 0), (0, 7), (7, 0), (0, u32::MAX), (u32::MAX, 0)] {
            let image = image::GrayImage::from_raw(width, height, Vec::from([99, 100, 101])).unwrap();
            let pixels = GrayPixels::from(&image);

            assert_eq!((pixels.width(), pixels.height()), (width, height));
            assert!(pixels.data.is_empty());
            assert_eq!(pixels.data.as_ptr(), image.as_raw().as_ptr());
            assert!(GrayPixels::try_new(width, height, pixels.data).is_ok());
        }
    }

    #[test]
    fn grayscale_pixels_use_row_major_coordinates() {
        let pixels = GrayPixels::new(2, 2, &[1, 2, 3, 4]);
        assert_eq!(pixels.get(0, 0), 1);
        assert_eq!(pixels.get(1, 0), 2);
        assert_eq!(pixels.get(0, 1), 3);
        assert_eq!(pixels.get(1, 1), 4);
    }

    #[test]
    #[should_panic(expected = "grayscale pixel coordinates are out of bounds")]
    fn grayscale_x_coordinate_cannot_alias_the_next_row() {
        let _ = GrayPixels::new(2, 2, &[1, 2, 3, 4]).get(2, 0);
    }

    #[test]
    #[should_panic(expected = "grayscale pixel coordinates are out of bounds")]
    fn grayscale_y_coordinate_cannot_read_outside_the_image() {
        let _ = GrayPixels::new(2, 2, &[1, 2, 3, 4]).get(0, 2);
    }

    #[test]
    #[should_panic(expected = "grayscale pixel coordinates are out of bounds")]
    fn empty_grayscale_image_has_no_valid_coordinates() {
        let _ = GrayPixels::new(0, 0, &[]).get(0, 0);
    }

    #[test]
    fn checked_grayscale_view_and_pixel_access_use_the_same_coordinates() {
        let pixels = GrayPixels::try_new(2, 2, &[1, 2, 3, 4]).unwrap();
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(pixels.try_get(x, y), Some(pixels.get(x, y)));
            }
        }
        assert_eq!(pixels.try_get(2, 0), None);
        assert_eq!(pixels.try_get(0, 2), None);
        assert_eq!(pixels.try_get(u32::MAX, u32::MAX), None);
    }

    #[test]
    fn checked_grayscale_view_rejects_short_and_long_buffers() {
        for (data, actual) in [(&[1, 2, 3][..], 3), (&[1, 2, 3, 4, 5][..], 5)] {
            assert!(matches!(
                GrayPixels::try_new(2, 2, data),
                Err(GrayPixelsError::BufferLengthMismatch { expected: 4, actual: length }) if length == actual
            ));
        }
    }

    #[test]
    fn checked_grayscale_view_accepts_empty_buffers_for_zero_area() {
        for (width, height) in [(0, 0), (0, u32::MAX), (u32::MAX, 0)] {
            let pixels = GrayPixels::try_new(width, height, &[]).unwrap();
            assert_eq!(pixels.try_get(0, 0), None);
        }
    }

    #[test]
    fn checked_grayscale_view_rejects_unrepresentable_dimensions() {
        assert!(matches!(GrayPixels::try_new(u32::MAX, u32::MAX, &[]), Err(GrayPixelsError::DimensionsOverflow)));
    }

    #[test]
    fn checked_access_is_safe_for_an_unchecked_short_buffer() {
        let pixels = GrayPixels::new(2, 2, &[7]);
        assert_eq!(pixels.try_get(0, 0), Some(7));
        assert_eq!(pixels.try_get(1, 0), None);
        assert_eq!(pixels.try_get(0, 1), None);
        assert_eq!(GrayPixels::new(u32::MAX, u32::MAX, &[]).try_get(u32::MAX - 1, u32::MAX - 1), None);
    }
}
