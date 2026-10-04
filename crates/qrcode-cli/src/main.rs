//! `qrencodes` — command-line QR code generator.

fn main() -> std::process::ExitCode {
    qrcode_rs::cli_main()
}
