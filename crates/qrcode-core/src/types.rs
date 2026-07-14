//! Core data types: module [`Color`], [`EcLevel`], [`Version`], [`Mode`], the
//! [`QrError`] / [`QrResult`] error types, and string-parsing helpers.

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::{
    borrow::ToOwned,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use crate::cast::As;
use core::cmp::{Ordering, PartialOrd};
use core::fmt::{Display, Error, Formatter};
use core::ops::Not;
use core::str::FromStr;

//------------------------------------------------------------------------------
//{{{ QrResult

/// `QrError` encodes the error encountered when generating a QR code.
///
/// This enum is `#[non_exhaustive]`: future versions may add variants, so
/// external callers should match with a `_` arm.
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum QrError {
    /// The data is too long to encode into a QR code for the given version.
    DataTooLong,

    /// The provided version / error correction level combination is invalid.
    /// Carries the offending [`Version`] and [`EcLevel`].
    InvalidVersion {
        /// The version that was requested.
        version: Version,
        /// The error-correction level that was requested.
        ec_level: EcLevel,
    },

    /// Some characters in the data cannot be supported by the provided QR code
    /// version.
    UnsupportedCharacterSet,

    /// The provided ECI designator is invalid. Carries the offending `value`;
    /// a valid designator must be between 0 and 999999.
    InvalidEciDesignator {
        /// The invalid ECI designator value.
        value: u32,
    },

    /// A character not belonging to the character set is found. Carries the
    /// byte `position` (offset into the input) and the offending `byte` value.
    InvalidCharacter {
        /// Byte offset of the offending character within the input.
        position: usize,
        /// The offending byte value.
        byte: u8,
    },

    /// Invalid Structured Append parameter — the symbol count is not in 2..=16,
    /// or the position is not in 1..=total. Carries the offending `value`.
    InvalidStructuredAppend {
        /// The out-of-range symbol count or position.
        value: u8,
    },
}

impl Display for QrError {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match self {
            QrError::DataTooLong => fmt.write_str("data too long to encode"),
            QrError::InvalidVersion { version, ec_level } => {
                write!(fmt, "invalid version {version:?} for error correction level {ec_level:?}")
            }
            QrError::UnsupportedCharacterSet => fmt.write_str("unsupported character set for this version"),
            QrError::InvalidEciDesignator { value } => {
                write!(fmt, "invalid ECI designator {value} (must be 0..=999999)")
            }
            QrError::InvalidCharacter { position, byte } => {
                write!(fmt, "invalid character byte 0x{byte:02x} at position {position}")
            }
            QrError::InvalidStructuredAppend { value } => {
                write!(fmt, "invalid Structured Append parameter {value} (symbols must be 2..=16, position 1..=total)")
            }
        }
    }
}

impl ::core::error::Error for QrError {}

impl QrError {
    /// Returns an actionable hint for fixing this error, if one applies — useful
    /// for surfacing user-facing guidance alongside the error message.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qrcode_core::QrError;
    ///
    /// let err = QrError::DataTooLong;
    /// assert!(err.suggestion().is_some());
    /// ```
    #[must_use]
    pub fn suggestion(&self) -> Option<&'static str> {
        match self {
            QrError::DataTooLong => {
                Some("lower the error-correction level, use a larger version, or split the data with Structured Append")
            }
            QrError::InvalidVersion { .. } => Some(
                "the version / error-correction-level combination is unsupported (e.g. Micro QR supports only L/M)",
            ),
            QrError::UnsupportedCharacterSet => {
                Some("the data cannot be encoded in any mode supported by the chosen version")
            }
            QrError::InvalidEciDesignator { .. } => Some("ECI designators must be in the range 0..=999999"),
            QrError::InvalidCharacter { .. } => {
                Some("the input contains a byte that is invalid for the requested encoding mode")
            }
            QrError::InvalidStructuredAppend { .. } => {
                Some("Structured Append requires 2..=16 symbols and each position in 1..=total")
            }
        }
    }
}

/// `QrResult` is a convenient alias for a QR code generation result.
pub type QrResult<T> = Result<T, QrError>;

//}}}
//------------------------------------------------------------------------------
//{{{ Enum parsing error

/// Error returned when parsing an enum (`EcLevel`, `Version`, `Mode`) from a
/// string via `FromStr`. Carries a short static description of what was
/// expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumParseError(pub &'static str);

impl Display for EnumParseError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str(self.0)
    }
}

impl ::core::error::Error for EnumParseError {}

//}}}
//------------------------------------------------------------------------------
//{{{ Color

