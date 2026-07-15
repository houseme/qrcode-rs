//! Encoding benchmarks (criterion).
//!
//! Run with: `cargo bench --bench encoding`

use criterion::{Criterion, criterion_group, criterion_main};
use qrcode_rs::bits::Bits;
use qrcode_rs::canvas::Canvas;
#[cfg(feature = "bench-internals")]
use qrcode_rs::canvas::MaskPattern;
use qrcode_rs::ec;
use qrcode_rs::structured_append::StructuredAppend;
use qrcode_rs::{EcLevel, QrCode, Version};

fn prepared_canvas(version: Version, ec_level: EcLevel, payload: &[u8]) -> Canvas {
    let mut bits = Bits::new(version);
    bits.push_byte_data(payload).unwrap();
    bits.push_terminator(ec_level).unwrap();
    let data = bits.into_bytes();
    let (encoded_data, ec_data) = ec::construct_codewords(&data, version, ec_level).unwrap();

    let mut canvas = Canvas::new(version, ec_level);
    canvas.draw_all_functional_patterns();
    canvas.draw_data(&encoded_data, &ec_data);
    canvas
}

#[cfg(feature = "bench-internals")]
fn prepared_mask_candidates(version: Version, ec_level: EcLevel, payload: &[u8]) -> Vec<Canvas> {
    const ALL_PATTERNS_QR: [MaskPattern; 8] = [
        MaskPattern::Checkerboard,
        MaskPattern::HorizontalLines,
        MaskPattern::VerticalLines,
        MaskPattern::DiagonalLines,
        MaskPattern::LargeCheckerboard,
        MaskPattern::Fields,
        MaskPattern::Diamonds,
        MaskPattern::Meadow,
    ];

    ALL_PATTERNS_QR
        .iter()
        .map(|&pattern| {
            let mut canvas = prepared_canvas(version, ec_level, payload);
            canvas.apply_mask(pattern);
            canvas
        })
        .collect()
}

fn bench_encode(c: &mut Criterion) {
    let medium: Vec<u8> = (0..200).map(|i| (i % 256) as u8).collect();
    let long: Vec<u8> = (0..2000).map(|i| (i % 256) as u8).collect();

    let mut g = c.benchmark_group("encode");
    g.bench_function("short", |b| b.iter(|| QrCode::new(std::hint::black_box(b"hello")).unwrap()));
    g.bench_function("medium", |b| b.iter(|| QrCode::new(std::hint::black_box(&medium)).unwrap()));
    g.bench_function("long", |b| b.iter(|| QrCode::new(std::hint::black_box(&long)).unwrap()));

    // Fixed tiny payload at increasing versions to measure canvas/mask scaling.
    for v in [1_i16, 10, 20, 30, 40] {
        g.bench_function(format!("v{v}"), |b| {
            b.iter(|| QrCode::with_version(std::hint::black_box(b"hello"), Version::Normal(v), EcLevel::L).unwrap())
        });
    }

    // Batch encoding of 1000 short inputs.
    let batch_inputs: Vec<&[u8]> = (0..1000).map(|_| &b"hi"[..]).collect();
    g.bench_function("batch_1000", |b| {
        b.iter(|| QrCode::batch(std::hint::black_box(&batch_inputs), EcLevel::M).unwrap())
    });
    g.finish();

    let fixed_payload = b"https://example.com/fixed-version";
    let mut fixed = c.benchmark_group("fixed_version");
    fixed.bench_function("dynamic_v5", |b| {
        b.iter(|| QrCode::with_version(std::hint::black_box(fixed_payload), Version::Normal(5), EcLevel::M).unwrap())
    });
    fixed.bench_function("const_v5", |b| {
        b.iter(|| QrCode::with_const_version::<5, _>(std::hint::black_box(fixed_payload), EcLevel::M).unwrap())
    });
    fixed.finish();

    let mut mask = c.benchmark_group("mask_selection");
    for v in [1_i16, 10, 20, 30, 40] {
        let canvas = prepared_canvas(Version::Normal(v), EcLevel::L, b"mask");
        mask.bench_function(format!("v{v}_apply_best_mask"), |b| b.iter(|| canvas.apply_best_mask()));
    }
    mask.finish();

    #[cfg(feature = "bench-internals")]
    {
        let mut mask_score = c.benchmark_group("mask_scoring");
        for v in [1_i16, 10, 20, 30, 40] {
            let candidates = prepared_mask_candidates(Version::Normal(v), EcLevel::L, b"mask");
            mask_score.bench_function(format!("v{v}_accelerated"), |b| {
                let mut scratch = Vec::new();
                b.iter(|| {
                    let mut best_score = u16::MAX;
                    for canvas in &candidates {
                        best_score = best_score.min(canvas.score_mask_for_bench(std::hint::black_box(&mut scratch)));
                    }
                    std::hint::black_box(best_score)
                })
            });

            mask_score.bench_function(format!("v{v}_scalar"), |b| {
                let mut scratch = Vec::new();
                b.iter(|| {
                    let mut best_score = u16::MAX;
                    for canvas in &candidates {
                        best_score =
                            best_score.min(canvas.score_mask_scalar_for_bench(std::hint::black_box(&mut scratch)));
                    }
                    std::hint::black_box(best_score)
                })
            });
        }
        mask_score.finish();
    }

    // Structured Append: split a payload across N symbols (per-symbol version
    // search via the tier-based path).
    let sa_payload: Vec<u8> = (0..400).map(|i| (i % 256) as u8).collect();
    let mut g = c.benchmark_group("structured_append");
    for &n in &[2_usize, 8, 16] {
        g.bench_function(format!("encode_{n}"), |b| {
            b.iter(|| {
                StructuredAppend::new(n as u8, std::hint::black_box(&sa_payload)).unwrap().encode(EcLevel::M).unwrap()
            })
        });
    }
    g.finish();
}

criterion_group!(benches, bench_encode);
criterion_main!(benches);
