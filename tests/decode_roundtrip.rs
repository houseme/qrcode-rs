//! Encode → render → rqrr decode roundtrip tests (only run with `decode-rqrr`).

#![cfg(feature = "decode-rqrr")]

use image::Luma;
use qrcode_rs::QrCode;
use qrcode_rs::decode::rqrr::RqrrDecoder;
use qrcode_rs::decode::{GrayPixels, QrDecoder};

fn decode_roundtrip(original: &[u8]) {
    let code = QrCode::new(original).unwrap();
    let image: image::GrayImage = code.render::<Luma<u8>>().min_dimensions(200, 200).build();
    let decoded = RqrrDecoder::new().decode(GrayPixels::from(&image)).expect("rqrr should decode the rendered QR");
    assert!(!decoded.is_empty(), "at least one code should be found");
    assert_eq!(decoded[0].data(), original, "payload should round-trip exactly");
    assert!(!decoded[0].version().is_micro(), "should be a normal QR version");
    assert_eq!(decoded[0].ec_level(), code.info().ec_level(), "ec level should round-trip");
}

#[test]
fn roundtrip_short_url() {
    decode_roundtrip(b"https://example.com");
}

#[test]
fn roundtrip_longer_payload() {
    decode_roundtrip(b"qrcode-rs decode bridge roundtrip test 0123456789 !@#$%");
}

#[test]
fn roundtrip_numeric() {
    decode_roundtrip(b"01234567890123456789");
}
