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
use qrcode_core::{EncodingMode, KanjiMode, Mode, Version};

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
/// [`SaParseError::MalformedStream`] if the version is not a normal QR version
/// in `1..=40`, the bits run out mid-field, a value is invalid for its mode, or
/// a segment uses an unsupported mode (including ECI and FNC1), or a shortened
/// terminator at the end of the stream contains non-zero bits.
pub fn parse_sa_datastream(bits: &[u8], version: Version) -> Result<SaSymbolData, SaParseError> {
    if !matches!(version, Version::Normal(1..=40)) {
        return Err(SaParseError::MalformedStream);
    }
    let mut r = BitReader::new(bits);

    let mode = r.read_bits(4).ok_or(SaParseError::MalformedStream)?;
    if mode != 0b0011 {
        return Err(SaParseError::NotStructuredAppend);
    }
    let sequence = r.read_bits(8).ok_or(SaParseError::MalformedStream)?;
    let position = ((sequence >> 4) + 1) as u8;
    let total = ((sequence & 0x0f) + 1) as u8;
    if total < 2 || position > total {
        return Err(SaParseError::MalformedStream);
    }
    let parity = r.read_bits(8).ok_or(SaParseError::MalformedStream)? as u8;

    let mut data = Vec::new();
    while r.remaining() >= 4 {
        let segment_mode = r.read_bits(4).ok_or(SaParseError::MalformedStream)?;
        match segment_mode {
            // A full terminator ends the payload; retain the existing tolerance
            // for any remaining alignment and pad bytes.
            0b0000 => return Ok(SaSymbolData { position, total, parity, data }),
            0b0001 => decode_numeric(&mut r, version, &mut data)?,
            0b0010 => decode_alpha(&mut r, version, &mut data)?,
            0b0100 => decode_byte(&mut r, version, &mut data)?,
            0b1000 => decode_kanji(&mut r, version, &mut data)?,
            // Unsupported modes must not make a partial payload look complete.
            _ => return Err(SaParseError::MalformedStream),
        }
    }

    // A normal QR terminator may be shortened when fewer than four bits remain.
    // Non-zero residual bits instead indicate a truncated next mode indicator.
    let remaining = r.remaining();
    if r.read_bits(remaining) != Some(0) {
        return Err(SaParseError::MalformedStream);
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
        if v >= 1000 {
            return Err(SaParseError::MalformedStream);
        }
        out.push(b'0' + (v / 100) as u8);
        out.push(b'0' + ((v / 10) % 10) as u8);
        out.push(b'0' + (v % 10) as u8);
        remaining -= 3;
    }
    if remaining == 2 {
        let v = r.read_bits(7).ok_or(SaParseError::MalformedStream)?;
        if v >= 100 {
            return Err(SaParseError::MalformedStream);
        }
        out.push(b'0' + (v / 10) as u8);
        out.push(b'0' + (v % 10) as u8);
    } else if remaining == 1 {
        let v = r.read_bits(4).ok_or(SaParseError::MalformedStream)?;
        if v >= 10 {
            return Err(SaParseError::MalformedStream);
        }
        out.push(b'0' + v as u8);
    }
    Ok(())
}

