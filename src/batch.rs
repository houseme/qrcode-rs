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

    /// Encodes, renders and writes each input directly to a Stored ZIP archive.
    ///
    /// Inputs are consumed lazily in order. Only one QR code and rendered
    /// payload are retained at a time, plus the ZIP central-directory metadata.
    /// The input iterator and renderer may retain additional storage.
    /// `render` receives the code and zero-based input index, as with
    /// [`Self::render_bytes`]. Generated names use this builder's settings.
    ///
    /// The archive must start at offset zero in `output`. Use a buffered writer
    /// for files or sockets. This method does not flush or close the output;
    /// errors may leave a partial archive. The caller controls cleanup and
    /// atomic publication. An empty iterator produces an empty ZIP archive.
    ///
    /// # Errors
    ///
    /// Each input is encoded, rendered and packed before reading the next one.
    /// Returns the first [`BatchZipError`] from these steps or finalization.
    pub fn render_zip<W, F, E>(self, output: &mut W, render: F) -> Result<(), BatchZipError<E>>
    where
        W: std::io::Write + ?Sized,
        I: IntoIterator,
        I::Item: AsRef<[u8]>,
        F: FnMut(&QrCode, usize) -> Result<Vec<u8>, E>,
    {
        self.render_zip_with(output, ZipCompression::Stored, render)
    }

    /// Encodes, renders and writes inputs directly to ZIP with compression.
    ///
    /// Uses the same lazy-input, naming and partial-output contract as
    /// [`Self::render_zip`]. Deflated entries additionally retain one compressed
    /// payload; previously rendered entries are not kept in memory.
    ///
    /// # Errors
    ///
    /// Returns the first encoding, rendering or packaging error in input order,
    /// or a packaging error when finalizing the archive.
    pub fn render_zip_with<W, F, E>(
        self,
        output: &mut W,
        compression: ZipCompression,
        mut render: F,
    ) -> Result<(), BatchZipError<E>>
    where
        W: std::io::Write + ?Sized,
        I: IntoIterator,
        I::Item: AsRef<[u8]>,
        F: FnMut(&QrCode, usize) -> Result<Vec<u8>, E>,
    {
        let Self { inputs, ec_level, file_prefix, file_extension, start_index } = self;
        let mut writer = ZipBytesWriter::new(output);
        for (index, input) in inputs.into_iter().enumerate() {
            let code = QrCode::with_error_correction_level(input, ec_level).map_err(BatchZipError::Encode)?;
            let bytes = render(&code, index).map_err(BatchZipError::Render)?;
            let name = batch_file_name(&file_prefix, start_index as u128 + index as u128, &file_extension);
            writer.write_file(&name, &bytes, compression).map_err(BatchZipError::Pack)?;
        }
        writer.finish().map_err(BatchZipError::Pack)?;
        Ok(())
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
        let mut writer = ZipBytesWriter::new(Vec::new());
        for entry in &self.entries {
            writer.write_file(entry.name(), entry.data(), compression)?;
        }
        writer.finish()
    }

    /// Writes an uncompressed ZIP directly to a caller-supplied output.
    ///
    /// Unlike [`Self::to_zip`], this does not allocate a buffer for the complete
    /// archive. Use a buffered output for files or sockets. The archive must
    /// start at offset zero in the output; seeking is not required.
    ///
    /// The output is neither flushed nor closed. On error it may contain a
    /// partial archive; the caller controls cleanup or atomic publication.
    ///
    /// # Errors
    ///
    /// Returns the same validation errors as [`Self::to_zip`], or
    /// [`BatchPackError::Io`] if writing fails.
    pub fn write_zip<W: std::io::Write + ?Sized>(&self, output: &mut W) -> Result<(), BatchPackError> {
        self.write_zip_with(output, ZipCompression::Stored)
    }

    /// Writes a ZIP directly to an output with the selected compression.
    ///
    /// Has the same output ownership and partial-write contract as
    /// [`Self::write_zip`]. Compressed entries require a temporary buffer for
    /// one entry, while Stored entries borrow their original payloads.
    ///
    /// # Errors
    ///
    /// Returns validation/compression errors from [`Self::to_zip_with`], or
    /// [`BatchPackError::Io`] if writing fails.
    pub fn write_zip_with<W: std::io::Write + ?Sized>(
        &self,
        output: &mut W,
        compression: ZipCompression,
    ) -> Result<(), BatchPackError> {
        let mut writer = ZipBytesWriter::new(output);
        for entry in &self.entries {
            writer.write_file(entry.name(), entry.data(), compression)?;
        }
        writer.finish()?;
        Ok(())
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

#[cfg(feature = "image")]
impl BatchOutput<QrCode> {
    /// Renders encoded QR symbols directly into a PNG contact sheet.
    ///
    /// Uses [`QrTemplate::minimal`](crate::QrTemplate::minimal), with 8×8
    /// modules and each symbol's standard quiet zone. Only the final RGBA
    /// canvas is allocated; individual tile images are not retained.
    ///
    /// # Errors
    ///
    /// Returns [`BatchPackError::EmptyBatch`] for an empty batch,
    /// [`BatchPackError::GridTooLarge`] if dimensions overflow, exceed 65,535
    /// pixels per side or the shared 256 MiB canvas budget, or allocation
    /// fails, and [`BatchPackError::Image`] if PNG encoding fails.
    ///
    /// The existing grid API for pre-rendered RGBA images retains its separate
    /// 1 GiB pixel-buffer limit.
    pub fn to_png_grid(&self, options: BatchGridOptions) -> Result<Vec<u8>, BatchPackError> {
        self.to_png_grid_with(options, &crate::QrTemplate::minimal())
    }

    /// Renders encoded QR symbols directly into a styled PNG contact sheet.
    ///
    /// Produces the same pixels as rendering each symbol with
    /// `render::<Rgba<u8>>().template(template)` and passing those images to
    /// the existing grid API. Tiles are centered in cells sized for the
    /// largest symbol. Pixels are replaced without alpha blending; unused
    /// cell space retains [`BatchGridOptions::background`].
    ///
    /// # Errors
    ///
    /// Returns the same errors and observes the same 256 MiB canvas budget as
    /// [`Self::to_png_grid`].
    pub fn to_png_grid_with(
        &self,
        options: BatchGridOptions,
        template: &crate::QrTemplate,
    ) -> Result<Vec<u8>, BatchPackError> {
        use crate::render::image::{DynamicImage, ImageFormat, Rgba, RgbaImage, encode_to_format};
        use crate::render::{Canvas, Pixel, StyledPixel};

        let (module_width, module_height) = template.module_size.unwrap_or_else(Rgba::<u8>::default_unit_size);
        let style = QrGridStyle {
            module_width: module_width.max(1),
            module_height: module_height.max(1),
            quiet_zone: template.quiet_zone,
            dark: Rgba::<u8>::from_hex(&template.dark_color),
            light: Rgba::<u8>::from_hex(&template.light_color),
        };
        let (cell_width, cell_height) =
            self.entries.iter().try_fold((0u32, 0u32), |(max_width, max_height), entry| {
                let (width, height, _) = style.dimensions(entry.data())?;
                Ok::<_, BatchPackError>((max_width.max(width), max_height.max(height)))
            })?;
        let geometry =
            GridGeometry::new(self.len(), cell_width, cell_height, options.columns, qrcode_render::MAX_BUFFER_BYTES)?;
        <(Rgba<u8>, RgbaImage) as Canvas>::validate_dimensions(
            geometry.width,
            geometry.height,
            &style.dark,
            &style.light,
        )
        .map_err(|_| BatchPackError::GridTooLarge)?;
        let sheet = allocate_grid(&geometry)?;
        let mut canvas = (Rgba(options.background), sheet);
        canvas.draw_dark_rect(0, 0, geometry.width, geometry.height);

        for (index, entry) in self.entries.iter().enumerate() {
            let code = entry.data();
            let (width, height, quiet_zone) = style.dimensions(code)?;
            let (left, top) = geometry.offset(index, width, height);
            canvas.0 = style.light;
            canvas.draw_dark_rect(left, top, width, height);
            canvas.0 = style.dark;

            for (y, row) in code.colors().chunks_exact(code.width()).enumerate() {
                let mut x = 0;
                while x < row.len() {
                    if row[x] == crate::Color::Light {
                        x += 1;
                        continue;
                    }
                    let start = x;
                    while x < row.len() && row[x] != crate::Color::Light {
                        x += 1;
                    }
                    canvas.draw_dark_rect(
                        left + (quiet_zone + start as u32) * style.module_width,
                        top + (quiet_zone + y as u32) * style.module_height,
                        (x - start) as u32 * style.module_width,
                        style.module_height,
                    );
                }
            }
        }

        encode_to_format(&DynamicImage::ImageRgba8(canvas.into_image()), ImageFormat::Png).map_err(Into::into)
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

/// Errors returned by a batch that encodes, renders and writes a ZIP stream.
#[derive(Debug)]
#[non_exhaustive]
pub enum BatchZipError<E> {
    /// Encoding the current input failed.
    Encode(QrError),
    /// Rendering the current code failed.
    Render(E),
    /// ZIP name validation, compression, output writing or finalization failed.
    Pack(BatchPackError),
}

impl<E: fmt::Display> fmt::Display for BatchZipError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => error.fmt(f),
            Self::Render(error) => error.fmt(f),
            Self::Pack(error) => error.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for BatchZipError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Encode(error) => error,
            Self::Render(error) => error,
            Self::Pack(error) => error,
        })
    }
}

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
    /// Compression or output writing failed while building a ZIP archive.
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

