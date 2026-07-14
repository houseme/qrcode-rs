use qrcode_rs::{ConstVersion, EcLevel, QrCode, Version};

fn main() {
    const VERSION: Version = ConstVersion::<5>::VALUE;

    let code = QrCode::with_const_version::<5, _>(b"https://example.com/fixed-version", EcLevel::M).unwrap();

    println!("version: {:?}", VERSION);
    println!("width: {}", code.width());
    println!("dark modules: {}", code.dark_modules().count());
}
