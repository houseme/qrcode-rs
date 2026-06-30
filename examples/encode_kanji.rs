//! Encode Shift-JIS Kanji data into a QR code.
//!
//! Kanji mode packs double-byte Shift-JIS characters at 13 bits each. We push
//! raw bytes via the `Bits` API and render the result as a terminal string.
//!
//! Run: `cargo run --example encode_kanji`

use qrcode_rs::bits::Bits;
use qrcode_rs::{EcLevel, QrCode, Version};

fn main() {
    // Two valid Shift-JIS kanji double-byte pairs: 0x935F, 0xE4AA.
    let kanji_bytes: &[u8] = &[0x93, 0x5f, 0xe4, 0xaa];

    let mut bits = Bits::new(Version::Normal(3));
    bits.push_kanji_data(kanji_bytes).unwrap();
    bits.push_terminator(EcLevel::M).unwrap();

    let code = QrCode::with_bits(bits, EcLevel::M).unwrap();
    println!("{}", code.render().dark_color('#').light_color(' ').build());
}
