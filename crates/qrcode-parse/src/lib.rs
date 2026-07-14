//! Content parsing for QR payloads.
//!
//! Parsers that turn a decoded QR payload (the raw bytes a scanner recovers)
//! back into structured values. These are the symmetric decode-side counterpart
//! to the `qrcode-rs` facade's `QrCode::for_wifi`, `QrCode::for_vcard`, and
//! `QrCode::for_gs1` constructors, which encode in the opposite direction.
//!
//! Encoding is one-way: a QR code symbol does not retain its input payload, so
//! these parsers operate on a `&str`/`&[u8]` the caller supplies.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod gs1;
pub mod vcard;
pub mod wifi;

use core::fmt::{Display, Error, Formatter};

/// Errors returned by the content parsers in this module.
///
/// Each parser returns `Result<T, ParseError>`. The enum is `#[non_exhaustive]`:
/// future versions may add variants, so external callers should match with a `_`
/// arm.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The input did not match the expected wire format (e.g. missing the
    /// `WIFI:` prefix or the `BEGIN:VCARD` sentinel).
    InvalidFormat,
    /// A required field was missing. Carries the field name.
    MissingField(&'static str),
    /// A field was present but its value was invalid (e.g. an unrecognized
    /// WiFi security mode).
    InvalidValue,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self {
            Self::InvalidFormat => f.write_str("invalid QR payload format"),
            Self::MissingField(name) => write!(f, "missing required field: {name}"),
            Self::InvalidValue => f.write_str("invalid field value"),
        }
    }
}

impl ::core::error::Error for ParseError {}
