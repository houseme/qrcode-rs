use qrcode_rs::{EcLevel, NumericMode, QrCode, QrError, Version};

fn main() {
    let code = QrCode::builder(b"01234567")
        .version(Version::Normal(1))
        .ec_level(EcLevel::M)
        .encoding_mode_typed::<NumericMode>()
        .unwrap_or_else(|err| {
            eprintln!("{err}");
            std::process::exit(1);
        })
        .build()
        .unwrap_or_else(|err| {
            eprintln!("{err}");
            std::process::exit(1);
        });

    assert_eq!(code.version(), Version::Normal(1));

    let invalid = QrCode::builder(b"12a").encoding_mode_typed::<NumericMode>();
    assert!(matches!(invalid, Err(QrError::InvalidCharacter { position: 2, byte: b'a' })));

    println!("typed numeric mode width: {}", code.width());
}
