use qrcode_rs::{EcLevel, QrCode};

fn main() {
    let inputs = ["alpha", "beta", "gamma"];

    for item in QrCode::stream_with_error_correction_level(inputs, EcLevel::H) {
        let code = item.unwrap_or_else(|err| {
            eprintln!("{err}");
            std::process::exit(1);
        });
        let text = code.render::<char>().quiet_zone(false).module_dimensions(1, 1).build();
        println!("{} modules: {}", code.width(), text.lines().next().unwrap_or_default());
    }
}
