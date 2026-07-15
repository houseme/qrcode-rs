//! Structured Append bit-stream parser.
//!
//! Given one symbol's data bit stream (the bytes a decoder recovers from the
//! data region of a single QR symbol), [`parse_sa_datastream`] reads the
//! Structured Append header (mode `0011`, the symbol-sequence indicator, and
//! the parity byte) and then decodes the data segments, returning the position,
//! total, parity, and the recovered payload bytes.
//!
//! This is decoder-agnostic: it works on whatever bytes your decoder hands you.
//! The `rqrr` adapter (behind the `rqrr` feature) can be paired with decoders
//! that expose raw symbol bytes for an end-to-end encode/render/decode
//! round-trip.

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::vec::Vec;

use core::fmt::{Display, Error, Formatter};
use qrcode_core::{Mode, Version};

/// Errors returned while parsing a Structured Append data bit stream.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaParseError {
    /// The decoded bit stream does not begin with the Structured Append mode
    /// indicator (`0011`).
    NotStructuredAppend,
    /// The bit stream was truncated or otherwise malformed while parsing a
    /// Structured Append header or data segment.
    MalformedStream,
}

impl Display for SaParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match self {
            Self::NotStructuredAppend => f.write_str("not a Structured Append symbol (no `0011` mode indicator)"),
            Self::MalformedStream => f.write_str("malformed Structured Append bit stream"),
        }
    }
}

impl ::core::error::Error for SaParseError {}

/// Reverse of the alphanumeric base-45 charset (ISO/IEC 18004 §8.4.3, Table 5):
/// base-45 value (0..44) → character byte.
const ALPHA_REV: [u8; 45] = *b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";

/// A Structured Append symbol parsed from a data bit stream (owned payload).
///
/// Produced by [`parse_sa_datastream`]. To recombine a sequence, rebuild
/// Structured Append symbol values from each symbol's `position` / `total` /
/// `parity` / `data` and pass them to the facade crate's reassembly helper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaSymbolData {
    /// 1-based position within the sequence (`1..=total`).
    pub position: u8,
    /// Total number of symbols in the sequence (`2..=16`).
    pub total: u8,
    /// The parity byte (XOR of the original full message; identical in every symbol).
    pub parity: u8,
    /// This symbol's recovered payload bytes.
    pub data: Vec<u8>,
}

/// A big-endian (MSB-first) reader over a byte slice — the read-side mirror of
/// the push-side `qrcode_core::bits::Bits`.
struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Reads `n` bits big-endian, or `None` if not enough bits remain.
    fn read_bits(&mut self, n: usize) -> Option<u32> {
        if n == 0 {
            return Some(0);
        }
        if self.pos.checked_add(n)? > self.data.len() * 8 {
            return None;
        }
        let mut val: u32 = 0;
        for _ in 0..n {
            let byte = self.data[self.pos >> 3];
            let bit = (byte >> (7 - (self.pos & 7))) & 1;
            val = (val << 1) | u32::from(bit);
            self.pos += 1;
        }
        Some(val)
    }

    fn remaining(&self) -> usize {
        self.data.len() * 8 - self.pos
    }
}

/// Decodes a 4-bit Structured Append nibble: `0` means 16, anything else is the
/// value itself (the only encoding of 16 in four bits).
fn nibble_to_value(nibble: u32) -> u8 {
    if nibble == 0 { 16 } else { nibble as u8 }
}

