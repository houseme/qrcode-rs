//! Shared image scanning and output preparation for CLI image commands.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use qrcode_image::image::{DynamicImage, GrayImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use serde::Serialize;

use crate::decode::rxing::{DecodeError, RxingDecoder, ScanOptions};
use crate::decode::{GrayPixels, ScanSymbol};
use crate::{EcLevel, Version};

pub(super) const DEFAULT_MAX_PIXELS: u64 = 16_777_216;
const MAX_ENCODED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_IMAGE_SIDE: u32 = 32_768;
const MAX_RETAINED_CANDIDATES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum OutputFormat {
    Text,
    Json,
    Raw,
}

pub(super) struct DecodeOptions<'a> {
    pub format: OutputFormat,
    pub output: Option<&'a Path>,
    pub allow_partial: bool,
    pub assemble: bool,
    pub invert: bool,
    pub max_pixels: u64,
}

pub(super) fn parse_max_pixels(input: &str) -> Result<u64, String> {
    let pixels = input.parse::<u64>().map_err(|_| "--max-pixels requires a positive integer".to_owned())?;
    if pixels == 0 {
        return Err("--max-pixels must be greater than zero".to_owned());
    }
    Ok(pixels)
}

// Limit the addressable encoded file, including files that grow after metadata
// validation. Seek follows the underlying reader contract; reads at the limit
// probe one byte to distinguish exact EOF from an oversized encoded stream.
struct BoundedReader<R> {
    inner: R,
    position: u64,
    limit: u64,
}

impl<R> BoundedReader<R> {
    fn new(inner: R, limit: u64) -> Self {
        Self { inner, position: 0, limit }
    }
}

impl<R: Read> Read for BoundedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let remaining = self.limit.saturating_sub(self.position);
        if remaining == 0 {
            let mut probe = [0];
            if self.inner.read(&mut probe)? == 0 {
                return Ok(0);
            }
            self.position = self.position.saturating_add(1);
            return Err(io::Error::new(io::ErrorKind::InvalidData, "encoded image exceeds the 64 MiB limit"));
        }
        let length = buffer.len().min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let read = self.inner.read(&mut buffer[..length])?;
        self.position += read as u64;
        Ok(read)
    }
}

impl<R: Seek> Seek for BoundedReader<R> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let position = self.inner.seek(position)?;
        self.position = position;
        Ok(position)
    }
}

fn image_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    limits.max_alloc = Some(qrcode_render::MAX_BUFFER_BYTES as u64);
    limits
}

fn validate_decoder_budget(width: u32, height: u32, decoded_bytes: u64, max_pixels: u64) -> Result<(), Box<dyn Error>> {
    if width == 0 || height == 0 || width > MAX_IMAGE_SIDE || height > MAX_IMAGE_SIDE {
        return Err("decoded image dimensions exceed the supported 32768 pixel side limit".into());
    }
    let gray_bytes = u64::from(width) * u64::from(height);
    if max_pixels == 0 || gray_bytes > max_pixels {
        return Err(format!("decoded image has {gray_bytes} pixels, exceeding the {max_pixels} pixel limit").into());
    }
    let combined = decoded_bytes.checked_add(gray_bytes).ok_or("decoded image buffer lengths overflow")?;
    if combined > qrcode_render::MAX_BUFFER_BYTES as u64
        || decoded_bytes > isize::MAX as u64
        || gray_bytes > isize::MAX as u64
    {
        return Err("decoded image and grayscale buffers exceed the 256 MiB resource limit".into());
    }
    Ok(())
}

fn decoder_to_gray(
    mut decoder: impl ImageDecoder,
    mut limits: Limits,
    max_pixels: u64,
) -> Result<GrayImage, Box<dyn Error>> {
    let (width, height) = decoder.dimensions();
    let decoded_bytes = decoder.total_bytes();
    validate_decoder_budget(width, height, decoded_bytes, max_pixels)?;
    // into_decoder does not reserve the decoded output allocation. Mirror
    // ImageReader::decode before constructing the full DynamicImage.
    limits.reserve(decoded_bytes)?;
    decoder.set_limits(limits)?;
    Ok(DynamicImage::from_decoder(decoder)?.into_luma8())
}

