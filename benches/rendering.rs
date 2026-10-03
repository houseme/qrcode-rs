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

        struct BenchmarkOutput(usize);
        impl std::io::Write for BenchmarkOutput {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                // Observe each header/payload slice without retaining an archive.
                let bytes = std::hint::black_box(bytes);
                self.0 += bytes.len();
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let mut group = c.benchmark_group("batch_zip");
        for length in [128_usize, 32 * 1024, 1024 * 1024] {
            let payload = (0..length).map(|index| (index.wrapping_mul(29) % 256) as u8).collect::<Vec<_>>();
            let batch = BatchOutput::from_entries([BatchEntry::new("二维码.bin", payload)]);
            group.throughput(criterion::Throughput::Bytes(length as u64));
            group.bench_function(format!("stored_{length}"), |b| {
                b.iter(|| std::hint::black_box(&batch).to_zip().unwrap())
            });
            group.bench_function(format!("stored_to_writer_{length}"), |b| {
                b.iter(|| {
                    let mut output = BenchmarkOutput(0);
                    std::hint::black_box(&batch).write_zip(&mut output).unwrap();
                    std::hint::black_box(output.0)
                })
            });
        }
        group.finish();
    }
    #[cfg(not(feature = "std"))]
    let _ = c;
}

fn bench_image_rectangles(c: &mut Criterion) {
    #[cfg(feature = "image")]
    {
        use image::{ImageBuffer, Luma, Rgb, Rgba};
        use qrcode_rs::render::Canvas;

        fn bench_pixel<P>(c: &mut Criterion, name: &str, dark: P, light: P)
        where
            P: image::Pixel + 'static,
        {
            let mut group = c.benchmark_group(format!("image_rect/{name}"));
            for width in [1u32, 2, 7, 8, 32] {
                for height in [1u32, 8, 64] {
                    // Allocate once and leave margins so each rectangle spans strided rows.
                    let mut canvas =
                        <(P, ImageBuffer<P, Vec<P::Subpixel>>) as Canvas>::new(width + 6, height + 4, dark, light);
                    group.throughput(criterion::Throughput::Elements(u64::from(width) * u64::from(height)));
                    group.bench_function(format!("{width}x{height}"), |b| {
                        b.iter(|| {
                            std::hint::black_box(&mut canvas).draw_dark_rect(
                                std::hint::black_box(3),
                                std::hint::black_box(2),
                                std::hint::black_box(width),
                                std::hint::black_box(height),
                            );
                        });
                    });
                }
            }
            group.finish();
        }

        bench_pixel(c, "luma8", Luma([0u8]), Luma([255]));
        bench_pixel(c, "rgb8", Rgb([17u8, 43, 89]), Rgb([251, 239, 227]));
        bench_pixel(c, "rgba8", Rgba([17u8, 43, 89, 131]), Rgba([251, 239, 227, 199]));
    }
    #[cfg(not(feature = "image"))]
    let _ = c;
}

criterion_group!(benches, bench_render, bench_batch_packaging, bench_image_rectangles);
criterion_main!(benches);
