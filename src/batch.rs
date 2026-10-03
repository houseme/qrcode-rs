//! Library-level helpers for batch rendering and packaging.
//!
//! The core encoder APIs expose [`QrCode::batch`](crate::QrCode::batch) and
//! streaming iterators. This module adds a small builder for callers that also
//! want stable file names, in-memory ZIP packaging, or PNG contact sheets
//! without going through the CLI.

use alloc::borrow::Cow;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use crate::{EcLevel, QrCode, QrError, QrResult};

/// Builder for library-level QR batch workflows.
///
/// The builder owns the input iterator until [`encode`](Self::encode),
/// [`render`](Self::render), or [`render_bytes`](Self::render_bytes) consumes it.
/// Generated entry names are stable and one-based by default:
/// `qr-0001.png`, `qr-0002.png`, and so on.
///
/// # Examples
///
/// ```rust
/// use qrcode_rs::batch::QrBatchBuilder;
///
/// let rendered = QrBatchBuilder::new(["alpha", "beta"])
///     .file_extension("txt")
///     .render::<char>()
///     .unwrap();
///
/// assert_eq!(rendered.entries()[0].name(), "qr-0001.txt");
/// assert_eq!(rendered.len(), 2);
/// ```
pub struct QrBatchBuilder<I> {
    inputs: I,
    ec_level: EcLevel,
    file_prefix: String,
    file_extension: String,
    start_index: usize,
}

impl<I> QrBatchBuilder<I> {
    /// Creates a batch builder over `inputs`.
    pub fn new(inputs: I) -> Self {
        Self {
            inputs,
            ec_level: EcLevel::M,
            file_prefix: "qr".to_string(),
            file_extension: "png".to_string(),
            start_index: 1,
        }
    }

    /// Sets the error-correction level used when encoding every input.
    #[must_use]
    pub fn ec_level(mut self, ec_level: EcLevel) -> Self {
        self.ec_level = ec_level;
        self
    }

    /// Sets the generated file-name prefix.
    ///
    /// The value is used as-is in names such as `ticket-0001.svg`. ZIP
    /// packaging validates final names before writing an archive.
    #[must_use]
    pub fn file_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.file_prefix = prefix.into();
        self
    }

    /// Sets the generated file-name extension without the leading dot.
    #[must_use]
    pub fn file_extension(mut self, extension: impl Into<String>) -> Self {
        self.file_extension = extension.into();
        self
    }

    /// Sets the first numeric suffix used in generated file names.
    ///
    /// Suffixes continue beyond `usize::MAX` without wrapping or repeating.
    #[must_use]
    pub const fn start_index(mut self, start_index: usize) -> Self {
        self.start_index = start_index;
        self
    }

    /// Encodes every input into a named [`QrCode`] entry.
    ///
    /// # Errors
    ///
    /// Returns the first encoding error in input order.
    pub fn encode(self) -> QrResult<BatchOutput<QrCode>>
    where
        I: IntoIterator,
        I::Item: AsRef<[u8]>,
    {
        let Self { inputs, ec_level, file_prefix, file_extension, start_index } = self;
        let mut entries = Vec::new();
        for (index, input) in inputs.into_iter().enumerate() {
            let name = batch_file_name(&file_prefix, start_index as u128 + index as u128, &file_extension);
            let code = QrCode::with_error_correction_level(input, ec_level)?;
            entries.push(BatchEntry { name, data: code });
        }
        Ok(BatchOutput { entries })
    }

    /// Encodes and renders every input with the selected pixel backend.
    ///
    /// # Errors
    ///
    /// Returns the first encoding error in input order.
    pub fn render<P>(self) -> QrResult<BatchOutput<P::Image>>
    where
        P: crate::render::Pixel,
        I: IntoIterator,
        I::Item: AsRef<[u8]>,
    {
        let Self { inputs, ec_level, file_prefix, file_extension, start_index } = self;
        let mut entries = Vec::new();
        for (index, input) in inputs.into_iter().enumerate() {
            let name = batch_file_name(&file_prefix, start_index as u128 + index as u128, &file_extension);
            let image = QrCode::with_error_correction_level(input, ec_level)?.render::<P>().build();
            entries.push(BatchEntry { name, data: image });
        }
        Ok(BatchOutput { entries })
    }

    /// Encodes every input and renders it to bytes with a caller-supplied
    /// renderer.
    ///
    /// This is the bridge for SVG/PDF/PNG/custom formats where the final byte
    /// encoding is controlled by the application.
    ///
    /// # Errors
    ///
    /// Returns [`BatchRenderError::Encode`] for QR encoding failures or
    /// [`BatchRenderError::Render`] for errors returned by `render`.
    pub fn render_bytes<F, E>(self, mut render: F) -> Result<BatchOutput<Vec<u8>>, BatchRenderError<E>>
    where
        I: IntoIterator,
        I::Item: AsRef<[u8]>,
        F: FnMut(&QrCode, usize) -> Result<Vec<u8>, E>,
    {
        let Self { inputs, ec_level, file_prefix, file_extension, start_index } = self;
        let mut entries = Vec::new();
        for (index, input) in inputs.into_iter().enumerate() {
            let code = QrCode::with_error_correction_level(input, ec_level).map_err(BatchRenderError::Encode)?;
            let name = batch_file_name(&file_prefix, start_index as u128 + index as u128, &file_extension);
            let bytes = render(&code, index).map_err(BatchRenderError::Render)?;
            entries.push(BatchEntry { name, data: bytes });
        }
        Ok(BatchOutput { entries })
    }
}