fn decode_alpha(r: &mut BitReader<'_>, version: Version, out: &mut Vec<u8>) -> Result<(), SaParseError> {
    let mut remaining =
        r.read_bits(Mode::Alphanumeric.length_bits_count(version)).ok_or(SaParseError::MalformedStream)? as usize;
    while remaining >= 2 {
        let v = r.read_bits(11).ok_or(SaParseError::MalformedStream)? as usize;
        let first = ALPHA_REV.get(v / 45).ok_or(SaParseError::MalformedStream)?;
        let second = ALPHA_REV.get(v % 45).ok_or(SaParseError::MalformedStream)?;
        out.push(*first);
        out.push(*second);
        remaining -= 2;
    }
    if remaining == 1 {
        let v = r.read_bits(6).ok_or(SaParseError::MalformedStream)? as usize;
        out.push(*ALPHA_REV.get(v).ok_or(SaParseError::MalformedStream)?);
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
        let pair = [(cp >> 8) as u8, (cp & 0xff) as u8];
        if !KanjiMode::validate(&pair) {
            return Err(SaParseError::MalformedStream);
        }
        out.extend_from_slice(&pair);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SaParseError, parse_sa_datastream};
    use alloc::vec::Vec;
    use qrcode_core::bits::Bits;
    use qrcode_core::{EcLevel, EncodingMode, KanjiMode, Version};

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
    fn value_16_decodes_from_max_nibble() {
        let bytes = sa_bytes(16, 16, 0x00, |_| {});
        let parsed = parse_sa_datastream(&bytes, Version::Normal(1)).unwrap();
        assert_eq!(parsed.position, 16);
        assert_eq!(parsed.total, 16);
    }

    #[test]
    fn invalid_header_range_is_rejected() {
        // Position nibble 1 means position 2, total nibble 0 means total 1.
        let bytes = [0x31, 0x00, 0x00];
        assert_eq!(parse_sa_datastream(&bytes, Version::Normal(1)), Err(SaParseError::MalformedStream));
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
    fn invalid_alphanumeric_pair_is_rejected_without_panicking() {
        let mut bytes = sa_bytes(1, 2, 0x00, |b| {
            b.push_alphanumeric_data(b"AA").unwrap();
        });
        // The pair value starts after the 20-bit SA header, 4-bit mode, and
        // 9-bit v1 character count. Set all 11 value bits to an invalid
        // base-45 pair (2047 > 44 * 45 + 44).
        for bit in 33..44 {
            bytes[bit / 8] |= 1 << (7 - bit % 8);
        }
        assert_eq!(parse_sa_datastream(&bytes, Version::Normal(1)), Err(SaParseError::MalformedStream));
    }

    #[test]
    fn invalid_alphanumeric_single_is_rejected_without_panicking() {
        let mut bytes = sa_bytes(1, 2, 0x00, |b| {
            b.push_alphanumeric_data(b"A").unwrap();
        });
        // The single-character value starts after the 33-bit prefix above.
        for bit in 33..39 {
            bytes[bit / 8] |= 1 << (7 - bit % 8);
        }
        assert_eq!(parse_sa_datastream(&bytes, Version::Normal(1)), Err(SaParseError::MalformedStream));
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

    #[test]
    fn numeric_values_outside_their_decimal_width_are_rejected() {
        // SA header (position 1 of 2), Numeric mode, count 3 / 2 / 1,
        // then the largest 10 / 7 / 4-bit value instead of decimal digits.
        for bytes in [
            &[0x30, 0x10, 0x01, 0x00, 0xff, 0xf0][..],
            &[0x30, 0x10, 0x01, 0x00, 0xbf, 0x80][..],
            &[0x30, 0x10, 0x01, 0x00, 0x7c, 0x00][..],
        ] {
            assert_eq!(parse_sa_datastream(bytes, Version::Normal(1)), Err(SaParseError::MalformedStream));
        }
    }

    #[test]
    fn largest_valid_numeric_values_round_trip() {
        for data in [b"999".as_slice(), b"99", b"9"] {
            let bytes = sa_bytes(1, 2, 0x00, |bits| {
                bits.push_numeric_data(data).unwrap();
            });
            assert_eq!(parse_sa_datastream(&bytes, Version::Normal(1)).unwrap().data, data);
        }
    }

    #[test]
    fn unsupported_modes_do_not_return_a_partial_payload() {
        for mode in [0b0011, 0b0101, 0b0110, 0b0111, 0b1001, 0b1111] {
            let mut bytes = sa_bytes(1, 2, 0x00, |bits| {
                bits.push_byte_data(b"a").unwrap();
            });
            // Replace the terminator immediately after the byte payload with
            // an unsupported mode, leaving a valid decoded prefix before it.
            bytes[5] = mode << 4;
            assert_eq!(parse_sa_datastream(&bytes, Version::Normal(1)), Err(SaParseError::MalformedStream));
        }
    }

    #[test]
    fn invalid_or_micro_versions_are_rejected_before_reading_counts() {
        let bytes = sa_bytes(1, 2, 0x00, |bits| {
            bits.push_numeric_data(b"123").unwrap();
        });
        for version in [
            Version::Normal(0),
            Version::Normal(41),
            Version::Normal(i16::MIN),
            Version::Normal(i16::MAX),
            Version::Micro(-1),
            Version::Micro(1),
            Version::Micro(4),
        ] {
            assert_eq!(parse_sa_datastream(&bytes, version), Err(SaParseError::MalformedStream));
        }
    }

    #[test]
    fn all_data_modes_round_trip_across_normal_version_count_tiers() {
        let expected = b"0123AC-\xff\x93\x5f\xe4\xaa";
        for number in [1, 9, 10, 26, 27, 40] {
            let version = Version::Normal(number);
            let mut bits = Bits::new(version);
            bits.push_structured_append_header(2, 3, 0x5a).unwrap();
            bits.push_numeric_data(b"0123").unwrap();
            bits.push_alphanumeric_data(b"AC-").unwrap();
            bits.push_byte_data(b"\xff").unwrap();
            bits.push_kanji_data(b"\x93\x5f\xe4\xaa").unwrap();
            bits.push_terminator(EcLevel::L).unwrap();

            let parsed = parse_sa_datastream(&bits.into_bytes(), version).unwrap();
            assert_eq!(parsed.position, 2, "version {number}");
            assert_eq!(parsed.total, 3, "version {number}");
            assert_eq!(parsed.parity, 0x5a, "version {number}");
            assert_eq!(parsed.data, expected, "version {number}");
        }
    }

    #[test]
    fn a_truncated_next_mode_does_not_return_an_incomplete_payload() {
        // SA header, Numeric "1", Byte "b", terminator. The Numeric payload
        // ends at bit 38, so taking five bytes leaves the next mode's "01".
        let complete = [0x30, 0x10, 0x01, 0x00, 0x45, 0x00, 0x58, 0x80];
        assert_eq!(parse_sa_datastream(&complete, Version::Normal(1)).unwrap().data, b"1b");
        let truncated = [0x30, 0x10, 0x01, 0x00, 0x45];
        assert_eq!(truncated.as_slice(), &complete[..5]);
        assert_eq!(parse_sa_datastream(&truncated, Version::Normal(1)), Err(SaParseError::MalformedStream));
    }

    #[test]
    fn shortened_terminators_accept_only_zero_bits() {
        let cases = [
            (sa_bytes(1, 2, 0, |bits| bits.push_alphanumeric_data(b"A").unwrap()), 5, b"A".as_slice()),
            (sa_bytes(1, 2, 0, |bits| bits.push_numeric_data(b"1").unwrap()), 5, b"1".as_slice()),
            (sa_bytes(1, 2, 0, |bits| bits.push_kanji_data(b"\x93\x5f").unwrap()), 6, b"\x93\x5f".as_slice()),
        ];
        // These data segments leave respectively one, two, and three bits at
        // the end of their last byte. Preserve each all-zero shortened ending.
        for (mut bytes, length, expected) in cases {
            bytes.truncate(length);
            assert_eq!(parse_sa_datastream(&bytes, Version::Normal(1)).unwrap().data, expected);
            *bytes.last_mut().unwrap() |= 1;
            assert_eq!(parse_sa_datastream(&bytes, Version::Normal(1)), Err(SaParseError::MalformedStream));
        }
    }

    #[test]
    fn a_full_terminator_preserves_tolerance_for_trailing_padding() {
        let mut bytes = sa_bytes(1, 2, 0, |bits| bits.push_byte_data(b"a").unwrap());
        // The four-bit terminator starts at bit 40. Leave it intact and replace
        // every following alignment/pad bit with one.
        bytes[5] |= 0x0f;
        bytes[6..].fill(0xff);
        assert_eq!(parse_sa_datastream(&bytes, Version::Normal(1)).unwrap().data, b"a");
    }

    #[test]
    fn reserved_kanji_values_outside_encoder_ranges_are_malformed() {
        for bytes in [
            [0x30, 0x10, 0x08, 0x01, 0xb9, 0xe8, 0x00],
            [0x30, 0x10, 0x08, 0x01, 0xb9, 0xf0, 0x00],
            [0x30, 0x10, 0x08, 0x01, 0xb9, 0xf8, 0x00],
        ] {
            // Values 5949 / 5950 / 5951 would produce 9FFD / 9FFE / 9FFF.
            assert_eq!(parse_sa_datastream(&bytes, Version::Normal(1)), Err(SaParseError::MalformedStream));
        }
    }

    #[test]
    fn every_kanji_value_matches_the_existing_core_encoder_contract() {
        for value in 0_u16..=8191 {
            let bytes = [0x30, 0x10, 0x08, 0x01, (value >> 5) as u8, (value << 3) as u8, 0x00];
            let parsed = parse_sa_datastream(&bytes, Version::Normal(1));
            if (5949..=5951).contains(&value) {
                assert_eq!(parsed, Err(SaParseError::MalformedStream));
                continue;
            }
            let parsed = parsed.unwrap();
            assert!(KanjiMode::validate(&parsed.data), "Kanji value {value}");
            let mut encoded = Bits::new(Version::Normal(1));
            encoded.push_structured_append_header(1, 2, 0).unwrap();
            encoded.push_kanji_data(&parsed.data).unwrap();
            assert_eq!(encoded.into_bytes(), &bytes[..6], "Kanji value {value}");
        }
    }
}
