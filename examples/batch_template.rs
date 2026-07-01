//! Batch-generate several QR codes, then render one with a style template.
//!
//! Requires the `image` feature (on by default).
//!
//! Run: `cargo run --example batch_template`

use image::Rgba;
use qrcode_rs::{EcLevel, QrCode, QrTemplate};

fn main() {
    let urls = ["https://a.com", "https://b.com", "https://c.com"];
    let codes = QrCode::batch(urls, EcLevel::M).unwrap();
    println!("encoded {} codes", codes.len());

    // Render the first code with the dark_mode template.
    let img = codes[0].render::<Rgba<u8>>().template(&QrTemplate::dark_mode()).min_dimensions(200, 200).build();
    img.save("/tmp/qrcode-batch-dark.png").expect("save");
    println!("wrote /tmp/qrcode-batch-dark.png ({}x{})", img.width(), img.height());
}
