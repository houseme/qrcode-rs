#![no_main]

use libfuzzer_sys::fuzz_target;
use qrcode_rs::parse::{gs1, vcard, wifi};

fuzz_target!(|data: &[u8]| {
    let _ = gs1::Gs1Result::parse(data);
    if let Ok(text) = core::str::from_utf8(data) {
        let _ = wifi::WifiConfig::parse(text);
        let _ = vcard::VCard::parse(text);
    }
});
