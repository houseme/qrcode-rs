//! Parse a GS1 application-identifier payload into its elements.
//!
//! Run: `cargo run --example parse_gs1`

use qrcode_rs::parse::gs1::Gs1Result;

fn main() {
    // GTIN (01) + best-before date (15), then a variable-length lot (10)
    // separated from a variable-length serial (21) by the GS byte 0x1D.
    let mut data = b"010491234512345915970331".to_vec();
    data.extend_from_slice(b"10LOT-7");
    data.push(0x1D); // GS separator between variable-length AIs
    data.extend_from_slice(b"21SN-0001");

    let result = Gs1Result::parse(&data).expect("valid GS1 payload");
    println!("Parsed {} element(s):", result.elements().len());
    for el in result.elements() {
        let value = std::str::from_utf8(el.value()).unwrap_or("<non-utf8>");
        println!("  AI {:>4} | {:<42} | {}", el.ai(), el.description(), value);
    }
}