/// One named result in a batch output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchEntry<T> {
    name: String,
    data: T,
}

impl<T> BatchEntry<T> {
    /// Creates a new named batch entry.
    pub fn new(name: impl Into<String>, data: T) -> Self {
        Self { name: name.into(), data }
    }

    /// Returns the generated or caller-supplied entry name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the entry payload.
    #[must_use]
    pub const fn data(&self) -> &T {
        &self.data
    }

    /// Consumes the entry and returns its payload.
    #[must_use]
    pub fn into_data(self) -> T {
        self.data
    }
}

/// Named results produced by a batch workflow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchOutput<T> {
    entries: Vec<BatchEntry<T>>,
}

impl<T> BatchOutput<T> {
    /// Creates a batch output from pre-rendered named entries.
    pub fn from_entries(entries: impl IntoIterator<Item = BatchEntry<T>>) -> Self {
        Self { entries: entries.into_iter().collect() }
    }

    /// Returns all named entries.
    #[must_use]
    pub fn entries(&self) -> &[BatchEntry<T>] {
        &self.entries
    }

    /// Returns an iterator over named entries.
    pub fn iter(&self) -> core::slice::Iter<'_, BatchEntry<T>> {
        self.entries.iter()
    }

    /// Returns the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true when there are no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Consumes the batch output and returns its entries.
    #[must_use]
    pub fn into_entries(self) -> Vec<BatchEntry<T>> {
        self.entries
    }

    /// Maps every entry payload while preserving names and order.
    ///
    /// # Errors
    ///
    /// Returns the first error from `map` and stops processing further entries.
    pub fn try_map<U, E>(self, mut map: impl FnMut(T) -> Result<U, E>) -> Result<BatchOutput<U>, E> {
        let mut entries = Vec::with_capacity(self.entries.len());
        for entry in self.entries {
            entries.push(BatchEntry { name: entry.name, data: map(entry.data)? });
        }
        Ok(BatchOutput { entries })
    }
}

impl BatchOutput<Vec<u8>> {
    /// Packages byte entries into an uncompressed ZIP archive.
    ///
    /// # Errors
    ///
    /// Returns [`BatchPackError`] when names are invalid or the archive exceeds
    /// classic ZIP size/count limits.
    pub fn to_zip(&self) -> Result<Vec<u8>, BatchPackError> {
        self.to_zip_with(ZipCompression::Stored)
    }

    /// Packages byte entries into a ZIP archive with the selected compression.
    ///
    /// # Errors
    ///
    /// Returns [`BatchPackError`] when names are invalid, compression fails, or
    /// the archive exceeds classic ZIP size/count limits.
    pub fn to_zip_with(&self, compression: ZipCompression) -> Result<Vec<u8>, BatchPackError> {
        let mut writer = ZipBytesWriter::new();
        for entry in &self.entries {
            writer.write_file(entry.name(), entry.data(), compression)?;
        }
        writer.finish()
    }
}

