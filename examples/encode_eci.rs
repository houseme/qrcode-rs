//! Encode byte data under an ECI (Extended Channel Interpretation) designator.
//!
//! ECI designator 9 selects ISO-8859-5 (Latin/Cyrillic); compliant scanners
//! will then interpret the following bytes in that charset.
//!
//! Run: `cargo run --example encode_eci`

use qrcode_rs::bits::Bits;
use qrcode_rs::{EcLevel, QrCode, Version};

fn main() {
    let mut bits = Bits::new(Version::Normal(1));
    bits.push_eci_designator(9).unwrap(); // ISO-8859-5
    bits.push_byte_data(b"\xca\xfe\xe4\xe9\xea\xe1\xf2 QR").unwrap();
    bits.push_terminator(EcLevel::L).unwrap();

    let code = QrCode::with_bits(bits, EcLevel::L).unwrap();
    println!("{}", code.render().dark_color('#').light_color(' ').build());
}
