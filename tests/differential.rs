use qrcode_rs::QrCode;
use reference_qrcode::QrCode as ReferenceQrCode;

fn render_ours(data: &[u8]) -> String {
    QrCode::new(data)
        .unwrap()
        .render::<char>()
        .quiet_zone(false)
        .dark_color('#')
        .light_color('.')
        .module_dimensions(1, 1)
        .build()
}

fn render_reference(data: &[u8]) -> String {
    ReferenceQrCode::new(data)
        .unwrap()
        .render::<char>()
        .quiet_zone(false)
        .dark_color('#')
        .light_color('.')
        .module_dimensions(1, 1)
        .build()
}

#[test]
fn auto_encoding_matches_reference_crate_for_stable_fixtures() {
    for data in [
        b"HELLO WORLD".as_slice(),
        b"01234567890123456789",
        b"https://example.com/qrcode-rs",
        b"byte payload: \x00\xff\x10",
    ] {
        assert_eq!(render_ours(data), render_reference(data), "data={data:?}");
    }
}
