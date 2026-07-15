use proptest::prelude::*;
use qrcode_rs::structured_append::{SaSymbol, reassemble};
use qrcode_rs::{EcLevel, QrCode};

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
