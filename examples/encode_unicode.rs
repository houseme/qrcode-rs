use qrcode_rs::QrCode;
use qrcode_rs::render::unicode;

fn main() {
    let code = QrCode::new(b"Hello").unwrap();
    let string = code.render::<unicode::Dense1x2>().quiet_zone(false).build();
    println!("{}", string);
}
