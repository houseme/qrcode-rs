//! `qrencodes` — command-line QR code generator.

use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::{Parser, Subcommand, ValueEnum};
use qrcode_render::{ansi, colors, unicode};
use qrcode_rs::decode::rqrr::RqrrDecoder;
use qrcode_rs::decode::{GrayPixels, QrDecoder};
use qrcode_rs::{EcLevel, QrCode, QrSymbol, Version};
use rayon::prelude::*;

const MAX_PNG_SIDE: u64 = 65_535;
const MAX_PNG_PIXELS: u64 = 268_435_456;
static TEMP_OUTPUT_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

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
    /// Generate QR codes from non-empty records in `<FILE>` (`-` reads stdin).
    #[arg(long, value_name = "FILE")]
    batch: Option<PathBuf>,
    /// Batch record format.
    #[arg(long, value_enum, default_value_t = BatchFormat::Lines)]
    batch_format: BatchFormat,
    /// 1-based CSV column to encode when `--batch-format csv` is used.
    #[arg(long, default_value_t = 1, value_parser = parse_positive_usize)]
    batch_column: usize,
    /// JSON object key to encode when `--batch-format json` or `jsonl` is used.
    #[arg(long, default_value = "text")]
    batch_key: String,
    /// Render batch records in parallel while preserving output file order.
    #[arg(long)]
    parallel: bool,
    /// Batch output packaging.
    #[arg(long, value_enum, default_value_t = BatchPack::Directory)]
    batch_pack: BatchPack,
    /// Number of columns for `--batch-pack grid` (`0` chooses automatically).
    #[arg(long, default_value_t = 0)]
    grid_columns: usize,
    /// Extra commands such as image validation/decoding.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum BatchFormat {
    Lines,
    Csv,
    Json,
    Jsonl,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum BatchPack {
    Directory,
    Zip,
    Grid,
}

#[derive(Subcommand)]
enum Command {
    /// Decode and validate QR codes from an image file.
    Validate {
        /// Image file to decode.
        image: PathBuf,
        /// Require at least one decoded payload to equal this text.
        #[arg(long)]
        expect: Option<String>,
        /// Print decoded payload bytes as UTF-8 lossily.
        #[arg(long)]
        print_payload: bool,
    },
}

fn parse_ec_level(s: &str) -> Result<EcLevel, String> {
    EcLevel::from_str(s).map_err(|e| e.to_string())
}

fn parse_version(s: &str) -> Result<Version, String> {
    Version::from_str(s).map_err(|e| e.to_string())
}

fn parse_positive_usize(s: &str) -> Result<usize, String> {
    let value = s.parse::<usize>().map_err(|_| format!("invalid positive integer '{s}'"))?;
    if value == 0 {
        return Err("value must be greater than zero".to_owned());
    }
    Ok(value)
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
    if let Some(command) = &cli.command {
        return run_command(command);
    }
    if cli.batch.is_some() && cli.text.is_some() {
        return Err("--batch cannot be used together with TEXT".into());
    }
    if cli.batch.is_some() && cli.output.as_deref() == Some("-") {
        return Err("batch mode requires --output to name a directory or batch output file".into());
    }
    if cli.parallel && cli.batch.is_none() {
        return Err("--parallel requires --batch".into());
    }
    if matches!(cli.format, Format::Png) && cli.size == 0 {
        return Err("--size must be greater than zero for PNG output".into());
    }
    if cli.batch_pack == BatchPack::Grid && !matches!(cli.format, Format::Png) {
        return Err("--batch-pack grid requires --format png".into());
    }

    let quiet_zone = !cli.no_quiet_zone;
    let batch = cli.batch.is_some();
    if batch && cli.output.is_none() {
        return Err("batch mode requires --output <DIR|FILE>".into());
    }
    if batch {
        let written = render_batch(&cli, quiet_zone)?;
        if written == 0 {
            return Err("no non-empty input records found".into());
        }
    } else {
        let text = read_single_input(&cli)?;
        let bytes = render_one(&text, &cli, quiet_zone)?;
        write_output(&cli, &bytes, 0, false)?;
    }
    Ok(())
}

fn run_command(command: &Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::Validate { image, expect, print_payload } => validate_image(image, expect.as_deref(), *print_payload),
    }
}

fn read_single_input(cli: &Cli) -> Result<String, Box<dyn Error>> {
    if let Some(text) = &cli.text {
        return Ok(text.clone());
    }
    if std::io::stdin().is_terminal() {
        return Err("no input: pass TEXT or pipe data via stdin".into());
    }
    read_stdin()
}

fn render_batch(cli: &Cli, quiet_zone: bool) -> Result<usize, Box<dyn Error>> {
    if cli.batch_pack == BatchPack::Grid {
        return render_batch_grid(cli, quiet_zone);
    }
    if cli.batch_pack == BatchPack::Zip {
        return render_batch_zip(cli, quiet_zone);
    }

    if cli.parallel || cli.batch_format == BatchFormat::Json {
        let inputs = read_inputs(cli)?;
        if cli.parallel {
            let rendered = render_many_parallel(&inputs, cli, quiet_zone)?;
            for (index, bytes) in rendered.iter().enumerate() {
                write_output(cli, bytes, index, true)?;
            }
        } else {
            for (index, text) in inputs.iter().enumerate() {
                let bytes = render_one(text, cli, quiet_zone)?;
                write_output(cli, &bytes, index, true)?;
            }
        }
        return Ok(inputs.len());
    }

    let Some(path) = &cli.batch else {
        return Ok(0);
    };
    let mut written = 0;
    let mut source = open_record_source(path)?;
    let mut line_no = 0;
    let mut line = String::new();
    while let Some(text) = read_batch_payload(&mut *source, &mut line, &mut line_no, cli)? {
        let bytes = render_one(&text, cli, quiet_zone)?;
        write_output(cli, &bytes, written, true)?;
        written += 1;
    }
    Ok(written)
}

