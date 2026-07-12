//! vCard (`.vcf`) parsing and encoding.
//!
//! The encoder emits a minimal vCard 3.0 card; the parser is tolerant of vCard
//! 2.1 / 3.0 / 4.0, accepts `\n` or `\r\n` line endings, unfolds folded lines,
//! and ignores property parameters (e.g. the `;TYPE=cell` in `TEL;TYPE=cell:`).
//! This module owns the format so [`QrCode::for_vcard`](crate::QrCode::for_vcard)
//! and [`VCard::parse`] stay symmetric.

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::{borrow::ToOwned, format, string::String, vec, vec::Vec};

use crate::parse::ParseError;

/// A parsed vCard contact recovered from a `BEGIN:VCARD` … `END:VCARD` payload.
///
/// Each field holds the first value seen for its property (`FN`/`N`, `TEL`,
/// `EMAIL`, `ORG`, `URL`, `ADR`). The struct is `#[non_exhaustive]`: read via
/// the accessors; additional fields may appear in 1.x.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VCard {
    /// Formatted name (`FN`), falling back to the structured `N` property.
    name: Option<String>,
    /// Telephone number (`TEL`).
    phone: Option<String>,
    /// Email address (`EMAIL`).
    email: Option<String>,
    /// Organization (`ORG`).
    organization: Option<String>,
    /// URL (`URL`).
    url: Option<String>,
    /// Address (`ADR`), stored as the raw structured value.
    address: Option<String>,
}

impl VCard {
    /// Parses a vCard payload.
    ///
    /// Tolerant of versions 2.1 / 3.0 / 4.0, either line ending, line folding,
    /// and property parameters. The name comes from `FN`, or from `N` when no
    /// `FN` is present.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::InvalidFormat`] if no `BEGIN:VCARD` line is found.
    pub fn parse(s: &str) -> Result<Self, ParseError> {
        let mut began = false;
        let mut fn_name = None;
        let mut n_name = None;
        let mut phone = None;
        let mut email = None;
        let mut organization = None;
        let mut url = None;
        let mut address = None;

        for line in unfold(s) {
            let Some((prop, value)) = line.split_once(':') else {
                continue; // blank or keyless line — skip
            };
            // The property name is the segment before any `;` params.
            let key = prop.split(';').next().unwrap_or("").to_ascii_uppercase();
            match key.as_str() {
                "BEGIN" if value.eq_ignore_ascii_case("VCARD") => began = true,
                "FN" if fn_name.is_none() => fn_name = Some(value.to_owned()),
                "N" if n_name.is_none() => n_name = Some(parse_n(value)),
                "TEL" if phone.is_none() => phone = Some(value.to_owned()),
                "EMAIL" if email.is_none() => email = Some(value.to_owned()),
                "ORG" if organization.is_none() => organization = Some(parse_semicolons(value)),
                "URL" if url.is_none() => url = Some(value.to_owned()),
                "ADR" if address.is_none() => address = Some(value.to_owned()),
                _ => {}
            }
        }

        if !began {
            return Err(ParseError::InvalidFormat);
        }
        Ok(Self { name: fn_name.or(n_name), phone, email, organization, url, address })
    }

    /// The formatted name (`FN`), or a best-effort rendering of `N`.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The first telephone number (`TEL`).
    #[must_use]
    pub fn phone(&self) -> Option<&str> {
        self.phone.as_deref()
    }

    /// The first email address (`EMAIL`).
    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// The organization (`ORG`).
    #[must_use]
    pub fn organization(&self) -> Option<&str> {
        self.organization.as_deref()
    }

    /// The URL (`URL`).
    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// The raw structured address value (`ADR`).
    #[must_use]
    pub fn address(&self) -> Option<&str> {
        self.address.as_deref()
    }
}

/// Encodes a minimal vCard 3.0 card. The single source of truth shared with
/// [`QrCode::for_vcard`](crate::QrCode::for_vcard).
pub(crate) fn encode_vcard(name: &str, phone: &str, email: &str) -> String {
    format!("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:{name}\r\nTEL:{phone}\r\nEMAIL:{email}\r\nEND:VCARD\r\n")
}

/// Splits the payload into unfolded logical lines, normalizing `\r\n` and `\n`.
/// A line beginning with a space or tab is a continuation of the previous line
/// (vCard line folding) and is appended to it.
fn unfold(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in s.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if (line.starts_with(' ') || line.starts_with('\t')) && out.last().is_some() {
            // Continuation: drop the leading fold char and append.
            // The fold char is ASCII (1 byte), so index 1 is a valid boundary.
            if let Some(last) = out.last_mut() {
                last.push_str(&line[1..]);
            }
        } else {
            out.push(line.to_owned());
        }
    }
    out
}

/// Joins the non-empty components of a structured `N` value
/// (`Family;Given;Additional;Prefix;Suffix`) with single spaces.
fn parse_n(value: &str) -> String {
    let parts: Vec<&str> = value.split(';').filter(|p| !p.is_empty()).collect();
    parts.join(" ")
}

/// Joins a multi-component value (`ORG` can be `Company;Unit`) with `; `.
fn parse_semicolons(value: &str) -> String {
    value.split(';').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_minimal_card() {
        let payload = encode_vcard("John Doe", "+1234567890", "john@example.com");
        let card = VCard::parse(&payload).unwrap();
        assert_eq!(card.name(), Some("John Doe"));
        assert_eq!(card.phone(), Some("+1234567890"));
        assert_eq!(card.email(), Some("john@example.com"));
        assert_eq!(card.organization(), None);
    }

    #[test]
    fn parse_vcard4_with_params_and_lf() {
        let s = "BEGIN:VCARD\nVERSION:4.0\nFN:Jane Roe\nTEL;TYPE=cell:+15551234\nEMAIL:jane@example.org\nORG:Acme;Widgets\nURL:https://example.org\nADR;TYPE=home:;;123 Main St;Springfield;IL;62701;USA\nEND:VCARD\n";
        let card = VCard::parse(s).unwrap();
        assert_eq!(card.name(), Some("Jane Roe"));
        assert_eq!(card.phone(), Some("+15551234"));
        assert_eq!(card.email(), Some("jane@example.org"));
        assert_eq!(card.organization(), Some("Acme; Widgets"));
        assert_eq!(card.url(), Some("https://example.org"));
        assert_eq!(card.address(), Some(";;123 Main St;Springfield;IL;62701;USA"));
    }

    #[test]
    fn name_falls_back_to_structured_n() {
        let s = "BEGIN:VCARD\nVERSION:3.0\nN:Doe;John;;;Jr\nEND:VCARD\n";
        let card = VCard::parse(s).unwrap();
        assert_eq!(card.name(), Some("Doe John Jr"));
    }

    #[test]
    fn unfolds_folded_lines() {
        // A folded URL: the second line is a continuation.
        let s = "BEGIN:VCARD\nVERSION:3.0\nFN:Fold\nURL:https://exa\n mple.org/x\nEND:VCARD\n";
        let card = VCard::parse(s).unwrap();
        assert_eq!(card.url(), Some("https://example.org/x"));
    }

    #[test]
    fn missing_begin_errors() {
        assert_eq!(VCard::parse("VERSION:3.0\nFN:Nope\n"), Err(ParseError::InvalidFormat));
    }
}
