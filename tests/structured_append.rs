//! Structured Append end-to-end: the encoder's split + parity rule is exactly
//! what `reassemble` reverses. No feature gate — Structured Append is pure core.
//!
//! Note: `rqrr` (this crate's `decode-rqrr` adapter) cannot decode Structured
//! Append symbols (it returns `UnknownDataType` for mode `0011`), so these
//! tests verify the symmetry contract directly rather than via an image
//! round-trip.

use qrcode_rs::structured_append::{SaSymbol, reassemble};
use qrcode_rs::{EcLevel, QrCode, QrError};

/// Re-derives the per-symbol chunks the encoder produces (mirroring
/// `StructuredAppend::encode`: even split via `div_ceil`, clamped start), so we
/// can build `SaSymbol`s without decoding the symbols.
fn split_like_encoder(payload: &[u8], symbols: u8) -> Vec<SaSymbol<'_>> {
    let n = usize::from(symbols);
    let chunk = payload.len().div_ceil(n);
    let parity = payload.iter().fold(0u8, |acc, &byte| acc ^ byte);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let start = (i * chunk).min(payload.len());
        let end = ((i + 1) * chunk).min(payload.len());
        out.push(SaSymbol { position: i as u8 + 1, total: symbols, parity, data: &payload[start..end] });
    }
    out
}

#[test]
fn end_to_end_split_and_reassemble() {
    let payload = b"The quick brown fox jumps over the lazy dog 0123456789";
    let symbols = 3u8;
    let codes = QrCode::structured_append(payload, symbols, EcLevel::M).unwrap();
    assert_eq!(codes.len(), usize::from(symbols));

    let parts = split_like_encoder(payload, symbols);
    assert_eq!(reassemble(&parts).unwrap(), payload);
}

#[test]
fn rejects_invalid_symbol_counts() {
    assert_eq!(
        QrCode::structured_append(b"x", 1, EcLevel::M).err(),
        Some(QrError::InvalidStructuredAppend { value: 1 })
    );
    assert_eq!(
        QrCode::structured_append(b"x", 17, EcLevel::M).err(),
        Some(QrError::InvalidStructuredAppend { value: 17 })
    );
}

#[test]
fn all_versions_are_normal_for_counts_2_to_16() {
    let payload = b"structured append across many symbol counts";
    for symbols in 2u8..=16 {
        let codes = QrCode::structured_append(payload, symbols, EcLevel::L).unwrap();
        assert_eq!(codes.len(), usize::from(symbols), "symbols={symbols}");
        assert!(codes.iter().all(|c| !c.version().is_micro()), "symbols={symbols} produced a Micro QR");
    }
}

#[test]
fn split_rule_round_trips_for_every_count() {
    // For every valid count, the encoder's split rule round-trips through reassemble.
    let payload = b"round-trip the split rule for every symbol count";
    for symbols in 2u8..=16 {
        let parts = split_like_encoder(payload, symbols);
        assert_eq!(reassemble(&parts).unwrap(), payload, "symbols={symbols}");
    }
}