fn load_gray(path: &Path, max_pixels: u64) -> Result<GrayImage, Box<dyn Error>> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err("input image must be a regular file".into());
    }
    if metadata.len() > MAX_ENCODED_BYTES {
        return Err("encoded image exceeds the 64 MiB limit".into());
    }
    let format = ImageFormat::from_path(path)?;
    let buffered = BufReader::new(BoundedReader::new(file, MAX_ENCODED_BYTES));
    let limits = image_limits();
    let mut reader = ImageReader::with_format(buffered, format);
    reader.limits(limits.clone());
    decoder_to_gray(reader.into_decoder()?, limits, max_pixels)
}

fn scan_options(max_pixels: u64, invert: bool) -> ScanOptions {
    let mut options = ScanOptions::default();
    options.max_pixels = max_pixels;
    options.inverted = invert;
    options
}

pub(super) fn validate_image(path: &Path, expect: Option<&str>, print_payload: bool) -> Result<(), Box<dyn Error>> {
    let image =
        load_gray(path, DEFAULT_MAX_PIXELS).map_err(|error| super::path_error("read validation image", path, error))?;
    let candidates = RxingDecoder::new()
        .scan_with_options(GrayPixels::from(&image), scan_options(DEFAULT_MAX_PIXELS, false))
        .map_err(|error| super::path_error("decode validation image", path, error))?;
    let mut decoded = Vec::new();
    for candidate in candidates {
        decoded.push(candidate.map_err(|error| super::path_error("decode validation image", path, error))?);
    }
    if decoded.is_empty() {
        return Err("no QR codes found in image".into());
    }
    if let Some(expected) = expect
        && !decoded.iter().any(|symbol| symbol.decoded().data() == expected.as_bytes())
    {
        return Err(format!("decoded {} QR code(s), but none matched the expected payload", decoded.len()).into());
    }
    super::write_stdout(|output| {
        writeln!(output, "valid: decoded {} QR code(s)", decoded.len())?;
        if print_payload || expect.is_none() {
            for (index, symbol) in decoded.iter().enumerate() {
                writeln!(output, "{}: {}", index + 1, String::from_utf8_lossy(symbol.decoded().data()))?;
            }
        }
        Ok(())
    })
}

struct FoundSymbol {
    symbol: ScanSymbol,
    source: PathBuf,
    candidate_index: usize,
    index: usize,
}

struct CandidateFailure {
    source: PathBuf,
    candidate_index: usize,
    message: String,
}

fn collect_candidates(
    path: &Path,
    candidates: Vec<Result<ScanSymbol, DecodeError>>,
    allow_partial: bool,
    symbols: &mut Vec<FoundSymbol>,
    failures: &mut Vec<CandidateFailure>,
    decoded_bytes: &mut usize,
) -> Result<(), Box<dyn Error>> {
    for (candidate, result) in candidates.into_iter().enumerate() {
        if symbols.len().checked_add(failures.len()).ok_or("decoded result count overflows")? >= MAX_RETAINED_CANDIDATES
        {
            return Err("decoded results exceed the 4096 candidate aggregate limit".into());
        }
        match result {
            Ok(symbol) => {
                let total = decoded_bytes
                    .checked_add(symbol.decoded().data().len())
                    .filter(|&total| total <= qrcode_render::MAX_BUFFER_BYTES)
                    .ok_or("decoded payloads exceed the 256 MiB aggregate limit")?;
                symbols.push(FoundSymbol {
                    symbol,
                    source: path.to_owned(),
                    candidate_index: candidate + 1,
                    index: symbols.len() + 1,
                });
                *decoded_bytes = total;
            }
            Err(error) => {
                if !allow_partial {
                    return Err(super::path_error("decode image candidate", path, error));
                }
                failures.push(CandidateFailure {
                    source: path.to_owned(),
                    candidate_index: candidate + 1,
                    message: error.to_string(),
                });
            }
        }
    }
    Ok(())
}

