//! Encoding benchmarks (criterion).
//!
//! Run with: `cargo bench --bench encoding`

use criterion::{Criterion, criterion_group, criterion_main};
use qrcode_rs::{EcLevel, QrCode, Version};

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
    g.finish();
}

criterion_group!(benches, bench_encode);
criterion_main!(benches);