/// Parses a Structured Append data bit stream into its header and payload.
///
/// `bits` is one symbol's data region (as recovered by a decoder); `version`
/// sets the character-count bit widths used by each data segment. The function
/// reads the 20-bit Structured Append header, then decodes Numeric /
/// Alphanumeric / Byte / Kanji segments until the terminator, returning the
/// recovered bytes.
///
/// # Errors
///
/// Returns [`SaParseError::NotStructuredAppend`] if the stream does not begin
/// with the Structured Append mode indicator (`0011`), or
/// [`SaParseError::MalformedStream`] if the bits run out mid-field.
pub fn parse_sa_datastream(bits: &[u8], version: Version) -> Result<SaSymbolData, SaParseError> {
    let mut r = BitReader::new(bits);

    let mode = r.read_bits(4).ok_or(SaParseError::MalformedStream)?;
    if mode != 0b0011 {
        return Err(SaParseError::NotStructuredAppend);
    }
    let sequence = r.read_bits(8).ok_or(SaParseError::MalformedStream)?;
    let position = nibble_to_value(sequence >> 4);
    let total = nibble_to_value(sequence & 0x0f);
    let parity = r.read_bits(8).ok_or(SaParseError::MalformedStream)? as u8;

    let mut data = Vec::new();
    while r.remaining() >= 4 {
        let segment_mode = r.read_bits(4).ok_or(SaParseError::MalformedStream)?;
        match segment_mode {
            0b0000 => break, // terminator
            0b0001 => decode_numeric(&mut r, version, &mut data)?,
            0b0010 => decode_alpha(&mut r, version, &mut data)?,
            0b0100 => decode_byte(&mut r, version, &mut data)?,
            0b1000 => decode_kanji(&mut r, version, &mut data)?,
            // FNC1 / ECI / unknown — stop with the payload parsed so far.
            _ => break,
        }
    }

    Ok(SaSymbolData { position, total, parity, data })
}

fn decode_byte(r: &mut BitReader<'_>, version: Version, out: &mut Vec<u8>) -> Result<(), SaParseError> {
    let count = r.read_bits(Mode::Byte.length_bits_count(version)).ok_or(SaParseError::MalformedStream)? as usize;
    for _ in 0..count {
        let byte = r.read_bits(8).ok_or(SaParseError::MalformedStream)? as u8;
        out.push(byte);
    }
    Ok(())
}

fn decode_numeric(r: &mut BitReader<'_>, version: Version, out: &mut Vec<u8>) -> Result<(), SaParseError> {
    let mut remaining =
        r.read_bits(Mode::Numeric.length_bits_count(version)).ok_or(SaParseError::MalformedStream)? as usize;
    while remaining >= 3 {
        let v = r.read_bits(10).ok_or(SaParseError::MalformedStream)?;
        out.push(b'0' + (v / 100) as u8);
        out.push(b'0' + ((v / 10) % 10) as u8);
        out.push(b'0' + (v % 10) as u8);
        remaining -= 3;
    }
    if remaining == 2 {
        let v = r.read_bits(7).ok_or(SaParseError::MalformedStream)?;
        out.push(b'0' + (v / 10) as u8);
        out.push(b'0' + (v % 10) as u8);
    } else if remaining == 1 {
        let v = r.read_bits(4).ok_or(SaParseError::MalformedStream)?;
        out.push(b'0' + v as u8);
    }
    Ok(())
}

fn decode_alpha(r: &mut BitReader<'_>, version: Version, out: &mut Vec<u8>) -> Result<(), SaParseError> {
    let mut remaining =
        r.read_bits(Mode::Alphanumeric.length_bits_count(version)).ok_or(SaParseError::MalformedStream)? as usize;
    while remaining >= 2 {
        let v = r.read_bits(11).ok_or(SaParseError::MalformedStream)? as usize;
        out.push(ALPHA_REV[v / 45]);
        out.push(ALPHA_REV[v % 45]);
        remaining -= 2;
    }
    if remaining == 1 {
        let v = r.read_bits(6).ok_or(SaParseError::MalformedStream)? as usize;
        out.push(ALPHA_REV[v]);
    }
    Ok(())
}

