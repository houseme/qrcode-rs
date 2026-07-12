//! Encode a QR code, render it to an image, then decode it back with `rqrr`.
//!
//! Requires the `decode-rqrr` feature.
//!
//! Run: `cargo run --example decode_roundtrip --features decode-rqrr`

use image::Luma;
use qrcode_rs::QrCode;
use qrcode_rs::decode::rqrr::RqrrDecoder;
use qrcode_rs::decode::{GrayPixels, QrDecoder};

fn main() {
    let payload = b"https://example.com/decode-bridge";
    let code = QrCode::new(payload).unwrap();
    let image: image::GrayImage = code.render::<Luma<u8>>().min_dimensions(200, 200).build();

    let decoded = RqrrDecoder::new().decode(GrayPixels::from(&image)).expect("decode should succeed");

    println!("decoded {} code(s)", decoded.len());
    println!("payload: {}", std::str::from_utf8(decoded[0].data()).unwrap());
}
