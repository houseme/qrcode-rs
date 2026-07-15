use qrcode_rs::QrCode;

fn main() {
    let code = QrCode::new(b"https://example.com/async-render").unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });
    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });
    let text = runtime.block_on(code.render_async::<char>()).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });

    println!("rendered {} chars asynchronously", text.len());
}
