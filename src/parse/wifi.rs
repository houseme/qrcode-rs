//! WiFi configuration (`WIFI:`) parsing and encoding.
//!
//! The wire format is `WIFI:T:<auth>;S:<ssid>;P:<password>;;` with optional
//! `;H:true` for hidden networks, though the parser accepts the fields in any
//! order. The characters `\\ ; , " :` are backslash-escaped inside the SSID and
//! password. This module owns the format so [`QrCode::for_wifi`](crate::QrCode::for_wifi)
//! and [`WifiConfig::parse`] stay symmetric.

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::{string::String, vec, vec::Vec};

use crate::parse::ParseError;

/// WiFi authentication mode for a [`WifiConfig`].
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WifiSecurity {
    /// WPA / WPA2 / WPA3 (all encoded as `T:WPA`).
    Wpa,
    /// WEP (`T:WEP`).
    Wep,
    /// Open / no passphrase (`T:nopass`).
    None,
}

impl WifiSecurity {
    /// The `T:` wire value for this security mode.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Wpa => "WPA",
            Self::Wep => "WEP",
            Self::None => "nopass",
        }
    }

    fn from_wire(value: &str) -> Self {
        match value.to_ascii_uppercase().as_str() {
            "WPA" | "WPA2" | "WPA3" => Self::Wpa,
            "WEP" => Self::Wep,
            _ => Self::None, // "nopass", empty, or unknown → open
        }
    }
}

/// A parsed WiFi configuration recovered from a `WIFI:` QR payload.
///
/// `#[non_exhaustive]`: fields may grow in 1.x without a breaking change;
/// construct via [`WifiConfig::parse`] and read via the accessors.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiConfig {
    /// The network name (SSID), unescaped.
    ssid: String,
    /// The passphrase, if any (`P:` field present and non-empty).
    password: Option<String>,
    /// The authentication mode (`T:` field).
    security: WifiSecurity,
    /// Whether the network is hidden (`H:true`).
    hidden: bool,
}

impl WifiConfig {
    /// Parses a `WIFI:` QR payload into a [`WifiConfig`].
    ///
    /// Fields may appear in any order; the `WIFI:` prefix is required. The SSID
    /// (`S:`) field is mandatory; all others are optional.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::InvalidFormat`] if the `WIFI:` prefix is missing,
    /// or [`ParseError::MissingField`] if the SSID field is absent.
    pub fn parse(s: &str) -> Result<Self, ParseError> {
        let rest = strip_wifi_prefix(s).ok_or(ParseError::InvalidFormat)?;

        let mut ssid: Option<String> = None;
        let mut password = None;
        let mut security = WifiSecurity::None;
        let mut hidden = false;

        for field in split_fields(rest) {
            if field.is_empty() {
                continue;
            }
            let Some((key, value)) = field.split_once(':') else {
                continue; // skip a malformed keyless field
            };
            match key {
                "S" => ssid = Some(unescape(value)),
                "T" => security = WifiSecurity::from_wire(value),
                "P" => {
                    let pw = unescape(value);
                    password = if pw.is_empty() { None } else { Some(pw) };
                }
                "H" => hidden = value.eq_ignore_ascii_case("true"),
                _ => {}
            }
        }

        let ssid = ssid.ok_or(ParseError::MissingField("S (ssid)"))?;
        Ok(Self { ssid, password, security, hidden })
    }

    /// The network name (SSID).
    #[must_use]
    pub fn ssid(&self) -> &str {
        &self.ssid
    }

    /// The passphrase, if a non-empty `P:` field was present.
    #[must_use]
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    /// The authentication mode.
    #[must_use]
    pub fn security(&self) -> WifiSecurity {
        self.security
    }

    /// Whether the network is hidden (`H:true`).
    #[must_use]
    pub fn hidden(&self) -> bool {
        self.hidden
    }
}

/// Encodes a WiFi configuration into the `WIFI:` wire format consumed by phone
/// cameras. This is the single source of truth shared with
/// [`QrCode::for_wifi`](crate::QrCode::for_wifi).
pub(crate) fn encode_wifi(ssid: &str, password: &str, auth: &str) -> String {
    let mut payload = String::from("WIFI:T:");
    payload.push_str(auth);
    payload.push_str(";S:");
    push_escaped(&mut payload, ssid);
    payload.push_str(";P:");
    push_escaped(&mut payload, password);
    payload.push_str(";;");
    payload
}

