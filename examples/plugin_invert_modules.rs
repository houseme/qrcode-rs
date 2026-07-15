use qrcode_rs::render::plugin::{InvertModulesPlugin, PlainTextRendererPlugin};
use qrcode_rs::{PluginRegistry, QrCode, RenderConfig, RenderOutput};

fn main() {
    let code = QrCode::new(b"https://example.com/plugin-invert").unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });

    let mut registry = PluginRegistry::new();
    registry.register_plugin(&PlainTextRendererPlugin);
    registry.register_plugin(&InvertModulesPlugin);

    let config = RenderConfig::new().with_option("dark", "#").with_option("light", " ").with_option("quiet_zone", "2");
    let output = code.render_with(&registry, PlainTextRendererPlugin::RENDERER_NAME, &config).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });

    if let RenderOutput::Text(text) = output {
        println!("{text}");
    }
}
