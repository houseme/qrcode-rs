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
//! this module only *encodes* the split. To recombine symbols a decoder has
//! scanned, use [`reassemble`] with the per-symbol metadata that decoder
//! supplies.

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::vec::Vec;

use core::cmp::min;
use core::fmt::{Display, Error, Formatter};

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
/// Mirrors [`bits::encode_auto`](crate::bits::encode_auto)'s tier-based search:
/// the segment split is constant within a character-count tier (the per-segment
/// length is tier-invariant), so we optimize once per tier (V9 / V26 / V40) and
/// let [`bits::find_min_version`] pick the smallest fitting version. This runs
/// the optimizer ~3× per chunk instead of ~40×. The constant 20-bit header is
/// added to the encoded length before the capacity check.
fn encode_one_symbol(data: &[u8], position: u8, total: u8, parity: u8, ec: EcLevel) -> QrResult<QrCode> {
    let segments = Parser::new(data).collect::<Vec<Segment>>();
    for &checkpoint in &[Version::Normal(9), Version::Normal(26), Version::Normal(40)] {
        let opt = Optimizer::new(segments.iter().copied(), checkpoint).collect::<Vec<_>>();
        // +20 bits: the Structured Append header (4-bit mode + 8-bit sequence
        // indicator + 8-bit parity) prepended before the data mode indicator.
        let total_len = total_encoded_len(&opt, checkpoint) + 20;
        if total_len <= bits::data_capacity_bits(checkpoint, ec)? {
            let version = bits::find_min_version(total_len, ec);
            let mut bits = Bits::new(version);
            bits.reserve(total_len);
            bits.push_structured_append_header(position, total, parity)?;
            bits.push_segments(data, opt.into_iter())?;
            bits.push_terminator(ec)?;
            return QrCode::with_bits(bits, ec);
        }
    }
    Err(QrError::DataTooLong)
}

/// Errors returned by [`reassemble`] when decoded Structured Append symbols
/// cannot be recombined.
///
/// The enum is `#[non_exhaustive]`: future versions may add variants, so
/// external callers should match with a `_` arm.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaError {
    /// Fewer symbols were supplied than the total count requires.
    Incomplete,
    /// Two symbols claim the same position. Carries the duplicated position.
    DuplicatePosition(u8),
    /// The symbols disagree on the total symbol count.
    CountMismatch,
    /// The symbols disagree on the parity byte.
    ParityMismatch,
    /// A total count or position is outside its valid range (`2..=16` /
    /// `1..=total`). Carries the offending value.
    OutOfRange(u8),
    /// The decoded bit stream does not begin with the Structured Append mode
    /// indicator (`0011`) — the symbol is not part of a Structured Append
    /// sequence (or the wrong bytes were supplied).
    NotStructuredAppend,
    /// The bit stream was truncated or otherwise malformed while parsing a
    /// Structured Append header or data segment.
    MalformedStream,
}

impl Display for SaError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self {
            Self::Incomplete => f.write_str("incomplete Structured Append sequence (symbols missing)"),
            Self::DuplicatePosition(position) => {
                write!(f, "duplicate Structured Append position {position}")
            }
            Self::CountMismatch => f.write_str("Structured Append symbols disagree on the total count"),
            Self::ParityMismatch => f.write_str("Structured Append symbols disagree on the parity byte"),
            Self::OutOfRange(value) => {
                write!(f, "Structured Append value {value} out of range (total 2..=16, position 1..=total)")
            }
            Self::NotStructuredAppend => f.write_str("not a Structured Append symbol (no `0011` mode indicator)"),
            Self::MalformedStream => f.write_str("malformed Structured Append bit stream"),
        }
    }
}

impl ::core::error::Error for SaError {}

/// One decoded Structured Append symbol's metadata, ready for [`reassemble`].
///
/// Build this from whatever your decoder exposes: the position and total count
/// (the high and low nibbles of the 8-bit symbol-sequence indicator), the
/// parity byte, and that symbol's decoded payload bytes. The fields are public
/// so it can be constructed with a struct literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaSymbol<'a> {
    /// 1-based position of this symbol within the sequence (`1..=total`).
    pub position: u8,
    /// Total number of symbols in the sequence (`2..=16`).
    pub total: u8,
    /// The parity byte (identical in every symbol of the sequence).
    pub parity: u8,
    /// This symbol's decoded payload bytes.
    pub data: &'a [u8],
}

