//! PNG contact-sheet benchmarks with encoding outside the measured loop.
//!
//! Run with: `cargo bench --bench batch_grid`

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use qrcode_rs::batch::{BatchEntry, BatchGridOptions, BatchOutput};
use qrcode_rs::render::image::Rgba;
use qrcode_rs::{EcLevel, QrCode, QrTemplate, Version};

fn mixed_codes(count: usize) -> BatchOutput<QrCode> {
    BatchOutput::from_entries((0..count).map(|index| {
        let code = if index % 5 == 0 {
            QrCode::with_version(b"123", Version::Micro(1), EcLevel::L).unwrap()
        } else {
            let version = [Version::Normal(1), Version::Normal(4), Version::Normal(8)][index % 3];
            let ec_level = [EcLevel::L, EcLevel::M, EcLevel::Q, EcLevel::H][index % 4];
            QrCode::with_version(format!("i{index:03}").as_bytes(), version, ec_level).unwrap()
        };
        BatchEntry::new(format!("tile-{index}.png"), code)
    }))
}

fn bench_batch_png_grid(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_png_grid");
    let template = QrTemplate::corporate().with_module_size(3, 5);
    let options = BatchGridOptions::default().background([9, 21, 37, 73]);

    for count in [17, 64] {
        let codes = mixed_codes(count);
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("tile_images", count), &codes, |b, codes| {
            b.iter(|| {
                let tiles = BatchOutput::from_entries(std::hint::black_box(codes).iter().map(|entry| {
                    BatchEntry::new(entry.name(), entry.data().render::<Rgba<u8>>().template(&template).build())
                }));
                std::hint::black_box(tiles.to_png_grid(options).unwrap())
            });
        });
        group.bench_with_input(BenchmarkId::new("direct_qr", count), &codes, |b, codes| {
            b.iter(|| std::hint::black_box(codes).to_png_grid_with(options, &template).unwrap());
        });
    }

    group.finish();
}

criterion_group!(benches, bench_batch_png_grid);
criterion_main!(benches);
