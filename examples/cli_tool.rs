//! A minimal programmatic QR generator using `clap` (mirrors the `qrencodes` CLI).
//!
//! Requires the `cli` feature (which provides `clap`).
//!
//! Run: `cargo run --example cli_tool --features cli -- "hello"`

use clap::Parser;
use qrcode_rs::QrCode;

#[derive(Parser)]
#[command(name = "cli-tool", about = "Generate a QR code from the command line")]
struct Cli {
    /// Text to encode.
    text: String,
}

fn main() {
    let cli = Cli::parse();
    let code = QrCode::new(cli.text.as_bytes()).unwrap();
    println!("{}", code.render().dark_color('#').light_color(' ').build());
}