struct Payload<'a> {
    data: Cow<'a, [u8]>,
    sources: Vec<&'a Path>,
    symbol_indices: Vec<usize>,
    assembly: Option<(u8, u8)>,
}

fn require_symbols(symbols: &[FoundSymbol], failures: &[CandidateFailure]) -> Result<(), Box<dyn Error>> {
    if !symbols.is_empty() {
        return Ok(());
    }
    if failures.is_empty() {
        Err("no QR codes found in image".into())
    } else {
        Err(format!("no QR codes decoded successfully ({} candidate(s) failed)", failures.len()).into())
    }
}

#[derive(Serialize)]
struct JsonVersion {
    kind: &'static str,
    number: i16,
}

#[derive(Serialize)]
struct JsonHeader {
    position: u8,
    total: u8,
    parity: u8,
}

#[derive(Serialize)]
struct JsonSymbol<'a> {
    index: usize,
    candidate_index: usize,
    source_image: Cow<'a, str>,
    data: &'a [u8],
    text: Option<&'a str>,
    version: JsonVersion,
    ec_level: &'static str,
    structured_append: Option<JsonHeader>,
}

#[derive(Serialize)]
struct JsonFailure<'a> {
    source_image: Cow<'a, str>,
    candidate_index: usize,
    message: &'a str,
}

#[derive(Serialize)]
struct JsonAssembly<'a> {
    total: u8,
    parity: u8,
    data: &'a [u8],
    text: Option<&'a str>,
    symbol_indices: &'a [usize],
    source_images: Vec<Cow<'a, str>>,
}

#[derive(Serialize)]
struct JsonReport<'a> {
    symbols: Vec<JsonSymbol<'a>>,
    errors: Vec<JsonFailure<'a>>,
    assemblies: Vec<JsonAssembly<'a>>,
}

struct LimitedBuffer {
    bytes: Vec<u8>,
    limit: usize,
}

impl LimitedBuffer {
    fn new(limit: usize) -> Self {
        Self { bytes: Vec::new(), limit }
    }
}

impl Write for LimitedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let required =
            self.bytes.len().checked_add(bytes.len()).filter(|&length| length <= self.limit).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "decoded output exceeds the 256 MiB limit")
            })?;
        if required > self.bytes.capacity() {
            let capacity = required.max(self.bytes.capacity().saturating_mul(2).min(self.limit));
            self.bytes.try_reserve_exact(capacity - self.bytes.len()).map_err(io::Error::other)?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn symbol_json(found: &FoundSymbol) -> JsonSymbol<'_> {
    let decoded = found.symbol.decoded();
    let version = match decoded.version() {
        Version::Normal(number) => JsonVersion { kind: "normal", number },
        Version::Micro(number) => JsonVersion { kind: "micro", number },
    };
    let ec_level = match decoded.ec_level() {
        EcLevel::L => "L",
        EcLevel::M => "M",
        EcLevel::Q => "Q",
        EcLevel::H => "H",
    };
    let structured_append = found.symbol.structured_append().map(|header| JsonHeader {
        position: header.position(),
        total: header.total(),
        parity: header.parity(),
    });
    JsonSymbol {
        index: found.index,
        candidate_index: found.candidate_index,
        source_image: found.source.to_string_lossy(),
        data: decoded.data(),
        text: std::str::from_utf8(decoded.data()).ok(),
        version,
        ec_level,
        structured_append,
    }
}

