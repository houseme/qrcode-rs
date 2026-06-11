use qrcode_rs::QrCode;
use qrcode_rs::render::html::Color;

fn main() {
    let code = QrCode::new(b"https://github.com/houseme/qrcode-rs").unwrap();
    let html = code.render::<Color>().build();
    println!("{html}");
}
