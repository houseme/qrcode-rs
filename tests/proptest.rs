use proptest::prelude::*;
use qrcode_rs::structured_append::{SaSymbol, reassemble};
use qrcode_rs::{EcLevel, QrCode, Version};

fn ec_levels() -> impl Strategy<Value = EcLevel> {
    prop::sample::select(vec![EcLevel::L, EcLevel::M, EcLevel::Q, EcLevel::H])
}

fn short_payload() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 1..128)
}

fn split_like_structured_append(payload: &[u8], symbols: u8) -> Vec<SaSymbol<'_>> {
    let n = usize::from(symbols);
    let chunk = payload.len().div_ceil(n);
    let parity = payload.iter().fold(0u8, |acc, &byte| acc ^ byte);
    (0..n)
        .map(|index| {
            let start = (index * chunk).min(payload.len());
            let end = ((index + 1) * chunk).min(payload.len());
            SaSymbol { position: index as u8 + 1, total: symbols, parity, data: &payload[start..end] }
        })
        .collect()
}

proptest! {
    #[test]
    fn encode_short_payload_keeps_width_in_normal_qr_bounds(data in short_payload()) {
        let code = QrCode::new(&data)?;

        prop_assert!((21..=177).contains(&code.width()));
        prop_assert_eq!(code.width() * code.width(), code.colors().len());
    }
    #[test]
    fn stream_matches_batch_for_short_payloads(inputs in prop::collection::vec(short_payload(), 0..16)) {
        let streamed = QrCode::stream_with_error_correction_level(inputs.iter(), EcLevel::H)
            .collect::<Result<Vec<_>, _>>()?;
        let batched = QrCode::batch(inputs.iter(), EcLevel::H)?;

        prop_assert_eq!(streamed.len(), batched.len());
        for (left, right) in streamed.iter().zip(batched.iter()) {
            prop_assert_eq!(left.version(), right.version());
            prop_assert_eq!(left.colors(), right.colors());
        }
    }

    #[test]
    fn structured_append_split_rule_reassembles_short_payloads(data in short_payload(), symbols in 2u8..=16) {
        let codes = QrCode::structured_append(&data, symbols, EcLevel::M)?;
        let parts = split_like_structured_append(&data, symbols);

        prop_assert_eq!(codes.len(), usize::from(symbols));
        prop_assert_eq!(reassemble(&parts).unwrap(), data);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        // These properties probe the fixed-version API, so keep them
        // exhaustive enough to find boundary errors without making the normal
        // test suite quadratic in payload size.
        cases: 32,
        .. ProptestConfig::default()
    })]

    #[test]
    fn arbitrary_byte_payload_never_panics(data in prop::collection::vec(any::<u8>(), 0..=256), ec in ec_levels()) {
        // A too-large payload is an ordinary `Err`, not a panic.  Keep the
        // call inside catch_unwind so malformed/random bytes exercise the same
        // public boundary as valid payloads.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = QrCode::with_error_correction_level(&data, ec);
        }));
        prop_assert!(result.is_ok(), "encoding panicked for ec={ec:?}, len={}", data.len());
    }

    #[test]
    fn arbitrary_vec_payload_never_panics(data in any::<Vec<u8>>()) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = QrCode::new(&data);
        }));
        prop_assert!(result.is_ok(), "encoding panicked for len={}", data.len());
    }

    #[test]
    fn automatic_version_is_the_smallest_fitting_version(data in prop::collection::vec(any::<u8>(), 0..=128), ec in ec_levels()) {
        let automatic = QrCode::with_error_correction_level(&data, ec)?;
        let version = automatic.version();
        prop_assert!(QrCode::with_version(&data, version, ec).is_ok());
        if let Version::Normal(number) = version {
            if number > 1 {
                let previous = QrCode::with_version(&data, Version::Normal(number - 1), ec);
                prop_assert!(previous.is_err(), "version {:?} also fits, ec={ec:?}, len={}", Version::Normal(number - 1), data.len());
            }
        }
    }
}
