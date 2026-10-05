#![no_main]

use libfuzzer_sys::fuzz_target;
use qrcode_rs::decode::GrayPixels;
use qrcode_rs::decode::rxing::{RxingDecoder, ScanOptions};

fuzz_target!(|data: &[u8]| {
    if data.len() < 3 {
        return;
    }
    let width = u32::from(data[0] % 128) + 1;
    let height = u32::from(data[1] % 128) + 1;
    let payload = &data[2..];
    let pixels: Vec<_> = payload.iter().copied().cycle().take((width * height) as usize).collect();
    let image = GrayPixels::try_new(width, height, &pixels).unwrap();
    let mut options = ScanOptions::default();
    options.max_pixels = 128 * 128;
    options.max_finder_patterns = 16;
    options.max_results = 8;
    options.try_harder = data[2] & 1 == 0;
    options.inverted = data[2] & 2 != 0;
    let _ = RxingDecoder.scan_with_options(image, options);
});