#[cfg(feature = "image")]
impl BatchOutput<crate::render::image::RgbaImage> {
    /// Encodes rendered RGBA images into a single PNG contact sheet.
    ///
    /// # Errors
    ///
    /// Returns [`BatchPackError::EmptyBatch`] for an empty batch,
    /// [`BatchPackError::GridTooLarge`] if the sheet dimensions overflow,
    /// exceed 65,535 pixels per side or 268,435,456 total pixels, or its pixel
    /// buffer cannot be allocated, or
    /// [`BatchPackError::Image`] if PNG encoding fails.
    pub fn to_png_grid(&self, options: BatchGridOptions) -> Result<Vec<u8>, BatchPackError> {
        encode_png_grid(self.entries.iter().map(BatchEntry::data), options)
    }
}

/// PNG contact-sheet layout options for batch images.
#[cfg(feature = "image")]
#[cfg_attr(docsrs, doc(cfg(feature = "image")))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatchGridOptions {
    /// Requested column count. Zero picks the smallest square-ish grid.
    pub columns: usize,
    /// RGBA background used for unused cell space.
    pub background: [u8; 4],
}

#[cfg(feature = "image")]
impl Default for BatchGridOptions {
    fn default() -> Self {
        Self { columns: 0, background: [255, 255, 255, 255] }
    }
}

#[cfg(feature = "image")]
impl BatchGridOptions {
    /// Sets the requested column count. Zero enables automatic columns.
    #[must_use]
    pub const fn columns(mut self, columns: usize) -> Self {
        self.columns = columns;
        self
    }

    /// Sets the RGBA background color.
    #[must_use]
    pub const fn background(mut self, background: [u8; 4]) -> Self {
        self.background = background;
        self
    }
}

/// ZIP compression method for batch byte packaging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZipCompression {
    /// Store entries without compression.
    Stored,
    /// Compress entries with DEFLATE.
    #[cfg(feature = "batch-zip-deflate")]
    #[cfg_attr(docsrs, doc(cfg(feature = "batch-zip-deflate")))]
    Deflated,
}

/// Errors returned by batch byte rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BatchRenderError<E> {
    /// Encoding an input as QR modules failed.
    Encode(QrError),
    /// The caller-supplied byte renderer failed.
    Render(E),
}

impl<E: fmt::Display> fmt::Display for BatchRenderError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => error.fmt(f),
            Self::Render(error) => error.fmt(f),
        }
    }
}

impl<E> std::error::Error for BatchRenderError<E> where E: std::error::Error + 'static {}

/// Errors returned by library-level batch packaging helpers.
#[derive(Debug)]
pub enum BatchPackError {
    /// The batch has no entries.
    EmptyBatch,
    /// A ZIP entry name is empty, absolute, contains backslashes, or contains
    /// `.` / `..` path components.
    InvalidEntryName {
        /// Invalid entry name.
        name: String,
    },
    /// A ZIP entry name exceeds the classic ZIP `u16` length field.
    EntryNameTooLong {
        /// Invalid entry name.
        name: String,
        /// Entry name length in bytes.
        len: usize,
    },
    /// A ZIP entry is too large for classic ZIP.
    EntryTooLarge {
        /// Entry name.
        name: String,
        /// Entry payload length in bytes.
        len: usize,
    },
    /// The ZIP archive is too large for classic ZIP.
    ArchiveTooLarge,
    /// The ZIP archive has more entries than classic ZIP supports.
    TooManyEntries {
        /// Entry count.
        count: usize,
    },
    /// The PNG contact sheet exceeds its dimension or allocation budget.
    #[cfg(feature = "image")]
    GridTooLarge,
    /// PNG encoding failed.
    #[cfg(feature = "image")]
    Image(crate::render::image::image::ImageError),
    /// Compression failed while building a ZIP archive.
    Io(std::io::Error),
}

