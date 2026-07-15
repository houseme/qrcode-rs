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
