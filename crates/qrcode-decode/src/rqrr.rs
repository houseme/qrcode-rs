//! [`rqrr`](https://crates.io/crates/rqrr)-backed [`QrDecoder`] adapter.
//!
//! Enables decoding a rendered QR image back to its payload via the `rqrr`
//! crate. rqrr performs its own adaptive thresholding, so a plain grayscale
//! [`GrayPixels`] view is all that is required.

pub use rqrr::DeQRError;

use crate::{DecodedQrCode, GrayPixels, QrDecoder};
use alloc::vec::Vec;
use qrcode_core::{EcLevel, Version};

/// A [`QrDecoder`] backed by the [`rqrr`] crate.
///
/// Empty dimensions or an invalid grayscale buffer return
/// [`DeQRError::InvalidGridSize`] before image preparation. Errors from detected
/// QR grids are propagated without skipping unsuccessful grids.
#[derive(Default, Debug, Clone, Copy)]
pub struct RqrrDecoder;

impl RqrrDecoder {
    /// Creates a new `RqrrDecoder`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl QrDecoder for RqrrDecoder {
    type Error = DeQRError;

    fn decode(&self, image: GrayPixels<'_>) -> Result<Vec<DecodedQrCode>, Self::Error> {
        if image.width() == 0 || image.height() == 0 {
            return Err(DeQRError::InvalidGridSize);
        }
        let image =
            GrayPixels::try_new(image.width(), image.height(), image.data).map_err(|_| DeQRError::InvalidGridSize)?;
        let mut prep =
            rqrr::PreparedImage::prepare_from_greyscale(image.width() as usize, image.height() as usize, |x, y| {
                image.get(x as u32, y as u32)
            });
        let grids = prep.detect_grids();
        let mut out = Vec::new();
        for grid in grids {
            let mut bytes: Vec<u8> = Vec::new();
            let meta = grid.decode_to(&mut bytes)?;
            out.push(DecodedQrCode::new(bytes, map_version(meta.version), map_ec(meta.ecc_level)));
        }
        Ok(out)
    }
}

/// Maps an `rqrr` version to a crate [`Version`] (rqrr decodes only normal QR,
/// so the value is always in `1..=40`).
fn map_version(v: rqrr::Version) -> Version {
    Version::Normal(v.0 as i16)
}

/// Maps an `rqrr` ecc level to a crate [`EcLevel`].
///
/// `rqrr` stores the raw QR format-information EC bits (`M=00, L=01, H=10,
/// Q=11`), not the sequential index, so the mapping is non-trivial.
fn map_ec(level: u16) -> EcLevel {
    match level {
        0 => EcLevel::M, // format-info 00
        1 => EcLevel::L, // 01
        2 => EcLevel::H, // 10
        _ => EcLevel::Q, // 11
    }
}

#[cfg(test)]
mod tests {
    use super::{DeQRError, RqrrDecoder};
    use crate::{GrayPixels, QrDecoder};

    #[test]
    fn decoder_rejects_zero_axes_before_preparing_an_image() {
        for (width, height) in [(0, 0), (0, 1), (1, 0), (u32::MAX, 0), (0, u32::MAX)] {
            assert_eq!(RqrrDecoder::new().decode(GrayPixels::new(width, height, &[])), Err(DeQRError::InvalidGridSize));
        }
    }

    #[test]
    fn decoder_rejects_invalid_buffers_before_reading_pixels() {
        for pixels in
            [GrayPixels::new(2, 2, &[0]), GrayPixels::new(2, 2, &[0; 5]), GrayPixels::new(u32::MAX, u32::MAX, &[])]
        {
            assert_eq!(RqrrDecoder::new().decode(pixels), Err(DeQRError::InvalidGridSize));
        }
    }

    #[test]
    fn decoder_accepts_a_valid_nonempty_image_without_a_qr_code() {
        assert_eq!(RqrrDecoder::new().decode(GrayPixels::try_new(1, 1, &[255]).unwrap()), Ok(alloc::vec![]));
    }
}
