//! Rendering benchmarks (criterion).
//!
//! Run with: `cargo bench --bench rendering`

use criterion::{Criterion, criterion_group, criterion_main};
use qrcode_rs::QrCode;
use qrcode_rs::render::Renderer;

fn bench_render(c: &mut Criterion) {
    let code = QrCode::new(b"https://example.com/qrcode-rs").unwrap();
    let borrowed = code.as_ref();

    let mut g = c.benchmark_group("render");
    g.bench_function("string", |b| b.iter(|| code.render::<char>().dark_color('#').light_color(' ').build()));
    g.bench_function("borrowed_symbol_string", |b| {
        b.iter(|| Renderer::<char>::from_symbol(&borrowed).dark_color('#').light_color(' ').build())
    });
    g.bench_function("unicode_dense1x2", |b| b.iter(|| code.render::<qrcode_rs::render::unicode::Dense1x2>().build()));
    g.bench_function("ansi", |b| b.iter(|| code.render::<qrcode_rs::render::ansi::Color>().build()));

    #[cfg(feature = "svg")]
    {
        use qrcode_rs::render::svg;
        g.bench_function("svg", |b| b.iter(|| code.render::<svg::Color>().build()));
    }

    #[cfg(feature = "image")]
    {
        use image::Rgba;
        g.bench_function("image", |b| b.iter(|| code.render::<Rgba<u8>>().min_dimensions(200, 200).build()));
    }

    #[cfg(feature = "eps")]
    {
        use qrcode_rs::render::eps;
        g.bench_function("eps", |b| b.iter(|| code.render::<eps::Color>().build()));
    }

    #[cfg(feature = "html")]
    {
        use qrcode_rs::render::html;
        g.bench_function("html_table", |b| b.iter(|| code.render::<html::Color>().build()));
    }

    #[cfg(feature = "pic")]
    {
        use qrcode_rs::render::pic;
        g.bench_function("pic", |b| b.iter(|| code.render::<pic::Color>().build()));
    }

    #[cfg(feature = "pdf")]
    {
        use qrcode_rs::render::pdf;
        g.bench_function("pdf_rgb", |b| b.iter(|| code.render::<pdf::Color>().build()));
        g.bench_function("pdf_cmyk", |b| b.iter(|| code.render::<pdf::CmykColor>().build()));
    }

    g.finish();
}

fn bench_batch_packaging(c: &mut Criterion) {
    #[cfg(feature = "std")]
    {
        use qrcode_rs::batch::{BatchEntry, BatchOutput};

        let mut group = c.benchmark_group("batch_zip");
        for length in [128_usize, 32 * 1024, 1024 * 1024] {
            let payload = (0..length).map(|index| (index.wrapping_mul(29) % 256) as u8).collect::<Vec<_>>();
            let batch = BatchOutput::from_entries([BatchEntry::new("二维码.bin", payload)]);
            group.throughput(criterion::Throughput::Bytes(length as u64));
            group.bench_function(format!("stored_{length}"), |b| {
                b.iter(|| std::hint::black_box(&batch).to_zip().unwrap())
            });
        }
        group.finish();
    }
    #[cfg(not(feature = "std"))]
    let _ = c;
}

criterion_group!(benches, bench_render, bench_batch_packaging);
criterion_main!(benches);