/// Reassembles the original message from decoded Structured Append symbols.
///
/// Validates that every symbol agrees on the total count and parity, that the
/// positions form a complete `1..=total` set (no gaps, no duplicates), then
/// orders them by position and concatenates their data.
///
/// The parity byte each symbol carries is the XOR of the *original* full
/// message, not of any one symbol — so `reassemble` only checks that all
/// symbols report the *same* parity. To confirm the reassembled bytes match a
/// known original, XOR them yourself and compare to that shared parity.
///
/// # Errors
///
/// Returns [`SaError`] if `parts` is empty, disagrees on the total count or
/// parity, holds an out-of-range value, is incomplete, or repeats a position.
pub fn reassemble(parts: &[SaSymbol<'_>]) -> Result<Vec<u8>, SaError> {
    let Some(first) = parts.first() else { return Err(SaError::Incomplete) };
    if !(2..=16).contains(&first.total) {
        return Err(SaError::OutOfRange(first.total));
    }
    let total = first.total;
    let parity = first.parity;
    for p in parts {
        if p.total != total {
            return Err(SaError::CountMismatch);
        }
        if p.parity != parity {
            return Err(SaError::ParityMismatch);
        }
        if !(1..=total).contains(&p.position) {
            return Err(SaError::OutOfRange(p.position));
        }
    }
    if parts.len() != usize::from(total) {
        return Err(SaError::Incomplete);
    }
    let mut seen = [false; 16];
    for p in parts {
        let idx = usize::from(p.position - 1);
        if seen[idx] {
            return Err(SaError::DuplicatePosition(p.position));
        }
        seen[idx] = true;
    }
    let mut ordered: Vec<&SaSymbol<'_>> = parts.iter().collect();
    ordered.sort_by_key(|s| s.position);
    let mut out = Vec::new();
    for s in ordered {
        out.extend_from_slice(s.data);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::StructuredAppend;
    use crate::types::{EcLevel, QrError};
    use alloc::{vec, vec::Vec};

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

#[cfg(test)]
mod reassemble_tests {
    use super::{SaError, SaSymbol, reassemble};
    use alloc::vec::Vec;

    fn sym(position: u8, total: u8, parity: u8, data: &[u8]) -> SaSymbol<'_> {
        SaSymbol { position, total, parity, data }
    }

    #[test]
    fn test_reassemble_ok() {
        let parts = [sym(1, 3, 0x5a, b"hel"), sym(2, 3, 0x5a, b"lo "), sym(3, 3, 0x5a, b"world")];
        assert_eq!(reassemble(&parts).unwrap(), b"hello world");
    }

    #[test]
    fn test_reassemble_out_of_order() {
        // Supplied as 3, 1, 2 — reassemble must order by position.
        let parts = [sym(3, 3, 0x5a, b"wor"), sym(1, 3, 0x5a, b"hel"), sym(2, 3, 0x5a, b"lo")];
        assert_eq!(reassemble(&parts).unwrap(), b"hellowor");
    }

    #[test]
    fn test_reassemble_empty() {
        assert_eq!(reassemble(&[]), Err(SaError::Incomplete));
    }

    #[test]
    fn test_reassemble_incomplete() {
        let parts = [sym(1, 3, 0x5a, b"a"), sym(2, 3, 0x5a, b"b")];
        assert_eq!(reassemble(&parts), Err(SaError::Incomplete));
    }

    #[test]
    fn test_reassemble_duplicate() {
        let parts = [sym(1, 3, 0x5a, b"a"), sym(1, 3, 0x5a, b"b"), sym(3, 3, 0x5a, b"c")];
        assert_eq!(reassemble(&parts), Err(SaError::DuplicatePosition(1)));
    }

    #[test]
    fn test_reassemble_count_mismatch() {
        let parts = [sym(1, 3, 0x5a, b"a"), sym(2, 4, 0x5a, b"b")];
        assert_eq!(reassemble(&parts), Err(SaError::CountMismatch));
    }

    #[test]
    fn test_reassemble_parity_mismatch() {
        let parts = [sym(1, 2, 0x5a, b"a"), sym(2, 2, 0x5b, b"b")];
        assert_eq!(reassemble(&parts), Err(SaError::ParityMismatch));
    }

    #[test]
    fn test_reassemble_out_of_range_total() {
        assert_eq!(reassemble(&[sym(1, 1, 0, b"a")]), Err(SaError::OutOfRange(1)));
        assert_eq!(reassemble(&[sym(1, 17, 0, b"a")]), Err(SaError::OutOfRange(17)));
    }

    #[test]
    fn test_reassemble_out_of_range_position() {
        let parts = [sym(0, 2, 0x5a, b"a"), sym(2, 2, 0x5a, b"b")];
        assert_eq!(reassemble(&parts), Err(SaError::OutOfRange(0)));
        let parts = [sym(1, 2, 0x5a, b"a"), sym(3, 2, 0x5a, b"b")];
        assert_eq!(reassemble(&parts), Err(SaError::OutOfRange(3)));
    }

    #[test]
    fn test_reassemble_max_sequence() {
        // 16 symbols (the spec maximum): positions 1..=16, each carrying one byte.
        let bytes: Vec<u8> = (1u8..=16).collect();
        let parts: Vec<SaSymbol<'_>> = bytes
            .iter()
            .enumerate()
            .map(|(i, _)| SaSymbol {
                position: u8::try_from(i + 1).unwrap(),
                total: 16,
                parity: 0xff,
                data: &bytes[i..=i],
            })
            .collect();
        assert_eq!(reassemble(&parts).unwrap(), bytes);
    }
}