fn prepare_payloads(symbols: &[FoundSymbol], assemble: bool) -> Result<Vec<Payload<'_>>, Box<dyn Error>> {
    let mut payloads = Vec::new();
    let mut groups: BTreeMap<(u8, u8), Vec<&FoundSymbol>> = BTreeMap::new();
    for found in symbols {
        if assemble && let Some(header) = found.symbol.structured_append() {
            groups.entry((header.total(), header.parity())).or_default().push(found);
            continue;
        }
        payloads.push(Payload {
            data: Cow::Borrowed(found.symbol.decoded().data()),
            sources: vec![found.source.as_path()],
            symbol_indices: vec![found.index],
            assembly: None,
        });
    }
    for ((total, parity), members) in groups {
        let data = crate::structured_append::reassemble_decoded(members.iter().map(|member| &member.symbol))
            .map_err(|error| format!("Structured Append group (total={total}, parity={parity}): {error}"))?;
        payloads.push(Payload {
            data: Cow::Owned(data),
            sources: members.iter().map(|member| member.source.as_path()).collect(),
            symbol_indices: members.iter().map(|member| member.index).collect(),
            assembly: Some((total, parity)),
        });
    }
    payloads.sort_by_key(|payload| payload.symbol_indices[0]);
    Ok(payloads)
}

fn prepare_output<'a>(
    format: OutputFormat,
    symbols: &[FoundSymbol],
    payloads: &'a [Payload<'_>],
    failures: &[CandidateFailure],
) -> Result<Cow<'a, [u8]>, Box<dyn Error>> {
    match format {
        OutputFormat::Raw => {
            if payloads.len() != 1 {
                return Err("raw output requires exactly one logical payload; use --assemble for complete Structured Append groups".into());
            }
            Ok(Cow::Borrowed(payloads[0].data.as_ref()))
        }
        OutputFormat::Text => {
            for payload in payloads {
                std::str::from_utf8(&payload.data)
                    .map_err(|_| "text output requires valid UTF-8; use --format json or raw for binary payloads")?;
            }
            let length = payloads.iter().try_fold(0usize, |length, payload| {
                length
                    .checked_add(payload.data.len())
                    .and_then(|length| length.checked_add(1))
                    .ok_or("text output length exceeds the supported buffer size")
            })?;
            if length > qrcode_render::MAX_BUFFER_BYTES {
                return Err("decoded text output exceeds the 256 MiB limit".into());
            }
            let mut output = Vec::new();
            output.try_reserve_exact(length)?;
            for payload in payloads {
                output.extend_from_slice(&payload.data);
                output.push(b'\n');
            }
            Ok(Cow::Owned(output))
        }
        OutputFormat::Json => {
            let errors = failures
                .iter()
                .map(|failure| JsonFailure {
                    source_image: failure.source.to_string_lossy(),
                    candidate_index: failure.candidate_index,
                    message: &failure.message,
                })
                .collect();
            let assemblies = payloads
                .iter()
                .filter_map(|payload| {
                    payload.assembly.map(|(total, parity)| JsonAssembly {
                        total,
                        parity,
                        data: payload.data.as_ref(),
                        text: std::str::from_utf8(&payload.data).ok(),
                        symbol_indices: &payload.symbol_indices,
                        source_images: payload.sources.iter().map(|source| source.to_string_lossy()).collect(),
                    })
                })
                .collect();
            // Serialize byte slices directly, without building a Value::Number
            // tree or retaining another copy of every binary fragment.
            let report = JsonReport { symbols: symbols.iter().map(symbol_json).collect(), errors, assemblies };
            let mut output = LimitedBuffer::new(qrcode_render::MAX_BUFFER_BYTES);
            serde_json::to_writer(&mut output, &report)?;
            output.write_all(b"\n")?;
            Ok(Cow::Owned(output.bytes))
        }
    }
}

