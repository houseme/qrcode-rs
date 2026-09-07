#![no_main]

use libfuzzer_sys::fuzz_target;
use qrcode_image::{RenderOptions, RgbaImage, render_rgba};
use qrcode_rs::QrCode;

// Exercises the image backend with bounded dimensions.  A successful QR
// encoding is at most version 40 (177 modules), so the largest image this
// target can allocate remains bounded while still covering every renderer
// branch selected by the input.
fuzz_target!(|data: &[u8]| {
    let Some((&selector, payload)) = data.split_first() else {
        return;
    };
    let Ok(code) = QrCode::new(payload) else {
        return;
    };

    let module_size = u32::from(selector % 8) + 1;
    let options = RenderOptions::default()
        .module_size(module_size, module_size)
        .quiet_zone(u32::from(selector % 5));
    let image: Result<RgbaImage, _> = render_rgba(&code, options);
    let _ = image;
});