impl fmt::Display for BatchPackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBatch => f.write_str("batch has no entries"),
            Self::InvalidEntryName { name } => write!(f, "invalid ZIP entry name: {name}"),
            Self::EntryNameTooLong { name, len } => {
                write!(f, "ZIP entry name is too long: {name} ({len} bytes)")
            }
            Self::EntryTooLarge { name, len } => write!(f, "ZIP entry is too large: {name} ({len} bytes)"),
            Self::ArchiveTooLarge => f.write_str("ZIP archive is larger than 4 GiB"),
            Self::TooManyEntries { count } => write!(f, "ZIP archive has too many entries: {count}"),
            #[cfg(feature = "image")]
            Self::GridTooLarge => f.write_str("PNG grid dimensions are too large"),
            #[cfg(feature = "image")]
            Self::Image(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for BatchPackError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            #[cfg(feature = "image")]
            Self::Image(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for BatchPackError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(feature = "image")]
impl From<crate::render::image::image::ImageError> for BatchPackError {
    fn from(error: crate::render::image::image::ImageError) -> Self {
        Self::Image(error)
    }
}

fn batch_file_name(prefix: &str, index: u128, extension: &str) -> String {
    format!("{prefix}-{index:04}.{extension}")
}

fn validate_zip_name(name: &str) -> Result<(), BatchPackError> {
    if name.is_empty() || name.starts_with('/') || name.contains('\\') {
        return Err(BatchPackError::InvalidEntryName { name: name.to_string() });
    }
    if name.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
        return Err(BatchPackError::InvalidEntryName { name: name.to_string() });
    }
    Ok(())
}

struct ZipBytesWriter {
    bytes: Vec<u8>,
    offset: u64,
    entries: Vec<ZipEntry>,
}

struct ZipEntry {
    name: String,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    compression_method: u16,
    local_header_offset: u32,
}

impl ZipBytesWriter {
    fn new() -> Self {
        Self { bytes: Vec::new(), offset: 0, entries: Vec::new() }
    }