pub(super) fn decode_images(paths: &[PathBuf], options: DecodeOptions<'_>) -> Result<(), Box<dyn Error>> {
    let mut symbols = Vec::new();
    let mut failures = Vec::new();
    let mut decoded_bytes = 0;
    for path in paths {
        let image =
            load_gray(path, options.max_pixels).map_err(|error| super::path_error("read decode image", path, error))?;
        let candidates = RxingDecoder::new()
            .scan_with_options(GrayPixels::from(&image), scan_options(options.max_pixels, options.invert))
            .map_err(|error| super::path_error("scan decode image", path, error))?;
        collect_candidates(path, candidates, options.allow_partial, &mut symbols, &mut failures, &mut decoded_bytes)?;
    }
    require_symbols(&symbols, &failures)?;
    let payloads = prepare_payloads(&symbols, options.assemble)?;
    let output = prepare_output(options.format, &symbols, &payloads, &failures)?;
    match options.output {
        Some(path) if path != Path::new("-") => {
            super::atomic_output::write(path, &output)
                .map_err(|error| super::path_error("write output file", path, error))?;
        }
        _ => super::write_stdout(|writer| writer.write_all(&output))?,
    }
    if !failures.is_empty() {
        eprintln!("warning: {} QR candidate(s) failed; output contains available valid payloads", failures.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::cell::Cell;
    use std::io::Cursor;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn bounded_reader_accepts_exact_eof_and_rejects_extra_encoded_bytes() {
        let mut exact = BoundedReader::new(Cursor::new(b"abc"), 3);
        let mut bytes = Vec::new();
        exact.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"abc");
        let mut excess = BoundedReader::new(Cursor::new(b"abcd"), 3);
        bytes.clear();
        assert_eq!(excess.read_to_end(&mut bytes).unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_eq!(bytes, b"abc");
        assert_eq!(excess.read(&mut []).unwrap(), 0);
    }

    #[test]
    fn bounded_reader_preserves_seek_rewind_and_past_eof_behavior() {
        let mut reader = BoundedReader::new(Cursor::new(b"abc"), 3);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(reader.seek(SeekFrom::Start(0)).unwrap(), 0);
        bytes.clear();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"abc");
        assert_eq!(reader.seek(SeekFrom::End(-1)).unwrap(), 2);
        assert_eq!(reader.seek(SeekFrom::Current(-1)).unwrap(), 1);
        let mut one = [0];
        reader.read_exact(&mut one).unwrap();
        assert_eq!(one, [b'b']);
        assert_eq!(reader.seek(SeekFrom::Start(10)).unwrap(), 10);
        assert_eq!(reader.read(&mut one).unwrap(), 0);
    }

    static DIRECTORY_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("qrcode-image-loader-{}-{sequence}", std::process::id()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn bounded_reader_rejects_growth_after_initial_metadata_check() {
        let directory = TestDirectory::new();
        let path = directory.0.join("growing.png");
        std::fs::write(&path, b"abc").unwrap();
        let file = File::open(&path).unwrap();
        assert_eq!(file.metadata().unwrap().len(), 3);
        let mut reader = BoundedReader::new(file, 3);
        let mut bytes = [0; 3];
        reader.read_exact(&mut bytes).unwrap();
        File::options().append(true).open(&path).unwrap().write_all(b"d").unwrap();
        assert_eq!(reader.read(&mut [0]).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    struct MockDecoder {
        width: u32,
        height: u32,
        bytes: u64,
        panic_on_read: bool,
        limit: Rc<Cell<Option<u64>>>,
    }

    impl ImageDecoder for MockDecoder {
        fn dimensions(&self) -> (u32, u32) {
            (self.width, self.height)
        }
        fn color_type(&self) -> qrcode_image::image::ColorType {
            qrcode_image::image::ColorType::L8
        }
        fn total_bytes(&self) -> u64 {
            self.bytes
        }
        fn set_limits(&mut self, limits: Limits) -> qrcode_image::image::ImageResult<()> {
            self.limit.set(limits.max_alloc);
            Ok(())
        }
        fn read_image(self, buffer: &mut [u8]) -> qrcode_image::image::ImageResult<()> {
            assert!(!self.panic_on_read, "invalid budgets must not reach full decoding");
            buffer.fill(42);
            Ok(())
        }
        fn read_image_boxed(self: Box<Self>, buffer: &mut [u8]) -> qrcode_image::image::ImageResult<()> {
            (*self).read_image(buffer)
        }
    }

    #[test]
    fn decoder_budgets_are_validated_before_full_decoding_or_large_allocation() {
        for (width, height, bytes, max_pixels) in [
            (0, 1, 0, DEFAULT_MAX_PIXELS),
            (MAX_IMAGE_SIDE + 1, 1, 1, DEFAULT_MAX_PIXELS),
            (1024, 1024, 1, 100),
            (4096, 4096, qrcode_render::MAX_BUFFER_BYTES as u64, DEFAULT_MAX_PIXELS),
            (1, 1, u64::MAX, DEFAULT_MAX_PIXELS),
        ] {
            let decoder = MockDecoder { width, height, bytes, panic_on_read: true, limit: Rc::new(Cell::new(None)) };
            assert!(decoder_to_gray(decoder, image_limits(), max_pixels).is_err());
        }
    }

    #[test]
    fn manual_decoder_path_reserves_the_output_before_decoding() {
        let limit = Rc::new(Cell::new(None));
        let decoder = MockDecoder { width: 1, height: 1, bytes: 1, panic_on_read: false, limit: Rc::clone(&limit) };
        let image = decoder_to_gray(decoder, image_limits(), 1).unwrap();
        assert_eq!(image.as_raw(), &[42]);
        assert_eq!(limit.get(), Some(qrcode_render::MAX_BUFFER_BYTES as u64 - 1));
    }

    #[test]
    fn encoded_file_metadata_limit_rejects_sparse_oversized_files_before_header_reads() {
        let directory = TestDirectory::new();
        let path = directory.0.join("oversized.png");
        File::create(&path).unwrap().set_len(MAX_ENCODED_BYTES + 1).unwrap();
        assert!(load_gray(&path, DEFAULT_MAX_PIXELS).unwrap_err().to_string().contains("64 MiB"));
    }

    #[test]
    fn raw_bytes_are_exact_and_text_validates_every_payload_before_output() {
        let payloads = [Payload {
            data: Cow::Owned(vec![0, 255, 128, 10]),
            sources: Vec::new(),
            symbol_indices: vec![1],
            assembly: Some((2, 0)),
        }];
        let raw = prepare_output(OutputFormat::Raw, &[], &payloads, &[]).unwrap();
        assert_eq!(raw.as_ref(), &[0, 255, 128, 10]);
        assert!(matches!(raw, Cow::Borrowed(_)));
        assert!(prepare_output(OutputFormat::Text, &[], &payloads, &[]).is_err());
        let json = prepare_output(OutputFormat::Json, &[], &payloads, &[]).unwrap();
        let value: Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(value["assemblies"][0]["data"], json!([0, 255, 128, 10]));
        assert!(value["assemblies"][0]["text"].is_null());
        assert!(value.get("payloads").is_none());
    }

    #[test]
    fn partial_candidate_failures_are_retained_but_all_failed_is_never_success() {
        let mut symbols = Vec::new();
        let mut failures = Vec::new();
        let candidate = Err(DecodeError::InvalidOptions("mock failed candidate"));
        collect_candidates(Path::new("image.png"), vec![candidate.clone()], false, &mut symbols, &mut failures, &mut 0)
            .unwrap_err();
        collect_candidates(Path::new("image.png"), vec![candidate], true, &mut symbols, &mut failures, &mut 0).unwrap();
        assert_eq!(failures.len(), 1);
        assert!(require_symbols(&symbols, &failures).is_err());
    }

    #[test]
    fn output_limits_fail_without_partial_writes_or_large_allocations() {
        let mut output = LimitedBuffer::new(3);
        output.write_all(b"abc").unwrap();
        assert_eq!(output.write_all(b"d").unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_eq!(output.bytes, b"abc");
        let mut failures = (0..MAX_RETAINED_CANDIDATES)
            .map(|_| CandidateFailure {
                source: PathBuf::from("input.png"),
                candidate_index: 1,
                message: "failure".to_owned(),
            })
            .collect::<Vec<_>>();
        let error = collect_candidates(
            Path::new("input.png"),
            vec![Err(DecodeError::InvalidOptions("extra"))],
            true,
            &mut Vec::new(),
            &mut failures,
            &mut 0,
        )
        .unwrap_err();
        assert!(error.to_string().contains("4096 candidate"));
        assert_eq!(failures.len(), MAX_RETAINED_CANDIDATES);
    }
}
