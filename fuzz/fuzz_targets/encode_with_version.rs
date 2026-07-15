#![no_main]

use libfuzzer_sys::fuzz_target;
use qrcode_rs::{EcLevel, QrCode, Version};

fuzz_target!(|data: &[u8]| {
    let Some((&selector, payload)) = data.split_first() else {
        return;
    };
    let version = Version::Normal(i16::from(selector % 40) + 1);
    let ec = match selector % 4 {
        0 => EcLevel::L,
        1 => EcLevel::M,
        2 => EcLevel::Q,
        _ => EcLevel::H,
    };
    let _ = QrCode::with_version(payload, version, ec);
});
