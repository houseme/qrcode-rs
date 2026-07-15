use qrcode_rs::{ConstVersion, ConstVersionEncoder, EcLevel, Encoder, QrCode, Version};

fn main() {
    const VERSION_5: Version = ConstVersion::<5>::VALUE;

    let direct =
        QrCode::with_const_version::<5, _>(b"https://example.com/const-version", EcLevel::M).unwrap_or_else(|err| {
            eprintln!("{err}");
            std::process::exit(1);
        });
    let encoded =
        ConstVersionEncoder::<5>::new(EcLevel::M).encode(b"https://example.com/const-version").unwrap_or_else(|err| {
            eprintln!("{err}");
            std::process::exit(1);
        });

    assert_eq!(direct.version(), VERSION_5);
    assert_eq!(encoded.version(), VERSION_5);
    println!("fixed version: {:?}, width: {}", VERSION_5, encoded.width());
}
