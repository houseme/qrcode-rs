//! Structured Append — splitting one payload across multiple QR symbols.
//!
//! Encodes a single message as a 3-symbol Structured Append sequence
//! (ISO/IEC 18004 §7.4): every symbol carries the 20-bit sequence header and
//! the shared parity byte, so a spec-aware scanner can reassemble them in order.
//!
//! Note: this crate's bundled decoder (`rqrr`, behind `decode-rqrr`) reads these
//! symbols as valid but returns `UnknownDataType` for the `0011` mode (it cannot
//! decode Structured Append). To recover the data, use an SA-aware decoder — or
//! feed its recovered bit stream to
//! [`parse_sa_datastream`](qrcode_rs::decode::sa_parse::parse_sa_datastream) —
//! then [`reassemble`](qrcode_rs::structured_append::reassemble) the symbols.
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
