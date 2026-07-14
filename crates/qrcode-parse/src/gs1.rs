//! GS1 application-identifier (AI) parsing.
//!
//! A GS1 QR payload is a sequence of application-identifier/value pairs. Fixed-
//! length AIs are concatenated directly; variable-length AIs are terminated by
//! the GS separator byte (`0x1D`) which a QR decoder emits for FNC1 in GS1 mode
//! (see `bits.rs`). This parser splits such a byte stream back into elements.
//!
//! It ships with a curated table of the most common AIs plus the
//! measure-measure families (`310n`–`369n`). Unknown AIs are surfaced
//! best-effort (2-digit id, value up to the next GS) so real-world data is
//! never silently truncated; full coverage of all 200+ GS1 AIs is out of scope.

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::{borrow::ToOwned, string::String, vec, vec::Vec};

use crate::ParseError;

/// The length kind of a GS1 application identifier.
#[derive(Clone, Copy, Debug)]
enum Len {
    /// A fixed number of bytes (no GS separator follows).
    Fixed(usize),
    /// Variable length, terminated by `0x1D` (GS) or end of data.
    Variable,
}

/// A static entry in the curated application-identifier table.
struct AiSpec {
    /// The application-identifier digits (2–4).
    ai: &'static str,
    /// Human-readable title.
    desc: &'static str,
    /// Value length kind.
    len: Len,
}

/// Curated table of common GS1 application identifiers.
#[rustfmt::skip]
const AI_TABLE: &[AiSpec] = &[
    AiSpec { ai: "00",  desc: "SSCC (serial shipping container code)",     len: Len::Fixed(18) },
    AiSpec { ai: "01",  desc: "GTIN (global trade item number)",            len: Len::Fixed(14) },
    AiSpec { ai: "02",  desc: "GTIN of contained items",                    len: Len::Fixed(14) },
    AiSpec { ai: "10",  desc: "Batch or lot number",                         len: Len::Variable },
    AiSpec { ai: "11",  desc: "Production date (YYMMDD)",                    len: Len::Fixed(6) },
    AiSpec { ai: "13",  desc: "Packaging date (YYMMDD)",                     len: Len::Fixed(6) },
    AiSpec { ai: "15",  desc: "Best-before date (YYMMDD)",                   len: Len::Fixed(6) },
    AiSpec { ai: "16",  desc: "Sell-by date (YYMMDD)",                       len: Len::Fixed(6) },
    AiSpec { ai: "17",  desc: "Expiration date (YYMMDD)",                   len: Len::Fixed(6) },
    AiSpec { ai: "20",  desc: "Internal product variant",                    len: Len::Fixed(2) },
    AiSpec { ai: "21",  desc: "Serial number",                               len: Len::Variable },
    AiSpec { ai: "22",  desc: "Consumer product variant",                    len: Len::Variable },
    AiSpec { ai: "30",  desc: "Count of items (variable measure)",          len: Len::Variable },
    AiSpec { ai: "37",  desc: "Count of items contained",                   len: Len::Variable },
    AiSpec { ai: "240", desc: "Additional item identification",             len: Len::Variable },
    AiSpec { ai: "241", desc: "Customer part number",                        len: Len::Variable },
    AiSpec { ai: "242", desc: "Made-to-order variation number",             len: Len::Variable },
    AiSpec { ai: "243", desc: "Packaging component number",                  len: Len::Variable },
    AiSpec { ai: "244", desc: "Ground crew badge number",                    len: Len::Variable },
    AiSpec { ai: "400", desc: "Customer order number",                       len: Len::Variable },
    AiSpec { ai: "401", desc: "Consignment number",                          len: Len::Variable },
    AiSpec { ai: "402", desc: "Global shipment identification number",      len: Len::Fixed(17) },
    AiSpec { ai: "403", desc: "Routing code",                                 len: Len::Variable },
    AiSpec { ai: "410", desc: "Ship to / deliver to GLN",                    len: Len::Fixed(13) },
    AiSpec { ai: "411", desc: "Bill to / invoice to GLN",                    len: Len::Fixed(13) },
    AiSpec { ai: "412", desc: "Purchased from GLN",                          len: Len::Fixed(13) },
    AiSpec { ai: "413", desc: "Ship for / deliver for GLN",                  len: Len::Fixed(13) },
    AiSpec { ai: "414", desc: "Identification of a physical location (GLN)", len: Len::Fixed(13) },
    AiSpec { ai: "415", desc: "GLN of the invoicing party",                  len: Len::Fixed(13) },
    AiSpec { ai: "416", desc: "GLN of the physical location",                len: Len::Fixed(13) },
    AiSpec { ai: "420", desc: "Ship to / deliver to postal code",            len: Len::Variable },
    AiSpec { ai: "421", desc: "Ship to / deliver to postal code (with ISO country prefix)", len: Len::Variable },
    AiSpec { ai: "422", desc: "Country of origin (ISO 3166)",                len: Len::Fixed(3) },
    AiSpec { ai: "423", desc: "Country of initial processing",               len: Len::Variable },
    AiSpec { ai: "425", desc: "Country of disassembly",                      len: Len::Variable },
    AiSpec { ai: "426", desc: "Country of processing (ISO 3166)",            len: Len::Fixed(3) },
    AiSpec { ai: "427", desc: "Country subdivision (ISO 3166-2)",            len: Len::Variable },
];