fn decode_kanji(r: &mut BitReader<'_>, version: Version, out: &mut Vec<u8>) -> Result<(), SaParseError> {
    let count = r.read_bits(Mode::Kanji.length_bits_count(version)).ok_or(SaParseError::MalformedStream)? as usize;
    for _ in 0..count {
        let n = r.read_bits(13).ok_or(SaParseError::MalformedStream)?;
        let high = n / 0xc0;
        let low = n % 0xc0;
        let bytes = (high << 8) | low;
        let cp = if bytes < 0x1f00 { bytes + 0x8140 } else { bytes + 0xc140 };
        out.push((cp >> 8) as u8);
        out.push((cp & 0xff) as u8);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SaParseError, parse_sa_datastream};
    use alloc::vec::Vec;
    use qrcode_core::bits::Bits;
    use qrcode_core::{EcLevel, Version};

    /// Builds a single symbol's data stream (SA header + `data` via `push`) and
    /// returns the bytes, mirroring what a decoder would recover.
    fn sa_bytes<F>(position: u8, total: u8, parity: u8, push: F) -> Vec<u8>
    where
        F: FnOnce(&mut Bits),
    {
        let mut bits = Bits::new(Version::Normal(1));
        bits.push_structured_append_header(position, total, parity).unwrap();
        push(&mut bits);
        bits.push_terminator(EcLevel::M).unwrap();
        bits.into_bytes()
    }

    #[test]
    fn header_only_parses_back() {
        // v1.5.0 header vector: position 1, total 3, parity 0x5a, no data.
        let bytes = sa_bytes(1, 3, 0x5a, |_| {});
        let parsed = parse_sa_datastream(&bytes, Version::Normal(1)).unwrap();
        assert_eq!(parsed.position, 1);
        assert_eq!(parsed.total, 3);
        assert_eq!(parsed.parity, 0x5a);
        assert!(parsed.data.is_empty());
    }

    #[test]
    fn value_16_decodes_from_zero_nibble() {
        let bytes = sa_bytes(16, 16, 0x00, |_| {});
        let parsed = parse_sa_datastream(&bytes, Version::Normal(1)).unwrap();
        assert_eq!(parsed.position, 16);
        assert_eq!(parsed.total, 16);
    }

    #[test]
    fn byte_segment_round_trips() {
        let bytes = sa_bytes(1, 2, 0x03, |b| {
            b.push_byte_data(b"ab").unwrap();
        });
        let parsed = parse_sa_datastream(&bytes, Version::Normal(1)).unwrap();
        assert_eq!(parsed.data, b"ab");
    }

    #[test]
    fn numeric_segment_round_trips() {
        let bytes = sa_bytes(1, 2, 0x00, |b| {
            b.push_numeric_data(b"01234567").unwrap();
        });
        let parsed = parse_sa_datastream(&bytes, Version::Normal(1)).unwrap();
        assert_eq!(parsed.data, b"01234567");
    }

    #[test]
    fn alphanumeric_segment_round_trips() {
        let bytes = sa_bytes(1, 2, 0x00, |b| {
            b.push_alphanumeric_data(b"AC-42").unwrap();
        });
        let parsed = parse_sa_datastream(&bytes, Version::Normal(1)).unwrap();
        assert_eq!(parsed.data, b"AC-42");
    }

    #[test]
    fn kanji_segment_round_trips() {
        let bytes = sa_bytes(1, 2, 0x00, |b| {
            b.push_kanji_data(b"\x93\x5f\xe4\xaa").unwrap();
        });
        let parsed = parse_sa_datastream(&bytes, Version::Normal(1)).unwrap();
        assert_eq!(parsed.data, b"\x93\x5f\xe4\xaa");
    }

    #[test]
    fn non_structured_append_stream_is_rejected() {
        // A plain byte-mode symbol — first mode is 0100, not 0011.
        let mut bits = Bits::new(Version::Normal(1));
        bits.push_byte_data(b"hello").unwrap();
        bits.push_terminator(EcLevel::M).unwrap();
        let bytes = bits.into_bytes();
        assert_eq!(parse_sa_datastream(&bytes, Version::Normal(1)), Err(SaParseError::NotStructuredAppend));
    }

    #[test]
    fn truncated_stream_is_malformed() {
        // Starts with the SA mode `0011`, but only 4 bits remain — not enough
        // for the 8-bit symbol-sequence indicator.
        assert_eq!(parse_sa_datastream(&[0b0011_0000], Version::Normal(1)), Err(SaParseError::MalformedStream));
    }
}