/// The color of a module.
///
/// Guaranteed to be a single byte (`#[repr(u8)]`) — useful for FFI and dense
/// storage. `Light = 0`, `Dark = 1`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum Color {
    /// The module is light colored.
    Light = 0,
    /// The module is dark colored.
    Dark = 1,
}

impl Color {
    /// Selects a value according to color of the module. Equivalent to
    /// `if self != Color::Light { dark } else { light }`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use qrcode_core::types::Color;
    /// assert_eq!(Color::Light.select(1, 0), 0);
    /// assert_eq!(Color::Dark.select("black", "white"), "black");
    /// ```
    pub fn select<T>(self, dark: T, light: T) -> T {
        match self {
            Color::Light => light,
            Color::Dark => dark,
        }
    }
}

impl Not for Color {
    type Output = Self;
    fn not(self) -> Self {
        match self {
            Color::Light => Color::Dark,
            Color::Dark => Color::Light,
        }
    }
}

//}}}
//------------------------------------------------------------------------------
//{{{ Error correction level

/// The error correction level. It allows the original information be recovered
/// even if parts of the code is damaged.
#[derive(Debug, PartialEq, Eq, Copy, Clone, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EcLevel {
    /// Low error correction. Allows up to 7% of wrong blocks.
    L = 0,

    /// Medium error correction (default). Allows up to 15% of wrong blocks.
    M = 1,

    /// "Quartile" error correction. Allows up to 25% of wrong blocks.
    Q = 2,

    /// High error correction. Allows up to 30% of wrong blocks.
    H = 3,
}

impl FromStr for EcLevel {
    type Err = EnumParseError;

    /// Parses a case-insensitive single letter: `L`, `M`, `Q` or `H`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use core::str::FromStr;
    /// use qrcode_core::types::EcLevel;
    /// assert_eq!(EcLevel::from_str("H"), Ok(EcLevel::H));
    /// assert_eq!(EcLevel::from_str("m"), Ok(EcLevel::M));
    /// assert!(EcLevel::from_str("X").is_err());
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "l" | "L" => Ok(EcLevel::L),
            "m" | "M" => Ok(EcLevel::M),
            "q" | "Q" => Ok(EcLevel::Q),
            "h" | "H" => Ok(EcLevel::H),
            _ => Err(EnumParseError("expected one of L, M, Q, H")),
        }
    }
}

//}}}
//------------------------------------------------------------------------------
//{{{ Version

/// In QR code terminology, `Version` means the size of the generated image.
/// Larger version means the size of code is larger, and therefore can carry
/// more information.
///
/// The smallest version is `Version::Normal(1)` of size 21×21, and the largest
/// is `Version::Normal(40)` of size 177×177.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Version {
    /// A normal QR code version. The parameter should be between 1 and 40.
    Normal(i16),

    /// A Micro QR code version. The parameter should be between 1 and 4.
    Micro(i16),
}

impl Version {
    /// Get the number of "modules" on each size of the QR code, i.e. the width
    /// and height of the code.
    pub const fn width(self) -> i16 {
        match self {
            Version::Normal(v) => v * 4 + 17,
            Version::Micro(v) => v * 2 + 9,
        }
    }

    /// Obtains an object from a hard-coded table.
    ///
    /// The table must be a 44×4 array. The outer array represents the content
    /// for each version. The first 40 entry corresponds to QR code versions 1
    /// to 40, and the last 4 corresponds to Micro QR code version 1 to 4. The
    /// inner array represents the content in each error correction level, in
    /// the order [L, M, Q, H].
    ///
    /// # Errors
    ///
    /// If the entry compares equal to the default value of `T`, this method
    /// returns `Err(QrError::InvalidVersion)`.
    pub fn fetch<T>(self, ec_level: EcLevel, table: &[[T; 4]]) -> QrResult<T>
    where
        T: PartialEq + Default + Copy,
    {
        match self {
            Version::Normal(v @ 1..=40) => {
                return Ok(table[(v - 1).as_usize()][ec_level as usize]);
            }
            Version::Micro(v @ 1..=4) => {
                let obj = table[(v + 39).as_usize()][ec_level as usize];
                if obj != T::default() {
                    return Ok(obj);
                }
            }
            _ => {}
        }
        Err(QrError::InvalidVersion { version: self, ec_level })
    }

    /// The number of bits needed to encode the mode indicator.
    pub fn mode_bits_count(self) -> usize {
        if let Version::Micro(a) = self { (a - 1).as_usize() } else { 4 }
    }

