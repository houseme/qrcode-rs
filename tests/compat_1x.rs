#![cfg(feature = "compat-1x")]

use qrcode_rs::{Color, EcLevel, QrCode, Version};

#[test]
fn one_x_constructors_render_chain_and_indexing_stay_available() {
    let code = QrCode::with_version(b"01234567", Version::Normal(1), EcLevel::M).unwrap();

    assert_eq!(code.width(), 21);
    assert_eq!(code.colors().len(), code.width() * code.width());
    assert!(matches!(code[(0, 0)], Color::Dark | Color::Light));

    let rendered = code.render::<char>().quiet_zone(false).dark_color('#').light_color(' ').build();
    assert_eq!(rendered.lines().count(), code.width());
    assert!(rendered.contains('#'));
}

#[test]
fn one_x_payload_helpers_stay_available() {
    let helpers = [
        QrCode::for_text("hello"),
        QrCode::for_url("https://example.com"),
        QrCode::for_wifi("MyNetwork", "p\\a;ss", "WPA"),
        QrCode::for_vcard("John Doe", "+1234567890", "john@example.com"),
        QrCode::for_gs1("010491234512345915970331301234561842"),
    ];

    for code in helpers {
        assert!(code.unwrap().width() >= 21);
    }
}

#[test]
fn one_x_batch_api_stays_available() {
    let codes = QrCode::batch(["alpha", "beta", "gamma"], EcLevel::Q).unwrap();

    assert_eq!(codes.len(), 3);
    assert!(codes.iter().all(|code| code.error_correction_level() == EcLevel::Q));
}

#[cfg(feature = "eps")]
#[test]
fn one_x_render_template_stays_available() {
    let code = QrCode::new(b"template").unwrap();
    let eps = code.render::<qrcode_rs::render::eps::Color>().template(&qrcode_rs::QrTemplate::corporate()).build();

    assert!(eps.contains("0 0.2 0.4 setrgbcolor"));
}
