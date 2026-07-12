//! Parse a `WIFI:` QR payload into a structured WiFi configuration.
//!
//! Run: `cargo run --example parse_wifi`

use qrcode_rs::parse::wifi::WifiConfig;

fn main() {
    // The password `p;ss` contains a semicolon, escaped as `\;` per the spec.
    let payload = r#"WIFI:T:WPA;S:GuestNetwork;P:p\;ss;H:false;;"#;
    let cfg = WifiConfig::parse(payload).expect("valid WiFi payload");
    println!("SSID:     {}", cfg.ssid());
    println!("Password: {:?}", cfg.password());
    println!("Security: {}", cfg.security().as_str());
    println!("Hidden:   {}", cfg.hidden());
}