    /// Checks whether is version refers to a Micro QR code.
    pub fn is_micro(self) -> bool {
        matches!(self, Version::Micro(_))
    }
}

impl FromStr for Version {
    type Err = EnumParseError;

    /// Parses a normal QR version `1..=40` (e.g. `"5"` → `Normal(5)`) or a
    /// Micro QR version `M1..M4` (e.g. `"m2"` → `Micro(2)`), case-insensitively.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use core::str::FromStr;
    /// use qrcode_core::types::Version;
    /// assert_eq!(Version::from_str("5"), Ok(Version::Normal(5)));
    /// assert_eq!(Version::from_str("M4"), Ok(Version::Micro(4)));
    /// assert!(Version::from_str("99").is_err());
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let (micro, digits) = match s.chars().next() {
            Some('M' | 'm') => (true, &s[1..]),
            _ => (false, s),
        };
        let n: i16 = digits.parse().map_err(|_| EnumParseError("expected a version number"))?;
        if micro {
            if (1..=4).contains(&n) {
                Ok(Version::Micro(n))
            } else {
                Err(EnumParseError("Micro QR version must be between M1 and M4"))
            }
        } else if (1..=40).contains(&n) {
            Ok(Version::Normal(n))
        } else {
            Err(EnumParseError("QR version must be between 1 and 40"))
        }
    }
}

//}}}
//------------------------------------------------------------------------------
//{{{ Mode indicator

/// The mode indicator, which specifies the character set of the encoded data.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Mode {
    /// The data contains only characters 0 to 9.
    Numeric,

    /// The data contains only uppercase letters (A–Z), numbers (0–9) and a few
    /// punctuations marks (space, `$`, `%`, `*`, `+`, `-`, `.`, `/`, `:`).
    Alphanumeric,

    /// The data contains arbitrary binary data.
    Byte,

    /// The data contains Shift-JIS-encoded double-byte text.
    Kanji,
}

impl Mode {
    /// Computes the number of bits needed to encode the data length.
    ///
    ///     use qrcode_core::types::{Version, Mode};
    ///
    ///     assert_eq!(Mode::Numeric.length_bits_count(Version::Normal(1)), 10);
    ///
    /// This method will return `Err(QrError::UnsupportedCharacterSet)` if the
    /// mode is not supported in the given version.
    pub fn length_bits_count(self, version: Version) -> usize {
        match version {
            Version::Micro(a) => {
                let a = a.as_usize();
                match self {
                    Mode::Numeric => 2 + a,
                    Mode::Alphanumeric | Mode::Byte => 1 + a,
                    Mode::Kanji => a,
                }
            }
            Version::Normal(1..=9) => match self {
                Mode::Numeric => 10,
                Mode::Alphanumeric => 9,
                Mode::Byte | Mode::Kanji => 8,
            },
            Version::Normal(10..=26) => match self {
                Mode::Numeric => 12,
                Mode::Alphanumeric => 11,
                Mode::Byte => 16,
                Mode::Kanji => 10,
            },
            Version::Normal(_) => match self {
                Mode::Numeric => 14,
                Mode::Alphanumeric => 13,
                Mode::Byte => 16,
                Mode::Kanji => 12,
            },
        }
    }

    /// Computes the number of bits needed to some data of a given raw length.
    ///
    ///     use qrcode_core::types::Mode;
    ///
    ///     assert_eq!(Mode::Numeric.data_bits_count(7), 24);
    ///
    /// Note that in Kanji mode, the `raw_data_len` is the number of Kanjis,
    /// i.e. half the total size of bytes.
    pub fn data_bits_count(self, raw_data_len: usize) -> usize {
        match self {
            Mode::Numeric => (raw_data_len * 10).div_ceil(3),
            Mode::Alphanumeric => (raw_data_len * 11).div_ceil(2),
            Mode::Byte => raw_data_len * 8,
            Mode::Kanji => raw_data_len * 13,
        }
    }

    /// Find the lowest common mode which both modes are compatible with.
    ///
    ///     use qrcode_core::types::Mode;
    ///
    ///     let a = Mode::Numeric;
    ///     let b = Mode::Kanji;
    ///     let c = a.max(b);
    ///     assert!(a <= c);
    ///     assert!(b <= c);
    ///
    #[must_use]
    pub fn max(self, other: Self) -> Self {
        match self.partial_cmp(&other) {
            Some(Ordering::Greater) => self,
            Some(_) => other,
            None => Mode::Byte,
        }
    }
}

