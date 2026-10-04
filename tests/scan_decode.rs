//! Rendered Normal, Micro, and Structured Append image regressions.

#![cfg(all(feature = "decode-rxing", feature = "image"))]

use image::{GrayImage, Luma, imageops};
use qrcode_rs::bits::Bits;
use qrcode_rs::decode::rxing::{DecodeError, RxingDecoder, ScanOptions};
use qrcode_rs::decode::{GrayPixels, ScanSymbol};
use qrcode_rs::structured_append::{SaError, StructuredAppend, reassemble_decoded};
use qrcode_rs::{EcLevel, Mode, QrCode, Version};

const MODULE: u32 = 6;

fn render(code: &QrCode) -> GrayImage {
    code.render::<Luma<u8>>().module_dimensions(MODULE, MODULE).build()
}

fn scan(image: &GrayImage, inverted: bool) -> Vec<Result<ScanSymbol, DecodeError>> {
    // Exercise the borrowed public boundary without depending on its optional
    // From<&GrayImage> convenience implementation.
    let view = GrayPixels::try_new(image.width(), image.height(), image.as_raw()).unwrap();
    let decoder = RxingDecoder::new();
    if inverted {
        let mut options = ScanOptions::default();
        options.inverted = true;
        decoder.scan_with_options(view, options).expect("valid grayscale image should be scanned")
    } else {
        decoder.scan(view).expect("valid grayscale image should be scanned")
    }
}

fn scan_one(image: &GrayImage, inverted: bool) -> ScanSymbol {
    let mut results = scan(image, inverted);
    assert_eq!(results.len(), 1, "single-symbol image: {results:?}");
    results.pop().unwrap().expect("rendered symbol should decode")
}

fn assert_symbol(symbol: &ScanSymbol, code: &QrCode, payload: &[u8]) {
    assert_eq!(symbol.decoded().data(), payload, "payload for {:?}", code.version());
    assert_eq!(symbol.decoded().version(), code.version());
    assert_eq!(symbol.decoded().ec_level(), code.info().ec_level());
}

fn assert_roundtrip(code: &QrCode, payload: &[u8]) {
    let symbol = scan_one(&render(code), false);
    assert_symbol(&symbol, code, payload);
    assert!(symbol.structured_append().is_none());
    assert_eq!(symbol.into_decoded().into_data(), payload);
}

fn forced(payload: &[u8], version: Version, ec: EcLevel, mode: Mode) -> QrCode {
    let mut bits = Bits::new(version);
    match mode {
        Mode::Numeric => bits.push_numeric_data(payload).unwrap(),
        Mode::Alphanumeric => bits.push_alphanumeric_data(payload).unwrap(),
        Mode::Byte => bits.push_byte_data(payload).unwrap(),
        Mode::Kanji => bits.push_kanji_data(payload).unwrap(),
    }
    bits.push_terminator(ec).unwrap();
    QrCode::with_bits(bits, ec).unwrap()
}

fn montage(images: &[GrayImage]) -> GrayImage {
    let gap = MODULE * 4;
    let width = images.iter().map(GrayImage::width).sum::<u32>() + gap * (images.len() as u32 + 1);
    let height = images.iter().map(GrayImage::height).max().unwrap() + 2 * gap;
    let mut out = GrayImage::from_pixel(width, height, Luma([255]));
    let mut x = gap;
    for image in images {
        imageops::replace(&mut out, image, i64::from(x), i64::from(gap));
        x += image.width() + gap;
    }
    out
}

fn scan_montage(codes: &[QrCode]) -> Vec<ScanSymbol> {
    let images = codes.iter().map(render).collect::<Vec<_>>();
    let symbols = scan(&montage(&images), false).into_iter().filter_map(Result::ok).collect::<Vec<_>>();
    assert_eq!(symbols.len(), codes.len(), "every independent symbol must be retained");
    symbols
}

fn assert_sa_header(symbol: &ScanSymbol, position: u8, total: u8, parity: u8) {
    let header = symbol.structured_append().expect("Structured Append header must be retained");
    assert_eq!(header.position(), position);
    assert_eq!(header.total(), total);
    assert_eq!(header.parity(), parity);
}

fn sa_code(position: u8, total: u8, parity: u8, payload: &[u8]) -> QrCode {
    let mut bits = Bits::new(Version::Normal(1));
    bits.push_structured_append_header(position, total, parity).unwrap();
    bits.push_byte_data(payload).unwrap();
    bits.push_terminator(EcLevel::M).unwrap();
    QrCode::with_bits(bits, EcLevel::M).unwrap()
}

