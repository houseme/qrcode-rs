//! Type-level QR encoding modes.
//!
//! This module complements the runtime [`Mode`] enum with marker
//! types that can be used in generic APIs such as
//! [`Bits::push_mode_data`](crate::bits::Bits::push_mode_data).

use crate::Mode;

/// A type-level QR encoding mode.
pub trait EncodingMode {
    /// Runtime mode represented by this marker type.
    const MODE: Mode;

    /// Returns whether `data` is valid for this mode.
    fn validate(data: &[u8]) -> bool;

    /// Returns the first invalid byte for this mode, if any.
    fn invalid_character(data: &[u8]) -> Option<(usize, u8)>;

    /// Returns the number of logical characters represented by `data`.
    ///
    /// This differs from `data.len()` for Kanji mode, where every character is
    /// represented by two Shift-JIS bytes.
    fn character_count(data: &[u8]) -> usize {
        data.len()
    }

    /// Approximate payload bits per logical character for this mode.
    fn bits_per_character() -> f64;
}

/// Type-level numeric mode marker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NumericMode;

impl EncodingMode for NumericMode {
    const MODE: Mode = Mode::Numeric;

    fn validate(data: &[u8]) -> bool {
        data.iter().all(u8::is_ascii_digit)
    }

    fn invalid_character(data: &[u8]) -> Option<(usize, u8)> {
        data.iter().copied().enumerate().find(|(_, byte)| !byte.is_ascii_digit())
    }

    fn bits_per_character() -> f64 {
        10.0 / 3.0
    }
}

/// Type-level alphanumeric mode marker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AlphanumericMode;

impl EncodingMode for AlphanumericMode {
    const MODE: Mode = Mode::Alphanumeric;

    fn validate(data: &[u8]) -> bool {
        data.iter().copied().all(is_alphanumeric)
    }

    fn invalid_character(data: &[u8]) -> Option<(usize, u8)> {
        data.iter().copied().enumerate().find(|(_, byte)| !is_alphanumeric(*byte))
    }

    fn bits_per_character() -> f64 {
        11.0 / 2.0
    }
}

/// Type-level byte mode marker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ByteMode;

impl EncodingMode for ByteMode {
    const MODE: Mode = Mode::Byte;

    fn validate(_data: &[u8]) -> bool {
        true
    }

    fn invalid_character(_data: &[u8]) -> Option<(usize, u8)> {
        None
    }

    fn bits_per_character() -> f64 {
        8.0
    }
}

/// Type-level Shift-JIS Kanji mode marker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KanjiMode;

impl EncodingMode for KanjiMode {
    const MODE: Mode = Mode::Kanji;

    fn validate(data: &[u8]) -> bool {
        Self::invalid_character(data).is_none()
    }

    fn invalid_character(data: &[u8]) -> Option<(usize, u8)> {
        for (index, chunk) in data.chunks(2).enumerate() {
            if chunk.len() != 2 {
                return Some((index * 2, chunk[0]));
            }
            let hi = chunk[0];
            let lo = chunk[1];
            let codepoint = u16::from(hi) << 8 | u16::from(lo);
            if !is_kanji_codepoint(codepoint) {
                return Some((index * 2, hi));
            }
        }
        None
    }

    fn character_count(data: &[u8]) -> usize {
        data.len() / 2
    }

    fn bits_per_character() -> f64 {
        13.0
    }
}

fn is_alphanumeric(byte: u8) -> bool {
    matches!(
        byte,
        b'0'..=b'9' | b'A'..=b'Z' | b' ' | b'$' | b'%' | b'*' | b'+' | b'-' | b'.' | b'/' | b':'
    )
}

fn is_kanji_codepoint(codepoint: u16) -> bool {
    (0x8140..=0x9ffc).contains(&codepoint) || (0xe040..=0xebbf).contains(&codepoint)
}

#[cfg(test)]
mod tests {
    use super::{AlphanumericMode, ByteMode, EncodingMode, KanjiMode, NumericMode};
    use crate::Mode;

    #[test]
    fn numeric_mode_validates_ascii_digits() {
        assert!(NumericMode::validate(b"0123456789"));
        assert_eq!(NumericMode::invalid_character(b"12a"), Some((2, b'a')));
        assert_eq!(NumericMode::MODE, Mode::Numeric);
    }

    #[test]
    fn alphanumeric_mode_validates_qr_alphabet() {
        assert!(AlphanumericMode::validate(b"HELLO WORLD-42"));
        assert_eq!(AlphanumericMode::invalid_character(b"hello"), Some((0, b'h')));
        assert_eq!(AlphanumericMode::MODE, Mode::Alphanumeric);
    }

    #[test]
    fn byte_mode_accepts_any_bytes() {
        assert!(ByteMode::validate(&[0, b'a', 255]));
        assert_eq!(ByteMode::invalid_character(&[0, b'a', 255]), None);
        assert_eq!(ByteMode::MODE, Mode::Byte);
    }

    #[test]
    fn kanji_mode_validates_shift_jis_pairs() {
        assert!(KanjiMode::validate(b"\x93\x5f\xe4\xaa"));
        assert_eq!(KanjiMode::character_count(b"\x93\x5f\xe4\xaa"), 2);
        assert_eq!(KanjiMode::invalid_character(b"\x93"), Some((0, 0x93)));
        assert_eq!(KanjiMode::invalid_character(b"\x01\x02"), Some((0, 0x01)));
        assert_eq!(KanjiMode::MODE, Mode::Kanji);
    }
}
