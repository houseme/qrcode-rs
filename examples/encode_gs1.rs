//! Encode a GS1 data carrier (FNC1 in first position), e.g. a GTIN with
//! application identifiers.
//!
//! Run: `cargo run --example encode_gs1`

use qrcode_rs::QrCode;

fn main() {
    // (01) GTIN + (15) best-before + (10) lot, etc.
    let code = QrCode::for_gs1("010491234512345915970331301234561842").unwrap();
    println!("{}", code.render().dark_color('#').light_color(' ').build());
}
