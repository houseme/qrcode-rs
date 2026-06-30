//! Generate accessible alt text describing a QR code's payload — suitable for
//! an `<img alt="…">` or an inline SVG's `aria-label`.
//!
//! Run: `cargo run --example alt_text`

use qrcode_rs::QrCode;

fn main() {
    let payload = "https://example.com";
    let _code = QrCode::new(payload.as_bytes()).unwrap();
    println!("{}", QrCode::alt_text(payload));
}