/// A resolved application identifier match: how many digits it occupies, its
/// description, and its value length kind.
struct AiMatch {
    /// Number of leading digits forming the AI (2, 3, or 4).
    len: usize,
    /// Human-readable title.
    desc: &'static str,
    /// Value length kind.
    length: Len,
}

/// Returns `true` if the 3-digit value is a GS1 trade-item-measure family
/// prefix (`310`–`316`, `320`–`326`, `334`–`337`, `340`–`357`, `360`–`369`).
/// These are 4-digit AIs (3-digit prefix + 1 decimal-count digit) with a fixed
/// 6-digit value. The gaps (e.g. `317`–`319`, `37x`) are deliberately excluded
/// so the 2-digit AI `37` (count) is not misread as a measure code.
fn is_measure_family(d: u32) -> bool {
    (310..=316).contains(&d)
        || (320..=326).contains(&d)
        || (334..=337).contains(&d)
        || (340..=357).contains(&d)
        || (360..=369).contains(&d)
}

/// Parses the first `n` bytes as a decimal number, or `None` if there are
/// fewer than `n` bytes or any is not an ASCII digit.
fn digits(data: &[u8], n: usize) -> Option<u32> {
    if data.len() < n {
        return None;
    }
    let mut v = 0u32;
    for &b in &data[..n] {
        if !b.is_ascii_digit() {
            return None;
        }
        v = v * 10 + u32::from(b - b'0');
    }
    Some(v)
}

/// Matches the application identifier at the start of `data`, preferring the
/// longest match. Measure families are checked before the curated table.
fn match_ai(data: &[u8]) -> Option<AiMatch> {
    // Measure family: 3-digit prefix with a 4th digit present → 4-digit AI, fixed 6.
    if let Some(d3) = digits(data, 3)
        && digits(data, 4).is_some()
        && is_measure_family(d3)
    {
        return Some(AiMatch { len: 4, desc: "Trade item measure (GS1 310n–369n family)", length: Len::Fixed(6) });
    }
    // Longest exact table match (4 → 3 → 2).
    for &n in &[4usize, 3, 2] {
        if data.len() >= n {
            let Ok(prefix) = core::str::from_utf8(&data[..n]) else {
                continue;
            };
            if let Some(spec) = AI_TABLE.iter().find(|s| s.ai == prefix) {
                return Some(AiMatch { len: n, desc: spec.desc, length: spec.len });
            }
        }
    }
    None
}

/// One application-identifier/value pair split out of a GS1 payload.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gs1Element {
    /// The application-identifier digits (e.g. `"01"`).
    ai: String,
    /// The raw value bytes (binary-safe; GS1 values are usually ASCII).
    value: Vec<u8>,
    /// Human-readable description of the AI (always a static string).
    description: &'static str,
}

impl Gs1Element {
    /// The application-identifier digits.
    #[must_use]
    pub fn ai(&self) -> &str {
        &self.ai
    }

    /// The raw value bytes.
    #[must_use]
    pub fn value(&self) -> &[u8] {
        &self.value
    }

    /// Human-readable description of the AI.
    #[must_use]
    pub fn description(&self) -> &'static str {
        self.description
    }
}

/// A parsed GS1 payload: the recovered [`Gs1Element`]s plus the original bytes.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gs1Result {
    /// The recovered application-identifier/value pairs, in order.
    elements: Vec<Gs1Element>,
    /// The original input bytes.
    raw_data: Vec<u8>,
}

