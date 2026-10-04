//! Diagnostic analysis benchmarks; symbol construction is outside timing.
//!
//! Run with: `cargo bench --bench analysis`

use criterion::{Criterion, criterion_group, criterion_main};
use qrcode_rs::{EcLevel, QrCode, Version};

fn prepared_codes() -> Vec<(String, QrCode)> {
    let mut codes = Vec::new();
    for version in [1, 2, 7, 10, 20, 32, 40] {
        let code = QrCode::with_version(b"1", Version::Normal(version), EcLevel::L).unwrap();
        codes.push((format!("normal_v{version}"), code));
    }
    for version in 1..=4 {
        let code = QrCode::with_version(b"1", Version::Micro(version), EcLevel::L).unwrap();
        codes.push((format!("micro_m{version}"), code));
    }
    codes
}

fn scan_functional_modules(code: &QrCode) -> usize {
    let width = code.width();
    (0..width).map(|y| (0..width).filter(|&x| code.is_functional(x, y)).count()).sum()
}

fn bench_analysis(c: &mut Criterion) {
    let codes = prepared_codes();
    let mut analysis = c.benchmark_group("analysis");
    for (name, code) in &codes {
        analysis.bench_function(name, |b| b.iter(|| std::hint::black_box(code).analyze()));
    }
    analysis.finish();

    let mut functional = c.benchmark_group("functional_scan");
    for (name, code) in &codes {
        functional.bench_function(name, |b| b.iter(|| scan_functional_modules(std::hint::black_box(code))));
    }
    functional.finish();
}

criterion_group!(benches, bench_analysis);
criterion_main!(benches);
