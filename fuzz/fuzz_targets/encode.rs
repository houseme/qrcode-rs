#![no_main]

use libfuzzer_sys::fuzz_target;
use qrcode_rs::QrCode;

fuzz_target!(|data: &[u8]| {
    let _ = QrCode::new(data);
});