impl Gs1Result {
    /// Parses a GS1 payload (`&[u8]` with `0x1D` separators between
    /// variable-length AIs) into its elements.
    ///
    /// Tolerant: unknown AIs are surfaced best-effort rather than rejected, so
    /// a payload using an AI outside the curated table still yields elements
    /// (with a generic description) instead of failing.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::InvalidFormat`] only if the input is non-empty but
    /// does not begin with an application-identifier digit.
    pub fn parse(data: &[u8]) -> Result<Self, ParseError> {
        if data.is_empty() {
            return Ok(Self { elements: Vec::new(), raw_data: Vec::new() });
        }
        if !data[0].is_ascii_digit() {
            return Err(ParseError::InvalidFormat);
        }

        let mut elements = Vec::new();
        let mut i = 0;
        while i < data.len() {
            let rest = &data[i..];
            let (ai_len, desc, length) = match match_ai(rest) {
                Some(m) => (m.len, m.desc, m.length),
                None => {
                    // Unknown AI: take up to 2 leading digits as the id and
                    // consume the value up to the next GS separator.
                    let ai_len = rest.iter().take(2).take_while(|b| b.is_ascii_digit()).count().max(1);
                    let ai_len = ai_len.min(rest.len());
                    let value_end = sep_or_end(rest, ai_len);
                    elements.push(Gs1Element {
                        ai: core::str::from_utf8(&rest[..ai_len]).unwrap_or("").to_owned(),
                        value: rest[ai_len..value_end].to_vec(),
                        description: "Unknown application identifier",
                    });
                    i += value_end + usize::from(rest.get(value_end) == Some(&0x1D));
                    continue;
                }
            };

            let ai = core::str::from_utf8(&rest[..ai_len]).unwrap_or("").to_owned();
            i += ai_len;
            let value_end = match length {
                Len::Fixed(n) => (i + n).min(data.len()),
                Len::Variable => sep_or_end(data, i),
            };
            elements.push(Gs1Element { ai, value: data[i..value_end].to_vec(), description: desc });
            i = value_end;
            // Skip the GS separator that terminates a variable-length value.
            if matches!(length, Len::Variable) && data.get(i) == Some(&0x1D) {
                i += 1;
            }
        }

        Ok(Self { elements, raw_data: data.to_vec() })
    }

    /// The recovered application-identifier/value pairs, in order.
    #[must_use]
    pub fn elements(&self) -> &[Gs1Element] {
        &self.elements
    }

    /// The original input bytes.
    #[must_use]
    pub fn raw_data(&self) -> &[u8] {
        &self.raw_data
    }
}

/// Index (relative to `start`) of the next `0x1D` separator, or the data length
/// if none remains.
fn sep_or_end(data: &[u8], start: usize) -> usize {
    data[start..].iter().position(|&b| b == 0x1D).map_or(data.len(), |p| start + p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gtin_dates_batch_serial() {
        // 01 (GTIN, fixed 14) + 15 (best-before, fixed 6) + 10 (batch, var) + 21 (serial, var)
        let mut data = Vec::new();
        data.extend_from_slice(b"0104912345123459"); // 01 + GTIN14
        data.extend_from_slice(b"15970331"); // 15 + YYMMDD6
        data.extend_from_slice(b"10BATCH123"); // 10 + lot (variable)
        data.push(0x1D);
        data.extend_from_slice(b"21SERIAL9"); // 21 + serial (variable, last)

        let r = Gs1Result::parse(&data).unwrap();
        let e = r.elements();
        assert_eq!(e.len(), 4);
        assert_eq!(e[0].ai(), "01");
        assert_eq!(e[0].value(), b"04912345123459");
        assert!(e[0].description().contains("GTIN"));
        assert_eq!(e[1].ai(), "15");
        assert_eq!(e[1].value(), b"970331");
        assert_eq!(e[2].ai(), "10");
        assert_eq!(e[2].value(), b"BATCH123");
        assert_eq!(e[3].ai(), "21");
        assert_eq!(e[3].value(), b"SERIAL9");
        assert_eq!(r.raw_data(), &data[..]);
    }

    #[test]
    fn parses_measure_family() {
        // 3100 = net weight (kg, 0 decimals); 6-digit value.
        let data = b"3100001234";
        let r = Gs1Result::parse(data).unwrap();
        let e = r.elements();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].ai(), "3100");
        assert_eq!(e[0].value(), b"001234");
        assert!(e[0].description().contains("measure"));
    }

    #[test]
    fn count_ai_not_misread_as_measure() {
        // "37" is count (variable); must not be swallowed by the measure range.
        let mut data = Vec::new();
        data.extend_from_slice(b"0112345678901237"); // 01 GTIN14
        data.extend_from_slice(b"3712"); // 37 count + "12" (variable, to end)
        let r = Gs1Result::parse(&data).unwrap();
        let e = r.elements();
        assert_eq!(e[1].ai(), "37");
        assert_eq!(e[1].value(), b"12");
    }

    #[test]
    fn unknown_ai_is_surfaced() {
        // "91" is not in the curated table.
        let r = Gs1Result::parse(b"91ABCDEF").unwrap();
        let e = r.elements();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].ai(), "91");
        assert_eq!(e[0].value(), b"ABCDEF");
        assert_eq!(e[0].description(), "Unknown application identifier");
    }

    #[test]
    fn empty_input_yields_no_elements() {
        let r = Gs1Result::parse(b"").unwrap();
        assert!(r.elements().is_empty());
    }

    #[test]
    fn non_digit_start_errors() {
        assert_eq!(Gs1Result::parse(b"X123"), Err(ParseError::InvalidFormat));
    }

    #[test]
    fn gln_fixed_length() {
        // 410 (ship-to GLN, fixed 13).
        let r = Gs1Result::parse(b"4101234567890128").unwrap();
        let e = r.elements();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].ai(), "410");
        assert_eq!(e[0].value(), b"1234567890128");
    }
}
