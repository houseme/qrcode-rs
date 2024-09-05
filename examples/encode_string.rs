use qrcode_rs::QrCode;

fn main() {
    let code = QrCode::new(b"Hello").unwrap();
    let string = code.render::<char>().dark_color('#').quiet_zone(false).module_dimensions(2, 1).build();
    println!("{}", string);
}