    fn write_file(&mut self, name: &str, bytes: &[u8], compression: ZipCompression) -> Result<(), BatchPackError> {
        validate_zip_name(name)?;
        let name_bytes = name.as_bytes();
        let name_len = u16::try_from(name_bytes.len())
            .map_err(|_| BatchPackError::EntryNameTooLong { name: name.to_string(), len: name_bytes.len() })?;
        let uncompressed_size = u32::try_from(bytes.len())
            .map_err(|_| BatchPackError::EntryTooLarge { name: name.to_string(), len: bytes.len() })?;
        let local_header_offset = u32::try_from(self.offset).map_err(|_| BatchPackError::ArchiveTooLarge)?;
        let crc32 = crc32(bytes);
        let (compression_method, payload) = compress_zip_payload(bytes, compression)?;
        let compressed_size = u32::try_from(payload.len())
            .map_err(|_| BatchPackError::EntryTooLarge { name: name.to_string(), len: payload.len() })?;

        self.write_u32(0x0403_4b50);
        self.write_u16(20);
        self.write_u16(1 << 11); // Entry names are encoded as UTF-8.
        self.write_u16(compression_method);
        self.write_u16(0);
        self.write_u16(0);
        self.write_u32(crc32);
        self.write_u32(compressed_size);
        self.write_u32(uncompressed_size);
        self.write_u16(name_len);
        self.write_u16(0);
        self.write_all(name_bytes);
        self.write_all(&payload);
        self.entries.push(ZipEntry {
            name: name.to_string(),
            crc32,
            compressed_size,
            uncompressed_size,
            compression_method,
            local_header_offset,
        });
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<u8>, BatchPackError> {
        let central_dir_offset = u32::try_from(self.offset).map_err(|_| BatchPackError::ArchiveTooLarge)?;
        let entry_count = u16::try_from(self.entries.len())
            .map_err(|_| BatchPackError::TooManyEntries { count: self.entries.len() })?;

        for index in 0..self.entries.len() {
            let name = self.entries[index].name.clone();
            let name_bytes = name.as_bytes();
            let name_len = u16::try_from(name_bytes.len())
                .map_err(|_| BatchPackError::EntryNameTooLong { name: name.clone(), len: name_bytes.len() })?;
            let compression_method = self.entries[index].compression_method;
            let crc32 = self.entries[index].crc32;
            let compressed_size = self.entries[index].compressed_size;
            let uncompressed_size = self.entries[index].uncompressed_size;
            let local_header_offset = self.entries[index].local_header_offset;

            self.write_u32(0x0201_4b50);
            self.write_u16(20);
            self.write_u16(20);
            self.write_u16(1 << 11); // Match the local header's UTF-8 flag.
            self.write_u16(compression_method);
            self.write_u16(0);
            self.write_u16(0);
            self.write_u32(crc32);
            self.write_u32(compressed_size);
            self.write_u32(uncompressed_size);
            self.write_u16(name_len);
            self.write_u16(0);
            self.write_u16(0);
            self.write_u16(0);
            self.write_u16(0);
            self.write_u32(0);
            self.write_u32(local_header_offset);
            self.write_all(name_bytes);
        }

        let central_dir_size = self
            .offset
            .checked_sub(u64::from(central_dir_offset))
            .and_then(|size| u32::try_from(size).ok())
            .ok_or(BatchPackError::ArchiveTooLarge)?;
        self.write_u32(0x0605_4b50);
        self.write_u16(0);
        self.write_u16(0);
        self.write_u16(entry_count);
        self.write_u16(entry_count);
        self.write_u32(central_dir_size);
        self.write_u32(central_dir_offset);
        self.write_u16(0);
        Ok(self.bytes)
    }

    fn write_all(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
        self.offset += bytes.len() as u64;
    }

    fn write_u16(&mut self, value: u16) {
        self.write_all(&value.to_le_bytes());
    }

    fn write_u32(&mut self, value: u32) {
        self.write_all(&value.to_le_bytes());
    }
}

fn compress_zip_payload(bytes: &[u8], compression: ZipCompression) -> Result<(u16, Cow<'_, [u8]>), BatchPackError> {
    match compression {
        ZipCompression::Stored => Ok((0, Cow::Borrowed(bytes))),
        #[cfg(feature = "batch-zip-deflate")]
        ZipCompression::Deflated => {
            use std::io::Write;

            let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(bytes)?;
            Ok((8, Cow::Owned(encoder.finish()?)))
        }
    }
}

const CRC32_TABLE: [u32; 256] = {
    let mut table = [0; 256];
    let mut index = 0;
    while index < table.len() {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            let mask = 0u32.wrapping_sub(value & 1);
            value = (value >> 1) ^ (0xedb8_8320 & mask);
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
};

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff;
    for &byte in bytes {
        crc = (crc >> 8) ^ CRC32_TABLE[((crc ^ u32::from(byte)) & 0xff) as usize];
    }
    !crc
}

#[cfg(feature = "image")]
fn encode_png_grid<'a>(
    images: impl IntoIterator<Item = &'a crate::render::image::RgbaImage>,
    options: BatchGridOptions,
) -> Result<Vec<u8>, BatchPackError> {
    use crate::render::image::{DynamicImage, ImageFormat, Rgba, RgbaImage, encode_to_format};

    let images = images.into_iter().collect::<Vec<_>>();
    if images.is_empty() {
        return Err(BatchPackError::EmptyBatch);
    }
    let columns_usize = grid_columns(options.columns, images.len())?;
    let rows_usize = images.len().div_ceil(columns_usize);
    let cell_width = images.iter().map(|image| image.width()).max().unwrap_or(1);
    let cell_height = images.iter().map(|image| image.height()).max().unwrap_or(1);
    let columns = u32::try_from(columns_usize).map_err(|_| BatchPackError::GridTooLarge)?;
    let rows = u32::try_from(rows_usize).map_err(|_| BatchPackError::GridTooLarge)?;
    let sheet_width = cell_width.checked_mul(columns).ok_or(BatchPackError::GridTooLarge)?;
    let sheet_height = cell_height.checked_mul(rows).ok_or(BatchPackError::GridTooLarge)?;
    let buffer_len = grid_buffer_len(sheet_width, sheet_height)?;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(buffer_len).map_err(|_| BatchPackError::GridTooLarge)?;
    pixels.resize(buffer_len, 0);
    let mut sheet = RgbaImage::from_raw(sheet_width, sheet_height, pixels).ok_or(BatchPackError::GridTooLarge)?;
    for pixel in sheet.pixels_mut() {
        *pixel = Rgba(options.background);
    }

    for (index, &image) in images.iter().enumerate() {
        let col = u32::try_from(index % columns_usize).map_err(|_| BatchPackError::GridTooLarge)?;
        let row = u32::try_from(index / columns_usize).map_err(|_| BatchPackError::GridTooLarge)?;
        let left = col * cell_width + (cell_width - image.width()) / 2;
        let top = row * cell_height + (cell_height - image.height()) / 2;
        crate::render::image::image::imageops::replace(&mut sheet, image, i64::from(left), i64::from(top));
    }

    encode_to_format(&DynamicImage::ImageRgba8(sheet), ImageFormat::Png).map_err(Into::into)
}

