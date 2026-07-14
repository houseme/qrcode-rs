//! Structured Append symbol validity via the bundled `rqrr` decoder.
//!
//! `rqrr` cannot decode Structured Append symbols — it reaches the `0011` mode
//! indicator and returns `UnknownDataType`. But getting that far means it
//! successfully read the format info, detected the grid, and ECC-corrected the
//! data stream: i.e. the rendered symbol is well-formed and externally
//! recognized as Structured Append. (The pure-core parser
//! [`parse_sa_datastream`](qrcode_rs::decode::sa_parse::parse_sa_datastream)
//! recovers the payload from the byte stream a full decoder would expose.)

#![cfg(feature = "decode-rqrr")]

use image::Luma;
use qrcode_rs::EcLevel;
use qrcode_rs::decode::rqrr::RqrrDecoder;
use qrcode_rs::decode::{GrayPixels, QrDecoder};
use qrcode_rs::structured_append::StructuredAppend;

#[test]
fn rqrr_recognizes_structured_append_symbols() {
    let codes = StructuredAppend::new(3, b"split across three symbols").unwrap().encode(EcLevel::M).unwrap();
    assert_eq!(codes.len(), 3);
    for code in &codes {
        let img: image::GrayImage = code.render::<Luma<u8>>().min_dimensions(200, 200).build();
        // rqrr reaches mode `0011` and returns UnknownDataType (not a format /
        // grid / ECC error) — proving the symbol is valid Structured Append.
        let result = RqrrDecoder::new().decode(GrayPixels::from(&img));
        assert!(matches!(result, Err(rqrr::DeQRError::UnknownDataType)), "expected UnknownDataType, got {result:?}");
    }
}
