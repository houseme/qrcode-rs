//! `qrencodes` — command-line QR code generator.

use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

mod atomic_output;
mod image_commands;

use atomic_output::AtomicOutputFile;

use crate::batch::{BatchEntry, BatchGridOptions, BatchOutput, BatchPackError};
use crate::{EcLevel, QrCode, QrSymbol, Version};
use clap::{Parser, Subcommand, ValueEnum};
use qrcode_render::{ansi, colors, unicode};
use rayon::prelude::*;
use serde::Deserialize;

const MAX_PNG_SIDE: u64 = 65_535;
const MAX_PNG_PIXELS: u64 = 268_435_456;
const ZIP_PARALLEL_CHUNK_SIZE: usize = 64;

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
    /// Decode Normal, Micro, or Structured Append QR symbols from images.
    Decode {
        /// Input image files, scanned in argument order.
        #[arg(value_name = "IMAGE", required = true, num_args = 1..)]
        images: Vec<PathBuf>,
        /// Output representation; text requires valid UTF-8.
        #[arg(long, value_enum, default_value_t = image_commands::OutputFormat::Text)]
        format: image_commands::OutputFormat,
        /// Write output to a regular file ("-" means stdout).
        #[arg(long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Return valid candidates while reporting failed candidates.
        #[arg(long)]
        allow_partial: bool,
        /// Reassemble complete Structured Append groups explicitly.
        #[arg(long)]
        assemble: bool,
        /// Scan inverted images.
        #[arg(long)]
        invert: bool,
        /// Maximum pixels per input image.
        #[arg(long, default_value_t = image_commands::DEFAULT_MAX_PIXELS, value_parser = image_commands::parse_max_pixels)]
        max_pixels: u64,
    },
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Self::Validate { .. } => "validate",
            Self::Decode { .. } => "decode",
        }
    }
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

/// Runs the shared CLI from process arguments and returns its exit status.
pub fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug)]
struct FilePathError {
    operation: &'static str,
    path: PathBuf,
    source: Box<dyn Error>,
}

impl fmt::Display for FilePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} '{}': {}", self.operation, self.path.display(), self.source)
    }
}

impl Error for FilePathError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

