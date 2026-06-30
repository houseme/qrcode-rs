//! Demonstrate matching on the enriched `QrError` type.
//!
//! `QrError` variants carry structured context (version/ec-level, ECI value,
//! byte position) so callers can report precisely what went wrong. The enum is
//! `#[non_exhaustive]`, so always include a catch-all arm.
//!
//! Run: `cargo run --example error_handling`

use qrcode_rs::{EcLevel, QrCode, QrError, Version};

fn main() {
    // An incompatible version / error-correction combination.
    match QrCode::with_version(b"0", Version::Micro(2), EcLevel::H) {
        Ok(_) => println!("encoded successfully"),
        Err(QrError::InvalidVersion { version, ec_level }) => {
            println!("cannot use {version:?} with error correction level {ec_level:?}");
        }
        Err(other) => println!("other error: {other}"),
    }

    // Data too long for any version.
    let huge: Vec<u8> = (0..5000).map(|i| (i % 256) as u8).collect();
    match QrCode::new(&huge) {
        Ok(_) => println!("encoded successfully"),
        Err(e) => println!("failed: {e}"),
    }
}
