//! Parse a vCard QR payload into a structured contact.
//!
//! Run: `cargo run --example parse_vcard`

use qrcode_rs::parse::vcard::VCard;

fn main() {
    let payload = "BEGIN:VCARD\nVERSION:3.0\nFN:Ada Lovelace\nORG:Analytical Engine;Math\nTEL:+15551234\nEMAIL:ada@example.org\nURL:https://example.org/ada\nEND:VCARD\n";
    let card = VCard::parse(payload).expect("valid vCard");
    println!("Name:  {:?}", card.name());
    println!("Org:   {:?}", card.organization());
    println!("Phone: {:?}", card.phone());
    println!("Email: {:?}", card.email());
    println!("URL:   {:?}", card.url());
}
