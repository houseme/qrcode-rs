//! ISO/IEC 18004 §7.4 Structured Append — splitting one payload across
//! 2..=16 QR symbols.
//!
//! Each symbol in a Structured Append sequence carries a 20-bit header (see
//! [`Bits::push_structured_append_header`]) at the very start of its bit
//! stream, before the data mode indicator, so a spec-aware decoder can
//! reassemble the original message in order and verify it via the shared
//! parity byte.
//!
//! Encoding is one-way — a [`QrCode`] does not retain its input payload — so
//! this module only *encodes* the split.

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::vec::Vec;

use core::cmp::min;

use crate::QrCode;
use crate::bits::{self, Bits};
use crate::optimize::{Optimizer, Parser, Segment, total_encoded_len};
use crate::types::{EcLevel, QrError, QrResult, Version};

/// A Structured Append sequence builder: splits one payload across 2..=16 QR
/// symbols.
///
/// Construct with [`StructuredAppend::new`], then call
/// [`StructuredAppend::encode`] to emit the symbols. Every emitted symbol is a
/// complete, independently-scannable QR code that also carries the Structured
/// Append header, so a compliant reader knows how the symbols belong together.
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct StructuredAppend<'a> {
    /// Number of symbols in the sequence (`2..=16`).
    symbols: u8,
    /// The original, un-split payload.
    payload: &'a [u8],
    /// XOR of every payload byte — the Structured Append parity, stored
    /// identically in every emitted symbol.
    parity: u8,
}

impl<'a> StructuredAppend<'a> {
    /// Creates a builder that will split `payload` across `symbols` QR symbols.
    ///
    /// `symbols` must be in `2..=16` (per ISO/IEC 18004 §7.4 a Structured
    /// Append sequence has at least two symbols). The parity byte (the XOR of
    /// every payload byte) is computed once here and reused for every symbol.
    ///
    /// # Errors
    ///
    /// Returns [`QrError::InvalidStructuredAppend`] if `symbols` is not in
    /// `2..=16`.
    ///
    /// ```
    /// use qrcode_rs::structured_append::StructuredAppend;
    /// use qrcode_rs::EcLevel;
    ///
    /// let sa = StructuredAppend::new(3, b"split across three symbols")?;
    /// let codes = sa.encode(EcLevel::M)?;
    /// assert_eq!(codes.len(), 3);
    /// # Ok::<(), qrcode_rs::QrError>(())
    /// ```
    pub fn new(symbols: u8, payload: &'a [u8]) -> QrResult<Self> {
        if !(2..=16).contains(&symbols) {
            return Err(QrError::InvalidStructuredAppend { value: symbols });
        }
        let parity = payload.iter().fold(0u8, |acc, &byte| acc ^ byte);
        Ok(Self { symbols, payload, parity })
    }

    /// The number of symbols the payload will be split across.
    #[must_use]
    pub const fn symbols(&self) -> u8 {
        self.symbols
    }

    /// The Structured Append parity byte (the XOR of every payload byte),
    /// stored identically in every emitted symbol so a reader can verify it has
    /// reassembled the right group.
    #[must_use]
    pub const fn parity(&self) -> u8 {
        self.parity
    }

    /// The payload being split.
    #[must_use]
    pub fn payload(&self) -> &'a [u8] {
        self.payload
    }

    /// Encodes the payload into [`Self::symbols`] QR codes.
    ///
    /// The payload is split as evenly as possible (the last symbol may be
    /// shorter). Each symbol is encoded at the smallest version that fits its
    /// chunk plus the 20-bit Structured Append header, at error-correction
    /// level `ec`. Symbols in a sequence may therefore differ in version — this
    /// is permitted by the standard.
    ///
    /// # Errors
    ///
    /// Returns [`QrError::DataTooLong`] if any chunk cannot fit even version 40
    /// at the requested error-correction level.
    pub fn encode(&self, ec: EcLevel) -> QrResult<Vec<QrCode>> {
        let n = usize::from(self.symbols);
        let chunk = self.payload.len().div_ceil(n);
        let mut codes = Vec::with_capacity(n);
        for i in 0..n {
            // Clamp `start` so a payload shorter than `n * chunk` yields empty
            // trailing slices (valid `&[len..len]`) rather than panicking.
            let start = min(i * chunk, self.payload.len());
            let end = min((i + 1) * chunk, self.payload.len());
            let piece = &self.payload[start..end];
            let code = encode_one_symbol(piece, i as u8 + 1, self.symbols, self.parity, ec)?;
            codes.push(code);
        }
        Ok(codes)
    }
}