struct ZipBytesWriter<W> {
    output: W,
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

impl<W: std::io::Write> ZipBytesWriter<W> {
    fn new(output: W) -> Self {
        Self { output, offset: 0, entries: Vec::new() }
    }

    fn write_file(&mut self, name: &str, bytes: &[u8], compression: ZipCompression) -> Result<(), BatchPackError> {
        validate_zip_name(name)?;
        let name_bytes = name.as_bytes();
        let name_len = u16::try_from(name_bytes.len())
            .map_err(|_| BatchPackError::EntryNameTooLong { name: name.to_string(), len: name_bytes.len() })?;
        let uncompressed_size = u32::try_from(bytes.len())
            .map_err(|_| BatchPackError::EntryTooLarge { name: name.to_string(), len: bytes.len() })?;
        let local_header_offset = u32::try_from(self.offset).map_err(|_| BatchPackError::ArchiveTooLarge)?;
        let crc32 = zip_crc32(bytes);
        let (compression_method, payload) = compress_zip_payload(bytes, compression)?;
        let compressed_size = u32::try_from(payload.len())
            .map_err(|_| BatchPackError::EntryTooLarge { name: name.to_string(), len: payload.len() })?;

        self.write_u32(0x0403_4b50)?;
        self.write_u16(20)?;
        self.write_u16(1 << 11)?; // Entry names are encoded as UTF-8.
        self.write_u16(compression_method)?;
        self.write_u16(0)?;
        self.write_u16(0)?;
        self.write_u32(crc32)?;
        self.write_u32(compressed_size)?;
        self.write_u32(uncompressed_size)?;
        self.write_u16(name_len)?;
        self.write_u16(0)?;
        self.write_all(name_bytes)?;
        self.write_all(&payload)?;
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

    fn finish(mut self) -> Result<W, BatchPackError> {
        let central_dir_offset = u32::try_from(self.offset).map_err(|_| BatchPackError::ArchiveTooLarge)?;
        let entry_count = u16::try_from(self.entries.len())
            .map_err(|_| BatchPackError::TooManyEntries { count: self.entries.len() })?;

        // Move metadata out so central headers can borrow names without a
        // second name allocation for every entry.
        let entries = core::mem::take(&mut self.entries);
        for entry in entries {
            let name_bytes = entry.name.as_bytes();
            let name_len = u16::try_from(name_bytes.len())
                .map_err(|_| BatchPackError::EntryNameTooLong { name: entry.name.clone(), len: name_bytes.len() })?;
            let ZipEntry { crc32, compressed_size, uncompressed_size, compression_method, local_header_offset, .. } =
                entry;

            self.write_u32(0x0201_4b50)?;
            self.write_u16(20)?;
            self.write_u16(20)?;
            self.write_u16(1 << 11)?; // Match the local header's UTF-8 flag.
            self.write_u16(compression_method)?;
            self.write_u16(0)?;
            self.write_u16(0)?;
            self.write_u32(crc32)?;
            self.write_u32(compressed_size)?;
            self.write_u32(uncompressed_size)?;
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
            .ok_or(BatchPackError::ArchiveTooLarge)?;
        self.write_u32(0x0605_4b50)?;
        self.write_u16(0)?;
        self.write_u16(0)?;
        self.write_u16(entry_count)?;
        self.write_u16(entry_count)?;
        self.write_u32(central_dir_size)?;
        self.write_u32(central_dir_offset)?;
        self.write_u16(0)?;
        Ok(self.output)
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), BatchPackError> {
        self.output.write_all(bytes)?;
        self.offset += bytes.len() as u64;
        Ok(())
    }

    fn write_u16(&mut self, value: u16) -> Result<(), BatchPackError> {
        self.write_all(&value.to_le_bytes())
    }

    fn write_u32(&mut self, value: u32) -> Result<(), BatchPackError> {
        self.write_all(&value.to_le_bytes())
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

static ZIP_CRC32_TABLES: [[u32; 256]; 8] = {
    let mut tables = [[0; 256]; 8];
    let mut index = 0;
    while index < 256 {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            let mask = 0u32.wrapping_sub(value & 1);
            value = (value >> 1) ^ (0xedb8_8320 & mask);
            bit += 1;
        }
        tables[0][index] = value;
        index += 1;
    }
    let mut slice = 1;
    while slice < 8 {
        index = 0;
        while index < 256 {
            let previous = tables[slice - 1][index];
            tables[slice][index] = (previous >> 8) ^ tables[0][(previous & 0xff) as usize];
            index += 1;
        }
        slice += 1;
    }
    tables
};

/// Computes the IEEE CRC32 used by ZIP packaging and the companion CLI.
///
/// This implementation detail supports the same checksum for every byte slice,
/// including empty and unaligned slices. Inputs below 256 bytes use the scalar
/// table path; larger inputs process eight bytes per step before a scalar tail.
#[doc(hidden)]
#[inline]
#[must_use]
pub fn zip_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff;
    let mut tail = bytes;
    if bytes.len() >= 256 {
        let (chunks, remainder) = bytes.as_chunks::<8>();
        for chunk in chunks {
            let first = crc ^ u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            crc = ZIP_CRC32_TABLES[7][(first & 0xff) as usize]
                ^ ZIP_CRC32_TABLES[6][((first >> 8) & 0xff) as usize]
                ^ ZIP_CRC32_TABLES[5][((first >> 16) & 0xff) as usize]
                ^ ZIP_CRC32_TABLES[4][(first >> 24) as usize]
                ^ ZIP_CRC32_TABLES[3][usize::from(chunk[4])]
                ^ ZIP_CRC32_TABLES[2][usize::from(chunk[5])]
                ^ ZIP_CRC32_TABLES[1][usize::from(chunk[6])]
                ^ ZIP_CRC32_TABLES[0][usize::from(chunk[7])];
        }
        tail = remainder;
    }
    for &byte in tail {
        crc = (crc >> 8) ^ ZIP_CRC32_TABLES[0][((crc ^ u32::from(byte)) & 0xff) as usize];
    }
    !crc
}

#[cfg(feature = "image")]
fn encode_png_grid<'a>(
    images: impl IntoIterator<Item = &'a crate::render::image::RgbaImage>,
    options: BatchGridOptions,
) -> Result<Vec<u8>, BatchPackError> {
    use crate::render::image::{DynamicImage, ImageFormat, Rgba, encode_to_format};

    let images = images.into_iter().collect::<Vec<_>>();
    if images.is_empty() {
        return Err(BatchPackError::EmptyBatch);
    }
    let cell_width = images.iter().map(|image| image.width()).max().unwrap_or(1);
    let cell_height = images.iter().map(|image| image.height()).max().unwrap_or(1);
    let geometry = GridGeometry::new(images.len(), cell_width, cell_height, options.columns, 1024 * 1024 * 1024)?;
    let mut sheet = allocate_grid(&geometry)?;
    for pixel in sheet.pixels_mut() {
        *pixel = Rgba(options.background);
    }

    for (index, &image) in images.iter().enumerate() {
        let (left, top) = geometry.offset(index, image.width(), image.height());
        crate::render::image::image::imageops::replace(&mut sheet, image, i64::from(left), i64::from(top));
    }

    encode_to_format(&DynamicImage::ImageRgba8(sheet), ImageFormat::Png).map_err(Into::into)
}

#[cfg(all(test, feature = "image"))]
fn grid_buffer_len(width: u32, height: u32) -> Result<usize, BatchPackError> {
    grid_buffer_len_with_limit(width, height, 1024 * 1024 * 1024)
}

#[cfg(feature = "image")]
fn grid_buffer_len_with_limit(width: u32, height: u32, limit: usize) -> Result<usize, BatchPackError> {
    const MAX_GRID_SIDE: u32 = 65_535;

    if width == 0 || height == 0 || width > MAX_GRID_SIDE || height > MAX_GRID_SIDE {
        return Err(BatchPackError::GridTooLarge);
    }
    let len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(BatchPackError::GridTooLarge)?;
    if len > limit || len > isize::MAX as usize {
        return Err(BatchPackError::GridTooLarge);
    }
    Ok(len)
}

#[cfg(feature = "image")]
struct QrGridStyle {
    module_width: u32,
    module_height: u32,
    quiet_zone: bool,
    dark: crate::render::image::Rgba<u8>,
    light: crate::render::image::Rgba<u8>,
}

#[cfg(feature = "image")]
impl QrGridStyle {
    fn dimensions(&self, code: &QrCode) -> Result<(u32, u32, u32), BatchPackError> {
        let quiet_zone = if self.quiet_zone { qrcode_core::QrSymbol::quiet_zone(code) } else { 0 };
        let modules = u32::try_from(code.width())
            .ok()
            .and_then(|width| quiet_zone.checked_mul(2).and_then(|quiet| width.checked_add(quiet)))
            .ok_or(BatchPackError::GridTooLarge)?;
        let width = modules.checked_mul(self.module_width).ok_or(BatchPackError::GridTooLarge)?;
        let height = modules.checked_mul(self.module_height).ok_or(BatchPackError::GridTooLarge)?;
        Ok((width, height, quiet_zone))
    }
}

#[cfg(feature = "image")]
struct GridGeometry {
    columns: usize,
    cell_width: u32,
    cell_height: u32,
    width: u32,
    height: u32,
    buffer_len: usize,
}

#[cfg(feature = "image")]
impl GridGeometry {
    fn new(
        count: usize,
        cell_width: u32,
        cell_height: u32,
        requested: usize,
        budget: usize,
    ) -> Result<Self, BatchPackError> {
        let columns = grid_columns(requested, count)?;
        let rows = count.div_ceil(columns);
        let columns_u32 = u32::try_from(columns).map_err(|_| BatchPackError::GridTooLarge)?;
        let rows_u32 = u32::try_from(rows).map_err(|_| BatchPackError::GridTooLarge)?;
        let width = cell_width.checked_mul(columns_u32).ok_or(BatchPackError::GridTooLarge)?;
        let height = cell_height.checked_mul(rows_u32).ok_or(BatchPackError::GridTooLarge)?;
        let buffer_len = grid_buffer_len_with_limit(width, height, budget)?;
        Ok(Self { columns, cell_width, cell_height, width, height, buffer_len })
    }

