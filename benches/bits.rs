//! Payload bit-writing benchmarks with allocation outside the timed loop.
//!
//! Run with: `cargo bench --bench bits`

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use qrcode_rs::bits::Bits;
use qrcode_rs::{Mode, Version};

fn prepared_bits(payload_len: usize, prefix_len: usize) -> Bits {
    let mut bits = Bits::new(Version::Normal(40));
    // Reserve a conservative byte-mode upper bound, including a partial byte.
    bits.reserve(payload_len * 8 + prefix_len + 28);
    if prefix_len > 0 {
        bits.push_number_checked(prefix_len, (1 << prefix_len) - 1).unwrap();
    }
    bits
}

fn bench_byte_packing(c: &mut Criterion) {
    let mut group = c.benchmark_group("byte_packing");
    for len in [0, 1, 16, 256, 2048] {
        let payload = (0..len).map(|index| ((index * 73 + 19) % 256) as u8).collect::<Vec<_>>();
        for offset in [0, 4, 7] {
            // A normal version 40 byte header contributes 20 bits.
            let prefix_len = (offset + 4) % 8;
            group.bench_function(format!("len_{len}_offset_{offset}"), |b| {
                b.iter_batched(
                    || prepared_bits(payload.len(), prefix_len),
                    |mut bits| {
                        bits.push_byte_data(std::hint::black_box(&payload)).unwrap();
                        bits
                    },
                    BatchSize::SmallInput,
                )
            });
        }
    }
    group.finish();
}

fn bench_other_modes(c: &mut Criterion) {
    let mut group = c.benchmark_group("mode_write_control");
    for (name, mode) in [("numeric", Mode::Numeric), ("alphanumeric", Mode::Alphanumeric), ("kanji", Mode::Kanji)] {
        for len in [16, 1024] {
            let payload = match mode {
                Mode::Numeric => (0..len).map(|index| b'0' + (index % 10) as u8).collect::<Vec<_>>(),
                Mode::Alphanumeric => (0..len).map(|index| b"AB12-CD34"[index % 9]).collect::<Vec<_>>(),
                Mode::Kanji => b"\x93\x5f".repeat(len),
                Mode::Byte => unreachable!(),
            };
            group.bench_function(format!("{name}_{len}"), |b| {
                b.iter_batched(
                    || prepared_bits(payload.len(), 0),
                    |mut bits| {
                        let data = std::hint::black_box(&payload);
                        match mode {
                            Mode::Numeric => bits.push_numeric_data(data),
                            Mode::Alphanumeric => bits.push_alphanumeric_data(data),
                            Mode::Kanji => bits.push_kanji_data(data),
                            Mode::Byte => unreachable!(),
                        }
                        .unwrap();
                        bits
                    },
                    BatchSize::SmallInput,
                )
            });
        }
    }
    group.finish();
}

fn bench_byte_packing_control(c: &mut Criterion) {
    #[cfg(feature = "bench-internals")]
    {
        let mut group = c.benchmark_group("byte_packing_control");
        for len in [1, 256, 2048] {
            let payload = (0..len).map(|index| ((index * 73 + 19) % 256) as u8).collect::<Vec<_>>();
            for offset in [0, 4, 7] {
                let prefix_len = (offset + 4) % 8;
                for scalar in [false, true] {
                    let implementation = if scalar { "scalar" } else { "current" };
                    group.bench_function(format!("len_{len}_offset_{offset}_{implementation}"), |b| {
                        b.iter_batched(
                            || prepared_bits(payload.len(), prefix_len),
                            |mut bits| {
                                let data = std::hint::black_box(&payload);
                                if scalar {
                                    bits.push_byte_data_scalar_for_bench(data).unwrap();
                                } else {
                                    bits.push_byte_data(data).unwrap();
                                }
                                bits
                            },
                            BatchSize::SmallInput,
                        )
                    });
                }
            }
        }
        group.finish();
    }
    #[cfg(not(feature = "bench-internals"))]
    let _ = c;
}

criterion_group!(benches, bench_byte_packing, bench_other_modes, bench_byte_packing_control);
criterion_main!(benches);