/// Encodes a single payload chunk as one QR symbol carrying its Structured
/// Append header, picking the smallest version that fits the chunk plus the
/// 20-bit header overhead.
///
/// Mirrors the `for v in 1..=40` version search used by the other encoders
/// (e.g. `QrCode::for_gs1`), but adds the constant 20-bit header overhead to
/// the segment length before comparing against the version's data capacity.
/// `push_terminator` independently re-checks capacity as a safety net.
fn encode_one_symbol(data: &[u8], position: u8, total: u8, parity: u8, ec: EcLevel) -> QrResult<QrCode> {
    let segments = Parser::new(data).collect::<Vec<Segment>>();
    for v in 1..=40u8 {
        let version = Version::Normal(i16::from(v));
        let opt = Optimizer::new(segments.iter().copied(), version).collect::<Vec<_>>();
        // 20 bits: the Structured Append header (4-bit mode + 8-bit sequence
        // indicator + 8-bit parity) prepended before the data mode indicator.
        let total_len = total_encoded_len(&opt, version) + 20;
        let Some(capacity) = bits::data_capacity_bits(version, ec).ok() else {
            continue;
        };
        if total_len > capacity {
            continue;
        }
        let mut bits = Bits::new(version);
        bits.push_structured_append_header(position, total, parity)?;
        bits.push_segments(data, opt.into_iter())?;
        bits.push_terminator(ec)?;
        return QrCode::with_bits(bits, ec);
    }
    Err(QrError::DataTooLong)
}

#[cfg(test)]
mod tests {
    use super::StructuredAppend;
    use crate::types::{EcLevel, QrError};

    #[test]
    fn test_new_rejects_out_of_range() {
        assert_eq!(StructuredAppend::new(1, b"x").err(), Some(QrError::InvalidStructuredAppend { value: 1 }));
        assert_eq!(StructuredAppend::new(17, b"x").err(), Some(QrError::InvalidStructuredAppend { value: 17 }));
    }

    #[test]
    fn test_new_accepts_bounds() {
        assert!(StructuredAppend::new(2, b"x").is_ok());
        assert!(StructuredAppend::new(16, b"x").is_ok());
    }

    #[test]
    fn test_parity_xor() {
        assert_eq!(StructuredAppend::new(3, &[0x01, 0x02, 0x03]).unwrap().parity(), 0x00);
        assert_eq!(StructuredAppend::new(2, &[0xff, 0x0f]).unwrap().parity(), 0xf0);
        // 0..=255 each value appears once → XOR is 0.
        let bytes: Vec<u8> = (0u8..=255).collect();
        assert_eq!(StructuredAppend::new(2, &bytes).unwrap().parity(), 0);
    }

    #[test]
    fn test_parity_empty() {
        assert_eq!(StructuredAppend::new(2, b"").unwrap().parity(), 0);
    }

    #[test]
    fn test_encode_count_and_versions() {
        let payload = b"Split this payload across several QR symbols for resilience.";
        let codes = StructuredAppend::new(3, payload).unwrap().encode(EcLevel::M).unwrap();
        assert_eq!(codes.len(), 3);
        for code in &codes {
            assert!(!code.info().version().is_micro(), "Structured Append must be Normal QR");
        }
    }

    #[test]
    fn test_encode_all_normal_across_counts() {
        let payload = b"the quick brown fox jumps over the lazy dog";
        for n in 2..=16u8 {
            let codes = StructuredAppend::new(n, payload).unwrap().encode(EcLevel::L).unwrap();
            assert_eq!(codes.len(), usize::from(n));
            assert!(codes.iter().all(|c| !c.info().version().is_micro()), "n={n} produced a Micro QR");
        }
    }

    #[test]
    fn test_encode_empty_payload() {
        // Degenerate but well-formed: each symbol carries only header + terminator.
        let codes = StructuredAppend::new(2, b"").unwrap().encode(EcLevel::M).unwrap();
        assert_eq!(codes.len(), 2);
        assert!(codes.iter().all(|c| !c.info().version().is_micro()));
    }

    #[test]
    fn test_encode_deterministic() {
        let payload = b"deterministic encoding";
        let a = StructuredAppend::new(3, payload).unwrap().encode(EcLevel::M).unwrap();
        let b = StructuredAppend::new(3, payload).unwrap().encode(EcLevel::M).unwrap();
        // Identical inputs → identical module grids.
        for (a, b) in a.iter().zip(b.iter()) {
            assert_eq!(a.to_colors(), b.to_colors());
        }
    }

    #[test]
    fn test_encode_too_long() {
        // 16 symbols × 4000 bytes > version-40-H capacity → each chunk overflows.
        let payload = vec![0u8; 16 * 4000];
        let result = StructuredAppend::new(16, &payload).unwrap().encode(EcLevel::H);
        assert_eq!(result.err(), Some(QrError::DataTooLong));
    }
}
