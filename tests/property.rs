//! Security-oriented properties for public encoding and decoding boundaries.
//!
//! These tests intentionally treat errors as ordinary outcomes: arbitrary
//! bytes may be too large or malformed, but they must not panic or allocate an
//! unbounded QR matrix.

use proptest::prelude::*;
use qrcode_rs::bits::Bits;
use qrcode_rs::decode::sa_parse::parse_sa_datastream;
use qrcode_rs::{EcLevel, QrCode, Version};

fn ec_levels() -> impl Strategy<Value = EcLevel> {
    prop::sample::select(vec![EcLevel::L, EcLevel::M, EcLevel::Q, EcLevel::H])
}

proptest! {
    #![proptest_config(ProptestConfig {
        // Keep this security boundary test quick in regular CI; cargo-fuzz
        // provides the unbounded input stream for longer campaigns.
        cases: 64,
        .. ProptestConfig::default()
    })]

    #[test]
    fn no_panic_on_any_input(data in prop::collection::vec(any::<u8>(), 0..=512), ec in ec_levels()) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = QrCode::with_error_correction_level(&data, ec);
        }));
        prop_assert!(result.is_ok(), "encoding panicked for ec={ec:?}, len={}", data.len());
    }

    #[test]
    fn output_size_and_memory_are_bounded(data in prop::collection::vec(any::<u8>(), 0..=512)) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| QrCode::new(&data)));
        prop_assert!(result.is_ok(), "encoding panicked for len={}", data.len());
        if let Ok(Ok(code)) = result {
            let width = code.width();
            prop_assert!((11..=177).contains(&width), "unexpected QR width {width}");
            prop_assert_eq!(code.colors().len(), width * width);
            prop_assert!(code.colors().len() <= 177usize * 177);
        }
    }

    #[test]
    fn automatic_version_is_minimal(data in prop::collection::vec(any::<u8>(), 0..=128), ec in ec_levels()) {
        let automatic = QrCode::with_error_correction_level(&data, ec)?;
        let version = automatic.version();
        prop_assert!(QrCode::with_version(&data, version, ec).is_ok());
        if let Version::Normal(number) = version {
            if number > 1 {
                prop_assert!(
                    QrCode::with_version(&data, Version::Normal(number - 1), ec).is_err(),
                    "previous version unexpectedly fits: {number:?}"
                );
            }
        }
    }

    #[test]
    fn sa_parser_round_trips_byte_segments(data in prop::collection::vec(any::<u8>(), 0..=128)) {
        let version = Version::Normal(40);
        let parity = data.iter().fold(0u8, |acc, &byte| acc ^ byte);
        let mut bits = Bits::new(version);
        bits.push_structured_append_header(1, 2, parity)?;
        bits.push_byte_data(&data)?;
        bits.push_terminator(EcLevel::M)?;

        let parsed = parse_sa_datastream(&bits.into_bytes(), version)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(parsed.position, 1);
        prop_assert_eq!(parsed.total, 2);
        prop_assert_eq!(parsed.parity, parity);
        prop_assert_eq!(parsed.data, data);
    }

    #[test]
    fn sa_parser_never_panics_on_arbitrary_stream(
        selector in any::<u8>(),
        stream in prop::collection::vec(any::<u8>(), 0..=512),
    ) {
        let version = Version::Normal(i16::from(selector % 40) + 1);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = parse_sa_datastream(&stream, version);
        }));
        prop_assert!(result.is_ok(), "SA parser panicked for version={version:?}, len={}", stream.len());
    }
}