#[test]
fn normal_version_and_ec_metadata_round_trip() {
    for number in [1, 9, 10, 26, 27, 40] {
        for ec in [EcLevel::L, EcLevel::M, EcLevel::Q, EcLevel::H] {
            let code = forced(b"meta", Version::Normal(number), ec, Mode::Byte);
            assert_roundtrip(&code, b"meta");
        }
    }
}

#[test]
fn normal_modes_binary_unicode_and_empty_payloads_round_trip() {
    for (mode, data) in [
        (Mode::Numeric, b"01234567890".as_slice()),
        (Mode::Alphanumeric, b"A1 $%*+-./:".as_slice()),
        (Mode::Byte, b"\x00\xff\x80\xc0\xaf\xed\xa0\x80".as_slice()),
        (Mode::Kanji, b"\x93\x5f\xe4\xaa".as_slice()),
    ] {
        for ec in [EcLevel::L, EcLevel::M, EcLevel::Q, EcLevel::H] {
            let code = forced(data, Version::Normal(2), ec, mode);
            assert_roundtrip(&code, data);
        }
    }
    for data in [b"".as_slice(), "中文 QR 😀 café".as_bytes()] {
        let code = QrCode::new(data).unwrap();
        assert_roundtrip(&code, data);
    }
    let binary = (0..=u8::MAX).collect::<Vec<_>>();
    let code = QrCode::new(&binary).unwrap();
    assert_roundtrip(&code, &binary);
}

#[test]
fn eci_and_mixed_segments_keep_original_bytes() {
    let utf8 = "中😀".as_bytes();
    let mut bits = Bits::new(Version::Normal(2));
    bits.push_eci_designator(26).unwrap();
    bits.push_byte_data(utf8).unwrap();
    bits.push_terminator(EcLevel::M).unwrap();
    assert_roundtrip(&QrCode::with_bits(bits, EcLevel::M).unwrap(), utf8);

    let mut bits = Bits::new(Version::Normal(3));
    bits.push_numeric_data(b"0123456789").unwrap();
    bits.push_alphanumeric_data(b"A1%*").unwrap();
    bits.push_byte_data(utf8).unwrap();
    bits.push_kanji_data(b"\x93\x5f").unwrap();
    bits.push_terminator(EcLevel::M).unwrap();
    let payload = [b"0123456789".as_slice(), b"A1%*", utf8, b"\x93\x5f"].concat();
    assert_roundtrip(&QrCode::with_bits(bits, EcLevel::M).unwrap(), &payload);
}

#[test]
fn all_legal_micro_modes_and_ec_levels_round_trip() {
    let mut combinations = 0;
    for number in 1..=4 {
        let levels: &[EcLevel] = match number {
            1 => &[EcLevel::L],
            2 | 3 => &[EcLevel::L, EcLevel::M],
            4 => &[EcLevel::L, EcLevel::M, EcLevel::Q],
            _ => unreachable!(),
        };
        for &ec in levels {
            for (mode, payload) in [
                (Mode::Numeric, b"123".as_slice()),
                (Mode::Alphanumeric, b"A1".as_slice()),
                (Mode::Byte, b"\x00\xff".as_slice()),
                (Mode::Kanji, b"\x93\x5f".as_slice()),
            ] {
                if number == 1 && mode != Mode::Numeric || number == 2 && matches!(mode, Mode::Byte | Mode::Kanji) {
                    continue;
                }
                let version = Version::Micro(number);
                assert_roundtrip(&forced(payload, version, ec, mode), payload);
                // A zero-length segment still has a real mode/count header.
                assert_roundtrip(&forced(&[], version, ec, mode), &[]);
                combinations += 1;
            }
            assert_roundtrip(&QrCode::with_version([], Version::Micro(number), ec).unwrap(), &[]);
        }
    }
    assert_eq!(combinations, 25);
}

#[test]
fn nonzero_micro_half_codewords_round_trip() {
    for (number, ec, payload) in [
        (1, EcLevel::L, b"9999".as_slice()),
        (1, EcLevel::L, b"99999".as_slice()),
        (3, EcLevel::L, b"99999999999999999999999".as_slice()),
        (3, EcLevel::M, b"999999999999999999".as_slice()),
    ] {
        assert_roundtrip(&forced(payload, Version::Micro(number), ec, Mode::Numeric), payload);
    }
}

fn micro_data_coordinates(width: usize) -> Vec<(usize, usize)> {
    let mut coordinates = Vec::new();
    let mut reading_up = true;
    let mut right = width - 1;
    while right > 0 {
        for row in 0..width {
            let y = if reading_up { width - 1 - row } else { row };
            for x in [right, right - 1] {
                if x != 0 && y != 0 && !(x < 9 && y < 9) {
                    coordinates.push((x, y));
                }
            }
        }
        reading_up = !reading_up;
        right -= 2;
    }
    coordinates
}

