//! Encode an FNC1 (GS1 / AIM) data carrier.
//!
//! FNC1 in first position marks a GS1 barcode. Here we emit first-position
//! FNC1 followed by a numeric payload.
//!
//! Run: `cargo run --example encode_fnc1`

use qrcode_rs::bits::Bits;
use qrcode_rs::{EcLevel, QrCode, Version};

fn main() {
    let mut bits = Bits::new(Version::Normal(2));
    bits.push_fnc1_first_position().unwrap();
    bits.push_numeric_data(b"0123456789").unwrap();
    bits.push_terminator(EcLevel::M).unwrap();

    let code = QrCode::with_bits(bits, EcLevel::M).unwrap();
    println!("{}", code.render().dark_color('#').light_color(' ').build());
}
