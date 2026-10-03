//! Isolated Reed-Solomon and block-interleaving benchmarks.
//!
//! Run with: `cargo bench --bench ec`

use criterion::{Criterion, criterion_group, criterion_main};
use qrcode_rs::bits::Bits;
use qrcode_rs::ec::{construct_codewords, create_error_correction_code};
use qrcode_rs::{EcLevel, Version};

fn padded_payload(version: Version, ec_level: EcLevel) -> Vec<u8> {
    let mut bits = Bits::new(version);
    bits.push_numeric_data(b"1").unwrap();
    bits.push_terminator(ec_level).unwrap();
    bits.into_bytes()
}

fn bench_remainders(c: &mut Criterion) {
    let mut group = c.benchmark_group("rs_remainder");
    for (name, data_len, degree) in [
        ("micro_m1", 3, 2),
        ("micro_m4_q", 10, 14),
        ("normal_v1_m", 16, 10),
        ("large_block", 123, 30),
        ("public_max_degree", 128, 69),
    ] {
        let mixed = (0..data_len).map(|index| ((index * 73 + 19) % 256) as u8).collect::<Vec<_>>();
        let zero = vec![0; data_len];
        for (pattern, data) in [("mixed", mixed), ("zero", zero)] {
            group.bench_function(format!("{name}_{pattern}"), |b| {
                b.iter(|| create_error_correction_code(std::hint::black_box(&data), degree))
            });
        }
    }
    group.finish();
}

fn bench_codewords(c: &mut Criterion) {
    let mut group = c.benchmark_group("codewords");
    for (name, version, ec_level) in [
        ("normal_v1_m", Version::Normal(1), EcLevel::M),
        ("normal_v5_q", Version::Normal(5), EcLevel::Q),
        ("normal_v10_l", Version::Normal(10), EcLevel::L),
        ("normal_v20_h", Version::Normal(20), EcLevel::H),
        ("normal_v40_l", Version::Normal(40), EcLevel::L),
        ("normal_v40_m", Version::Normal(40), EcLevel::M),
        ("normal_v40_q", Version::Normal(40), EcLevel::Q),
        ("normal_v40_h", Version::Normal(40), EcLevel::H),
        ("micro_m1_l", Version::Micro(1), EcLevel::L),
        ("micro_m2_m", Version::Micro(2), EcLevel::M),
        ("micro_m3_l", Version::Micro(3), EcLevel::L),
        ("micro_m3_m", Version::Micro(3), EcLevel::M),
        ("micro_m4_q", Version::Micro(4), EcLevel::Q),
    ] {
        let data = padded_payload(version, ec_level);
        group.bench_function(name, |b| {
            b.iter(|| construct_codewords(std::hint::black_box(&data), version, ec_level).unwrap())
        });
    }
    group.finish();
}

fn bench_remainder_control(c: &mut Criterion) {
    #[cfg(feature = "bench-internals")]
    {
        use qrcode_rs::ec::create_error_correction_code_modulo_for_bench;

        let mut group = c.benchmark_group("rs_remainder_control");
        for (name, data_len, degree) in
            [("micro_m1", 3, 2), ("normal_v1_m", 16, 10), ("large_block", 123, 30), ("public_max_degree", 128, 69)]
        {
            let mixed = (0..data_len).map(|index| ((index * 73 + 19) % 256) as u8).collect::<Vec<_>>();
            let zero = vec![0; data_len];
            for (pattern, data) in [("mixed", mixed), ("zero", zero)] {
                group.bench_function(format!("{name}_{pattern}_current"), |b| {
                    b.iter(|| create_error_correction_code(std::hint::black_box(&data), degree))
                });
                group.bench_function(format!("{name}_{pattern}_modulo"), |b| {
                    b.iter(|| create_error_correction_code_modulo_for_bench(std::hint::black_box(&data), degree))
                });
            }
        }
        group.finish();
    }
    #[cfg(not(feature = "bench-internals"))]
    let _ = c;
}

criterion_group!(benches, bench_remainders, bench_codewords, bench_remainder_control);
criterion_main!(benches);
