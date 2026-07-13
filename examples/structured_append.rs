//! Structured Append — splitting one payload across multiple QR symbols.
//!
//! Encodes a single message as a 3-symbol Structured Append sequence
//! (ISO/IEC 18004 §7.4): every symbol carries the 20-bit sequence header and
//! the shared parity byte, so a spec-aware scanner can reassemble them in order.
//!
//! Note: this crate's bundled decoder (`rqrr`, behind `decode-rqrr`) does not
//! parse Structured Append symbols — it returns `UnknownDataType` for the
//! `0011` mode. Read these symbols with an SA-aware scanner, or decode the bit
//! stream yourself and feed [`reassemble`](qrcode_rs::structured_append::reassemble).
//!
//! Run: `cargo run --example structured_append`

use qrcode_rs::{EcLevel, QrCode};

fn main() {
    let payload = b"Split across multiple QR symbols for resilience!";
    let symbols = 3;

    let codes = QrCode::structured_append(payload, symbols, EcLevel::M).expect("payload fits in 3 symbols at level M");

    println!("Structured Append: {symbols} symbols for {} bytes\n", payload.len());
    for (i, code) in codes.iter().enumerate() {
        println!(
            "── symbol {}/{} (version {:?}, {}×{} modules) ──",
            i + 1,
            symbols,
            code.version(),
            code.width(),
            code.width()
        );
        println!("{}", code.render().dark_color('#').light_color(' ').build());
    }
}