fn flip_module(image: &mut GrayImage, x: usize, y: usize, quiet: u32, module: u32) {
    let pixel_x = (x as u32 + quiet) * module;
    let pixel_y = (y as u32 + quiet) * module;
    for y in pixel_y..pixel_y + module {
        for x in pixel_x..pixel_x + module {
            image.get_pixel_mut(x, y).0[0] ^= 0xff;
        }
    }
}

#[test]
fn micro_m3_m_recovers_four_errors_without_spending_one_on_the_half_codeword() {
    let payload = b"999999999999999999";
    let code = forced(payload, Version::Micro(3), EcLevel::M, Mode::Numeric);
    let mut bits = Bits::new(Version::Micro(3));
    bits.push_numeric_data(payload).unwrap();
    bits.push_terminator(EcLevel::M).unwrap();
    assert_eq!(bits.into_bytes().last(), Some(&0xe0));
    let coordinates = micro_data_coordinates(code.width());
    assert_eq!(code.width(), 15);
    assert_eq!(coordinates.len(), 68 + 8 * 8);
    let mut image: GrayImage = code.render::<Luma<u8>>().module_dimensions(8, 8).build();
    for offset in [0, 8, 16, 24] {
        let (x, y) = coordinates[offset];
        assert!(!code.is_functional(x, y));
        flip_module(&mut image, x, y, 2, 8);
    }
    let symbol = scan_one(&image, false);
    assert_symbol(&symbol, &code, payload);
}

#[test]
fn rotation_mirroring_and_explicit_inversion_round_trip() {
    let fixtures = [
        (QrCode::new("rotation 中文".as_bytes()).unwrap(), "rotation 中文".as_bytes()),
        (forced(b"123", Version::Micro(1), EcLevel::L, Mode::Numeric), b"123".as_slice()),
        (forced(b"\x00\xff", Version::Micro(3), EcLevel::M, Mode::Byte), b"\x00\xff".as_slice()),
        (forced(b"A1", Version::Micro(4), EcLevel::Q, Mode::Alphanumeric), b"A1".as_slice()),
        (sa_code(1, 2, b'A' ^ b'B', b"A"), b"A".as_slice()),
    ];
    for (code, payload) in fixtures {
        let image = render(&code);
        for transformed in [
            imageops::rotate90(&image),
            imageops::rotate180(&image),
            imageops::rotate270(&image),
            imageops::flip_horizontal(&image),
            imageops::flip_vertical(&image),
        ] {
            let symbol = scan_one(&transformed, false);
            assert_symbol(&symbol, &code, payload);
            if payload == b"A" {
                assert_sa_header(&symbol, 1, 2, b'A' ^ b'B');
            }
        }
        let mut inverted = image;
        imageops::invert(&mut inverted);
        let symbol = scan_one(&inverted, true);
        assert_symbol(&symbol, &code, payload);
        if payload == b"A" {
            assert_sa_header(&symbol, 1, 2, b'A' ^ b'B');
        }
    }
}

#[test]
fn one_image_retains_normal_micro_and_structured_append_symbols() {
    let codes = [
        QrCode::new(b"normal binary \x00\xff").unwrap(),
        forced(b"A1", Version::Micro(4), EcLevel::Q, Mode::Alphanumeric),
        sa_code(1, 2, b'A' ^ b'B', b"A"),
    ];
    let symbols = scan_montage(&codes);
    for (code, payload) in codes.iter().zip([b"normal binary \x00\xff".as_slice(), b"A1", b"A"]) {
        let symbol = symbols.iter().find(|symbol| symbol.decoded().data() == payload).unwrap();
        assert_symbol(symbol, code, payload);
    }
    assert_eq!(symbols.iter().filter(|symbol| symbol.structured_append().is_some()).count(), 1);
}

#[test]
fn all_structured_append_counts_scan_and_reassemble_in_reverse_order() {
    let payload = b"SA raw \x00\xff fragments across all valid counts";
    for total in 2..=16 {
        let builder = StructuredAppend::new(total, payload).unwrap();
        let codes = builder.encode(EcLevel::M).unwrap();
        let chunk = payload.len().div_ceil(usize::from(total));
        let mut symbols = Vec::with_capacity(codes.len());
        for (index, code) in codes.iter().enumerate() {
            let symbol = scan_one(&render(code), false);
            let start = (index * chunk).min(payload.len());
            let end = ((index + 1) * chunk).min(payload.len());
            assert_symbol(&symbol, code, &payload[start..end]);
            assert_sa_header(&symbol, index as u8 + 1, total, builder.parity());
            symbols.push(symbol);
        }
        symbols.reverse();
        assert_eq!(reassemble_decoded(&symbols).unwrap(), payload, "total={total}");
    }
}

