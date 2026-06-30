//! Render a QR code to a PNG image with custom dark/light colors.
//!
//! Requires the `image` feature (on by default).
//!
//! Run: `cargo run --example custom_colors`

use image::Rgb;
use qrcode_rs::QrCode;

fn main() {
    let code = QrCode::new(b"https://example.com").unwrap();
    let image = code
        .render::<Rgb<u8>>()
        .min_dimensions(300, 300)
        .dark_color(Rgb([26, 26, 46])) // #1a1a2e
        .light_color(Rgb([224, 224, 224])) // #e0e0e0
        .quiet_zone(true)
        .build();

    image.save("/tmp/qrcode-custom.png").expect("failed to save image");
    println!("wrote /tmp/qrcode-custom.png ({}x{})", image.width(), image.height());
}
