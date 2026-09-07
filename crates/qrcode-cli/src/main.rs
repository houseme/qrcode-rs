//! `qrencodes` — command-line QR code generator.

use std::error::Error;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

use clap::{Parser, ValueEnum};
use qrcode_render::{ansi, colors, unicode};
use qrcode_rs::{EcLevel, QrCode, Version};

#[derive(Parser)]
#[command(name = "qrencodes", version, about = "Generate QR codes in various output formats")]
struct Cli {
    /// Text to encode. If omitted, reads from stdin (when piped or redirected).
    text: Option<String>,
    /// Write output to `<FILE>` instead of stdout ("-" means stdout).
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,
    /// Output format.
    #[arg(short, long, value_enum, default_value_t = Format::Unicode)]
    format: Format,
    /// Error correction level: L, M, Q or H.
    #[arg(short = 'e', long, default_value = "M", value_parser = parse_ec_level)]
    ec_level: EcLevel,
    /// QR version (1-40) or Micro QR (M1-M4). Omit for automatic selection.
    #[arg(short = 'v', long = "qr-version", value_name = "VERSION", value_parser = parse_version)]
    qr_version: Option<Version>,
    /// Module size in pixels (raster formats only).
    #[arg(short, long, default_value_t = 10)]
    size: u32,
    /// Disable the quiet zone (it is included by default).
    #[arg(long)]
    no_quiet_zone: bool,
    /// Dark module color as a CSS hex string.
    #[arg(long, default_value = "#000000")]
    dark: String,
    /// Light module color as a CSS hex string.
    #[arg(long, default_value = "#ffffff")]
    light: String,
    /// Swap the dark and light colors.
    #[arg(long)]
    invert: bool,
    /// Unicode renderer sub-mode.
    #[arg(long, value_enum, default_value_t = UnicodeMode::Dense1x2)]
    unicode_mode: UnicodeMode,
    /// Generate one QR code per non-empty line of `<FILE>`.
    #[arg(long, value_name = "FILE")]
    batch: Option<PathBuf>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Format {
    String,
    Unicode,
    Ansi,
    Svg,
    Png,
    Eps,
    Pic,
    Html,
    Pdf,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum UnicodeMode {
    #[value(name = "dense1x2")]
    Dense1x2,
    #[value(name = "dense2x2")]
    Dense2x2,
    #[value(name = "dense3x2")]
    Dense3x2,
    #[value(name = "braille")]
    Braille,
}

fn parse_ec_level(s: &str) -> Result<EcLevel, String> {
    EcLevel::from_str(s).map_err(|e| e.to_string())
}

fn parse_version(s: &str) -> Result<Version, String> {
    Version::from_str(s).map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    let inputs = read_inputs(&cli)?;
    let quiet_zone = !cli.no_quiet_zone;
    let batch = cli.batch.is_some();
    if batch && cli.output.is_none() {
        return Err("batch mode requires --output <DIR>".into());
    }
    for (index, text) in inputs.iter().enumerate() {
        let bytes = render_one(text, &cli, quiet_zone)?;
        write_output(&cli, &bytes, index, batch)?;
    }
    Ok(())
}

fn read_inputs(cli: &Cli) -> Result<Vec<String>, Box<dyn Error>> {
    if let Some(path) = &cli.batch {
        let content = std::fs::read_to_string(path)?;
        return Ok(content.lines().filter(|line| !line.trim().is_empty()).map(String::from).collect());
    }
    if let Some(text) = &cli.text {
        return Ok(vec![text.clone()]);
    }
    if std::io::stdin().is_terminal() {
        return Err("no input: pass TEXT or pipe data via stdin".into());
    }
    let mut buf = String::new();
    std::io::stdin().lock().read_to_string(&mut buf)?;
    if buf.ends_with('\n') {
        buf.pop();
    }
    if buf.ends_with('\r') {
        buf.pop();
    }
    Ok(vec![buf])
}

fn render_one(text: &str, cli: &Cli, quiet_zone: bool) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut builder = QrCode::builder(text.as_bytes()).ec_level(cli.ec_level);
    if let Some(version) = cli.qr_version {
        builder = builder.version(version);
    }
    let code = builder.build()?;
    let (dark_str, light_str) = if cli.invert { (&cli.light, &cli.dark) } else { (&cli.dark, &cli.light) };
    let needs_rgb = matches!(cli.format, Format::Ansi | Format::Png | Format::Eps | Format::Pdf);
    let (dark_rgb, light_rgb) =
        if needs_rgb { (parse_rgb(dark_str, "dark")?, parse_rgb(light_str, "light")?) } else { ((0, 0, 0), (0, 0, 0)) };
    let bytes = match cli.format {
        Format::String => {
            code.render::<char>().quiet_zone(quiet_zone).dark_color('#').light_color(' ').build().into_bytes()
        }
        Format::Unicode => unicode_render(&code, cli.unicode_mode, quiet_zone).into_bytes(),
        Format::Ansi => code
            .render::<ansi::Color>()
            .quiet_zone(quiet_zone)
            .dark_color(ansi::Color::new(dark_rgb.0, dark_rgb.1, dark_rgb.2))
            .light_color(ansi::Color::new(light_rgb.0, light_rgb.1, light_rgb.2))
            .build()
            .into_bytes(),
        Format::Svg => code
            .render::<qrcode_svg::Color>()
            .quiet_zone(quiet_zone)
            .dark_color(qrcode_svg::Color(dark_str.as_str()))
            .light_color(qrcode_svg::Color(light_str.as_str()))
            .build()
            .into_bytes(),
        Format::Png => {
            use qrcode_image::{DynamicImage, ImageFormat, Rgba};
            let image = code
                .render::<Rgba<u8>>()
                .quiet_zone(quiet_zone)
                .module_dimensions(cli.size, cli.size)
                .dark_color(Rgba([dark_rgb.0, dark_rgb.1, dark_rgb.2, 255]))
                .light_color(Rgba([light_rgb.0, light_rgb.1, light_rgb.2, 255]))
                .build();
            qrcode_image::encode_to_format(&DynamicImage::ImageRgba8(image), ImageFormat::Png)?
        }
        Format::Eps => code
            .render::<qrcode_eps::Color>()
            .quiet_zone(quiet_zone)
            .dark_color(qrcode_eps::Color(to_unit(&dark_rgb)))
            .light_color(qrcode_eps::Color(to_unit(&light_rgb)))
            .build()
            .into_bytes(),
        Format::Pic => code.render::<qrcode_pic::Color>().quiet_zone(quiet_zone).build().into_bytes(),
        Format::Html => code
            .render::<qrcode_html::Color>()
            .quiet_zone(quiet_zone)
            .dark_color(qrcode_html::Color(dark_str.as_str()))
            .light_color(qrcode_html::Color(light_str.as_str()))
            .build()
            .into_bytes(),
        Format::Pdf => code
            .render::<qrcode_pdf::Color>()
            .quiet_zone(quiet_zone)
            .dark_color(qrcode_pdf::Color(to_unit(&dark_rgb)))
            .light_color(qrcode_pdf::Color(to_unit(&light_rgb)))
            .build(),
    };
    Ok(bytes)
}

fn unicode_render(code: &QrCode, mode: UnicodeMode, quiet_zone: bool) -> String {
    match mode {
        UnicodeMode::Dense1x2 => code.render::<unicode::Dense1x2>().quiet_zone(quiet_zone).build(),
        UnicodeMode::Dense2x2 => code.render::<unicode::Dense2x2>().quiet_zone(quiet_zone).build(),
        UnicodeMode::Dense3x2 => code.render::<unicode::Dense3x2>().quiet_zone(quiet_zone).build(),
        UnicodeMode::Braille => code.render::<unicode::Braille>().quiet_zone(quiet_zone).build(),
    }
}

fn parse_rgb(s: &str, which: &str) -> Result<(u8, u8, u8), Box<dyn Error>> {
    colors::hex_to_rgb(s).ok_or_else(|| format!("invalid {which} color '{s}' (expected #rgb or #rrggbb)").into())
}

fn to_unit(&(r, g, b): &(u8, u8, u8)) -> [f64; 3] {
    [r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0]
}

fn write_output(cli: &Cli, bytes: &[u8], index: usize, batch: bool) -> Result<(), Box<dyn Error>> {
    if batch {
        let dir = cli.output.as_ref().expect("batch requires --output, checked in run()");
        std::fs::create_dir_all(dir)?;
        let path = Path::new(dir).join(format!("qr-{:04}.{}", index + 1, ext_for(cli.format)));
        std::fs::write(&path, bytes)?;
        eprintln!("wrote {}", path.display());
        return Ok(());
    }
    match &cli.output {
        Some(path) if path == "-" => std::io::stdout().lock().write_all(bytes)?,
        Some(path) => std::fs::write(path, bytes)?,
        None => std::io::stdout().lock().write_all(bytes)?,
    }
    Ok(())
}

fn ext_for(format: Format) -> &'static str {
    match format {
        Format::Png => "png",
        Format::Svg => "svg",
        Format::Eps => "eps",
        Format::Pdf => "pdf",
        Format::Html => "html",
        Format::Pic => "pic",
        Format::String | Format::Unicode | Format::Ansi => "txt",
    }
}