fn render_batch_grid(cli: &Cli, quiet_zone: bool) -> Result<usize, Box<dyn Error>> {
    let Some(output) = &cli.output else {
        return Err("batch mode requires --output <DIR|FILE>".into());
    };
    let inputs = read_inputs(cli)?;
    if inputs.is_empty() {
        return Err("no non-empty input records found".into());
    }

    let images = if cli.parallel {
        render_many_png_images_parallel(&inputs, cli, quiet_zone)?
    } else {
        inputs.iter().map(|text| render_png_image(text, cli, quiet_zone)).collect::<Result<Vec<_>, _>>()?
    };
    let bytes = encode_png_grid(&images, cli)?;
    std::fs::write(output, bytes)?;
    eprintln!("wrote {output}");
    Ok(inputs.len())
}

fn render_batch_zip(cli: &Cli, quiet_zone: bool) -> Result<usize, Box<dyn Error>> {
    let Some(output) = &cli.output else {
        return Err("batch mode requires --output <DIR|FILE>".into());
    };
    let mut archive = ZipStoreWriter::create(Path::new(output))?;

    let written = if cli.parallel {
        let inputs = read_inputs(cli)?;
        let rendered = render_many_parallel(&inputs, cli, quiet_zone)?;
        for (index, bytes) in rendered.iter().enumerate() {
            archive.write_file(&batch_file_name(index, cli.format), bytes)?;
        }
        rendered.len()
    } else if cli.batch_format == BatchFormat::Json {
        let inputs = read_inputs(cli)?;
        for (index, text) in inputs.iter().enumerate() {
            let bytes = render_one(text, cli, quiet_zone)?;
            archive.write_file(&batch_file_name(index, cli.format), &bytes)?;
        }
        inputs.len()
    } else {
        let Some(path) = &cli.batch else {
            return Ok(0);
        };
        let mut written = 0;
        let mut source = open_record_source(path)?;
        let mut line_no = 0;
        let mut line = String::new();
        while let Some(text) = read_batch_payload(&mut *source, &mut line, &mut line_no, cli)? {
            let bytes = render_one(&text, cli, quiet_zone)?;
            archive.write_file(&batch_file_name(written, cli.format), &bytes)?;
            written += 1;
        }
        written
    };

    if written == 0 {
        return Err("no non-empty input records found".into());
    }
    archive.finish()?;
    eprintln!("wrote {output}");
    Ok(written)
}