impl PartialOrd for Mode {
    /// Defines a partial ordering between modes. If `a <= b`, then `b` contains
    /// a superset of all characters supported by `a`.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (a, b) if a == b => Some(Ordering::Equal),
            (Mode::Numeric, Mode::Alphanumeric) | (_, Mode::Byte) => Some(Ordering::Less),
            (Mode::Alphanumeric, Mode::Numeric) | (Mode::Byte, _) => Some(Ordering::Greater),
            _ => None,
        }
    }
}

impl FromStr for Mode {
    type Err = EnumParseError;

    /// Parses a mode by name, case-insensitively: `numeric`, `alphanumeric`,
    /// `byte` or `kanji`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use core::str::FromStr;
    /// use qrcode_core::types::Mode;
    /// assert_eq!(Mode::from_str("Byte"), Ok(Mode::Byte));
    /// assert!(Mode::from_str("utf8").is_err());
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "numeric" => Ok(Mode::Numeric),
            "alphanumeric" => Ok(Mode::Alphanumeric),
            "byte" => Ok(Mode::Byte),
            "kanji" => Ok(Mode::Kanji),
            _ => Err(EnumParseError("expected numeric, alphanumeric, byte or kanji")),
        }
    }
}

#[cfg(test)]
mod parse_tests {
    use core::str::FromStr;

    use crate::types::{Color, EcLevel, EnumParseError, Mode, QrError, Version};

    #[test]
    fn test_ec_level_from_str() {
        assert_eq!(EcLevel::from_str("L"), Ok(EcLevel::L));
        assert_eq!(EcLevel::from_str("q"), Ok(EcLevel::Q));
        assert_eq!(EcLevel::from_str("  H "), Ok(EcLevel::H));
        assert_eq!(EcLevel::from_str("X"), Err(EnumParseError("expected one of L, M, Q, H")));
    }

    #[test]
    fn test_version_from_str() {
        assert_eq!(Version::from_str("1"), Ok(Version::Normal(1)));
        assert_eq!(Version::from_str("40"), Ok(Version::Normal(40)));
        assert_eq!(Version::from_str("m3"), Ok(Version::Micro(3)));
        assert!(Version::from_str("0").is_err());
        assert!(Version::from_str("41").is_err());
        assert!(Version::from_str("M5").is_err());
        assert!(Version::from_str("abc").is_err());
    }

    #[test]
    fn test_mode_from_str() {
        assert_eq!(Mode::from_str("Numeric"), Ok(Mode::Numeric));
        assert_eq!(Mode::from_str("ALPHANUMERIC"), Ok(Mode::Alphanumeric));
        assert_eq!(Mode::from_str(" kanji "), Ok(Mode::Kanji));
        assert!(Mode::from_str("text").is_err());
    }

    #[test]
    fn test_color_repr() {
        assert_eq!(Color::Light as u8, 0);
        assert_eq!(Color::Dark as u8, 1);
        assert_eq!(core::mem::size_of::<Color>(), 1);
    }

    #[test]
    fn test_error_suggestions() {
        // Every current variant has an actionable suggestion.
        assert!(QrError::DataTooLong.suggestion().is_some());
        assert!(QrError::InvalidVersion { version: Version::Normal(1), ec_level: EcLevel::M }.suggestion().is_some());
        assert!(QrError::UnsupportedCharacterSet.suggestion().is_some());
        assert!(QrError::InvalidEciDesignator { value: 1_000_000 }.suggestion().is_some());
        assert!(QrError::InvalidCharacter { position: 0, byte: 0 }.suggestion().is_some());
        assert!(QrError::InvalidStructuredAppend { value: 17 }.suggestion().is_some());
    }
}

#[cfg(test)]
mod mode_tests {
    use crate::types::Mode::{Alphanumeric, Byte, Kanji, Numeric};

    #[test]
    fn test_mode_order() {
        assert!(Numeric < Alphanumeric);
        assert!(Byte > Kanji);
        assert!(Numeric.partial_cmp(&Kanji).is_none());
    }

    #[test]
    fn test_max() {
        assert_eq!(Byte.max(Kanji), Byte);
        assert_eq!(Numeric.max(Alphanumeric), Alphanumeric);
        assert_eq!(Alphanumeric.max(Alphanumeric), Alphanumeric);
        assert_eq!(Numeric.max(Kanji), Byte);
        assert_eq!(Kanji.max(Numeric), Byte);
        assert_eq!(Alphanumeric.max(Numeric), Alphanumeric);
        assert_eq!(Kanji.max(Kanji), Kanji);
    }
}

//}}}