/// Strips a case-insensitive `WIFI:` prefix, returning the remainder.
fn strip_wifi_prefix(s: &str) -> Option<&str> {
    const PREFIX: &[u8] = b"WIFI:";
    let bytes = s.as_bytes();
    if bytes.len() >= PREFIX.len() && bytes[..PREFIX.len()].eq_ignore_ascii_case(PREFIX) {
        // "WIFI:" is ASCII, so byte index 5 is a valid char boundary.
        Some(&s[PREFIX.len()..])
    } else {
        None
    }
}

/// Splits the payload body on unescaped `;`, preserving escape sequences inside
/// each field. Returns the raw (still-escaped) field strings.
fn split_fields(rest: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            cur.push('\\');
            if let Some(next) = chars.next() {
                cur.push(next);
            }
        } else if c == ';' {
            fields.push(core::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    fields.push(cur);
    fields
}

/// Reverses [`push_escaped`]: `\<c>` → `c`, copying other chars verbatim.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Backslash-escapes the characters that are special in a WiFi QR payload.
fn push_escaped(out: &mut String, s: &str) {
    for c in s.chars() {
        if matches!(c, ';' | ',' | '"' | '\\' | ':') {
            out.push('\\');
        }
        out.push(c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_special_chars() {
        // The classic escape example from the encoder doctest.
        let original = "a;b,c\"d\\e:f";
        let payload = encode_wifi(original, "p\\a;ss", "WPA");
        let cfg = WifiConfig::parse(&payload).unwrap();
        assert_eq!(cfg.ssid(), original);
        assert_eq!(cfg.password(), Some("p\\a;ss"));
        assert_eq!(cfg.security(), WifiSecurity::Wpa);
        assert!(!cfg.hidden());
    }

    #[test]
    fn parse_accepts_any_field_order() {
        // The de-facto standard order is S,T,P,H — different from our encoder.
        let s = "WIFI:S:MyNet;T:WEP;P:secret;H:true;;";
        let cfg = WifiConfig::parse(s).unwrap();
        assert_eq!(cfg.ssid(), "MyNet");
        assert_eq!(cfg.password(), Some("secret"));
        assert_eq!(cfg.security(), WifiSecurity::Wep);
        assert!(cfg.hidden());
    }

    #[test]
    fn parse_unescapes_semicolon_in_ssid() {
        let cfg = WifiConfig::parse("WIFI:T:WPA;S:My\\;Net;P:pw;;").unwrap();
        assert_eq!(cfg.ssid(), "My;Net");
    }

    #[test]
    fn parse_nopass_security() {
        let cfg = WifiConfig::parse("WIFI:S:Open;T:nopass;;").unwrap();
        assert_eq!(cfg.security(), WifiSecurity::None);
        assert_eq!(cfg.password(), None);
    }

    #[test]
    fn parse_missing_prefix_errors() {
        assert_eq!(WifiConfig::parse("S:Net;T:WPA;;"), Err(ParseError::InvalidFormat));
    }

    #[test]
    fn parse_missing_ssid_errors() {
        assert_eq!(WifiConfig::parse("WIFI:T:WPA;P:pw;;"), Err(ParseError::MissingField("S (ssid)")));
    }

    #[test]
    fn security_round_trips() {
        for sec in [WifiSecurity::Wpa, WifiSecurity::Wep, WifiSecurity::None] {
            let payload = encode_wifi("net", "", sec.as_str());
            assert_eq!(WifiConfig::parse(&payload).unwrap().security(), sec);
        }
    }

    #[test]
    fn escaped_helper_preserves_behavior() {
        // Guards the moved `push_escaped` (formerly lib.rs `push_escaped_wifi`).
        let mut out = String::new();
        push_escaped(&mut out, "a;b,c\"d\\e:f");
        assert_eq!(out, "a\\;b\\,c\\\"d\\\\e\\:f");
    }
}