fn path_error(operation: &'static str, path: &Path, source: impl Into<Box<dyn Error>>) -> Box<dyn Error> {
    Box::new(FilePathError { operation, path: path.to_owned(), source: source.into() })
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    if cli.batch.is_some() && cli.text.is_some() {
        return Err("--batch cannot be used together with TEXT".into());
    }
    if let Some(command) = &cli.command {
        let name = command.name();
        if cli.text.is_some() {
            return Err(format!("TEXT cannot be used together with {name}").into());
        }
        if cli.batch.is_some() {
            return Err(format!("--batch cannot be used together with {name}").into());
        }
        return run_command(command);
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
        Command::Decode { images, format, output, allow_partial, assemble, invert, max_pixels } => {
            image_commands::decode_images(
                images,
                image_commands::DecodeOptions {
                    format: *format,
                    output: output.as_deref(),
                    allow_partial: *allow_partial,
                    assemble: *assemble,
                    invert: *invert,
                    max_pixels: *max_pixels,
                },
            )
        }
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
        let count = inputs.len();
        if cli.parallel {
            let rendered = render_many_parallel(&inputs, cli, quiet_zone)?;
            for (index, bytes) in rendered.into_iter().enumerate() {
                write_output(cli, &bytes, index, true)?;
            }
        } else {
            for (index, text) in inputs.into_iter().enumerate() {
                let bytes = render_one(&text, cli, quiet_zone)?;
                write_output(cli, &bytes, index, true)?;
            }
        }
        return Ok(count);
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
    let mut entries = Vec::new();
    let mut pending = Vec::with_capacity(if cli.parallel { ZIP_PARALLEL_CHUNK_SIZE } else { 0 });
    let input_result = for_each_batch_payload(cli, |text| {
        if cli.parallel {
            pending.push(text);
            if pending.len() == ZIP_PARALLEL_CHUNK_SIZE {
                encode_grid_chunk(&mut pending, &mut entries, cli, quiet_zone)?;
            }
        } else {
            entries.push(BatchEntry::new("", encode_grid_symbol(&text, cli, quiet_zone)?));
        }
        Ok(())
    });
    encode_grid_chunk(&mut pending, &mut entries, cli, quiet_zone)?;
    input_result?;
    if entries.is_empty() {
        return Err("no non-empty input records found".into());
    }
    let count = entries.len();
    let bytes = encode_png_grid(BatchOutput::from_entries(entries), cli, quiet_zone)?;
    atomic_output::write(Path::new(output), &bytes)
        .map_err(|error| path_error("write output file", Path::new(output), error))?;
    eprintln!("wrote {output}");
    Ok(count)
}

fn encode_grid_symbol(text: &str, cli: &Cli, quiet_zone: bool) -> Result<QrCode, Box<dyn Error>> {
    let mut builder = QrCode::builder(text.as_bytes()).ec_level(cli.ec_level);
    if let Some(version) = cli.qr_version {
        builder = builder.version(version);
    }
    let code = builder.build()?;
    validate_png_size(&code, cli.size, quiet_zone)?;
    Ok(code)
}

fn encode_grid_chunk(
    pending: &mut Vec<String>,
    entries: &mut Vec<BatchEntry<QrCode>>,
    cli: &Cli,
    quiet_zone: bool,
) -> Result<(), Box<dyn Error>> {
    if pending.is_empty() {
        return Ok(());
    }
    let results = pending
        .par_iter()
        .map(|text| encode_grid_symbol(text, cli, quiet_zone).map_err(|error| error.to_string()))
        .collect::<Vec<_>>();
    pending.clear();
    for result in results {
        entries.push(BatchEntry::new("", result?));
    }
    Ok(())
}

fn render_batch_zip(cli: &Cli, quiet_zone: bool) -> Result<usize, Box<dyn Error>> {
    let Some(output) = &cli.output else {
        return Err("batch mode requires --output <DIR|FILE>".into());
    };
    let mut archive = ZipStoreWriter::create(Path::new(output))?;

    let mut written = 0;
    let mut pending = Vec::with_capacity(if cli.parallel { ZIP_PARALLEL_CHUNK_SIZE } else { 0 });
    let input_result = for_each_batch_payload(cli, |text| {
        if cli.parallel {
            pending.push(text);
            if pending.len() == ZIP_PARALLEL_CHUNK_SIZE {
                write_parallel_zip_chunk(&mut pending, &mut archive, &mut written, cli, quiet_zone)?;
            }
        } else {
            let bytes = render_one(&text, cli, quiet_zone)?;
            archive.write_file(&batch_file_name(written, cli.format), &bytes)?;
            written += 1;
        }
        Ok(())
    });
    // Earlier pending records take priority over a later input/JSON error.
    // Failed rendering clears the chunk, so its error is not retried here.
    write_parallel_zip_chunk(&mut pending, &mut archive, &mut written, cli, quiet_zone)?;
    input_result?;

    if written == 0 {
        return Err("no non-empty input records found".into());
    }
    archive.finish().map_err(|error| path_error("finish ZIP output", Path::new(output), error))?;
    eprintln!("wrote {output}");
    Ok(written)
}

fn write_parallel_zip_chunk(
    pending: &mut Vec<String>,
    archive: &mut ZipStoreWriter,
    written: &mut usize,
    cli: &Cli,
    quiet_zone: bool,
) -> Result<(), Box<dyn Error>> {
    if pending.is_empty() {
        return Ok(());
    }
    let rendered = render_many_parallel(pending, cli, quiet_zone);
    pending.clear();
    for bytes in rendered? {
        archive.write_file(&batch_file_name(*written, cli.format), &bytes)?;
        *written += 1;
    }
    Ok(())
}

fn render_many_parallel(inputs: &[String], cli: &Cli, quiet_zone: bool) -> Result<Vec<Vec<u8>>, Box<dyn Error>> {
    let rendered = inputs
        .par_iter()
        .map(|text| render_one(text, cli, quiet_zone).map_err(|err| err.to_string()))
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
    let file = File::open(path).map_err(|error| path_error("open batch input", path, error))?;
    Ok(Box::new(BufReader::new(file)))
}

fn read_inputs(cli: &Cli) -> Result<Vec<String>, Box<dyn Error>> {
    let mut inputs = Vec::new();
    for_each_batch_payload(cli, |text| {
        inputs.push(text);
        Ok(())
    })?;
    Ok(inputs)
}

fn for_each_batch_payload(
    cli: &Cli,
    mut consume: impl FnMut(String) -> Result<(), Box<dyn Error>>,
) -> Result<usize, Box<dyn Error>> {
    if let Some(path) = &cli.batch {
        if cli.batch_format == BatchFormat::Json {
            // Preserve the concrete reader type through serde_json's byte reads.
            if path == Path::new("-") {
                let stdin = std::io::stdin();
                if stdin.is_terminal() {
                    return Err("batch input '-' requires piped data via stdin".into());
                }
                return for_each_json_payload(BufReader::new(stdin.lock()), &cli.batch_key, consume);
            }
            let file = File::open(path).map_err(|error| path_error("open batch input", path, error))?;
            return for_each_json_payload(BufReader::new(file), &cli.batch_key, consume);
        }
        let mut source = open_record_source(path)?;
        let mut count = 0;
        let mut line_no = 0;
        let mut line = String::new();
        while let Some(text) = read_batch_payload(&mut *source, &mut line, &mut line_no, cli)? {
            consume(text)?;
            count += 1;
        }
        return Ok(count);
    }
    consume(read_single_input(cli)?)?;
    Ok(1)
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

#[cfg(test)]
fn extract_json_payloads(content: &str, json_key: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let mut inputs = Vec::new();
    for_each_json_payload(content.as_bytes(), json_key, |text| {
        inputs.push(text);
        Ok(())
    })?;
    Ok(inputs)
}

fn for_each_json_payload<R: Read>(
    source: R,
    json_key: &str,
    mut consume: impl FnMut(String) -> Result<(), Box<dyn Error>>,
) -> Result<usize, Box<dyn Error>> {
    let mut deserializer = serde_json::Deserializer::from_reader(source);
    let mut consume_error = None;
    let result = serde::Deserializer::deserialize_any(
        &mut deserializer,
        JsonPayloadVisitor { json_key, consume: &mut consume, consume_error: &mut consume_error, count: 0 },
    );
    if let Some(error) = consume_error {
        return Err(error);
    }
    let count = result?;
    deserializer.end()?;
    Ok(count)
}

struct JsonPayloadVisitor<'a, F> {
    json_key: &'a str,
    consume: &'a mut F,
    consume_error: &'a mut Option<Box<dyn Error>>,
    count: usize,
}

impl<F: FnMut(String) -> Result<(), Box<dyn Error>>> JsonPayloadVisitor<'_, F> {
    fn emit<E: serde::de::Error>(&mut self, value: serde_json::Value) -> Result<(), E> {
        let text = match value {
            serde_json::Value::String(text) => text,
            serde_json::Value::Object(mut object) => match object.remove(self.json_key) {
                None => return Ok(()),
                Some(serde_json::Value::String(text)) => text,
                Some(_) => return Err(E::custom(format!("JSON key '{}' must be a string", self.json_key))),
            },
            _ => return Err(E::custom("JSON batch records must be strings or objects")),
        };
        if text.trim().is_empty() {
            return Ok(());
        }
        if let Err(error) = (self.consume)(text) {
            let message = error.to_string();
            *self.consume_error = Some(error);
            return Err(E::custom(message));
        }
        self.count += 1;
        Ok(())
    }
}

