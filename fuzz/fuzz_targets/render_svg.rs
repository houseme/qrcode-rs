#![no_main]

use libfuzzer_sys::fuzz_target;
use qrcode_rs::QrCode;
use qrcode_rs::render::svg;

fuzz_target!(|data: &[u8]| {
    if let Ok(code) = QrCode::new(data) {
        let _ = code.render::<svg::Color<'static>>().build();
    }
});
