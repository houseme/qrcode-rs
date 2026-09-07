use qrcode_rs::{EcLevel, Mode, QrCode, Version};
use reference_qrcode::QrCode as ReferenceQrCode;
use reference_qrcode::bits::Bits as ReferenceBits;
use reference_qrcode::types::{EcLevel as ReferenceEcLevel, Version as ReferenceVersion};

fn render_ours_code(code: QrCode) -> String {
    // Use the facade's stable text helper here.  The split renderer also
    // implements the fallible core `Builder` trait, so a bare `.build()` can
    // become ambiguous when both APIs are in scope.
    code.to_debug_str('#', '.')
}

fn render_reference_code(code: ReferenceQrCode) -> String {
    code.render::<char>().quiet_zone(false).dark_color('#').light_color('.').module_dimensions(1, 1).build()
}

fn render_ours(data: &[u8]) -> String {
    render_ours_code(QrCode::new(data).unwrap())
}

fn render_reference(data: &[u8]) -> String {
    render_reference_code(ReferenceQrCode::new(data).unwrap())
}

fn reference_ec(ec: EcLevel) -> ReferenceEcLevel {
    match ec {
        EcLevel::L => ReferenceEcLevel::L,
        EcLevel::M => ReferenceEcLevel::M,
        EcLevel::Q => ReferenceEcLevel::Q,
        EcLevel::H => ReferenceEcLevel::H,
    }
}

fn reference_version(version: Version) -> ReferenceVersion {
    match version {
        Version::Normal(value) => ReferenceVersion::Normal(value),
        Version::Micro(value) => ReferenceVersion::Micro(value),
    }
}

fn render_ours_fixed(data: &[u8], version: Version, ec: EcLevel) -> String {
    render_ours_code(QrCode::with_version(data, version, ec).unwrap())
}

fn render_reference_fixed(data: &[u8], version: Version, ec: EcLevel) -> String {
    render_reference_code(ReferenceQrCode::with_version(data, reference_version(version), reference_ec(ec)).unwrap())
}

fn render_reference_forced(data: &[u8], version: Version, ec: EcLevel, mode: Mode) -> String {
    let version = reference_version(version);
    let ec = reference_ec(ec);
    let mut bits = ReferenceBits::new(version);
    match mode {
        Mode::Numeric => bits.push_numeric_data(data).unwrap(),
        Mode::Alphanumeric => bits.push_alphanumeric_data(data).unwrap(),
        Mode::Byte => bits.push_byte_data(data).unwrap(),
        // This test intentionally excludes Kanji: the reference crate's
        // Shift-JIS conversion support is not a stable cross-version contract.
        Mode::Kanji => unreachable!("Kanji is not part of this differential matrix"),
    }
    bits.push_terminator(ec).unwrap();
    render_reference_code(ReferenceQrCode::with_bits(bits, ec).unwrap())
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

#[test]
fn auto_encoding_matches_reference_across_error_correction_levels() {
    for ec in [EcLevel::L, EcLevel::M, EcLevel::Q, EcLevel::H] {
        for data in [b"01234567890123456789".as_slice(), b"HELLO WORLD".as_slice(), b"raw \x00\xff bytes"] {
            let ours = render_ours_code(QrCode::with_error_correction_level(data, ec).unwrap());
            let reference =
                render_reference_code(ReferenceQrCode::with_error_correction_level(data, reference_ec(ec)).unwrap());
            assert_eq!(ours, reference, "data={data:?}, ec={ec:?}");
        }
    }
}

#[test]
fn fixed_versions_match_reference_across_all_error_correction_levels() {
    let data = b"01234567";
    for version in [
        Version::Normal(1),
        Version::Normal(2),
        Version::Normal(5),
        Version::Normal(10),
        Version::Normal(20),
        Version::Normal(40),
    ] {
        for ec in [EcLevel::L, EcLevel::M, EcLevel::Q, EcLevel::H] {
            assert_eq!(
                render_ours_fixed(data, version, ec),
                render_reference_fixed(data, version, ec),
                "version={version:?}, ec={ec:?}"
            );
        }
    }
}

#[test]
fn forced_modes_match_reference_for_normal_qr() {
    let fixtures = [
        (Mode::Numeric, b"01234567890123456789".as_slice()),
        (Mode::Alphanumeric, b"HELLO WORLD 123".as_slice()),
        (Mode::Byte, b"byte payload: \x00\xff\x10".as_slice()),
    ];
    for (mode, data) in fixtures {
        for ec in [EcLevel::L, EcLevel::M, EcLevel::Q, EcLevel::H] {
            let ours = QrCode::builder(data).ec_level(ec).force_mode(mode).build().unwrap();
            assert_eq!(
                render_ours_code(ours.clone()),
                render_reference_forced(data, ours.version(), ec, mode),
                "mode={mode:?}, ec={ec:?}, version={:?}",
                ours.version()
            );
        }
    }
}
