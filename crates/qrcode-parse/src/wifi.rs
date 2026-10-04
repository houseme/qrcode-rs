//! WiFi configuration (`WIFI:`) parsing and encoding.
//!
//! The wire format is `WIFI:T:<auth>;S:<ssid>;P:<password>;;` with optional
//! `;H:true` for hidden networks, though the parser accepts the fields in any
//! order. The characters `\\ ; , " :` are backslash-escaped inside the SSID and
//! password. The `qrcode-rs` facade uses this same module for its convenience
//! constructor, so [`encode_wifi`] and [`WifiConfig::parse`] stay symmetric.
//! ASCII SSIDs made entirely of hex digits are enclosed in protocol quotes to
//! distinguish their text from a hex-encoded network name. Protocol quotes are
//! removed before unescaping; escaped quotes remain literal value characters.

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::string::String;

use crate::ParseError;

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
        if ["WPA", "WPA2", "WPA3"].iter().any(|auth| value.eq_ignore_ascii_case(auth)) {
            Self::Wpa
        } else if value.eq_ignore_ascii_case("WEP") {
            Self::Wep
        } else {
            Self::None // "nopass", empty, or unknown → open
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
    /// (`S:`) field is mandatory; all others are optional. A pair of unescaped
    /// outer quotes on an SSID or password denotes protocol quoting. Escaped
    /// quotes are part of the value and are preserved.
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
                "S" => ssid = Some(unescape_value(value)),
                "T" => security = WifiSecurity::from_wire(value),
                "P" => {
                    let pw = unescape_value(value);
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
/// cameras.
///
/// This is the single source of truth shared with the `qrcode-rs` facade's
/// `QrCode::for_wifi` constructor.
/// Non-empty ASCII SSIDs consisting only of hex digits are protocol-quoted so
/// readers treat them as text. Passwords retain their existing wire encoding,
/// including raw hex credentials; authentication values are copied as given.
pub fn encode_wifi(ssid: &str, password: &str, auth: &str) -> String {
    let mut payload = String::from("WIFI:T:");
    payload.push_str(auth);
    payload.push_str(";S:");
    let quote_ssid = !ssid.is_empty() && ssid.bytes().all(|byte| byte.is_ascii_hexdigit());
    if quote_ssid {
        payload.push('"');
    }
    push_escaped(&mut payload, ssid);
    if quote_ssid {
        payload.push('"');
    }
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
/// each field. Each raw (still-escaped) field borrows the input.
fn split_fields(rest: &str) -> impl Iterator<Item = &str> {
    let mut escaped = false;
    rest.split(move |c| {
        if escaped {
            escaped = false;
            false
        } else if c == '\\' {
            escaped = true;
            false
        } else {
            c == ';'
        }
    })
}

/// Removes protocol quotes while the escapes still distinguish literal quotes.
fn unescape_value(value: &str) -> String {
    let unquoted = value.strip_prefix('"').and_then(|body| body.strip_suffix('"'));
    let value = match unquoted {
        Some(body) if (body.bytes().rev().take_while(|&byte| byte == b'\\').count() & 1) == 0 => body,
        // A closing quote after an odd number of backslashes is escaped. An
        // even run escapes itself, leaving the closing quote as syntax.
        _ => value,
    };
    unescape(value)
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
    #[cfg(not(feature = "std"))]
    use alloc::format;

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

    #[test]
    fn borrowed_fields_preserve_escaped_delimiters_and_empty_fields() {
        let mut fields = split_fields("S:中文\\;网络;P:pass\\\\;H:true;;");
        assert_eq!(fields.next(), Some("S:中文\\;网络"));
        assert_eq!(fields.next(), Some("P:pass\\\\"));
        assert_eq!(fields.next(), Some("H:true"));
        assert_eq!(fields.next(), Some(""));
        assert_eq!(fields.next(), Some(""));
        assert_eq!(fields.next(), None);
    }

    #[test]
    fn duplicate_fields_keep_the_last_value_and_unknown_fields_are_ignored() {
        let cfg =
            WifiConfig::parse("wifi:S:first;X:忽略\\;字段;S:最后;T:WEP;T:wPa3;P:old;P:new;H:true;H:false;;").unwrap();
        assert_eq!(cfg.ssid(), "最后");
        assert_eq!(cfg.security(), WifiSecurity::Wpa);
        assert_eq!(cfg.password(), Some("new"));
        assert!(!cfg.hidden());
    }

    #[test]
    fn unicode_values_and_trailing_escape_keep_tolerant_behavior() {
        let payload = encode_wifi("网络;🦀\\", "密碼,\"测试", "WPA2");
        let cfg = WifiConfig::parse(&payload).unwrap();
        assert_eq!(cfg.ssid(), "网络;🦀\\");
        assert_eq!(cfg.password(), Some("密碼,\"测试"));
        assert_eq!(cfg.security(), WifiSecurity::Wpa);
        assert_eq!(WifiConfig::parse("WIFI:S:trailing\\").unwrap().ssid(), "trailing");
        assert_eq!(WifiConfig::parse("WIFI:S:open;T:unknown;;").unwrap().security(), WifiSecurity::None);
    }

    #[test]
    fn hex_text_ssids_use_protocol_quotes_without_changing_other_names() {
        for ssid in ["A", "a", "ABCD", "012345", "00ff", "aBcDeF"] {
            let expected = format!("WIFI:T:WPA;S:\"{ssid}\";P:password;;");
            let payload = encode_wifi(ssid, "password", "WPA");
            assert_eq!(payload, expected);
            assert_eq!(WifiConfig::parse(&payload).unwrap().ssid(), ssid);
        }
        for ssid in ["", "0xABCD", "network", "１２３４", "ábc", "A B", "abc-def"] {
            assert_eq!(encode_wifi(ssid, "password", "WPA"), format!("WIFI:T:WPA;S:{ssid};P:password;;"));
        }
    }

    #[test]
    fn protocol_and_escaped_literal_quotes_are_distinct_before_unescaping() {
        let protocol = WifiConfig::parse(r#"WIFI:T:WPA;S:"ABCD";P:"1234ABCD";;"#).unwrap();
        assert_eq!(protocol.ssid(), "ABCD");
        assert_eq!(protocol.password(), Some("1234ABCD"));

        let literal_wire = r#"WIFI:T:WPA;S:\"ABCD\";P:\"1234ABCD\";;"#;
        let literal = WifiConfig::parse(literal_wire).unwrap();
        assert_eq!(literal.ssid(), "\"ABCD\"");
        assert_eq!(literal.password(), Some("\"1234ABCD\""));
        assert_eq!(encode_wifi("\"ABCD\"", "\"1234ABCD\"", "WPA"), literal_wire);

        let nested = WifiConfig::parse(r#"WIFI:T:WPA;S:"\"ABCD\"";P:"\"1234ABCD\"";;"#).unwrap();
        assert_eq!(nested.ssid(), literal.ssid());
        assert_eq!(nested.password(), literal.password());

        let empty = WifiConfig::parse(r#"WIFI:S:"";P:"";;"#).unwrap();
        assert_eq!(empty.ssid(), "");
        assert_eq!(empty.password(), None);
    }

    #[test]
    fn closing_protocol_quotes_require_an_even_run_of_backslashes() {
        for slashes in 0..=7 {
            let raw = format!("\"name{}\"", "\\".repeat(slashes));
            let expected = if slashes % 2 == 0 {
                format!("name{}", "\\".repeat(slashes / 2))
            } else {
                format!("\"name{}\"", "\\".repeat(slashes / 2))
            };
            let payload = format!("WIFI:S:{raw};P:{raw};;");
            let parsed = WifiConfig::parse(&payload).unwrap();
            assert_eq!(parsed.ssid(), expected);
            assert_eq!(parsed.password(), Some(expected.as_str()));
        }
        assert_eq!(unescape_value("\""), "\"");
        assert_eq!(unescape_value("\"unclosed"), "\"unclosed");
        assert_eq!(unescape_value("trailing\\"), "trailing");
    }

    #[test]
    fn password_hex_keys_and_auth_values_keep_the_existing_wire_contract() {
        for auth in ["WPA", "wPa2", "WPA3", "WEP", "nopass", "unknown"] {
            for length in [10, 26, 58, 64] {
                let password = "a".repeat(length);
                let payload = encode_wifi("network", &password, auth);
                assert_eq!(payload, format!("WIFI:T:{auth};S:network;P:{password};;"));
                assert_eq!(WifiConfig::parse(&payload).unwrap().password(), Some(password.as_str()));
            }
        }
    }

    #[test]
    fn wifi_boundary_golden_cases_round_trip_for_all_existing_auth_families() {
        // The wire values are independent goldens: the SSID may need protocol
        // quotes, while the password continues to use literal escaping only.
        let values = [
            ("", "", ""),
            ("ABCD", r#""ABCD""#, "ABCD"),
            ("\"", r#"\""#, r#"\""#),
            ("\"quoted\"", r#"\"quoted\""#, r#"\"quoted\""#),
            ("\\", r#"\\"#, r#"\\"#),
            ("tail\\", r#"tail\\"#, r#"tail\\"#),
            ("\\;:,\"", r#"\\\;\:\,\""#, r#"\\\;\:\,\""#),
            ("\\\\;:,\"", r#"\\\\\;\:\,\""#, r#"\\\\\;\:\,\""#),
            ("网络🦀", "网络🦀", "网络🦀"),
        ];
        let auths = [
            ("WPA", WifiSecurity::Wpa),
            ("wPa2", WifiSecurity::Wpa),
            ("Wpa3", WifiSecurity::Wpa),
            ("WEP", WifiSecurity::Wep),
            ("wep", WifiSecurity::Wep),
            ("nopass", WifiSecurity::None),
            ("NOPASS", WifiSecurity::None),
            ("", WifiSecurity::None),
            ("unknown", WifiSecurity::None),
        ];
        let mut cases = 0;
        for (ssid, ssid_wire, _) in values {
            for (password, _, password_wire) in values {
                for (auth, security) in auths {
                    let payload = encode_wifi(ssid, password, auth);
                    assert_eq!(payload, format!("WIFI:T:{auth};S:{ssid_wire};P:{password_wire};;"));
                    let parsed = WifiConfig::parse(&payload).unwrap();
                    assert_eq!(parsed.ssid(), ssid);
                    assert_eq!(parsed.password(), (!password.is_empty()).then_some(password));
                    assert_eq!(parsed.security(), security);
                    assert!(!parsed.hidden());
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 729);
    }
}