impl<'de, F: FnMut(String) -> Result<(), Box<dyn Error>>> serde::de::Visitor<'de> for JsonPayloadVisitor<'_, F> {
    type Value = usize;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON array of strings or objects, or a single string or object")
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(mut self, mut sequence: A) -> Result<Self::Value, A::Error> {
        while let Some(value) = sequence.next_element::<serde_json::Value>()? {
            self.emit(value)?;
        }
        Ok(self.count)
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(mut self, map: A) -> Result<Self::Value, A::Error> {
        let value = serde_json::Value::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
        self.emit(value)?;
        Ok(self.count)
    }

    fn visit_str<E: serde::de::Error>(mut self, value: &str) -> Result<Self::Value, E> {
        self.emit(serde_json::Value::String(value.to_owned()))?;
        Ok(self.count)
    }

    fn visit_string<E: serde::de::Error>(mut self, value: String) -> Result<Self::Value, E> {
        self.emit(serde_json::Value::String(value))?;
        Ok(self.count)
    }
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
    image_commands::validate_image(path, expect, print_payload)
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
        Format::Svg => {
            let dark = css_hex_color(dark_str);
            let light = css_hex_color(light_str);
            code.render::<qrcode_svg::Color>()
                .quiet_zone(quiet_zone)
                .dark_color(qrcode_svg::Color(dark.as_ref()))
                .light_color(qrcode_svg::Color(light.as_ref()))
                .build()
                .into_bytes()
        }
        Format::Png => {
            use qrcode_image::{DynamicImage, ImageFormat};
            validate_png_size(&code, cli.size, quiet_zone)?;
            let image = render_png_image_with_colors(code, cli, quiet_zone, dark_rgb, light_rgb)?;
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
        Format::Html => {
            let dark = css_hex_color(dark_str);
            let light = css_hex_color(light_str);
            code.render::<qrcode_html::Color>()
                .quiet_zone(quiet_zone)
                .dark_color(qrcode_html::Color(dark.as_ref()))
                .light_color(qrcode_html::Color(light.as_ref()))
                .build()
                .into_bytes()
        }
        Format::Pdf => code
            .render::<qrcode_pdf::Color>()
            .quiet_zone(quiet_zone)
            .dark_color(qrcode_pdf::Color(to_unit(&dark_rgb)))
            .light_color(qrcode_pdf::Color(to_unit(&light_rgb)))
            .build(),
    };
    Ok(bytes)
}

