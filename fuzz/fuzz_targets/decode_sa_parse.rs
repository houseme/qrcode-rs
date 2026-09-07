#![no_main]

use libfuzzer_sys::fuzz_target;
use qrcode_rs::Version;
use qrcode_rs::decode::sa_parse::parse_sa_datastream;

// Feeds arbitrary decoder output to the Structured Append parser.  The first
// byte selects a valid normal-QR version so the remaining bytes exercise the
// header, segment counters, truncation, and terminator paths.
fuzz_target!(|data: &[u8]| {
    let Some((&selector, stream)) = data.split_first() else {
        return;
    };
    let version = Version::Normal(i16::from(selector % 40) + 1);
    let _ = parse_sa_datastream(stream, version);

    // Keep a second, structured input path so mutations reach segment
    // decoders quickly instead of needing to rediscover the 20-bit header on
    // every run. The count is fixed to one alphanumeric character; the input
    // controls its six-bit value and the remaining bytes exercise terminators
    // and truncation after that segment.
    let value = stream.first().copied().unwrap_or_default() & 0x3f;
    let mut focused = Vec::with_capacity(stream.len() + 5);
    focused.extend_from_slice(&[0x31, 0x20, 0x02, 0x00, 0x80 | (value << 1)]);
    focused.extend_from_slice(stream.get(1..).unwrap_or_default());
    let _ = parse_sa_datastream(&focused, version);
});
