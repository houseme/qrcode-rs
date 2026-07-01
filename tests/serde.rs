//! Serde round-trip tests (only run with the `serde` feature).

#![cfg(feature = "serde")]

use qrcode_rs::{Color, EcLevel, Mode, QrCode, QrCodeData, Version};

#[test]
fn qr_code_round_trip() {
    let code = QrCode::with_version(b"round-trip test", Version::Normal(5), EcLevel::M).unwrap();
    let json = serde_json::to_string(&code.to_serializable()).unwrap();
    let decoded: QrCodeData = serde_json::from_str(&json).unwrap();
    let rebuilt = QrCode::from_serializable(decoded);
    assert_eq!(code.colors(), rebuilt.colors());
    assert_eq!(code.version(), rebuilt.version());
    assert_eq!(code.error_correction_level(), rebuilt.error_correction_level());
}

#[test]
fn core_enums_serialize() {
    // Default serde enum representation.
    assert_eq!(serde_json::to_string(&Color::Dark).unwrap(), "\"Dark\"");
    assert_eq!(serde_json::to_string(&EcLevel::H).unwrap(), "\"H\"");
    assert_eq!(serde_json::to_string(&Version::Normal(5)).unwrap(), "{\"Normal\":5}");
    assert_eq!(serde_json::to_string(&Version::Micro(2)).unwrap(), "{\"Micro\":2}");
    assert_eq!(serde_json::to_string(&Mode::Byte).unwrap(), "\"Byte\"");
}