    fn offset(&self, index: usize, width: u32, height: u32) -> (u32, u32) {
        let col = (index % self.columns) as u32;
        let row = (index / self.columns) as u32;
        (
            col * self.cell_width + (self.cell_width - width) / 2,
            row * self.cell_height + (self.cell_height - height) / 2,
        )
    }
}

#[cfg(feature = "image")]
fn allocate_grid(geometry: &GridGeometry) -> Result<crate::render::image::RgbaImage, BatchPackError> {
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(geometry.buffer_len).map_err(|_| BatchPackError::GridTooLarge)?;
    pixels.resize(geometry.buffer_len, 0);
    crate::render::image::RgbaImage::from_raw(geometry.width, geometry.height, pixels)
        .ok_or(BatchPackError::GridTooLarge)
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

    // Retain an independently generated copy of the previous scalar table as
    // an oracle. It does not read the production slicing tables.
    const LEGACY_CRC32_TABLE: [u32; 256] = {
        let mut table = [0; 256];
        let mut index = 0;
        while index < table.len() {
            let mut value = index as u32;
            let mut bit = 0;
            while bit < 8 {
                value = if value & 1 == 0 { value >> 1 } else { (value >> 1) ^ 0xedb8_8320 };
                bit += 1;
            }
            table[index] = value;
            index += 1;
        }
        table
    };

    fn crc32_table_reference(bytes: &[u8]) -> u32 {
        let mut crc = 0xffff_ffff;
        for &byte in bytes {
            crc = (crc >> 8) ^ LEGACY_CRC32_TABLE[((crc ^ u32::from(byte)) & 0xff) as usize];
        }
        !crc
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
    fn crc32_matches_ieee_vectors() {
        assert_eq!(zip_crc32(b""), 0);
        assert_eq!(zip_crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(zip_crc32(b"abc"), 0x3524_41c2);
    }

    #[test]
    fn crc32_matches_bitwise_oracle_for_prefixes_and_unaligned_slices() {
        let bytes = (0..529_usize).map(|index| (index.wrapping_mul(73) % 256) as u8).collect::<Vec<_>>();
        for length in 0..=512 {
            for offset in 0..16 {
                let input = &bytes[offset..offset + length];
                let actual = zip_crc32(input);
                assert_eq!(actual, crc32_bitwise(input), "offset {offset}, length {length}");
                assert_eq!(actual, crc32_table_reference(input), "offset {offset}, length {length}");
            }
        }
    }

    #[test]
    fn crc32_matches_independent_oracles_for_large_and_unaligned_slices() {
        let bytes = (0..1_048_600_usize)
            .map(|index| ((index.wrapping_mul(37) ^ (index >> 9) ^ 0x5b) & 0xff) as u8)
            .collect::<Vec<_>>();
        for length in [32_768, 32_769, 65_535, 65_536, 1_048_576, 1_048_583] {
            for offset in 0..8 {
                let input = &bytes[offset..offset + length];
                let actual = zip_crc32(input);
                assert_eq!(actual, crc32_bitwise(input), "offset {offset}, length {length}");
                assert_eq!(actual, crc32_table_reference(input), "offset {offset}, length {length}");
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
    fn stored_zip_matches_independent_ieee_crc_wire_fixture() {
        // Generated independently with Python struct.pack ZIP headers and
        // zlib.crc32, using zero timestamps and UTF-8 name flags.
        let expected_hex = concat!(
            "504b03041400000800000000000000000000000000000000000009000000656d7074792e62696e504b03041400000800",
            "0000000000570d652480000000800000000d000000e4ba8ce7bbb4e7a0812e62696e000102030405060708090a0b0c0d",
            "0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d",
            "3e3f404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f606162636465666768696a6b6c6d",
            "6e6f707172737475767778797a7b7c7d7e7f504b0304140000080000000000006a1bd855050100000501000008000000",
            "62756c6b2e62696e000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f2021222324252627",
            "28292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f5051525354555657",
            "58595a5b5c5d5e5f606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f8081828384858687",
            "88898a8b8c8d8e8f909192939495969798999a9b9c9d9e9fa0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7",
            "b8b9babbbcbdbebfc0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7",
            "e8e9eaebecedeeeff0f1f2f3f4f5f6f7f8f9fafbfcfdfeff7461696c73504b0102140014000008000000000000000000",
            "000000000000000000090000000000000000000000000000000000656d7074792e62696e504b01021400140000080000",
            "00000000570d652480000000800000000d0000000000000000000000000027000000e4ba8ce7bbb4e7a0812e62696e50",
            "4b01021400140000080000000000006a1bd85505010000050100000800000000000000000000000000d200000062756c",
            "6b2e62696e504b05060000000003000300a8000000fd0100000000",
        );
        let expected = expected_hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        let mut bulk = (0..=255_u8).collect::<Vec<_>>();
        bulk.extend_from_slice(b"tails");
        let files = BatchOutput::from_entries([
            BatchEntry::new("empty.bin", Vec::new()),
            BatchEntry::new("二维码.bin", (0..128_u8).collect::<Vec<_>>()),
            BatchEntry::new("bulk.bin", bulk),
        ]);
        assert_eq!(files.to_zip().unwrap(), expected);
        let mut written = Vec::new();
        files.write_zip(&mut written).unwrap();
        assert_eq!(written, expected);
    }

    #[test]
    fn lazy_rendered_zip_matches_collected_pipeline_and_preserves_names() {
        fn render(code: &QrCode, index: usize) -> Result<Vec<u8>, core::convert::Infallible> {
            let image = code.render::<char>().dark_color('#').quiet_zone(false).build();
            Ok(format!("index={index}\n{image}").into_bytes())
        }
        for compression in [
            ZipCompression::Stored,
            #[cfg(feature = "batch-zip-deflate")]
            ZipCompression::Deflated,
        ] {
            let inputs = ["alpha", "12345", "二维码"];
            let builder = || {
                QrBatchBuilder::new(inputs)
                    .file_prefix("目录/二维码")
                    .file_extension("txt")
                    .start_index(usize::MAX)
                    .ec_level(EcLevel::H)
            };
            let expected = builder().render_bytes(render).unwrap().to_zip_with(compression).unwrap();
            let mut actual = Vec::new();
            builder().render_zip_with(&mut actual as &mut dyn std::io::Write, compression, render).unwrap();
            assert_eq!(actual, expected);
        }
        let mut empty = Vec::new();
        QrBatchBuilder::new(core::iter::empty::<&str>()).render_zip(&mut empty, render).unwrap();
        assert_eq!(empty, BatchOutput::<Vec<u8>>::from_entries([]).to_zip().unwrap());
    }

    #[test]
    fn lazy_rendered_zip_stops_at_encoding_and_rendering_errors() {
        use core::cell::Cell;

        let read = Cell::new(0);
        let rendered = Cell::new(0);
        let oversized = "x".repeat(4_000);
        let inputs = ["alpha", oversized.as_str(), "never read"].into_iter().inspect(|_| read.set(read.get() + 1));
        let mut output = Vec::new();
        let error = QrBatchBuilder::new(inputs)
            .render_zip(&mut output, |_, _| {
                rendered.set(rendered.get() + 1);
                Ok::<_, &'static str>(b"first".to_vec())
            })
            .unwrap_err();
        assert!(matches!(error, BatchZipError::Encode(QrError::DataTooLong)));
        assert_eq!((read.get(), rendered.get()), (2, 1));
        assert!(output.starts_with(b"PK\x03\x04"));
        assert!(!output.windows(4).any(|bytes| bytes == b"PK\x05\x06"));

        read.set(0);
        let inputs = ["alpha", "beta", "never read"].into_iter().inspect(|_| read.set(read.get() + 1));
        let error = QrBatchBuilder::new(inputs)
            .render_zip(&mut Vec::new(), |_, index| if index == 1 { Err("render failed") } else { Ok(Vec::new()) })
            .unwrap_err();
        assert!(matches!(error, BatchZipError::Render("render failed")));
        assert_eq!(read.get(), 2);
    }

    #[test]
    fn lazy_rendered_zip_stops_at_pack_errors_and_preserves_error_sources() {
        use core::cell::Cell;

        let read = Cell::new(0);
        let inputs = ["alpha", "never read"].into_iter().inspect(|_| read.set(read.get() + 1));
        let error = QrBatchBuilder::new(inputs)
            .file_prefix("../bad")
            .render_zip(&mut Vec::new(), |_, _| Ok::<_, core::convert::Infallible>(Vec::new()))
            .unwrap_err();
        assert!(matches!(error, BatchZipError::Pack(BatchPackError::InvalidEntryName { .. })));
        assert_eq!(read.get(), 1);
        assert!(std::error::Error::source(&error).unwrap().is::<BatchPackError>());

        struct FailOutput;
        impl std::io::Write for FailOutput {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "stream failed"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                panic!("the caller owns flushing")
            }
        }
        read.set(0);
        let inputs = ["alpha", "never read"].into_iter().inspect(|_| read.set(read.get() + 1));
        let error = QrBatchBuilder::new(inputs)
            .render_zip(&mut FailOutput, |_, _| Ok::<_, core::convert::Infallible>(Vec::new()))
            .unwrap_err();
        assert_eq!(read.get(), 1);
        assert_eq!(error.to_string(), "stream failed");
        let BatchZipError::Pack(BatchPackError::Io(error)) = error else { panic!("unexpected ZIP error") };
        assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn zip_output_handles_short_writes_and_interrupted_writes_without_flushing() {
        struct ShortWriter {
            bytes: Vec<u8>,
            interrupted: bool,
        }
        impl std::io::Write for ShortWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if !self.interrupted {
                    self.interrupted = true;
                    return Err(std::io::ErrorKind::Interrupted.into());
                }
                let count = bytes.len().min(3);
                self.bytes.extend_from_slice(&bytes[..count]);
                Ok(count)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                panic!("flushing is the caller's responsibility")
            }
        }
        let files = BatchOutput::from_entries([
            BatchEntry::new("二维码.txt", b"alpha".to_vec()),
            BatchEntry::new("sub/empty.bin", Vec::new()),
        ]);
        let mut output = ShortWriter { bytes: Vec::new(), interrupted: false };
        files.write_zip(&mut output as &mut dyn std::io::Write).unwrap();
        assert_eq!(output.bytes, files.to_zip().unwrap());

        #[cfg(feature = "batch-zip-deflate")]
        {
            output.bytes.clear();
            files.write_zip_with(&mut output, ZipCompression::Deflated).unwrap();
            assert_eq!(output.bytes, files.to_zip_with(ZipCompression::Deflated).unwrap());
        }
    }

    #[test]
    fn zip_output_preserves_write_errors_at_headers_payloads_and_directory() {
        struct FailingWriter {
            remaining: usize,
        }
        impl std::io::Write for FailingWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self.remaining == 0 {
                    return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test write failure"));
                }
                let count = bytes.len().min(self.remaining);
                self.remaining -= count;
                Ok(count)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                panic!("flushing is the caller's responsibility")
            }
        }
        let files = BatchOutput::from_entries([BatchEntry::new("qr.txt", b"alpha".to_vec())]);
        let size = files.to_zip().unwrap().len();
        for offset in [0, 17, 32, 36, 41, size - 1] {
            let error = files.write_zip(&mut FailingWriter { remaining: offset }).unwrap_err();
            match error {
                BatchPackError::Io(error) => {
                    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
                    assert_eq!(error.to_string(), "test write failure");
                }
                error => panic!("unexpected error at byte {offset}: {error}"),
            }
        }
    }

    #[test]
    fn zip_output_accepts_empty_archive_and_rejects_invalid_names() {
        let empty = BatchOutput::<Vec<u8>>::from_entries([]);
        let mut bytes = Vec::new();
        empty.write_zip(&mut bytes).unwrap();
        assert_eq!(bytes, b"PK\x05\x06\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0");
        let invalid = BatchOutput::from_entries([BatchEntry::new("../x", Vec::new())]);
        let mut bytes = Vec::new();
        assert!(matches!(invalid.write_zip(&mut bytes), Err(BatchPackError::InvalidEntryName { .. })));
        assert!(bytes.is_empty());
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

    #[cfg(feature = "image")]
    fn mixed_grid_codes() -> BatchOutput<QrCode> {
        use crate::Version;

        BatchOutput::from_entries([
            BatchEntry::new("micro1", QrCode::with_version(b"123", Version::Micro(1), EcLevel::L).unwrap()),
            BatchEntry::new("normal1", QrCode::with_version(b"abcd", Version::Normal(1), EcLevel::H).unwrap()),
            BatchEntry::new("micro3", QrCode::with_version(b"12345", Version::Micro(3), EcLevel::M).unwrap()),
            BatchEntry::new("normal4", QrCode::with_version(b"alice", Version::Normal(4), EcLevel::Q).unwrap()),
            BatchEntry::new("normal8", QrCode::with_version(b"bob", Version::Normal(8), EcLevel::L).unwrap()),
        ])
    }

    #[cfg(feature = "image")]
    fn old_tile_grid(codes: &BatchOutput<QrCode>, options: BatchGridOptions, template: &crate::QrTemplate) -> Vec<u8> {
        use crate::render::image::Rgba;

        BatchOutput::from_entries(
            codes.iter().map(|entry| {
                BatchEntry::new(entry.name(), entry.data().render::<Rgba<u8>>().template(template).build())
            }),
        )
        .to_png_grid(options)
        .unwrap()
    }

    #[cfg(feature = "image")]
    #[test]
    fn direct_qr_grid_matches_decoded_tile_grid_for_styles_and_layouts() {
        use crate::render::image::image;

        let codes = mixed_grid_codes();
        let mut alpha_hex = crate::QrTemplate::minimal().with_module_size(0, 0).with_quiet_zone(false);
        alpha_hex.dark_color = "#ff008080".into();
        alpha_hex.light_color = "#fff0".into();
        let mut invalid_hex = crate::QrTemplate::minimal().with_module_size(1, 1);
        invalid_hex.dark_color = "invalid".into();
        invalid_hex.light_color = "#aBc".into();
        let templates = [
            crate::QrTemplate::minimal(),
            crate::QrTemplate::corporate().with_module_size(2, 3),
            crate::QrTemplate::dark_mode().with_module_size(3, 2).with_quiet_zone(false),
            alpha_hex,
            invalid_hex,
        ];
        for template in templates {
            for columns in [0, 1, 3, 7] {
                for background in [[7, 8, 9, 73], [17, 19, 23, 0]] {
                    let options = BatchGridOptions { columns, background };
                    let direct = image::load_from_memory(&codes.to_png_grid_with(options, &template).unwrap())
                        .unwrap()
                        .to_rgba8();
                    let old = image::load_from_memory(&old_tile_grid(&codes, options, &template)).unwrap().to_rgba8();
                    assert_eq!(direct.dimensions(), old.dimensions(), "{template:?}, {options:?}");
                    assert!(direct.as_raw() == old.as_raw(), "pixel mismatch for {template:?}, {options:?}");
                }
            }
        }
    }

    #[cfg(feature = "image")]
    #[test]
    fn direct_qr_grid_defaults_match_minimal_template() {
        use crate::render::image::image;

        let codes = QrBatchBuilder::new(["alpha", "beta", "gamma"]).encode().unwrap();
        let options = BatchGridOptions::default().columns(2).background([7, 8, 9, 31]);
        let direct = image::load_from_memory(&codes.to_png_grid(options).unwrap()).unwrap().to_rgba8();
        let old =
            image::load_from_memory(&old_tile_grid(&codes, options, &crate::QrTemplate::minimal())).unwrap().to_rgba8();
        assert!(direct.as_raw() == old.as_raw());
        assert_eq!(direct.dimensions(), old.dimensions());
    }

    #[cfg(feature = "image")]
    #[test]
    fn direct_qr_grid_centers_mixed_symbols_and_replaces_transparent_background() {
        use crate::render::image::image;

        let codes = mixed_grid_codes();
        let options = BatchGridOptions::default().background([7, 8, 9, 0]);
        let template = crate::QrTemplate::minimal().with_module_size(2, 3);
        let image = image::load_from_memory(&codes.to_png_grid_with(options, &template).unwrap()).unwrap().to_rgba8();
        assert_eq!(image.dimensions(), (342, 342));
        assert_eq!(image.get_pixel(41, 63).0, options.background);
        assert_eq!(image.get_pixel(42, 63).0, [255, 255, 255, 255]);
        assert_eq!(image.get_pixel(46, 69).0, [0, 0, 0, 255]);
        assert_eq!(image.get_pixel(341, 341).0, options.background);
    }

    #[cfg(feature = "image")]
    #[test]
    fn direct_qr_grid_rejects_empty_overflow_and_canvas_budget_before_allocating() {
        let empty = BatchOutput::<QrCode>::from_entries([]);
        assert!(matches!(empty.to_png_grid(BatchGridOptions::default()), Err(BatchPackError::EmptyBatch)));
        let codes = QrBatchBuilder::new(["small"]).encode().unwrap();
        for columns in [usize::MAX, 65_536] {
            assert!(matches!(
                codes.to_png_grid(BatchGridOptions::default().columns(columns)),
                Err(BatchPackError::GridTooLarge)
            ));
        }
        for size in [(u32::MAX, 1), (1, u32::MAX), (1000, 1000)] {
            let template = crate::QrTemplate::minimal().with_module_size(size.0, size.1);
            assert!(matches!(
                codes.to_png_grid_with(BatchGridOptions::default(), &template),
                Err(BatchPackError::GridTooLarge)
            ));
        }
        assert_eq!(grid_buffer_len_with_limit(8192, 8192, qrcode_render::MAX_BUFFER_BYTES).unwrap(), 268_435_456);
        assert!(matches!(
            grid_buffer_len_with_limit(8193, 8192, qrcode_render::MAX_BUFFER_BYTES),
            Err(BatchPackError::GridTooLarge)
        ));
        // The existing pre-rendered-image API keeps its original 1 GiB limit.
        assert_eq!(grid_buffer_len(16_384, 16_384).unwrap(), 1_073_741_824);
    }
}
