use qrcode_rs::render::colors::CmykColor as PrintColor;
use qrcode_rs::render::{eps, pdf};
use qrcode_rs::{EcLevel, QrCode, Version};

fn main() {
    let code = QrCode::with_version(b"https://example.com/print", Version::Normal(3), EcLevel::Q).unwrap();
    let dark = PrintColor::new(0.85, 0.45, 0.0, 0.15);
    let light = PrintColor::new(0.0, 0.0, 0.0, 0.0);

    let eps_output = code
        .render::<eps::CmykColor>()
        .dark_color(dark.into())
        .light_color(light.into())
        .min_dimensions(240, 240)
        .build();

    let pdf_output = code
        .render::<pdf::CmykColor>()
        .dark_color(dark.into())
        .light_color(light.into())
        .min_dimensions(240, 240)
        .build();

    println!("EPS bytes: {}", eps_output.len());
    println!("PDF bytes: {}", pdf_output.len());
}
