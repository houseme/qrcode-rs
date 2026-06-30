//! Structured append — splitting a payload across multiple QR symbols.
//!
//! NOTE: this crate currently provides only the structured-append *mode
//! indicator* (`ExtendedMode::StructuredAppend`). Full multi-symbol sequence
//! splitting (position / parity / distributing data across N symbols) is not
//! implemented, so this example emits a single symbol carrying the mode
//! indicator for illustration only — it is not a valid structured-append
//! sequence. See the capability matrix (structured append: partial support).
//!
//! Run: `cargo run --example structured_append`

use qrcode_rs::bits::{Bits, ExtendedMode};
use qrcode_rs::{EcLevel, QrCode, Version};

fn main() {
    let mut bits = Bits::new(Version::Normal(2));
    bits.push_mode_indicator(ExtendedMode::StructuredAppend).unwrap();
    bits.push_byte_data(b"part of a sequence").unwrap();
    bits.push_terminator(EcLevel::M).unwrap();

    let code = QrCode::with_bits(bits, EcLevel::M).unwrap();
    println!("(single-symbol illustration of the structured-append mode indicator)");
    println!("{}", code.render().dark_color('#').light_color(' ').build());
}
