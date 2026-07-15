#![no_main]

use libfuzzer_sys::fuzz_target;
use qrcode_rs::EcLevel;
use qrcode_rs::structured_append::StructuredAppend;

fuzz_target!(|data: &[u8]| {
    let Some((&selector, payload)) = data.split_first() else {
        return;
    };
    let symbols = selector % 15 + 2;
    if let Ok(sequence) = StructuredAppend::new(symbols, payload) {
        let _ = sequence.encode(EcLevel::M);
    }
});