fn render_many_parallel(inputs: &[String], cli: &Cli, quiet_zone: bool) -> Result<Vec<Vec<u8>>, Box<dyn Error>> {
    let rendered = inputs
        .par_iter()
        .map(|text| render_one(text, cli, quiet_zone).map_err(|err| err.to_string()))
        .collect::<Vec<_>>();
    rendered.into_iter().collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn render_many_png_images_parallel(
    inputs: &[String],
    cli: &Cli,
    quiet_zone: bool,
) -> Result<Vec<qrcode_image::RgbaImage>, Box<dyn Error>> {
    let rendered = inputs
        .par_iter()
        .map(|text| render_png_image(text, cli, quiet_zone).map_err(|err| err.to_string()))
        .collect::<Vec<_>>();
    rendered.into_iter().collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn open_record_source(path: &Path) -> Result<Box<dyn BufRead>, Box<dyn Error>> {
    if path == Path::new("-") {
        if std::io::stdin().is_terminal() {
            return Err("batch input '-' requires piped data via stdin".into());
        }
        return Ok(Box::new(BufReader::new(std::io::stdin())));
    }
    Ok(Box::new(BufReader::new(File::open(path)?)))
}

fn read_inputs(cli: &Cli) -> Result<Vec<String>, Box<dyn Error>> {
    if let Some(path) = &cli.batch {
        if cli.batch_format == BatchFormat::Json {
            let mut content = String::new();
            open_record_source(path)?.read_to_string(&mut content)?;
            return extract_json_payloads(&content, &cli.batch_key);
        }

        let mut inputs = Vec::new();
        let mut source = open_record_source(path)?;
        let mut line_no = 0;
        let mut line = String::new();
        while let Some(text) = read_batch_payload(&mut *source, &mut line, &mut line_no, cli)? {
            inputs.push(text);
        }
        return Ok(inputs);
    }
    Ok(vec![read_single_input(cli)?])
}

fn read_stdin() -> Result<String, Box<dyn Error>> {
    let mut buf = String::new();
    std::io::stdin().lock().read_to_string(&mut buf)?;
    trim_line_end(&mut buf);
    Ok(buf)
}

fn read_batch_payload(
    source: &mut dyn BufRead,
    line: &mut String,
    line_no: &mut usize,
    cli: &Cli,
) -> Result<Option<String>, Box<dyn Error>> {
    if cli.batch_format == BatchFormat::Csv {
        return read_csv_payload(source, line, line_no, cli.batch_column);
    }
    loop {
        line.clear();
        if source.read_line(line)? == 0 {
            return Ok(None);
        }
        *line_no += 1;
        trim_line_end(line);
        if let Some(payload) = extract_batch_payload(line, cli.batch_format, cli.batch_column, &cli.batch_key)
            .map_err(|err| format!("batch line {line_no}: {err}"))?
        {
            return Ok(Some(payload));
        }
    }
}

fn read_csv_payload(
    source: &mut dyn BufRead,
    line: &mut String,
    line_no: &mut usize,
    column: usize,
) -> Result<Option<String>, Box<dyn Error>> {
    let mut parser = CsvRecordParser::default();
    let mut record_start = None;
    loop {
        line.clear();
        if source.read_line(line)? == 0 {
            if let Some(start) = record_start {
                return Err(format!("batch line {start}: unterminated quoted CSV field").into());
            }
            return Ok(None);
        }
        *line_no += 1;
        if record_start.is_none() && line.trim().is_empty() {
            continue;
        }
        let start = *record_start.get_or_insert(*line_no);
        let content = line.strip_suffix('\n').unwrap_or(line.as_str());
        let content_len = content.strip_suffix('\r').unwrap_or(content).len();
        parser.push(&line[..content_len]).map_err(|err| format!("batch line {start}: {err}"))?;
        if parser.state == CsvFieldState::Quoted {
            // A physical line ending inside quotes is part of the payload.
            parser.push(&line[content_len..]).map_err(|err| format!("batch line {start}: {err}"))?;
            continue;
        }
        let fields = parser.finish().map_err(|err| format!("batch line {start}: {err}"))?;
        if let Some(payload) = csv_payload(fields, column).map_err(|err| format!("batch line {start}: {err}"))? {
            return Ok(Some(payload));
        }
        parser = CsvRecordParser::default();
        record_start = None;
    }
}

fn trim_line_end(buf: &mut String) {
    if buf.ends_with('\n') {
        buf.pop();
    }
    if buf.ends_with('\r') {
        buf.pop();
    }
}

fn extract_batch_payload(
    record: &str,
    format: BatchFormat,
    csv_column: usize,
    json_key: &str,
) -> Result<Option<String>, Box<dyn Error>> {
    if record.trim().is_empty() {
        return Ok(None);
    }
    let text = match format {
        BatchFormat::Lines => record.to_owned(),
        BatchFormat::Csv => {
            return csv_payload(parse_csv_record(record)?, csv_column);
        }
        BatchFormat::Json => return Err("JSON records must be read with --batch-format json".into()),
        BatchFormat::Jsonl => {
            let value: serde_json::Value = serde_json::from_str(record)?;
            return extract_json_payload(&value, json_key);
        }
    };
    Ok(Some(text))
}

fn extract_json_payloads(content: &str, json_key: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let value: serde_json::Value = serde_json::from_str(content)?;
    let mut inputs = Vec::new();
    match &value {
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(text) = extract_json_payload(item, json_key)? {
                    inputs.push(text);
                }
            }
        }
        other => {
            if let Some(text) = extract_json_payload(other, json_key)? {
                inputs.push(text);
            }
        }
    }
    Ok(inputs)
}

fn extract_json_payload(value: &serde_json::Value, json_key: &str) -> Result<Option<String>, Box<dyn Error>> {
    let text = match value {
        serde_json::Value::String(text) => text,
        serde_json::Value::Object(object) => {
            let Some(field) = object.get(json_key) else {
                return Ok(None);
            };
            let Some(text) = field.as_str() else {
                return Err(format!("JSON key '{json_key}' must be a string").into());
            };
            text
        }
        _ => return Err("JSON batch records must be strings or objects".into()),
    };
    if text.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(text.to_owned()))
}

fn csv_payload(fields: Vec<String>, column: usize) -> Result<Option<String>, Box<dyn Error>> {
    let index = column.checked_sub(1).ok_or("CSV column must be greater than zero")?;
    let field = fields.into_iter().nth(index).ok_or_else(|| format!("CSV record has no column {column}"))?;
    if field.trim().is_empty() { Ok(None) } else { Ok(Some(field)) }
}

#[derive(Default, PartialEq, Eq)]
enum CsvFieldState {
    #[default]
    Start,
    Unquoted,
    Quoted,
    AfterQuote,
}

#[derive(Default)]
struct CsvRecordParser {
    fields: Vec<String>,
    field: String,
    state: CsvFieldState,
}

