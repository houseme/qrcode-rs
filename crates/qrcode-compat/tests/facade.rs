use qrcode_compat::{Color, EcLevel, QrCode, Version};

#[test]
fn compat_crate_reexports_one_x_facade() {
    let code = QrCode::with_version(b"01234567", Version::Normal(1), EcLevel::M).unwrap();

    assert_eq!(code.width(), 21);
    assert_eq!(code.colors().len(), code.width() * code.width());
    assert!(matches!(code[(0, 0)], Color::Dark | Color::Light));

    let rendered = code.render::<char>().quiet_zone(false).dark_color('#').light_color(' ').build();
    assert_eq!(rendered.lines().count(), code.width());
    assert!(rendered.contains('#'));
}

#[test]
fn compat_crate_reexports_payload_helpers() {
    let code = QrCode::for_wifi("MyNetwork", "p\\a;ss", "WPA").unwrap();

    assert!(code.width() >= 21);
}