#[cfg(feature = "image")]
fn grid_buffer_len(width: u32, height: u32) -> Result<usize, BatchPackError> {
    const MAX_GRID_SIDE: u32 = 65_535;
    const MAX_GRID_PIXELS: u64 = 268_435_456;

    let pixels = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || width > MAX_GRID_SIDE || height > MAX_GRID_SIDE || pixels > MAX_GRID_PIXELS {
        return Err(BatchPackError::GridTooLarge);
    }
    let bytes = pixels.checked_mul(4).ok_or(BatchPackError::GridTooLarge)?;
    let len = usize::try_from(bytes).map_err(|_| BatchPackError::GridTooLarge)?;
    if len > isize::MAX as usize {
        return Err(BatchPackError::GridTooLarge);
    }
    Ok(len)
}

#[cfg(feature = "image")]
fn grid_columns(requested: usize, count: usize) -> Result<usize, BatchPackError> {
    if count == 0 {
        return Err(BatchPackError::EmptyBatch);
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn crc32_matches_ieee_vectors() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(b"abc"), 0x3524_41c2);
    }

    #[test]
    fn crc32_matches_bitwise_oracle_for_prefixes_and_unaligned_slices() {
        let bytes = (0..65_539_usize).map(|index| (index.wrapping_mul(73) % 256) as u8).collect::<Vec<_>>();
        for length in (0..=256).chain([511, 512, 1023, 1024, 32_768, 65_536]) {
            for offset in 0..3 {
                let input = &bytes[offset..offset + length];
                assert_eq!(crc32(input), crc32_bitwise(input), "offset {offset}, length {length}");
            }
        }
    }

    #[test]
    fn builder_renders_entries_with_stable_names() {
        let rendered = QrBatchBuilder::new(["alpha", "beta"])
            .file_prefix("ticket")
            .file_extension("txt")
            .render::<char>()
            .unwrap();

        assert_eq!(rendered.entries()[1].name(), "ticket-0002.txt");
    }

    #[test]
    fn builder_indices_continue_past_usize_max_in_all_output_paths() {
        let encoded = QrBatchBuilder::new(["alpha", "beta"]).start_index(usize::MAX).encode().unwrap();
        let rendered = QrBatchBuilder::new(["alpha", "beta"]).start_index(usize::MAX).render::<char>().unwrap();
        let bytes = QrBatchBuilder::new(["alpha", "beta"])
            .start_index(usize::MAX)
            .render_bytes(|_, _| Ok::<_, core::convert::Infallible>(Vec::new()))
            .unwrap();
        let expected = format!("qr-{}.png", usize::MAX as u128 + 1);
        assert_eq!(encoded.entries()[1].name(), expected);
        assert_eq!(rendered.entries()[1].name(), expected);
        assert_eq!(bytes.entries()[1].name(), expected);
    }

    #[test]
    fn byte_batch_writes_stored_zip_archive() {
        let files = BatchOutput::from_entries([
            BatchEntry::new("qr-0001.svg", b"<svg>alpha</svg>".to_vec()),
            BatchEntry::new("qr-0002.svg", b"<svg>beta</svg>".to_vec()),
        ]);

        let archive = files.to_zip().unwrap();

        assert!(archive.starts_with(b"PK\x03\x04"));
        assert!(archive.windows(b"qr-0001.svg".len()).any(|window| window == b"qr-0001.svg"));
        assert!(archive.windows(b"<svg>beta</svg>".len()).any(|window| window == b"<svg>beta</svg>"));
    }

    #[test]
    fn stored_zip_payload_borrows_the_original_bytes() {
        let payload = b"stored payload";
        let (_, bytes) = compress_zip_payload(payload, ZipCompression::Stored).unwrap();
        assert!(matches!(bytes, Cow::Borrowed(_)));
        assert_eq!(bytes.as_ptr(), payload.as_ptr());
    }

    #[test]
    fn zip_marks_unicode_names_as_utf8_in_both_headers() {
        let name = "二维码.svg";
        let payload = b"<svg>alpha</svg>";
        let files = BatchOutput::from_entries([BatchEntry::new(name, payload.to_vec())]);
        let archive = files.to_zip().unwrap();
        let central_offset = 30 + name.len() + payload.len();

        assert_eq!(u16::from_le_bytes(archive[6..8].try_into().unwrap()), 1 << 11);
        assert_eq!(&archive[30..30 + name.len()], name.as_bytes());
        assert_eq!(&archive[central_offset..central_offset + 4], b"PK\x01\x02");
        assert_eq!(u16::from_le_bytes(archive[central_offset + 8..central_offset + 10].try_into().unwrap()), 1 << 11);
        assert_eq!(&archive[central_offset + 46..central_offset + 46 + name.len()], name.as_bytes());
        assert_eq!(&archive[30 + name.len()..central_offset], payload);
    }

    #[cfg(feature = "batch-zip-deflate")]
    #[test]
    fn byte_batch_writes_deflated_zip_archive() {
        let files = BatchOutput::from_entries([BatchEntry::new("qr-0001.txt", b"aaaaaaaaaaaaaaaaaaaa".to_vec())]);

        let archive = files.to_zip_with(ZipCompression::Deflated).unwrap();

        assert_eq!(&archive[8..10], &[8, 0]);
        assert!(archive.windows(b"qr-0001.txt".len()).any(|window| window == b"qr-0001.txt"));
    }

    #[test]
    fn zip_rejects_traversal_entry_names() {
        let files = BatchOutput::from_entries([BatchEntry::new("../qr.svg", b"bad".to_vec())]);

        let error = files.to_zip().unwrap_err();

        assert!(matches!(error, BatchPackError::InvalidEntryName { .. }));
    }

    #[cfg(feature = "image")]
    #[test]
    fn image_batch_writes_png_contact_sheet() {
        use crate::render::image::{Rgba, image};

        let rendered =
            QrBatchBuilder::new(["alpha", "beta", "gamma"]).file_extension("png").render::<Rgba<u8>>().unwrap();

        let png = rendered.to_png_grid(BatchGridOptions::default().columns(2)).unwrap();
        let image = image::load_from_memory(&png).unwrap();

        assert!(image.width() > image.height() / 2);
    }

    #[cfg(feature = "image")]
    #[test]
    fn png_grid_rejects_excessive_dimensions_before_allocation() {
        use crate::render::image::{Rgba, RgbaImage};

        let image = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]));
        let files = BatchOutput::from_entries([BatchEntry::new("qr.png", image)]);
        assert!(matches!(
            files.to_png_grid(BatchGridOptions::default().columns(65_536)),
            Err(BatchPackError::GridTooLarge)
        ));
        assert!(matches!(grid_buffer_len(16_385, 16_385), Err(BatchPackError::GridTooLarge)));
        assert!(matches!(grid_buffer_len(0, 1), Err(BatchPackError::GridTooLarge)));
        assert_eq!(grid_buffer_len(16_384, 16_384).unwrap(), 1_073_741_824);
    }

    #[cfg(feature = "image")]
    #[test]
    fn png_grid_keeps_pixels_and_fills_unused_cells() {
        use crate::render::image::{Rgba, RgbaImage, image};

        let first = RgbaImage::from_pixel(1, 1, Rgba([1, 2, 3, 255]));
        let second = RgbaImage::from_pixel(1, 1, Rgba([4, 5, 6, 255]));
        let files = BatchOutput::from_entries([BatchEntry::new("a.png", first), BatchEntry::new("b.png", second)]);
        let options = BatchGridOptions::default().columns(3).background([7, 8, 9, 255]);
        let png = files.to_png_grid(options).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (3, 1));
        assert_eq!(decoded.get_pixel(0, 0).0, [1, 2, 3, 255]);
        assert_eq!(decoded.get_pixel(1, 0).0, [4, 5, 6, 255]);
        assert_eq!(decoded.get_pixel(2, 0).0, [7, 8, 9, 255]);
    }
}