#[test]
fn structured_append_preserves_empty_fragments_and_split_utf8_bytes() {
    for payload in [b"x".as_slice(), b"".as_slice()] {
        let builder = StructuredAppend::new(3, payload).unwrap();
        let codes = builder.encode(EcLevel::M).unwrap();
        let mut symbols = Vec::new();
        for (index, code) in codes.iter().enumerate() {
            let symbol = scan_one(&render(code), false);
            assert_sa_header(&symbol, index as u8 + 1, 3, builder.parity());
            assert_eq!(symbol.decoded().data(), if index == 0 { payload } else { b"" });
            symbols.push(symbol);
        }
        assert_eq!(reassemble_decoded(&symbols).unwrap(), payload);
    }
    let payload = "中😀".as_bytes();
    let builder = StructuredAppend::new(2, payload).unwrap();
    let codes = builder.encode(EcLevel::M).unwrap();
    let symbols = codes.iter().map(|code| scan_one(&render(code), false)).collect::<Vec<_>>();
    assert_eq!(symbols[0].decoded().data(), &payload[..4]);
    assert_eq!(symbols[1].decoded().data(), &payload[4..]);
    assert!(symbols.iter().all(|symbol| core::str::from_utf8(symbol.decoded().data()).is_err()));
    assert_eq!(reassemble_decoded(&symbols).unwrap(), payload);
}

#[test]
fn structured_append_mixed_versions_and_montage_order_round_trip() {
    let payload = [vec![b'9'; 40], vec![b'a'; 40]].concat();
    let builder = StructuredAppend::new(2, &payload).unwrap();
    let mut codes = builder.encode(EcLevel::M).unwrap();
    assert_eq!(codes[0].version(), Version::Normal(2));
    assert_eq!(codes[1].version(), Version::Normal(3));
    codes.reverse();
    let symbols = scan_montage(&codes);
    assert_eq!(reassemble_decoded(&symbols).unwrap(), payload);
    for symbol in &symbols {
        let header = symbol.structured_append().unwrap();
        assert_eq!(header.total(), 2);
        assert_eq!(header.parity(), builder.parity());
        let (version, data) = if header.position() == 1 {
            (Version::Normal(2), &payload[..40])
        } else {
            assert_eq!(header.position(), 2);
            (Version::Normal(3), &payload[40..])
        };
        assert_eq!(symbol.decoded().version(), version);
        assert_eq!(symbol.decoded().data(), data);
    }
}

#[test]
fn explicit_reassembly_rejects_missing_duplicate_and_unrelated_symbols() {
    let codes = StructuredAppend::new(3, b"three-part payload").unwrap().encode(EcLevel::M).unwrap();
    let mut symbols = codes.iter().map(|code| scan_one(&render(code), false)).collect::<Vec<_>>();
    symbols.reverse();
    assert_eq!(reassemble_decoded(&symbols).unwrap(), b"three-part payload");
    assert_eq!(reassemble_decoded(&symbols[..2]), Err(SaError::Incomplete));
    assert_eq!(reassemble_decoded(&[]), Err(SaError::Incomplete));

    let missing_image = montage(&[render(&codes[0]), render(&codes[2])]);
    let missing = scan(&missing_image, false).into_iter().filter_map(Result::ok).collect::<Vec<_>>();
    assert_eq!(missing.len(), 2);
    assert_eq!(reassemble_decoded(&missing), Err(SaError::Incomplete));

    let duplicates = scan_montage(&[codes[0].clone(), codes[0].clone(), codes[2].clone()]);
    assert_eq!(reassemble_decoded(&duplicates), Err(SaError::DuplicatePosition(1)));

    let plain = scan_one(&render(&QrCode::new(b"ordinary symbol").unwrap()), false);
    assert_eq!(reassemble_decoded(&[plain]), Err(SaError::NotStructuredAppend));
}

#[test]
fn explicit_reassembly_checks_header_and_payload_parity() {
    for (codes, expected) in [
        ([sa_code(1, 2, 0, b"a"), sa_code(2, 3, 0, b"b")], SaError::CountMismatch),
        ([sa_code(1, 2, 0, b"a"), sa_code(2, 2, 1, b"b")], SaError::ParityMismatch),
        ([sa_code(1, 2, 0, b"a"), sa_code(2, 2, 0, b"b")], SaError::ParityMismatch),
    ] {
        let symbols = codes.iter().map(|code| scan_one(&render(code), false)).collect::<Vec<_>>();
        assert_eq!(symbols[0].decoded().data(), b"a");
        assert_eq!(symbols[1].decoded().data(), b"b");
        assert_eq!(reassemble_decoded(&symbols), Err(expected));
    }
}

