//! Render an accessible SVG QR code with ARIA attributes (`role="img"` and an
//! auto-generated `aria-label`) for screen readers.
//!
//! Requires the `svg` feature (on by default).
//!
//! Run: `cargo run --example accessible_svg`

use qrcode_rs::QrCode;
use qrcode_rs::render::svg;

fn main() {
    let payload = "https://example.com";
    let code = QrCode::new(payload.as_bytes()).unwrap();
    let image = code.render::<svg::Color>().build();
    let image = svg::aria_label(&image, &QrCode::alt_text(payload));
    println!("{image}");
}