fn render_png_image_with_colors(
    code: QrCode,
    cli: &Cli,
    quiet_zone: bool,
    dark_rgb: (u8, u8, u8),
    light_rgb: (u8, u8, u8),
) -> Result<qrcode_image::RgbaImage, qrcode_render::RenderError> {
    use qrcode_image::Rgba;

    code.render::<Rgba<u8>>()
        .quiet_zone(quiet_zone)
        .module_dimensions(cli.size, cli.size)
        .dark_color(Rgba([dark_rgb.0, dark_rgb.1, dark_rgb.2, 255]))
        .light_color(Rgba([light_rgb.0, light_rgb.1, light_rgb.2, 255]))
        .try_build()
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

fn encode_png_grid(codes: BatchOutput<QrCode>, cli: &Cli, quiet_zone: bool) -> Result<Vec<u8>, Box<dyn Error>> {
    let (dark_str, light_str) = if cli.invert { (&cli.light, &cli.dark) } else { (&cli.dark, &cli.light) };
    parse_rgb(dark_str, "dark")?;
    let light_rgb = parse_rgb(light_str, "light")?;
    let template = crate::QrTemplate::minimal()
        .with_dark_color(dark_str.clone())
        .with_light_color(light_str.clone())
        .with_module_size(cli.size, cli.size)
        .with_quiet_zone(quiet_zone);
    let options =
        BatchGridOptions::default().columns(cli.grid_columns).background([light_rgb.0, light_rgb.1, light_rgb.2, 255]);
    codes.to_png_grid_with(options, &template).map_err(|error| match error {
        BatchPackError::GridTooLarge => "PNG grid dimensions exceed side or backend resource limits".into(),
        error => Box::new(error) as Box<dyn Error>,
    })
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

fn css_hex_color(color: &str) -> Cow<'_, str> {
    // render_one has already validated RGB hex input. The markup backends
    // require a CSS hash even though the shared RGB parser accepts bare hex.
    if color.starts_with('#') { Cow::Borrowed(color) } else { Cow::Owned(format!("#{color}")) }
}

fn to_unit(&(r, g, b): &(u8, u8, u8)) -> [f64; 3] {
    [r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0]
}

fn stdout_write_result(result: std::io::Result<()>) -> Result<(), Box<dyn Error>> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn write_stdout(write: impl FnOnce(&mut std::io::StdoutLock<'_>) -> std::io::Result<()>) -> Result<(), Box<dyn Error>> {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    // Classify only errors from this stdout write/flush; file and render errors
    // propagate through their existing paths.
    stdout_write_result(write(&mut output).and_then(|()| output.flush()))
}

fn write_output(cli: &Cli, bytes: &[u8], index: usize, batch: bool) -> Result<(), Box<dyn Error>> {
    if batch {
        let Some(dir) = cli.output.as_ref() else {
            return Err("batch mode requires --output <DIR>".into());
        };
        std::fs::create_dir_all(dir).map_err(|error| path_error("create output directory", Path::new(dir), error))?;
        let path = Path::new(dir).join(batch_file_name(index, cli.format));
        atomic_output::write(&path, bytes).map_err(|error| path_error("write output file", &path, error))?;
        eprintln!("wrote {}", path.display());
        return Ok(());
    }
    match &cli.output {
        Some(path) if path == "-" => write_stdout(|output| output.write_all(bytes))?,
        Some(path) => atomic_output::write(Path::new(path), bytes)
            .map_err(|error| path_error("write output file", Path::new(path), error))?,
        None => write_stdout(|output| output.write_all(bytes))?,
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
    output: AtomicOutputFile,
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
        let output = AtomicOutputFile::create(path).map_err(|error| path_error("create ZIP output", path, error))?;
        Ok(Self { output, offset: 0, entries: Vec::new() })
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
        self.output.finish()?;
        Ok(())
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
        self.output.write_all(bytes)?;
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

fn crc32(bytes: &[u8]) -> u32 {
    crate::batch::zip_crc32(bytes)
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
    fn css_hex_color_borrows_existing_css_and_prefixes_only_bare_hex() {
        let original = "#AbC";
        let css = css_hex_color(original);
        assert!(matches!(css, Cow::Borrowed(_)));
        assert_eq!(css.as_ptr(), original.as_ptr());
        assert_eq!(css.as_ref(), original);
        assert_eq!(css_hex_color("AbC"), "#AbC");
        assert_eq!(css_hex_color("123456"), "#123456");
    }

    #[test]
    fn output_extensions_match_formats() {
        assert_eq!(ext_for(Format::Png), "png");
        assert_eq!(ext_for(Format::Svg), "svg");
        assert_eq!(ext_for(Format::Unicode), "txt");
    }

    #[test]
    fn stdout_result_silences_only_the_broken_pipe_error_kind() {
        assert!(stdout_write_result(Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))).is_ok());
        for kind in [std::io::ErrorKind::PermissionDenied, std::io::ErrorKind::WriteZero, std::io::ErrorKind::Other] {
            let error = stdout_write_result(Err(std::io::Error::from(kind))).unwrap_err();
            assert_eq!(error.downcast_ref::<std::io::Error>().unwrap().kind(), kind);
        }
    }

    #[test]
    fn file_path_context_preserves_the_original_io_error_source() {
        let error =
            path_error("write output file", Path::new("out.svg"), std::io::Error::from(std::io::ErrorKind::BrokenPipe));
        assert!(error.to_string().contains("write output file 'out.svg'"));
        assert_eq!(
            error.source().unwrap().downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::BrokenPipe
        );
    }

    #[test]
    fn crc32_matches_zip_reference_values() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(b"abc"), 0x3524_41c2);
    }

    fn crc32_bitwise(bytes: &[u8]) -> u32 {
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

    #[test]
    fn crc32_matches_bitwise_oracle_for_prefixes_and_unaligned_slices() {
        let bytes = (0..65_539).map(|index| ((index * 37 + 11) % 256) as u8).collect::<Vec<_>>();
        for offset in 0..3 {
            for length in (0..=256).chain([32_768, 65_536]) {
                let input = &bytes[offset..offset + length];
                assert_eq!(crc32(input), crc32_bitwise(input), "offset {offset}, length {length}");
            }
        }
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
    fn json_payload_reader_accepts_single_records_and_rejects_trailing_data() {
        assert_eq!(extract_json_payloads(r#""alpha""#, "text").unwrap(), ["alpha"]);
        assert_eq!(extract_json_payloads(r#"{"payload":"beta"}"#, "payload").unwrap(), ["beta"]);
        assert!(extract_json_payloads(r#"["alpha"] trailing"#, "text").is_err());
        assert!(extract_json_payloads(r#"["alpha"] ["beta"]"#, "text").is_err());
        assert!(extract_json_payloads(r#"["alpha", "#, "text").is_err());
    }

    #[test]
    fn json_payload_reader_stops_when_the_consumer_fails() {
        let mut visited = 0;
        let error = for_each_json_payload(r#"["alpha","beta",42]"#.as_bytes(), "text", |_| {
            visited += 1;
            Err("consumer failed".into())
        })
        .unwrap_err();
        assert_eq!(visited, 1);
        assert_eq!(error.to_string(), "consumer failed");
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
    fn run_rejects_generation_inputs_before_dispatching_validation() {
        for (text, batch, expected) in [
            (Some("alpha"), None, "TEXT cannot be used together with validate"),
            (None, Some("unused"), "--batch cannot be used together with validate"),
            (Some("alpha"), Some("unused"), "--batch cannot be used together with TEXT"),
        ] {
            let mut cli = cli_with_text(text);
            cli.batch = batch.map(PathBuf::from);
            cli.command =
                Some(Command::Validate { image: PathBuf::from("missing-image"), expect: None, print_payload: false });
            assert_eq!(run(cli).unwrap_err().to_string(), expected);
        }
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
    fn parallel_zip_reports_the_first_render_error_in_input_order() {
        let input = temporary_path("ordered-zip-error-input");
        let output = temporary_path("ordered-zip-error-output");
        fs::write(&input, format!("{}\nvalid\n", "x".repeat(4_000))).unwrap();
        let mut cli = cli_with_text(None);
        cli.batch = Some(input.clone());
        cli.output = Some(output.to_string_lossy().into_owned());
        cli.format = Format::Svg;
        cli.dark = "invalid-color".to_owned();
        cli.parallel = true;
        cli.batch_pack = BatchPack::Zip;
        let error = render_batch(&cli, true).unwrap_err().to_string();
        assert!(error.contains("data too long"), "wrong ordered error: {error}");
        assert!(!output.exists());
        fs::remove_file(input).unwrap();
    }

    #[test]
    fn unfinished_zip_cleans_up_temporary_file_and_preserves_output() {
        let output = temporary_path("unfinished-zip");
        fs::write(&output, b"original archive").unwrap();
        let temporary;
        {
            let mut archive = ZipStoreWriter::create(&output).unwrap();
            temporary = archive.output.temporary_path().to_path_buf();
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
        let mut archive = ZipStoreWriter::create(&output).unwrap();
        let temporary = archive.output.temporary_path().to_path_buf();
        archive.write_file("qr.txt", b"payload").unwrap();
        fs::create_dir(&output).unwrap();
        assert!(archive.finish().is_err());
        assert!(!temporary.exists());
        assert!(output.is_dir());
        fs::remove_dir(output).unwrap();
    }

    #[test]
    fn grid_symbol_budget_rejects_excessive_dimensions_before_allocation() {
        let mut cli = cli_with_text(None);
        cli.format = Format::Png;
        for (size, columns) in [(1, 65_536), (300, 1)] {
            cli.size = size;
            cli.grid_columns = columns;
            let code = QrCode::new("alpha").unwrap();
            let codes = BatchOutput::from_entries([BatchEntry::new("", code)]);
            let error = encode_png_grid(codes, &cli, true).unwrap_err().to_string();
            assert!(error.contains("PNG grid dimensions"));
            assert!(error.contains("resource limits"));
        }
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
