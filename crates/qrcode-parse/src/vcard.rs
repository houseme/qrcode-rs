//! vCard (`.vcf`) parsing and encoding.
//!
//! The encoder emits a minimal vCard 3.0 card; the parser is tolerant of vCard
//! 2.1 / 3.0 / 4.0, accepts `\n` or `\r\n` line endings, unfolds folded lines,
//! and ignores property parameters (e.g. the `;TYPE=cell` in `TEL;TYPE=cell:`).
//! The `qrcode-rs` facade uses this same module for its convenience
//! constructor, so [`encode_vcard`] and [`VCard::parse`] stay symmetric.

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::{
    borrow::{Cow, ToOwned},
    string::String,
};

#[cfg(feature = "std")]
use std::borrow::Cow;

use crate::ParseError;

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
    /// `FN` is present. Text values are unescaped, while `URL` and the raw
    /// structured `ADR` value are preserved. Only the first card is read;
    /// properties outside its `BEGIN:VCARD` / `END:VCARD` boundaries are ignored.
    /// A missing `END:VCARD` remains accepted for incomplete scanner payloads.
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
            let key = prop.split(';').next().unwrap_or("");
            if !began {
                if key.eq_ignore_ascii_case("BEGIN") && value.eq_ignore_ascii_case("VCARD") {
                    began = true;
                }
                continue;
            }
            if key.eq_ignore_ascii_case("END") && value.eq_ignore_ascii_case("VCARD") {
                break;
            }
            if key.eq_ignore_ascii_case("FN") && fn_name.is_none() {
                fn_name = Some(unescape_text(value));
            } else if key.eq_ignore_ascii_case("N") && n_name.is_none() {
                n_name = Some(parse_n(value));
            } else if key.eq_ignore_ascii_case("TEL") && phone.is_none() {
                phone = Some(unescape_text(value));
            } else if key.eq_ignore_ascii_case("EMAIL") && email.is_none() {
                email = Some(unescape_text(value));
            } else if key.eq_ignore_ascii_case("ORG") && organization.is_none() {
                organization = Some(parse_semicolons(value));
            } else if key.eq_ignore_ascii_case("URL") && url.is_none() {
                url = Some(value.to_owned());
            } else if key.eq_ignore_ascii_case("ADR") && address.is_none() {
                address = Some(value.to_owned());
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

/// Encodes a minimal vCard 3.0 card.
///
/// This is the single source of truth shared with the `qrcode-rs` facade's
/// `QrCode::for_vcard` constructor. Text values escape backslashes, commas,
/// semicolons, and newlines; CRLF and bare CR are normalized to LF.
pub fn encode_vcard(name: &str, phone: &str, email: &str) -> String {
    let mut out = String::from("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:");
    push_escaped_text(&mut out, name);
    out.push_str("\r\nTEL:");
    push_escaped_text(&mut out, phone);
    out.push_str("\r\nEMAIL:");
    push_escaped_text(&mut out, email);
    out.push_str("\r\nEND:VCARD\r\n");
    out
}

/// Splits the payload into unfolded logical lines, normalizing `\r\n` and `\n`.
/// A line beginning with a space or tab is a continuation of the previous line
/// (vCard line folding) and is appended to it.
fn unfold(s: &str) -> impl Iterator<Item = Cow<'_, str>> {
    let mut lines = s.split('\n').peekable();
    core::iter::from_fn(move || {
        let raw = lines.next()?;
        let mut line: Cow<'_, str> = Cow::Borrowed(raw.strip_suffix('\r').unwrap_or(raw));
        while let Some(next) = lines.peek() {
            let next = next.strip_suffix('\r').unwrap_or(next);
            if !next.starts_with(' ') && !next.starts_with('\t') {
                break;
            }
            // Allocate only when a continuation must be appended. The fold
            // character is ASCII, so byte index 1 is a valid boundary.
            line.to_mut().push_str(&next[1..]);
            let _ = lines.next();
        }
        Some(line)
    })
}

/// Joins the non-empty components of a structured `N` value
/// (`Family;Given;Additional;Prefix;Suffix`) with single spaces.
fn parse_n(value: &str) -> String {
    join_structured_text(value, " ")
}

/// Joins a multi-component value (`ORG` can be `Company;Unit`) with `; `.
fn parse_semicolons(value: &str) -> String {
    join_structured_text(value, "; ")
}

/// Joins components without treating an escaped semicolon as a separator.
fn join_structured_text(value: &str, separator: &str) -> String {
    let mut escaped = false;
    let parts = value.split(move |c| {
        if escaped {
            escaped = false;
            false
        } else if c == '\\' {
            escaped = true;
            false
        } else {
            c == ';'
        }
    });
    let mut out = String::with_capacity(value.len());
    let mut first = true;
    for part in parts.filter(|part| !part.is_empty()) {
        if !first {
            out.push_str(separator);
        }
        push_unescaped_text(&mut out, part);
        first = false;
    }
    out
}

fn unescape_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    push_unescaped_text(&mut out, value);
    out
}

/// Decodes vCard TEXT escapes while preserving malformed/unknown escapes.
fn push_unescaped_text(out: &mut String, value: &str) {
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(c @ ('\\' | ',' | ';')) => out.push(c),
            Some(c) => {
                out.push('\\');
                out.push(c);
            }
            None => out.push('\\'),
        }
    }
}