impl CsvRecordParser {
    fn push(&mut self, input: &str) -> Result<(), Box<dyn Error>> {
        for ch in input.chars() {
            match (&self.state, ch) {
                (CsvFieldState::Start, '"') => self.state = CsvFieldState::Quoted,
                (CsvFieldState::Start | CsvFieldState::Unquoted | CsvFieldState::AfterQuote, ',') => {
                    self.fields.push(core::mem::take(&mut self.field));
                    self.state = CsvFieldState::Start;
                }
                (CsvFieldState::Quoted, '"') => self.state = CsvFieldState::AfterQuote,
                (CsvFieldState::AfterQuote, '"') => {
                    self.field.push('"');
                    self.state = CsvFieldState::Quoted;
                }
                (CsvFieldState::Quoted, _) => self.field.push(ch),
                (CsvFieldState::Unquoted, '"') => return Err("quote inside an unquoted CSV field".into()),
                (CsvFieldState::AfterQuote, _) => return Err("unexpected character after a quoted CSV field".into()),
                (_, '\r' | '\n') => return Err("line ending inside an unquoted CSV field".into()),
                (CsvFieldState::Start | CsvFieldState::Unquoted, _) => {
                    self.field.push(ch);
                    self.state = CsvFieldState::Unquoted;
                }
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<String>, Box<dyn Error>> {
        if self.state == CsvFieldState::Quoted {
            return Err("unterminated quoted CSV field".into());
        }
        self.fields.push(self.field);
        Ok(self.fields)
    }
}

fn parse_csv_record(record: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let mut parser = CsvRecordParser::default();
    parser.push(record)?;
    parser.finish()
}

fn validate_image(path: &Path, expect: Option<&str>, print_payload: bool) -> Result<(), Box<dyn Error>> {
    let image = qrcode_image::image::open(path)?.to_luma8();
    let decoded = RqrrDecoder::new().decode(GrayPixels::from(&image))?;
    if decoded.is_empty() {
        return Err("no QR codes found in image".into());
    }
    if let Some(expected) = expect {
        let expected = expected.as_bytes();
        if !decoded.iter().any(|code| code.data() == expected) {
            return Err(format!("decoded {} QR code(s), but none matched the expected payload", decoded.len()).into());
        }
    }
    println!("valid: decoded {} QR code(s)", decoded.len());
    if print_payload || expect.is_none() {
        for (index, code) in decoded.iter().enumerate() {
            println!("{}: {}", index + 1, String::from_utf8_lossy(code.data()));
        }
    }
    Ok(())
}

fn render_one(text: &str, cli: &Cli, quiet_zone: bool) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut builder = QrCode::builder(text.as_bytes()).ec_level(cli.ec_level);
    if let Some(version) = cli.qr_version {
        builder = builder.version(version);
    }
    let code = builder.build()?;
    let (dark_str, light_str) = if cli.invert { (&cli.light, &cli.dark) } else { (&cli.dark, &cli.light) };
    let needs_rgb =
        matches!(cli.format, Format::Ansi | Format::Svg | Format::Png | Format::Eps | Format::Html | Format::Pdf);
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
            use qrcode_image::{DynamicImage, ImageFormat};
            validate_png_size(&code, cli.size, quiet_zone)?;
            let image = render_png_image_with_colors(code, cli, quiet_zone, dark_rgb, light_rgb);
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

fn render_png_image(text: &str, cli: &Cli, quiet_zone: bool) -> Result<qrcode_image::RgbaImage, Box<dyn Error>> {
    let mut builder = QrCode::builder(text.as_bytes()).ec_level(cli.ec_level);
    if let Some(version) = cli.qr_version {
        builder = builder.version(version);
    }
    let code = builder.build()?;
    let (dark_str, light_str) = if cli.invert { (&cli.light, &cli.dark) } else { (&cli.dark, &cli.light) };
    let dark_rgb = parse_rgb(dark_str, "dark")?;
    let light_rgb = parse_rgb(light_str, "light")?;
    validate_png_size(&code, cli.size, quiet_zone)?;
    Ok(render_png_image_with_colors(code, cli, quiet_zone, dark_rgb, light_rgb))
}

fn render_png_image_with_colors(
    code: QrCode,
    cli: &Cli,
    quiet_zone: bool,
    dark_rgb: (u8, u8, u8),
    light_rgb: (u8, u8, u8),
) -> qrcode_image::RgbaImage {
    use qrcode_image::Rgba;

    code.render::<Rgba<u8>>()
        .quiet_zone(quiet_zone)
        .module_dimensions(cli.size, cli.size)
        .dark_color(Rgba([dark_rgb.0, dark_rgb.1, dark_rgb.2, 255]))
        .light_color(Rgba([light_rgb.0, light_rgb.1, light_rgb.2, 255]))
        .build()
}

fn validate_png_size(code: &QrCode, module_size: u32, quiet_zone: bool) -> Result<(), Box<dyn Error>> {
    if module_size == 0 {
        return Err("--size must be greater than zero for PNG output".into());
    }
    let quiet_modules = if quiet_zone {
        u64::from(code.quiet_zone().checked_mul(2).ok_or("quiet zone width overflows u32")?)
    } else {
        0
    };
    let modules = u64::try_from(code.width())
        .map_err(|_| "rendered PNG module width exceeds u64::MAX")?
        .checked_add(quiet_modules)
        .ok_or("rendered PNG module width overflows u64")?;
    let side = modules.checked_mul(u64::from(module_size)).ok_or("rendered PNG dimensions overflow u64")?;
    if side > MAX_PNG_SIDE {
        return Err(format!("rendered PNG side {side}px exceeds the {MAX_PNG_SIDE}px limit").into());
    }
    let pixels = side * side;
    if pixels > MAX_PNG_PIXELS {
        return Err(format!("rendered PNG area {pixels} pixels exceeds the {MAX_PNG_PIXELS} pixel limit").into());
    }
    Ok(())
}

fn encode_png_grid(images: &[qrcode_image::RgbaImage], cli: &Cli) -> Result<Vec<u8>, Box<dyn Error>> {
    use qrcode_image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

    if images.is_empty() {
        return Err("no non-empty input records found".into());
    }
    let columns_usize = grid_columns(cli.grid_columns, images.len())?;
    let rows_usize = images.len().div_ceil(columns_usize);
    let cell_width = images.iter().map(qrcode_image::RgbaImage::width).max().unwrap_or(1);
    let cell_height = images.iter().map(qrcode_image::RgbaImage::height).max().unwrap_or(1);
    let columns = u32::try_from(columns_usize).map_err(|_| "grid column count exceeds u32::MAX")?;
    let rows = u32::try_from(rows_usize).map_err(|_| "grid row count exceeds u32::MAX")?;
    let sheet_width = cell_width.checked_mul(columns).ok_or("grid width exceeds u32::MAX")?;
    let sheet_height = cell_height.checked_mul(rows).ok_or("grid height exceeds u32::MAX")?;
    let (_, light_str) = if cli.invert { (&cli.light, &cli.dark) } else { (&cli.dark, &cli.light) };
    let light_rgb = parse_rgb(light_str, "light")?;
    let buffer_len = grid_buffer_len(sheet_width, sheet_height)?;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(buffer_len).map_err(|_| "could not allocate PNG grid pixel buffer")?;
    pixels.resize(buffer_len, 0);
    let mut sheet = RgbaImage::from_raw(sheet_width, sheet_height, pixels).ok_or("invalid PNG grid dimensions")?;
    for pixel in sheet.pixels_mut() {
        *pixel = Rgba([light_rgb.0, light_rgb.1, light_rgb.2, 255]);
    }

    for (index, image) in images.iter().enumerate() {
        let col = u32::try_from(index % columns_usize).map_err(|_| "grid column index exceeds u32::MAX")?;
        let row = u32::try_from(index / columns_usize).map_err(|_| "grid row index exceeds u32::MAX")?;
        let left = col * cell_width + (cell_width - image.width()) / 2;
        let top = row * cell_height + (cell_height - image.height()) / 2;
        qrcode_image::image::imageops::replace(&mut sheet, image, i64::from(left), i64::from(top));
    }

    qrcode_image::encode_to_format(&DynamicImage::ImageRgba8(sheet), ImageFormat::Png).map_err(Into::into)
}

fn grid_columns(requested: usize, count: usize) -> Result<usize, Box<dyn Error>> {
    if count == 0 {
        return Err("no non-empty input records found".into());
    }
    if requested > 0 {
        return Ok(requested);
    }
    let mut columns = 1usize;
    while columns.saturating_mul(columns) < count {
        columns += 1;
    }
    Ok(columns)
}

fn grid_buffer_len(width: u32, height: u32) -> Result<usize, Box<dyn Error>> {
    if width == 0 || height == 0 || u64::from(width) > MAX_PNG_SIDE || u64::from(height) > MAX_PNG_SIDE {
        return Err(format!("PNG grid dimensions {width}x{height} exceed the {MAX_PNG_SIDE}px side limit").into());
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > MAX_PNG_PIXELS {
        return Err(format!("PNG grid area {pixels} pixels exceeds the {MAX_PNG_PIXELS} pixel limit").into());
    }
    let bytes = pixels.checked_mul(4).ok_or("PNG grid buffer length overflows u64")?;
    let len = usize::try_from(bytes).map_err(|_| "PNG grid buffer length exceeds usize::MAX")?;
    if len > isize::MAX as usize {
        return Err("PNG grid buffer length exceeds isize::MAX".into());
    }
    Ok(len)
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
        let Some(dir) = cli.output.as_ref() else {
            return Err("batch mode requires --output <DIR>".into());
        };
        std::fs::create_dir_all(dir)?;
        let path = Path::new(dir).join(batch_file_name(index, cli.format));
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

fn batch_file_name(index: usize, format: Format) -> String {
    format!("qr-{:04}.{}", index + 1, ext_for(format))
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

struct ZipStoreWriter {
    file: Option<BufWriter<File>>,
    temporary_path: PathBuf,
    output_path: PathBuf,
    committed: bool,
    offset: u64,
    entries: Vec<ZipEntry>,
}

struct ZipEntry {
    name: String,
    crc32: u32,
    size: u32,
    local_header_offset: u32,
}

impl ZipStoreWriter {
    fn create(path: &Path) -> Result<Self, Box<dyn Error>> {
        let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
        for _ in 0..128 {
            let sequence = TEMP_OUTPUT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let temporary_path = parent.join(format!(".qrencodes-{}-{sequence}.tmp", std::process::id()));
            match File::options().write(true).create_new(true).open(&temporary_path) {
                Ok(file) => {
                    return Ok(Self {
                        file: Some(BufWriter::new(file)),
                        temporary_path,
                        output_path: path.to_owned(),
                        committed: false,
                        offset: 0,
                        entries: Vec::new(),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err("could not create a temporary ZIP output file".into())
    }

    fn write_file(&mut self, name: &str, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
        let name_bytes = name.as_bytes();
        let name_len = u16::try_from(name_bytes.len()).map_err(|_| "ZIP entry name is too long")?;
        let size = u32::try_from(bytes.len()).map_err(|_| "ZIP entry is larger than 4 GiB")?;
        let local_header_offset = u32::try_from(self.offset).map_err(|_| "ZIP archive is larger than 4 GiB")?;
        let crc32 = crc32(bytes);

        self.write_u32(0x0403_4b50)?;
        self.write_u16(20)?;
        self.write_u16(0)?;
        self.write_u16(0)?;
        self.write_u16(0)?;
        self.write_u16(0)?;
        self.write_u32(crc32)?;
        self.write_u32(size)?;
        self.write_u32(size)?;
        self.write_u16(name_len)?;
        self.write_u16(0)?;
        self.write_all(name_bytes)?;
        self.write_all(bytes)?;
        self.entries.push(ZipEntry { name: name.to_owned(), crc32, size, local_header_offset });
        Ok(())
    }

    fn finish(mut self) -> Result<(), Box<dyn Error>> {
        let central_dir_offset = u32::try_from(self.offset).map_err(|_| "ZIP archive is larger than 4 GiB")?;
        let entry_count = u16::try_from(self.entries.len()).map_err(|_| "ZIP archive has more than 65535 entries")?;

        for index in 0..self.entries.len() {
            let name = self.entries[index].name.clone();
            let name_bytes = name.as_bytes();
            let name_len = u16::try_from(name_bytes.len()).map_err(|_| "ZIP entry name is too long")?;
            let crc32 = self.entries[index].crc32;
            let size = self.entries[index].size;
            let local_header_offset = self.entries[index].local_header_offset;

            self.write_u32(0x0201_4b50)?;
            self.write_u16(20)?;
            self.write_u16(20)?;
            self.write_u16(0)?;
            self.write_u16(0)?;
            self.write_u16(0)?;
            self.write_u16(0)?;
            self.write_u32(crc32)?;
            self.write_u32(size)?;
            self.write_u32(size)?;
            self.write_u16(name_len)?;
            self.write_u16(0)?;
            self.write_u16(0)?;
            self.write_u16(0)?;
            self.write_u16(0)?;
            self.write_u32(0)?;
            self.write_u32(local_header_offset)?;
            self.write_all(name_bytes)?;
        }

        let central_dir_size = self
            .offset
            .checked_sub(u64::from(central_dir_offset))
            .and_then(|size| u32::try_from(size).ok())
            .ok_or("ZIP central directory is larger than 4 GiB")?;
        self.write_u32(0x0605_4b50)?;
        self.write_u16(0)?;
        self.write_u16(0)?;
        self.write_u16(entry_count)?;
        self.write_u16(entry_count)?;
        self.write_u32(central_dir_size)?;
        self.write_u32(central_dir_offset)?;
        self.write_u16(0)?;
        let mut file = self.file.take().ok_or("ZIP output file is already closed")?;
        file.flush()?;
        // Close the handle before renaming, including when the input and output
        // paths are identical. Drop removes this temporary file on any error.
        drop(file);
        std::fs::rename(&self.temporary_path, &self.output_path)?;
        self.committed = true;
        Ok(())
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
        self.file.as_mut().ok_or("ZIP output file is already closed")?.write_all(bytes)?;
        self.offset += bytes.len() as u64;
        Ok(())
    }

    fn write_u16(&mut self, value: u16) -> Result<(), Box<dyn Error>> {
        self.write_all(&value.to_le_bytes())
    }

    fn write_u32(&mut self, value: u32) -> Result<(), Box<dyn Error>> {
        self.write_all(&value.to_le_bytes())
    }
}

impl Drop for ZipStoreWriter {
    fn drop(&mut self) {
        // The handle must be closed before cleanup on platforms that forbid
        // removing an open file.
        drop(self.file.take());
        if !self.committed {
            let _ = std::fs::remove_file(&self.temporary_path);
        }
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn cli_with_text(text: Option<&str>) -> Cli {
        Cli {
            text: text.map(str::to_owned),
            output: None,
            format: Format::Unicode,
            ec_level: EcLevel::M,
            qr_version: None,
            size: 10,
            no_quiet_zone: false,
            dark: "#000000".to_owned(),
            light: "#ffffff".to_owned(),
            invert: false,
            unicode_mode: UnicodeMode::Dense1x2,
            batch: None,
            batch_format: BatchFormat::Lines,
            batch_column: 1,
            batch_key: "text".to_owned(),
            parallel: false,
            batch_pack: BatchPack::Directory,
            grid_columns: 0,
            command: None,
        }
    }

    fn temporary_path(name: &str) -> PathBuf {
        let nanos =
            SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock must be after UNIX_EPOCH").as_nanos();
        std::env::temp_dir().join(format!("qrcode-cli-{name}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn parse_rgb_accepts_short_and_long_hex() {
        assert_eq!(parse_rgb("#abc", "dark").unwrap(), (170, 187, 204));
        assert_eq!(parse_rgb("#123456", "light").unwrap(), (18, 52, 86));
    }

    #[test]
    fn parse_rgb_reports_the_color_name_for_invalid_input() {
        let error = parse_rgb("not-a-color", "light").unwrap_err().to_string();
        assert!(error.contains("invalid light color"));
    }

    #[test]
    fn output_extensions_match_formats() {
        assert_eq!(ext_for(Format::Png), "png");
        assert_eq!(ext_for(Format::Svg), "svg");
        assert_eq!(ext_for(Format::Unicode), "txt");
    }

    #[test]
    fn crc32_matches_zip_reference_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn read_inputs_skips_blank_batch_records() {
        let path = temporary_path("batch-input");
        fs::write(&path, " first\n\n  \nsecond\r\n").unwrap();
        let mut cli = cli_with_text(None);
        cli.batch = Some(path.clone());

        let inputs = read_inputs(&cli).unwrap();
        assert_eq!(inputs, vec![" first".to_owned(), "second".to_owned()]);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn csv_batch_payload_extracts_quoted_columns() {
        let payload = extract_batch_payload(r#"1,"hello, qr",x"#, BatchFormat::Csv, 2, "text").unwrap();
        assert_eq!(payload, Some("hello, qr".to_owned()));
    }

    #[test]
    fn csv_parser_preserves_escaped_quotes_and_empty_columns() {
        assert_eq!(parse_csv_record(r#",,"say ""hello""",last,"#).unwrap(), ["", "", "say \"hello\"", "last", ""]);
        assert_eq!(parse_csv_record(r#""","#).unwrap(), ["", ""]);
        assert_eq!(parse_csv_record(r#""""""#).unwrap(), ["\""]);
    }

    #[test]
    fn csv_parser_rejects_quotes_outside_field_boundaries() {
        for input in [r#"a"b",x"#, r#""a"x,b"#, r#""a" ,b"#, r#""unterminated"#, "a\nb,x"] {
            assert!(parse_csv_record(input).is_err(), "accepted malformed record: {input}");
        }
    }

    #[test]
    fn csv_reader_preserves_multiline_line_endings_and_skips_empty_payloads() {
        let mut source = std::io::Cursor::new(b"\n1,\"alpha\r\nbeta\"\r\n2,\"say \"\"hi\"\"\"\n3,\n");
        let mut line = String::new();
        let mut line_no = 0;
        assert_eq!(
            read_csv_payload(&mut source, &mut line, &mut line_no, 2).unwrap().as_deref(),
            Some("alpha\r\nbeta")
        );
        assert_eq!(line_no, 3);
        assert_eq!(read_csv_payload(&mut source, &mut line, &mut line_no, 2).unwrap().as_deref(), Some("say \"hi\""));
        assert!(read_csv_payload(&mut source, &mut line, &mut line_no, 2).unwrap().is_none());
    }

    #[test]
    fn csv_reader_reports_record_start_for_unclosed_multiline_field() {
        let mut source = std::io::Cursor::new(b"\n1,\"alpha\nbeta\n");
        let error = read_csv_payload(&mut source, &mut String::new(), &mut 0, 2).unwrap_err().to_string();
        assert_eq!(error, "batch line 2: unterminated quoted CSV field");
    }

    #[test]
    fn jsonl_batch_payload_extracts_named_string_key() {
        let payload = extract_batch_payload(r#"{"id":1,"payload":"hello"}"#, BatchFormat::Jsonl, 1, "payload").unwrap();
        assert_eq!(payload, Some("hello".to_owned()));
    }

    #[test]
    fn json_batch_payload_extracts_array_items() {
        let payloads = extract_json_payloads(r#"[{"payload":"alpha"},"beta",{"payload":""}]"#, "payload").unwrap();
        assert_eq!(payloads, vec!["alpha".to_owned(), "beta".to_owned()]);
    }

    #[test]
    fn jsonl_batch_payload_skips_missing_key() {
        let payload = extract_batch_payload(r#"{"id":1}"#, BatchFormat::Jsonl, 1, "payload").unwrap();
        assert_eq!(payload, None);
    }

    #[test]
    fn csv_batch_payload_reports_missing_column() {
        let error = extract_batch_payload("only-one", BatchFormat::Csv, 2, "text").unwrap_err().to_string();
        assert!(error.contains("no column 2"));
    }

    #[test]
    fn run_rejects_conflicting_batch_and_text_inputs() {
        let mut cli = cli_with_text(Some("payload"));
        cli.batch = Some(PathBuf::from("unused"));

        let error = run(cli).unwrap_err().to_string();
        assert!(error.contains("cannot be used together"));
    }

    #[test]
    fn run_rejects_zero_png_size() {
        let mut cli = cli_with_text(Some("payload"));
        cli.format = Format::Png;
        cli.size = 0;

        let error = run(cli).unwrap_err().to_string();
        assert!(error.contains("greater than zero"));
    }

    #[test]
    fn run_rejects_oversized_png_size() {
        let mut cli = cli_with_text(Some("payload"));
        cli.format = Format::Png;
        cli.size = 1_000_000;

        let error = run(cli).unwrap_err().to_string();
        assert!(error.contains("exceeds"));
    }

    #[test]
    fn run_rejects_invalid_svg_color() {
        let mut cli = cli_with_text(Some("payload"));
        cli.format = Format::Svg;
        cli.dark = r##"black"/><script>alert(1)</script>"##.to_owned();

        let error = run(cli).unwrap_err().to_string();
        assert!(error.contains("invalid dark color"));
    }

    #[test]
    fn run_rejects_empty_batch() {
        let input = temporary_path("empty-batch");
        let output = temporary_path("empty-output");
        fs::write(&input, "\n  \r\n").unwrap();
        let mut cli = cli_with_text(None);
        cli.batch = Some(input.clone());
        cli.output = Some(output.to_string_lossy().into_owned());

        let error = run(cli).unwrap_err().to_string();
        assert!(error.contains("no non-empty input records"));
        fs::remove_file(input).unwrap();
    }

    #[test]
    fn write_output_uses_stable_batch_names() {
        let output = temporary_path("batch-output");
        let mut cli = cli_with_text(Some("payload"));
        cli.output = Some(output.to_string_lossy().into_owned());

        write_output(&cli, b"qr", 2, true).unwrap();
        let file = output.join("qr-0003.txt");
        assert_eq!(fs::read(file).unwrap(), b"qr");
        fs::remove_dir_all(output).unwrap();
    }

    #[test]
    fn parallel_batch_writes_stable_order() {
        let input = temporary_path("parallel-batch-input");
        let output = temporary_path("parallel-batch-output");
        fs::write(&input, "alpha\nbeta\n").unwrap();
        let mut cli = cli_with_text(None);
        cli.batch = Some(input.clone());
        cli.output = Some(output.to_string_lossy().into_owned());
        cli.format = Format::Svg;
        cli.parallel = true;

        assert_eq!(render_batch(&cli, true).unwrap(), 2);
        let first = fs::read_to_string(output.join("qr-0001.svg")).unwrap();
        let second = fs::read_to_string(output.join("qr-0002.svg")).unwrap();
        assert_ne!(first, second);

        fs::remove_file(input).unwrap();
        fs::remove_dir_all(output).unwrap();
    }

    #[test]
    fn zip_batch_writes_stable_entry_names() {
        let input = temporary_path("zip-batch-input");
        let mut output = temporary_path("zip-batch-output");
        output.set_extension("zip");
        fs::write(&input, "alpha\nbeta\n").unwrap();
        let mut cli = cli_with_text(None);
        cli.batch = Some(input.clone());
        cli.output = Some(output.to_string_lossy().into_owned());
        cli.format = Format::Svg;
        cli.batch_pack = BatchPack::Zip;

        assert_eq!(render_batch(&cli, true).unwrap(), 2);
        let archive = fs::read(&output).unwrap();
        assert!(archive.starts_with(b"PK\x03\x04"));
        assert!(archive.windows(b"qr-0001.svg".len()).any(|window| window == b"qr-0001.svg"));
        assert!(archive.windows(b"qr-0002.svg".len()).any(|window| window == b"qr-0002.svg"));

        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn unfinished_zip_cleans_up_temporary_file_and_preserves_output() {
        let output = temporary_path("unfinished-zip");
        fs::write(&output, b"original archive").unwrap();
        let temporary;
        {
            let mut archive = ZipStoreWriter::create(&output).unwrap();
            temporary = archive.temporary_path.clone();
            archive.write_file("qr.txt", b"partially rendered").unwrap();
            assert_eq!(temporary.parent(), output.parent());
            assert_eq!(fs::read(&output).unwrap(), b"original archive");
        }
        assert!(!temporary.exists());
        assert_eq!(fs::read(&output).unwrap(), b"original archive");
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn zip_rename_failure_cleans_up_temporary_file() {
        let output = temporary_path("zip-rename-failure");
        fs::create_dir(&output).unwrap();
        let mut archive = ZipStoreWriter::create(&output).unwrap();
        let temporary = archive.temporary_path.clone();
        archive.write_file("qr.txt", b"payload").unwrap();
        assert!(archive.finish().is_err());
        assert!(!temporary.exists());
        assert!(output.is_dir());
        fs::remove_dir(output).unwrap();
    }

    #[test]
    fn grid_buffer_budget_rejects_overflow_and_excessive_area() {
        assert!(grid_buffer_len(65_536, 1).is_err());
        assert!(grid_buffer_len(16_385, 16_385).is_err());
        assert!(grid_buffer_len(0, 1).is_err());
        assert_eq!(grid_buffer_len(16_384, 16_384).unwrap(), 1_073_741_824);
    }

    #[test]
    fn grid_batch_writes_contact_sheet_png() {
        let input = temporary_path("grid-batch-input");
        let mut output = temporary_path("grid-batch-output");
        output.set_extension("png");
        fs::write(&input, "alpha\nbeta\ngamma\n").unwrap();
        let mut cli = cli_with_text(None);
        cli.batch = Some(input.clone());
        cli.output = Some(output.to_string_lossy().into_owned());
        cli.format = Format::Png;
        cli.size = 2;
        cli.batch_pack = BatchPack::Grid;
        cli.grid_columns = 2;

        assert_eq!(render_batch(&cli, true).unwrap(), 3);
        let image = qrcode_image::image::open(&output).unwrap();
        assert!(image.width() > image.height() / 2);
        assert_eq!((image.width() / 2) * 2, image.width());

        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn validate_image_accepts_matching_generated_png() {
        let mut path = temporary_path("validate");
        path.set_extension("png");
        let code = QrCode::new("validate me").unwrap();
        let image = code.render::<qrcode_image::Luma<u8>>().min_dimensions(200, 200).build();
        image.save(&path).unwrap();

        validate_image(&path, Some("validate me"), false).unwrap();
        fs::remove_file(path).unwrap();
    }
}