#[test]
fn parity_collisions_keep_all_fragments_and_reject_duplicate_positions() {
    let first = StructuredAppend::new(2, b"ABCD").unwrap();
    let second = StructuredAppend::new(2, b"EFKL").unwrap();
    assert_eq!(first.parity(), second.parity());
    let mut codes = first.encode(EcLevel::M).unwrap();
    codes.extend(second.encode(EcLevel::M).unwrap());
    let symbols = scan_montage(&codes);
    assert_eq!(reassemble_decoded(&symbols), Err(SaError::Incomplete));
    let repeated_position =
        symbols.into_iter().filter(|symbol| symbol.structured_append().unwrap().position() == 1).collect::<Vec<_>>();
    assert_eq!(repeated_position.len(), 2);
    assert_eq!(reassemble_decoded(&repeated_position), Err(SaError::DuplicatePosition(1)));
    // A matching eight-bit XOR is not a proof of group identity: even the
    // mixed payload AB + KL shares their parity. No automatic selection occurs.
    assert_eq!(b"ABKL".iter().fold(0_u8, |parity, &byte| parity ^ byte), first.parity());
}

#[test]
fn distinct_structured_append_groups_are_selected_explicitly() {
    let first = StructuredAppend::new(2, b"ABCD").unwrap();
    let second = StructuredAppend::new(2, b"ABEF").unwrap();
    assert_ne!(first.parity(), second.parity());
    let mut codes = first.encode(EcLevel::M).unwrap();
    codes.extend(second.encode(EcLevel::M).unwrap());
    let symbols = scan_montage(&codes);
    assert_eq!(reassemble_decoded(&symbols), Err(SaError::ParityMismatch));
    let (first_symbols, second_symbols): (Vec<_>, Vec<_>) =
        symbols.into_iter().partition(|symbol| symbol.structured_append().unwrap().parity() == first.parity());
    assert_eq!(first_symbols.len(), 2);
    assert_eq!(second_symbols.len(), 2);
    assert_eq!(reassemble_decoded(&first_symbols).unwrap(), b"ABCD");
    assert_eq!(reassemble_decoded(&second_symbols).unwrap(), b"ABEF");
}

#[test]
fn too_many_selected_fragments_preserve_metadata_error_precedence() {
    let image = render(&sa_code(1, 2, 0, b"a"));
    let mut symbols = (0..17).map(|_| scan_one(&image, false)).collect::<Vec<_>>();
    assert_eq!(reassemble_decoded(&symbols), Err(SaError::Incomplete));
    symbols[16] = scan_one(&render(&sa_code(2, 3, 0, b"b")), false);
    assert_eq!(reassemble_decoded(&symbols), Err(SaError::CountMismatch));
    symbols[16] = scan_one(&render(&sa_code(2, 2, 1, b"b")), false);
    assert_eq!(reassemble_decoded(&symbols), Err(SaError::ParityMismatch));
}

#[test]
fn damaged_candidate_does_not_discard_a_successful_symbol() {
    let bad_code = forced(b"damaged candidate", Version::Normal(2), EcLevel::M, Mode::Byte);
    let mut damaged = render(&bad_code);
    // Preserve every functional module so detection still identifies the code;
    // invert the data and EC modules far beyond its correction capacity.
    for y in 0..bad_code.width() {
        for x in 0..bad_code.width() {
            if !bad_code.is_functional(x, y) {
                flip_module(&mut damaged, x, y, 4, MODULE);
            }
        }
    }
    let alone = scan(&damaged, false);
    assert!(alone.iter().any(Result::is_err), "damaged fixture must produce a candidate error: {alone:?}");
    assert!(alone.iter().all(Result::is_err), "damaged fixture must not be successfully corrected: {alone:?}");

    let payload = b"good neighbor \x00\xff";
    let good_code = QrCode::new(payload).unwrap();
    let results = scan(&montage(&[damaged, render(&good_code)]), false);
    assert!(results.iter().any(Result::is_err), "bad candidate must remain reported: {results:?}");
    let recovered = results.into_iter().filter_map(Result::ok).collect::<Vec<_>>();
    assert_eq!(recovered.len(), 1);
    assert_symbol(&recovered[0], &good_code, payload);
}
