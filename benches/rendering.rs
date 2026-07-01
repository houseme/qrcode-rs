//! Rendering benchmarks (criterion).
//!
//! Run with: `cargo bench --bench rendering`

use criterion::{Criterion, criterion_group, criterion_main};
use qrcode_rs::QrCode;

fn bench_render(c: &mut Criterion) {
    let code = QrCode::new(b"https://example.com/qrcode-rs").unwrap();

    let mut g = c.benchmark_group("render");
    g.bench_function("string", |b| b.iter(|| code.render::<char>().dark_color('#').light_color(' ').build()));

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

    g.finish();
}

criterion_group!(benches, bench_render);
criterion_main!(benches);