fn push_escaped_text(out: &mut String, value: &str) {
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' | ',' | ';' => {
                out.push('\\');
                out.push(c);
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    let _ = chars.next();
                }
                out.push_str("\\n");
            }
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
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

    #[test]
    fn text_values_round_trip_unicode_delimiters_and_literal_escapes() {
        let name = "王,🦀; Doe\\nJunior\nSecond line";
        let phone = "+123;456,789\\ext";
        let email = "first,last\\name@example.invalid";
        let payload = encode_vcard(name, phone, email);
        let card = VCard::parse(&payload).unwrap();
        assert_eq!(card.name(), Some(name));
        assert_eq!(card.phone(), Some(phone));
        assert_eq!(card.email(), Some(email));
        assert!(payload.contains("FN:王\\,🦀\\; Doe\\\\nJunior\\nSecond line\r\n"));
    }

    #[test]
    fn newlines_cannot_inject_contact_properties() {
        let name = "Alice\nEMAIL:other@example.invalid";
        let payload = encode_vcard(name, "123", "real@example.invalid");
        let card = VCard::parse(&payload).unwrap();
        assert_eq!(card.name(), Some(name));
        assert_eq!(card.email(), Some("real@example.invalid"));
        assert_eq!(payload.matches("\r\nEMAIL:").count(), 1);
    }

    #[test]
    fn encoded_crlf_and_bare_cr_are_normalized_to_text_newlines() {
        let payload = encode_vcard("First\r\nSecond\rThird\nFourth", "123", "a@example.invalid");
        assert_eq!(VCard::parse(&payload).unwrap().name(), Some("First\nSecond\nThird\nFourth"));
    }

    #[test]
    fn first_record_ignores_properties_before_and_after_its_boundaries() {
        let card = VCard::parse(
            "FN:outside\nEMAIL:before@example.invalid\nBEGIN:VCARD\nFN:inside\nEND:VCARD\n\
             EMAIL:after@example.invalid\nBEGIN:VCARD\nTEL:second card\nEND:VCARD\n",
        )
        .unwrap();
        assert_eq!(card.name(), Some("inside"));
        assert_eq!(card.email(), None);
        assert_eq!(card.phone(), None);
    }

    #[test]
    fn incomplete_record_keeps_case_insensitive_first_property_semantics() {
        let card = VCard::parse(
            "begin:vcard\nN:Fallback;Name;;;\nfn;LANGUAGE=en:First\nFN:Second\n\
             tel;TYPE=cell:123\nTEL:456\nemail:first@example.invalid\nEMAIL:second@example.invalid\n",
        )
        .unwrap();
        assert_eq!(card.name(), Some("First"));
        assert_eq!(card.phone(), Some("123"));
        assert_eq!(card.email(), Some("first@example.invalid"));
    }

    #[test]
    fn structured_text_splits_only_unescaped_semicolons() {
        let card = VCard::parse(concat!(
            "BEGIN:VCARD\n",
            "N:Doe\\;Sr;John\\, Jr;;;\n",
            "ORG:Acme\\; Labs;Widgets\\, Inc.\n",
            "URL:https://example.invalid/literal\\n\n",
            "ADR:;;Street\\;Lane;Town;;123;Country\n",
            "END:VCARD\n",
        ))
        .unwrap();
        assert_eq!(card.name(), Some("Doe;Sr John, Jr"));
        assert_eq!(card.organization(), Some("Acme; Labs; Widgets, Inc."));
        assert_eq!(card.url(), Some("https://example.invalid/literal\\n"));
        assert_eq!(card.address(), Some(";;Street\\;Lane;Town;;123;Country"));
    }

    #[test]
    fn text_unescaping_preserves_unknown_and_trailing_backslashes() {
        let card = VCard::parse("BEGIN:VCARD\nFN:Unknown\\q and trailing\\\nEND:VCARD\n").unwrap();
        assert_eq!(card.name(), Some("Unknown\\q and trailing\\"));
        assert_eq!(unescape_text("upper\\Nlower\\n"), "upper\nlower\n");
    }

    #[test]
    fn unfolding_borrows_plain_lines_and_allocates_only_folded_lines() {
        let mut lines = unfold("BEGIN:VCARD\r\nFN:中文\r\n 🦀\r\n\t测试\r\nEND:VCARD\r\n");
        assert!(matches!(lines.next(), Some(Cow::Borrowed("BEGIN:VCARD"))));
        assert_eq!(lines.next(), Some(Cow::Owned(String::from("FN:中文🦀测试"))));
        assert!(matches!(lines.next(), Some(Cow::Borrowed("END:VCARD"))));
        assert!(matches!(lines.next(), Some(Cow::Borrowed(""))));
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn folded_text_is_unescaped_after_joining_continuations() {
        let card = VCard::parse("BEGIN:VCARD\r\nFN:王\\\r\n n🦀\\,\r\n\t测试\r\nEND:VCARD\r\n").unwrap();
        assert_eq!(card.name(), Some("王\n🦀,测试"));
    }

    #[test]
    fn doubled_backslash_before_semicolon_keeps_the_component_separator() {
        let card = VCard::parse(concat!(
            "BEGIN:VCARD\n",
            r"N:Family\\;Given;;;",
            "\n",
            r"ORG:Company\\;Unit",
            "\nEND:VCARD\n",
        ))
        .unwrap();
        assert_eq!(card.name(), Some(r"Family\ Given"));
        assert_eq!(card.organization(), Some(r"Company\; Unit"));
    }

    #[test]
    fn triple_backslash_before_semicolon_keeps_it_inside_the_component() {
        let card = VCard::parse(concat!(
            "BEGIN:VCARD\n",
            r"N:Family\\\;Given;;;",
            "\n",
            r"ORG:Company\\\;Unit",
            "\nEND:VCARD\n",
        ))
        .unwrap();
        assert_eq!(card.name(), Some(r"Family\;Given"));
        assert_eq!(card.organization(), Some(r"Company\;Unit"));
    }
}
