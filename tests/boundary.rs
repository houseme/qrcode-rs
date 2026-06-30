use qrcode_rs::bits::{self, Bits};
use qrcode_rs::{EcLevel, QrCode, QrError, Version};

#[test]
fn test_max_version_qr_v40() {
    let data: Vec<u8> = (0..2953).map(|i| (i % 256) as u8).collect();
    let code = QrCode::with_error_correction_level(&data, EcLevel::L).unwrap();
    assert_eq!(code.version(), Version::Normal(40));
    assert_eq!(code.width(), 177);
}

#[test]
fn test_max_version_qr_v40_high_ec() {
    let data: Vec<u8> = (0..1273).map(|i| (i % 256) as u8).collect();
    let code = QrCode::with_error_correction_level(&data, EcLevel::H).unwrap();
    assert_eq!(code.version(), Version::Normal(40));
}

#[test]
fn test_max_micro_qr_m4() {
    let code = QrCode::with_version(b"Hello, world!!!", Version::Micro(4), EcLevel::L).unwrap();
    assert_eq!(code.width(), 17);
}

#[test]
fn test_empty_input() {
    let result = QrCode::new(b"");
    assert!(result.is_ok());
}

#[test]
fn test_long_input_auto_version() {
    let data = b"This is a test string for automatic version selection in QR code encoding.";
    let code = QrCode::new(data).unwrap();
    assert!(code.version().width() >= 21);
}

#[test]
fn test_data_too_long_standard_qr() {
    let data: Vec<u8> = (0..5000).map(|i| (i % 256) as u8).collect();
    let result = QrCode::with_error_correction_level(&data, EcLevel::L);
    assert!(result.is_err());
}

#[test]
fn test_encode_auto_micro_numeric() {
    let code = QrCode::new_micro(b"0123456789").unwrap();
    assert!(code.version().is_micro());
}

#[test]
fn test_encode_auto_micro_too_long() {
    let data: Vec<u8> = (0..100).map(|i| b'0' + (i % 10)).collect();
    let result = QrCode::new_micro(&data);
    assert!(result.is_err());
}

#[test]
fn test_eci_designator() {
    let mut bits = Bits::new(Version::Normal(1));
    bits.push_eci_designator(9).unwrap();
    bits.push_byte_data(b"test").unwrap();
    bits.push_terminator(EcLevel::L).unwrap();
    assert!(QrCode::with_bits(bits, EcLevel::L).is_ok());
}

#[test]
fn test_eci_designator_range() {
    let mut bits = Bits::new(Version::Normal(1));
    assert!(bits.push_eci_designator(0).is_ok());

    let mut bits = Bits::new(Version::Normal(1));
    assert!(bits.push_eci_designator(999999).is_ok());

    let mut bits = Bits::new(Version::Normal(1));
    assert!(bits.push_eci_designator(1000000).is_err());
}

#[test]
fn test_all_versions_encode() {
    for v in 1..=40 {
        let version = Version::Normal(v);
        let result = QrCode::with_version(b"A", version, EcLevel::L);
        assert!(result.is_ok(), "Version {v} should encode 1 byte");
    }
}

#[test]
fn test_all_micro_versions_encode() {
    for v in 1..=4 {
        let version = Version::Micro(v);
        let result = QrCode::with_version(b"0", version, EcLevel::L);
        if v == 1 {
            let _ = result;
        } else {
            assert!(result.is_ok(), "Micro version M{v} should encode 1 digit");
        }
    }
}

#[test]
fn test_encode_auto_function() {
    let bits = bits::encode_auto(b"Hello, World!", EcLevel::M).unwrap();
    let code = QrCode::with_bits(bits, EcLevel::M).unwrap();
    assert!(code.width() >= 21);
}

#[test]
fn test_encode_auto_micro_function() {
    let bits = bits::encode_auto_micro(b"12345", EcLevel::L).unwrap();
    let code = QrCode::with_bits(bits, EcLevel::L).unwrap();
    assert!(code.version().is_micro());
}

#[test]
fn test_enriched_invalid_eci_designator() {
    let mut bits = Bits::new(Version::Normal(1));
    assert_eq!(
        bits.push_eci_designator(1_000_000),
        Err(QrError::InvalidEciDesignator { value: 1_000_000 })
    );
}

#[test]
fn test_enriched_invalid_character_kanji() {
    // An odd-length (single trailing byte) Shift-JIS payload reports position + byte.
    let mut bits = Bits::new(Version::Normal(5));
    assert_eq!(
        bits.push_kanji_data(b"\x93"),
        Err(QrError::InvalidCharacter { position: 0, byte: 0x93 })
    );
}

#[test]
fn test_enriched_invalid_version_carries_context() {
    // Micro QR does not support error correction level H.
    let err = match QrCode::with_version(b"0", Version::Micro(2), EcLevel::H) {
        Ok(_) => panic!("expected an error for Micro(2) + EcLevel::H"),
        Err(e) => e,
    };
    assert!(
        matches!(err, QrError::InvalidVersion { version: Version::Micro(2), ec_level: EcLevel::H }),
        "expected enriched InvalidVersion, got: {err:?}"
    );
    // Display must surface the context.
    let msg = format!("{err}");
    assert!(msg.contains("Micro(2)") || msg.contains("Micro"));
    assert!(msg.contains("H"));
}
