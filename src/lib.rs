//! QRCode encoder
//!
//! This crate provides a QR code and Micro QR code encoder for binary data.
//!
#![cfg_attr(feature = "image", doc = "```rust")]
#![cfg_attr(not(feature = "image"), doc = "```ignore")]
//! use qrcode_rs::QrCode;
//! use image::Luma;
//!
//! // Encode some data into bits.
//! let code = QrCode::new(b"01234567").unwrap();
//!
//! // Render the bits into an image.
//! let image = code.render::<Luma<u8>>().build();
//!
//! // Save the image.
//! # if cfg!(unix) {
//! image.save("/tmp/qrcode.png").unwrap();
//! # }
//!
//! // You can also render it into a string.
//! let string = code.render()
//!     .light_color(' ')
//!     .dark_color('#')
//!     .build();
//! println!("{}", string);
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]
#![deny(clippy::uninlined_format_args, clippy::manual_range_contains, clippy::semicolon_if_nothing_returned)]
#![allow(
    clippy::must_use_candidate, // This is just annoying.
)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod decode;
pub mod parse;
pub mod render;
pub mod structured_append;

// The encoding primitive layer lives in `qrcode-core` and is re-exported here
// so the public API (`qrcode_rs::bits::Bits`, `qrcode_rs::Version`, …) is unchanged.
pub use qrcode_core::{bits, canvas, ec, optimize, types};
// `cast` stays crate-private (not part of the public API); re-import it so
// `crate::cast::As` keeps resolving across the facade and render modules.
pub use crate::types::{Color, EcLevel, Mode, QrError, QrResult, Version};
use qrcode_core::cast;

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::{
    borrow::ToOwned,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use crate::cast::As;
use crate::render::{Pixel, Renderer};
use core::iter::FusedIterator;
use core::ops::Index;

/// The encoded QR code symbol.
///
/// `QrCode` is `Send + Sync`, so it can be shared or moved across threads
/// (e.g. for parallel rendering of many codes). This is verified at compile
/// time below.
#[derive(Clone)]
pub struct QrCode {
    content: Vec<Color>,
    version: Version,
    ec_level: EcLevel,
    width: usize,
}

// Compile-time guarantee that QrCode stays Send + Sync as fields evolve.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<QrCode>();
};

impl QrCode {
    /// Constructs a new QR code which automatically encodes the given data.
    ///
    /// This method uses the "medium" error correction level and automatically
    /// chooses the smallest QR code.
    ///
    ///     use qrcode_rs::QrCode;
    ///
    ///     let code = QrCode::new(b"Some data").unwrap();
    ///
    /// # Errors
    ///
    /// Returns error if the QR code cannot be constructed, e.g. when the data
    /// is too long.
    pub fn new<D: AsRef<[u8]>>(data: D) -> QrResult<Self> {
        Self::with_error_correction_level(data, EcLevel::M)
    }

    /// Constructs a new QR code which automatically encodes the given data at a
    /// specific error correction level.
    ///
    /// This method automatically chooses the smallest QR code.
    ///
    ///     use qrcode_rs::{QrCode, EcLevel};
    ///
    ///     let code = QrCode::with_error_correction_level(b"Some data", EcLevel::H).unwrap();
    ///
    /// # Errors
    ///
    /// Returns error if the QR code cannot be constructed, e.g. when the data
    /// is too long.
    pub fn with_error_correction_level<D: AsRef<[u8]>>(data: D, ec_level: EcLevel) -> QrResult<Self> {
        let bits = bits::encode_auto(data.as_ref(), ec_level)?;
        Self::with_bits(bits, ec_level)
    }

    /// Constructs a new Micro QR code which automatically encodes the given
    /// data.
    ///
    /// This method uses the "medium" error correction level and automatically
    /// chooses the smallest Micro QR code.
    ///
    ///     use qrcode_rs::QrCode;
    ///
    ///     let code = QrCode::new_micro(b"123").unwrap();
    ///
    /// # Errors
    ///
    /// Returns error if the data cannot be encoded as a Micro QR code, e.g.
    /// when the data is too long.
    pub fn new_micro<D: AsRef<[u8]>>(data: D) -> QrResult<Self> {
        Self::micro_with_error_correction_level(data, EcLevel::M)
    }

    /// Constructs a new Micro QR code which automatically encodes the given
    /// data at a specific error correction level.
    ///
    /// This method automatically chooses the smallest Micro QR code.
    ///
    ///     use qrcode_rs::{QrCode, EcLevel};
    ///
    ///     let code = QrCode::micro_with_error_correction_level(b"123", EcLevel::L).unwrap();
    ///
    /// # Errors
    ///
    /// Returns error if the data cannot be encoded as a Micro QR code, e.g.
    /// when the data is too long, or when the error correction level is not
    /// supported by any Micro QR version.
    pub fn micro_with_error_correction_level<D: AsRef<[u8]>>(data: D, ec_level: EcLevel) -> QrResult<Self> {
        let bits = bits::encode_auto_micro(data.as_ref(), ec_level)?;
        Self::with_bits(bits, ec_level)
    }

    /// Constructs a new QR code for the given version and error correction
    /// level.
    ///
    ///     use qrcode_rs::{QrCode, Version, EcLevel};
    ///
    ///     let code = QrCode::with_version(b"Some data", Version::Normal(5), EcLevel::M).unwrap();
    ///
    /// This method can also be used to generate Micro QR code.
    ///
    ///     use qrcode_rs::{QrCode, Version, EcLevel};
    ///
    ///     let micro_code = QrCode::with_version(b"123", Version::Micro(1), EcLevel::L).unwrap();
    ///
    /// # Errors
    ///
    /// Returns error if the QR code cannot be constructed, e.g. when the data
    /// is too long, or when the version and error correction level are
    /// incompatible.
    pub fn with_version<D: AsRef<[u8]>>(data: D, version: Version, ec_level: EcLevel) -> QrResult<Self> {
        let mut bits = bits::Bits::new(version);
        bits.push_optimal_data(data.as_ref())?;
        bits.push_terminator(ec_level)?;
        Self::with_bits(bits, ec_level)
    }

    /// Constructs a new QR code with encoded bits.
    ///
    /// Use this method only if there are very special need to manipulate the
    /// raw bits before encoding. Some examples are:
    ///
    /// * Encode data using specific character set with ECI
    /// * Use the FNC1 modes
    /// * Avoid the optimal segmentation algorithm
    ///
    /// See the `Bits` structure for detail.
    ///
    ///     #![allow(unused_must_use)]
    ///
    ///     use qrcode_rs::{QrCode, Version, EcLevel};
    ///     use qrcode_rs::bits::Bits;
    ///
    ///     let mut bits = Bits::new(Version::Normal(1));
    ///     bits.push_eci_designator(9);
    ///     bits.push_byte_data(b"\xca\xfe\xe4\xe9\xea\xe1\xf2 QR");
    ///     bits.push_terminator(EcLevel::L);
    ///     let qrcode = QrCode::with_bits(bits, EcLevel::L);
    ///
    /// # Errors
    ///
    /// Returns error if the QR code cannot be constructed, e.g. when the bits
    /// are too long, or when the version and error correction level are
    /// incompatible.
    pub fn with_bits(bits: bits::Bits, ec_level: EcLevel) -> QrResult<Self> {
        let version = bits.version();
        #[cfg(feature = "log")]
        log::debug!("qrcode_rs: encoding at version {version:?}, ec {ec_level:?}");
        let data = bits.into_bytes();
        let (encoded_data, ec_data) = ec::construct_codewords(&data, version, ec_level)?;
        let mut canvas = canvas::Canvas::new(version, ec_level);
        canvas.draw_all_functional_patterns();
        canvas.draw_data(&encoded_data, &ec_data);
        let canvas = canvas.apply_best_mask();
        let width = version.width().as_usize();
        #[cfg(feature = "log")]
        log::info!("qrcode_rs: encoded version {version:?} ec {ec_level:?} ({} modules)", width * width);
        Ok(Self { content: canvas.into_colors(), version, ec_level, width })
    }

    /// Encodes many inputs at once at the given error-correction level, stopping
    /// at the first input that fails to encode.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qrcode_rs::{QrCode, EcLevel};
    ///
    /// let codes = QrCode::batch(&["alpha", "beta", "gamma"], EcLevel::M).unwrap();
    /// assert_eq!(codes.len(), 3);
    /// ```
    pub fn batch<I, D>(inputs: I, ec_level: EcLevel) -> QrResult<Vec<Self>>
    where
        I: IntoIterator<Item = D>,
        D: AsRef<[u8]>,
    {
        inputs.into_iter().map(|d| Self::with_error_correction_level(d, ec_level)).collect()
    }

    /// Gets the version of this QR code.
    pub const fn version(&self) -> Version {
        self.version
    }

    /// Gets the error correction level of this QR code.
    pub const fn error_correction_level(&self) -> EcLevel {
        self.ec_level
    }

    /// Gets the number of modules per side, i.e. the width of this QR code.
    ///
    /// The width here does not contain the quiet zone paddings.
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Gets the maximum number of allowed erratic modules can be introduced
    /// before the data becomes corrupted. Note that errors should not be
    /// introduced to functional modules.
    pub fn max_allowed_errors(&self) -> usize {
        ec::max_allowed_errors(self.version, self.ec_level).expect("invalid version or ec_level")
    }

    /// Returns metadata about this QR code (version, error-correction level,
    /// dimensions, module count, error tolerance, and data capacity).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qrcode_rs::QrCode;
    ///
    /// let code = QrCode::new(b"hello").unwrap();
    /// let info = code.info();
    /// assert_eq!(info.width(), code.width());
    /// assert_eq!(info.module_count(), code.width() * code.width());
    /// assert!(info.data_capacity_bytes() > 0);
    /// ```
    #[must_use]
    pub fn info(&self) -> Info {
        Info {
            version: self.version,
            ec_level: self.ec_level,
            width: self.width,
            module_count: self.width * self.width,
            max_allowed_errors: self.max_allowed_errors(),
            data_capacity_bytes: bits::data_capacity_bits(self.version, self.ec_level).map(|b| b / 8).unwrap_or(0),
        }
    }

    /// Returns diagnostic stats for this code: dark-module ratio and the split
    /// between functional and data modules. Combine with [`QrCode::info`] for
    /// version / capacity. Computed on demand (scans the grid).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qrcode_rs::QrCode;
    ///
    /// let code = QrCode::new(b"hello").unwrap();
    /// let a = code.analyze();
    /// assert!(a.dark_ratio() > 0.0 && a.dark_ratio() < 1.0);
    /// assert_eq!(a.functional_modules() + a.data_modules(), code.width() * code.width());
    /// ```
    #[must_use]
    pub fn analyze(&self) -> Analysis {
        let total = self.width * self.width;
        let dark = self.content.iter().filter(|c| **c == Color::Dark).count();
        let functional =
            (0..self.width).map(|y| (0..self.width).filter(|x| self.is_functional(*x, y)).count()).sum::<usize>();
        Analysis {
            dark_ratio: if total == 0 { 0.0 } else { dark as f64 / total as f64 },
            functional_modules: functional,
            data_modules: total - functional,
        }
    }

    /// Checks whether a module at coordinate (x, y) is a functional module or
    /// not.
    pub fn is_functional(&self, x: usize, y: usize) -> bool {
        let x = x.try_into().expect("coordinate is too large for QR code");
        let y = y.try_into().expect("coordinate is too large for QR code");
        canvas::is_functional(self.version, self.version.width(), x, y)
    }

    /// Converts the QR code into a human-readable string. This is mainly for
    /// debugging only.
    pub fn to_debug_str(&self, on_char: char, off_char: char) -> String {
        self.render().quiet_zone(false).dark_color(on_char).light_color(off_char).build()
    }

    /// Returns the module colors as a borrowed slice — no allocation. Use this
    /// in preference to [`to_colors`](Self::to_colors) when you only need to
    /// read the modules.
    ///
    /// The slice is row-major, with `width() * width()` entries and no quiet
    /// zone.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qrcode_rs::QrCode;
    ///
    /// let code = QrCode::new(b"hi").unwrap();
    /// let colors = code.colors();
    /// assert_eq!(colors.len(), code.width() * code.width());
    /// ```
    pub fn colors(&self) -> &[Color] {
        &self.content
    }

    /// Converts the QR code to a vector of colors.
    pub fn to_colors(&self) -> Vec<Color> {
        self.content.clone()
    }

    /// Converts the QR code to a vector of colors.
    pub fn into_colors(self) -> Vec<Color> {
        self.content
    }

    /// Renders the QR code into an image. The result is an image builder, which
    /// you may do some additional configuration before copying it into a
    /// concrete image.
    ///  Note: the`image` crate itself also provides method to rotate the image,
    /// or overlay a logo on top of the QR code.
    /// # Examples
    ///
    #[cfg_attr(feature = "image", doc = " ```rust")]
    #[cfg_attr(not(feature = "image"), doc = " ```ignore")]
    /// # use qrcode_rs::QrCode;
    /// # use image::Rgb;
    ///
    /// let image = QrCode::new(b"hello").unwrap()
    ///                     .render()
    ///                     .dark_color(Rgb([0, 0, 128]))
    ///                     .light_color(Rgb([224, 224, 224])) // adjust colors
    ///                     .quiet_zone(false)          // disable quiet zone (white border)
    ///                     .min_dimensions(300, 300)   // sets minimum image size
    ///                     .build();
    /// ```
    ///
    pub fn render<P: Pixel>(&self) -> Renderer<'_, P> {
        let quiet_zone = if self.version.is_micro() { 2 } else { 4 };
        Renderer::new(&self.content, self.width, quiet_zone)
    }
}

impl QrCode {
    /// Creates a [`QrCodeBuilder`] for configuring and constructing a QR code.
    ///
    /// This is an ergonomic alternative to the `with_*` constructors. The
    /// builder uses the same encoding paths, so its output is identical to the
    /// equivalent constructor.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qrcode_rs::{QrCode, EcLevel};
    ///
    /// let code = QrCode::builder(b"https://example.com")
    ///     .ec_level(EcLevel::H)
    ///     .build()
    ///     .unwrap();
    /// # let _ = code;
    /// ```
    pub fn builder<D: AsRef<[u8]>>(data: D) -> QrCodeBuilder<D> {
        QrCodeBuilder::new(data)
    }

    /// Returns an iterator yielding one [`Row`] of modules at a time.
    ///
    /// Each row iterates over the module [`Color`]s from left to right. The
    /// quiet zone is *not* included.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qrcode_rs::QrCode;
    ///
    /// let code = QrCode::new(b"hi").unwrap();
    /// for row in code.rows() {
    ///     for color in row {
    ///         # let _ = color;
    ///     }
    /// }
    /// ```
    pub fn rows(&self) -> Rows<'_> {
        Rows { code: self, y: 0 }
    }

    /// Returns an iterator over the `(x, y)` coordinates of every dark module,
    /// convenient for custom rendering. The quiet zone is *not* included.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qrcode_rs::QrCode;
    ///
    /// let code = QrCode::new(b"hi").unwrap();
    /// let dark_count = code.dark_modules().count();
    /// # let _ = dark_count;
    /// ```
    pub fn dark_modules(&self) -> DarkModules<'_> {
        DarkModules { code: self, idx: 0 }
    }

    /// Encodes a URL, using high error correction (robust to print damage).
    ///
    /// # Errors
    ///
    /// Returns an error only if the URL is too long to encode.
    pub fn for_url<D: AsRef<[u8]>>(url: D) -> QrResult<Self> {
        Self::with_error_correction_level(url, EcLevel::H)
    }

    /// Encodes plain text at the default (medium) error correction level.
    ///
    /// # Errors
    ///
    /// Returns an error only if the text is too long to encode.
    pub fn for_text<D: AsRef<[u8]>>(text: D) -> QrResult<Self> {
        Self::new(text)
    }

    /// Encodes a WiFi configuration that most phone cameras will offer to join.
    ///
    /// `auth` is one of `WPA`, `WEP` or `nopass`. Special characters in the
    /// SSID/password are backslash-escaped per the WiFi QR specification.
    ///
    /// # Errors
    ///
    /// Returns an error if the resulting payload is too long to encode.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qrcode_rs::QrCode;
    ///
    /// let code = QrCode::for_wifi("MyNetwork", "p\\a;ss", "WPA").unwrap();
    /// # let _ = code;
    /// ```
    pub fn for_wifi(ssid: &str, password: &str, auth: &str) -> QrResult<Self> {
        Self::new(parse::wifi::encode_wifi(ssid, password, auth))
    }

    /// Encodes a minimal vCard 3.0 contact card.
    ///
    /// # Errors
    ///
    /// Returns an error if the resulting payload is too long to encode.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qrcode_rs::QrCode;
    ///
    /// let code = QrCode::for_vcard("John Doe", "+1234567890", "john@example.com").unwrap();
    /// # let _ = code;
    /// ```
    pub fn for_vcard(name: &str, phone: &str, email: &str) -> QrResult<Self> {
        Self::new(parse::vcard::encode_vcard(name, phone, email))
    }

    /// Encodes a GS1 data carrier (FNC1 in first position), e.g. a GTIN /
    /// application-identifier payload such as
    /// `"010491234512345915970331301234561842"`. Uses medium error correction
    /// and the smallest fitting version.
    ///
    /// # Errors
    ///
    /// Returns an error if the data is too long to encode.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qrcode_rs::QrCode;
    ///
    /// let code = QrCode::for_gs1("010491234512345915970331301234561842").unwrap();
    /// # let _ = code;
    /// ```
    pub fn for_gs1<D: AsRef<[u8]>>(data: D) -> QrResult<Self> {
        let data = data.as_ref();
        for v in 1..=40 {
            let version = Version::Normal(v);
            let mut bits = bits::Bits::new(version);
            if bits.push_fnc1_first_position().is_err()
                || bits.push_optimal_data(data).is_err()
                || bits.push_terminator(EcLevel::M).is_err()
            {
                continue;
            }
            return Self::with_bits(bits, EcLevel::M);
        }
        Err(QrError::DataTooLong)
    }

    /// Splits `payload` across `symbols` QR codes (2..=16) using Structured
    /// Append (ISO/IEC 18004 §7.4), each at error-correction level `ec`. Every
    /// symbol is the smallest version that fits its chunk plus the 20-bit
    /// Structured Append header.
    ///
    /// This is a thin convenience over
    /// [`crate::structured_append::StructuredAppend`]; see that type for the
    /// split and parity details, and [`crate::structured_append::reassemble`]
    /// for recombining decoded symbols.
    ///
    /// # Errors
    ///
    /// Returns [`QrError::InvalidStructuredAppend`] if `symbols` is not in
    /// `2..=16`, or [`QrError::DataTooLong`] if a chunk cannot fit even version
    /// 40 at `ec`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qrcode_rs::{EcLevel, QrCode};
    ///
    /// let codes = QrCode::structured_append(b"split across multiple symbols", 3, EcLevel::M)?;
    /// assert_eq!(codes.len(), 3);
    /// # Ok::<(), qrcode_rs::QrError>(())
    /// ```
    pub fn structured_append<D: AsRef<[u8]>>(payload: D, symbols: u8, ec: EcLevel) -> QrResult<Vec<Self>> {
        let sa = structured_append::StructuredAppend::new(symbols, payload.as_ref())?;
        sa.encode(ec)
    }

    /// Generates accessible alt text describing a QR code that encodes `data`.
    ///
    /// URLs are described as "linking to …"; other payloads as "containing: …".
    /// Use the result as the `alt` of an `<img>` or the `aria-label` of an inline
    /// SVG so assistive technology can describe the code without decoding it.
    ///
    /// This is an associated function (it does not require a constructed
    /// [`QrCode`]), so the input data does not need to be retained on the code.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qrcode_rs::QrCode;
    ///
    /// assert_eq!(QrCode::alt_text("https://example.com"), "QR code linking to https://example.com");
    /// assert_eq!(QrCode::alt_text("hello"), "QR code containing: hello");
    /// ```
    #[must_use]
    pub fn alt_text<D: AsRef<[u8]>>(data: D) -> String {
        let text = String::from_utf8_lossy(data.as_ref());
        if text.starts_with("http://") || text.starts_with("https://") {
            format!("QR code linking to {text}")
        } else {
            format!("QR code containing: {text}")
        }
    }

    /// Generates alt text with a custom formatter that receives the raw bytes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qrcode_rs::QrCode;
    ///
    /// let alt = QrCode::alt_text_custom("hello", |data| {
    ///     format!("A QR code with {} bytes", data.len())
    /// });
    /// assert_eq!(alt, "A QR code with 5 bytes");
    /// ```
    #[must_use]
    pub fn alt_text_custom<D: AsRef<[u8]>, F: FnOnce(&[u8]) -> String>(data: D, f: F) -> String {
        f(data.as_ref())
    }

    /// Encodes `data` forced into a single `mode` at a pinned version. Used by
    /// [`QrCodeBuilder::build`] when both a version and an encoding-mode hint
    /// are set.
    fn with_mode<D: AsRef<[u8]>>(data: D, version: Version, ec_level: EcLevel, mode: Mode) -> QrResult<Self> {
        let mut bits = bits::Bits::new(version);
        match mode {
            Mode::Numeric => bits.push_numeric_data(data.as_ref())?,
            Mode::Alphanumeric => bits.push_alphanumeric_data(data.as_ref())?,
            Mode::Byte => bits.push_byte_data(data.as_ref())?,
            Mode::Kanji => bits.push_kanji_data(data.as_ref())?,
        }
        bits.push_terminator(ec_level)?;
        Self::with_bits(bits, ec_level)
    }

    /// Encodes `data` forced into a single `mode`, auto-selecting the smallest
    /// fitting version. Used by [`QrCodeBuilder::build`] when an encoding-mode
    /// hint is set without a pinned version. Returns the underlying error
    /// (e.g. [`QrError::InvalidCharacter`]) if the data is incompatible with the
    /// forced mode.
    fn with_mode_auto<D: AsRef<[u8]>>(data: D, ec_level: EcLevel, mode: Mode) -> QrResult<Self> {
        let data = data.as_ref();
        let mut last_err = QrError::DataTooLong;
        for v in 1..=40 {
            let version = Version::Normal(v);
            let mut bits = bits::Bits::new(version);
            let pushed = match mode {
                Mode::Numeric => bits.push_numeric_data(data),
                Mode::Alphanumeric => bits.push_alphanumeric_data(data),
                Mode::Byte => bits.push_byte_data(data),
                Mode::Kanji => bits.push_kanji_data(data),
            };
            if let Err(e) = pushed {
                last_err = e;
                continue;
            }
            if let Err(e) = bits.push_terminator(ec_level) {
                last_err = e;
                continue;
            }
            return Self::with_bits(bits, ec_level);
        }
        Err(last_err)
    }
}

impl Index<(usize, usize)> for QrCode {
    type Output = Color;

    fn index(&self, (x, y): (usize, usize)) -> &Color {
        let index = y * self.width + x;
        &self.content[index]
    }
}

//------------------------------------------------------------------------------
//{{{ QrCodeBuilder

/// A builder for [`QrCode`], offering ergonomic, chainable configuration.
///
/// Construct one with [`QrCode::builder`]. The builder delegates to the
/// existing constructors, so its output is identical to calling them directly.
#[derive(Clone, Debug)]
pub struct QrCodeBuilder<D: AsRef<[u8]>> {
    data: D,
    ec_level: EcLevel,
    version: Option<Version>,
    micro: bool,
    mode_hint: Option<Mode>,
}

impl<D: AsRef<[u8]>> QrCodeBuilder<D> {
    fn new(data: D) -> Self {
        Self { data, ec_level: EcLevel::M, version: None, micro: false, mode_hint: None }
    }

    /// Sets the error correction level (default [`EcLevel::M`]).
    #[must_use]
    pub fn ec_level(mut self, ec_level: EcLevel) -> Self {
        self.ec_level = ec_level;
        self
    }

    /// Pins a specific QR [`Version`]. When set, `build()` behaves like
    /// [`QrCode::with_version`]. If [`micro`](Self::micro) is also set, the
    /// explicit version takes precedence.
    #[must_use]
    pub fn version(mut self, version: Version) -> Self {
        self.version = Some(version);
        self
    }

    /// Requests a Micro QR code (the smallest fitting Micro version), behaving
    /// like [`QrCode::micro_with_error_correction_level`] when no explicit
    /// [`version`](Self::version) is set.
    #[must_use]
    pub fn micro(mut self, yes: bool) -> Self {
        self.micro = yes;
        self
    }

    /// Hints the encoding [`Mode`] (e.g. [`Mode::Byte`]), bypassing automatic
    /// mode optimization. When a [`version`](Self::version) is also set it is
    /// used directly; otherwise the smallest fitting version for that mode is
    /// auto-selected.
    ///
    /// The data must be encodable in the chosen mode: [`Mode::Kanji`] validates
    /// its Shift-JIS pairs and [`Mode::Byte`] accepts anything, but
    /// [`Mode::Numeric`] / [`Mode::Alphanumeric`] assume their input already
    /// matches (as automatic optimization would never select them otherwise).
    #[must_use]
    pub fn encoding_mode(mut self, mode: Mode) -> Self {
        self.mode_hint = Some(mode);
        self
    }

    /// Forces a specific encoding [`Mode`], bypassing automatic optimization.
    /// This is an alias for [`encoding_mode`](Self::encoding_mode), provided for
    /// familiarity with the QR-code vocabulary.
    #[must_use]
    pub fn force_mode(self, mode: Mode) -> Self {
        self.encoding_mode(mode)
    }

    /// Builds the [`QrCode`].
    ///
    /// # Errors
    ///
    /// Propagates any [`QrError`] from the underlying encoder
    /// (e.g. data too long, or an incompatible version / error-correction
    /// combination).
    pub fn build(self) -> QrResult<QrCode> {
        if let Some(version) = self.version {
            if let Some(mode) = self.mode_hint {
                return QrCode::with_mode(self.data, version, self.ec_level, mode);
            }
            return QrCode::with_version(self.data, version, self.ec_level);
        }
        if let Some(mode) = self.mode_hint {
            return QrCode::with_mode_auto(self.data, self.ec_level, mode);
        }
        if self.micro {
            return QrCode::micro_with_error_correction_level(self.data, self.ec_level);
        }
        QrCode::with_error_correction_level(self.data, self.ec_level)
    }
}

//}}}
//------------------------------------------------------------------------------
//{{{ Info

/// Metadata about a constructed [`QrCode`], returned by [`QrCode::info`].
///
/// Fields that require retaining the input data or the chosen mask (e.g.
/// `encoding_modes`, `mask_pattern`, `remaining_capacity`) are intentionally
/// omitted to keep `QrCode` zero-overhead; they may be added in a later version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Info {
    version: Version,
    ec_level: EcLevel,
    width: usize,
    module_count: usize,
    max_allowed_errors: usize,
    data_capacity_bytes: usize,
}

impl Info {
    /// The QR [`Version`].
    #[must_use]
    pub const fn version(&self) -> Version {
        self.version
    }

    /// The error correction level.
    #[must_use]
    pub const fn ec_level(&self) -> EcLevel {
        self.ec_level
    }

    /// Modules per side (excluding the quiet zone).
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Total number of modules (`width * width`).
    #[must_use]
    pub const fn module_count(&self) -> usize {
        self.module_count
    }

    /// Maximum number of erroneous modules that can still be recovered.
    #[must_use]
    pub const fn max_allowed_errors(&self) -> usize {
        self.max_allowed_errors
    }

    /// Data capacity of this symbol in bytes.
    #[must_use]
    pub const fn data_capacity_bytes(&self) -> usize {
        self.data_capacity_bytes
    }
}

//}}}
//------------------------------------------------------------------------------
//{{{ Serde (QrCodeData)

/// A serializable view of a [`QrCode`] (matrix + metadata), enabled by the
/// `serde` feature. Round-trips via [`QrCode::to_serializable`] and
/// [`QrCode::from_serializable`].
#[cfg(feature = "serde")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QrCodeData {
    /// The [`Version`].
    pub version: Version,
    /// The error-correction level.
    pub ec_level: EcLevel,
    /// Modules per side (excluding the quiet zone).
    pub width: usize,
    /// Module colors, row-major (`width * width` entries).
    pub content: Vec<Color>,
}

#[cfg(feature = "serde")]
impl QrCode {
    /// Serializes this QR code into a [`QrCodeData`] (requires the `serde` feature).
    #[must_use]
    pub fn to_serializable(&self) -> QrCodeData {
        QrCodeData { version: self.version, ec_level: self.ec_level, width: self.width, content: self.content.clone() }
    }

    /// Reconstructs a [`QrCode`] from [`QrCodeData`] (requires the `serde` feature).
    ///
    /// `data` is trusted: `content.len()` must equal `width * width` (checked in
    /// debug builds). Pair with [`QrCode::to_serializable`].
    #[must_use]
    pub fn from_serializable(data: QrCodeData) -> Self {
        debug_assert_eq!(data.content.len(), data.width * data.width, "malformed QrCodeData");
        Self { content: data.content, version: data.version, ec_level: data.ec_level, width: data.width }
    }
}

//}}}
//------------------------------------------------------------------------------
//{{{ Analysis

/// Diagnostic stats for a constructed [`QrCode`], returned by [`QrCode::analyze`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Analysis {
    dark_ratio: f64,
    functional_modules: usize,
    data_modules: usize,
}

impl Analysis {
    /// Fraction of modules that are dark, in `0.0..=1.0`.
    #[must_use]
    pub const fn dark_ratio(&self) -> f64 {
        self.dark_ratio
    }

    /// Number of functional modules (finder / alignment / timing / format / version).
    #[must_use]
    pub const fn functional_modules(&self) -> usize {
        self.functional_modules
    }

    /// Number of data + error-correction modules (`width² − functional`).
    #[must_use]
    pub const fn data_modules(&self) -> usize {
        self.data_modules
    }
}

//}}}
//------------------------------------------------------------------------------
//{{{ QrTemplate

/// A reusable render-time style: dark/light hex colors, module size, and quiet
/// zone. Apply to a [`Renderer`] with [`Renderer::template`] when the pixel type
/// is a [`StyledPixel`](crate::render::StyledPixel).
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QrTemplate {
    /// Dark module color as a CSS hex string (e.g. `"#1a1a2e"`).
    pub dark_color: String,
    /// Light module color as a CSS hex string (e.g. `"#e0e0e0"`).
    pub light_color: String,
    /// Optional module dimensions `(width, height)` in output units/pixels.
    pub module_size: Option<(u32, u32)>,
    /// Whether to include the quiet zone.
    pub quiet_zone: bool,
}

impl QrTemplate {
    /// Black on white, default size, with quiet zone — the standard look.
    #[must_use]
    pub fn minimal() -> Self {
        Self { dark_color: "#000000".into(), light_color: "#ffffff".into(), module_size: None, quiet_zone: true }
    }

    /// Light modules on a dark background.
    #[must_use]
    pub fn dark_mode() -> Self {
        Self { dark_color: "#e0e0e0".into(), light_color: "#1a1a2e".into(), module_size: None, quiet_zone: true }
    }

    /// Pure black/white, maximum contrast (accessibility).
    #[must_use]
    pub fn high_contrast() -> Self {
        Self { dark_color: "#000000".into(), light_color: "#ffffff".into(), module_size: None, quiet_zone: true }
    }

    /// Corporate navy on white.
    #[must_use]
    pub fn corporate() -> Self {
        Self { dark_color: "#003366".into(), light_color: "#ffffff".into(), module_size: None, quiet_zone: true }
    }
}

//}}}
//------------------------------------------------------------------------------
//{{{ Module iterators

/// Iterator over the rows of a [`QrCode`], created by [`QrCode::rows`].
pub struct Rows<'a> {
    code: &'a QrCode,
    y: usize,
}

impl<'a> Iterator for Rows<'a> {
    type Item = Row<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let w = self.code.width;
        if self.y < w {
            let row = Row { code: self.code, y: self.y, x: 0 };
            self.y += 1;
            Some(row)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let rem = self.code.width - self.y;
        (rem, Some(rem))
    }
}

impl<'a> ExactSizeIterator for Rows<'a> {
    fn len(&self) -> usize {
        self.code.width - self.y
    }
}

impl<'a> FusedIterator for Rows<'a> {}

/// A single row of modules, yielded by [`Rows`]. Iterates over [`Color`]s from
/// left to right (quiet zone excluded).
pub struct Row<'a> {
    code: &'a QrCode,
    y: usize,
    x: usize,
}

impl<'a> Row<'a> {
    /// The number of modules in this row.
    #[must_use]
    pub fn len(&self) -> usize {
        self.code.width
    }

    /// Whether the row is empty (always `false` for a valid QR code).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.code.width == 0
    }
}

impl<'a> Iterator for Row<'a> {
    type Item = Color;

    fn next(&mut self) -> Option<Color> {
        let w = self.code.width;
        if self.x < w {
            let color = self.code.content[self.y * w + self.x];
            self.x += 1;
            Some(color)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let rem = self.code.width - self.x;
        (rem, Some(rem))
    }
}

impl<'a> ExactSizeIterator for Row<'a> {
    fn len(&self) -> usize {
        self.code.width - self.x
    }
}

impl<'a> FusedIterator for Row<'a> {}

/// Iterator over the `(x, y)` coordinates of every dark module in a [`QrCode`],
/// created by [`QrCode::dark_modules`].
pub struct DarkModules<'a> {
    code: &'a QrCode,
    idx: usize,
}

impl<'a> Iterator for DarkModules<'a> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<(usize, usize)> {
        let w = self.code.width;
        let content = &self.code.content;
        while self.idx < content.len() {
            let i = self.idx;
            self.idx += 1;
            if content[i] == Color::Dark {
                return Some((i % w, i / w));
            }
        }
        None
    }
}

impl<'a> FusedIterator for DarkModules<'a> {}

//}}}

#[cfg(test)]
mod tests {
    use crate::{EcLevel, QrCode, Version};

    #[test]
    fn test_annex_i_qr() {
        // This uses the ISO Annex I as test vector.
        let code = QrCode::with_version(b"01234567", Version::Normal(1), EcLevel::M).unwrap();
        assert_eq!(
            &*code.to_debug_str('#', '.'),
            "\
             #######..#.##.#######\n\
             #.....#..####.#.....#\n\
             #.###.#.#.....#.###.#\n\
             #.###.#.##....#.###.#\n\
             #.###.#.#.###.#.###.#\n\
             #.....#.#...#.#.....#\n\
             #######.#.#.#.#######\n\
             ........#..##........\n\
             #.#####..#..#.#####..\n\
             ...#.#.##.#.#..#.##..\n\
             ..#...##.#.#.#..#####\n\
             ....#....#.....####..\n\
             ...######..#.#..#....\n\
             ........#.#####..##..\n\
             #######..##.#.##.....\n\
             #.....#.#.#####...#.#\n\
             #.###.#.#...#..#.##..\n\
             #.###.#.##..#..#.....\n\
             #.###.#.#.##.#..#.#..\n\
             #.....#........##.##.\n\
             #######.####.#..#.#.."
        );
    }

    #[test]
    fn test_annex_i_micro_qr() {
        let code = QrCode::with_version(b"01234567", Version::Micro(2), EcLevel::L).unwrap();
        assert_eq!(
            &*code.to_debug_str('#', '.'),
            "\
             #######.#.#.#\n\
             #.....#.###.#\n\
             #.###.#..##.#\n\
             #.###.#..####\n\
             #.###.#.###..\n\
             #.....#.#...#\n\
             #######..####\n\
             .........##..\n\
             ##.#....#...#\n\
             .##.#.#.#.#.#\n\
             ###..#######.\n\
             ...#.#....##.\n\
             ###.#..##.###"
        );
    }
}

#[cfg(test)]
mod api_tests {
    use crate::{Color, EcLevel, Mode, QrCode, Version};

    fn colors(code: &QrCode) -> Vec<Color> {
        code.to_colors()
    }

    #[test]
    fn builder_matches_with_error_correction_level() {
        let direct = QrCode::with_error_correction_level(b"Some data", EcLevel::H).unwrap();
        let built = QrCode::builder(b"Some data").ec_level(EcLevel::H).build().unwrap();
        assert_eq!(colors(&direct), colors(&built));
        assert_eq!(direct.version(), built.version());
        assert_eq!(direct.error_correction_level(), built.error_correction_level());
    }

    #[test]
    fn builder_matches_with_version() {
        let direct = QrCode::with_version(b"Some data", Version::Normal(1), EcLevel::M).unwrap();
        let built = QrCode::builder(b"Some data").version(Version::Normal(1)).build().unwrap();
        assert_eq!(colors(&direct), colors(&built));
    }

    #[test]
    fn builder_micro_matches() {
        let direct = QrCode::micro_with_error_correction_level(b"123", EcLevel::L).unwrap();
        let built = QrCode::builder(b"123").ec_level(EcLevel::L).micro(true).build().unwrap();
        assert_eq!(colors(&direct), colors(&built));
        assert!(built.version().is_micro());
    }

    #[test]
    fn builder_version_wins_over_micro() {
        let built = QrCode::builder(b"01234567").version(Version::Micro(2)).micro(true).build().unwrap();
        assert_eq!(built.version(), Version::Micro(2));
    }

    #[test]
    fn builder_forces_byte_mode() {
        // Forcing Byte mode on digits must differ from the optimal (Numeric) mode.
        let optimal = QrCode::builder(b"01234567").version(Version::Normal(2)).build().unwrap();
        let byte = QrCode::builder(b"01234567").version(Version::Normal(2)).encoding_mode(Mode::Byte).build().unwrap();
        assert_ne!(colors(&optimal), colors(&byte));
    }

    #[test]
    fn rows_iterate_full_grid() {
        let code = QrCode::new(b"hello").unwrap();
        let w = code.width();
        let rows: Vec<Vec<Color>> = code.rows().map(|r| r.collect()).collect();
        assert_eq!(rows.len(), w);
        assert!(rows.iter().all(|r| r.len() == w));
        for y in 0..w {
            for x in 0..w {
                assert_eq!(rows[y][x], code[(x, y)]);
            }
        }
    }

    #[test]
    fn rows_exact_size() {
        let code = QrCode::new(b"hello").unwrap();
        let mut rows = code.rows();
        let total = rows.len();
        let mut counted = 0;
        while rows.next().is_some() {
            counted += 1;
            assert_eq!(rows.len(), total - counted);
        }
    }

    #[test]
    fn dark_modules_match_indexed_dark_cells() {
        let code = QrCode::new(b"hello").unwrap();
        let w = code.width();
        let expected: Vec<(usize, usize)> =
            (0..w).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| code[(x, y)] == Color::Dark).collect();
        let actual: Vec<(usize, usize)> = code.dark_modules().collect();
        // dark_modules scans in row-major order, matching the construction above.
        assert_eq!(expected, actual);
    }

    #[test]
    fn for_url_uses_high_ec() {
        let code = QrCode::for_url(b"https://example.com").unwrap();
        assert_eq!(code.error_correction_level(), EcLevel::H);
    }

    #[test]
    fn for_wifi_encodes_with_special_chars() {
        let code = QrCode::for_wifi("My;Net", "a,b", "WPA").unwrap();
        assert!(code.width() > 0);
    }

    #[test]
    fn for_vcard_encodes() {
        let code = QrCode::for_vcard("John Doe", "+1234567890", "john@example.com").unwrap();
        assert!(code.width() > 0);
    }

    #[test]
    fn for_gs1_encodes() {
        let code = QrCode::for_gs1("010491234512345915970331301234561842").unwrap();
        assert!(code.width() > 0);
        // GS1 uses FNC1 first position; smallest fitting version, medium EC.
        assert!(!code.version().is_micro());
        assert_eq!(code.error_correction_level(), crate::EcLevel::M);
    }

    #[test]
    fn structured_append_encodes_n_symbols() {
        let codes = QrCode::structured_append(b"hello structured append world", 3, EcLevel::M).unwrap();
        assert_eq!(codes.len(), 3);
        assert!(codes.iter().all(|c| !c.version().is_micro()));
    }

    #[test]
    fn structured_append_rejects_invalid_symbol_count() {
        assert_eq!(
            QrCode::structured_append(b"x", 1, EcLevel::M).err(),
            Some(crate::QrError::InvalidStructuredAppend { value: 1 })
        );
        assert_eq!(
            QrCode::structured_append(b"x", 17, EcLevel::M).err(),
            Some(crate::QrError::InvalidStructuredAppend { value: 17 })
        );
    }

    #[test]
    fn info_reports_metadata() {
        let code = QrCode::with_version(b"01234567", Version::Normal(1), crate::EcLevel::M).unwrap();
        let info = code.info();
        assert_eq!(info.version(), Version::Normal(1));
        assert_eq!(info.ec_level(), crate::EcLevel::M);
        assert_eq!(info.width(), code.width());
        assert_eq!(info.module_count(), code.width() * code.width());
        assert!(info.data_capacity_bytes() > 0);
        // higher EC level => fewer data bytes for the same version
        let code_h = QrCode::with_version(b"01234567", Version::Normal(1), crate::EcLevel::H).unwrap();
        assert!(info.data_capacity_bytes() > code_h.info().data_capacity_bytes());
    }

    #[test]
    fn colors_borrows_without_clone() {
        let code = QrCode::new(b"hello").unwrap();
        let borrowed = code.colors();
        assert_eq!(borrowed.len(), code.width() * code.width());
        // matches the cloning accessor
        assert_eq!(borrowed, code.to_colors().as_slice());
    }

    #[test]
    fn batch_encodes_many_and_short_circuits() {
        let codes = QrCode::batch(vec![b"hi"; 1000], crate::EcLevel::M).unwrap();
        assert_eq!(codes.len(), 1000);
        // short-circuit: a 5000-byte input cannot fit even v40-L.
        let huge: Vec<u8> = (0..5000).map(|i| (i % 256) as u8).collect();
        let mixed: Vec<&[u8]> = vec![&b"ok"[..], &huge[..], &b"also ok"[..]];
        assert!(QrCode::batch(mixed, crate::EcLevel::L).is_err());
    }

    #[cfg(feature = "eps")]
    #[test]
    fn template_applies_colors() {
        let code = QrCode::new(b"template").unwrap();
        let minimal = code.render::<crate::render::eps::Color>().template(&crate::QrTemplate::minimal()).build();
        let dark = code.render::<crate::render::eps::Color>().template(&crate::QrTemplate::dark_mode()).build();
        // minimal => black foreground ("0 0 0 setrgbcolor"); dark_mode changes it.
        assert!(minimal.contains("0 0 0 setrgbcolor"), "minimal should use a black foreground");
        assert!(!dark.contains("0 0 0 setrgbcolor"), "dark_mode should change the foreground");
        assert_ne!(minimal, dark);
    }

    #[test]
    fn analyze_reports_diagnostics() {
        let code = QrCode::with_version(b"01234567", Version::Normal(1), crate::EcLevel::M).unwrap();
        let a = code.analyze();
        let total = code.width() * code.width();
        assert!(a.functional_modules() > 0, "should have functional modules");
        assert!(a.data_modules() > 0, "should have data modules");
        assert_eq!(a.functional_modules() + a.data_modules(), total);
        assert!(a.dark_ratio() > 0.0 && a.dark_ratio() < 1.0);
        let dark = code.colors().iter().filter(|c| **c == Color::Dark).count();
        assert!((a.dark_ratio() - dark as f64 / total as f64).abs() < 1e-9);
    }

    #[test]
    fn force_mode_without_version_auto_selects() {
        // Forcing Byte on digits must differ from auto (Numeric) without pinning a version.
        let auto = QrCode::new(b"0123456789").unwrap();
        let forced_byte = QrCode::builder(b"0123456789").force_mode(Mode::Byte).build().unwrap();
        assert_ne!(colors(&auto), colors(&forced_byte));
        // Forcing Numeric on digits matches auto (which also picks Numeric).
        let forced_num = QrCode::builder(b"0123456789").force_mode(Mode::Numeric).build().unwrap();
        assert_eq!(colors(&auto), colors(&forced_num));
        // Odd-length Kanji input surfaces InvalidCharacter via the length check.
        let err = QrCode::builder(b"\x93").force_mode(Mode::Kanji).build();
        assert!(matches!(err, Err(crate::QrError::InvalidCharacter { .. })));
    }
}

#[cfg(all(test, feature = "image"))]
mod image_tests {
    use crate::{EcLevel, QrCode, Version};
    use image::{Luma, Rgb, load_from_memory};

    #[test]
    fn test_annex_i_qr_as_image() {
        let code = QrCode::new(b"01234567").unwrap();
        let image = code.render::<Luma<u8>>().build();
        let expected =
            load_from_memory(include_bytes!("../docs/images/test_annex_i_qr_as_image.png")).unwrap().to_luma8();
        assert_eq!(image.dimensions(), expected.dimensions());
        assert_eq!(image.into_raw(), expected.into_raw());
    }

    #[test]
    fn test_annex_i_micro_qr_as_image() {
        let code = QrCode::with_version(b"01234567", Version::Micro(2), EcLevel::L).unwrap();
        let image = code
            .render()
            .min_dimensions(200, 200)
            .dark_color(Rgb([128, 0, 0]))
            .light_color(Rgb([255, 255, 128]))
            .build();
        let expected =
            load_from_memory(include_bytes!("../docs/images/test_annex_i_micro_qr_as_image.png")).unwrap().to_rgb8();
        assert_eq!(image.dimensions(), expected.dimensions());
        assert_eq!(image.into_raw(), expected.into_raw());
    }
}

#[cfg(all(test, feature = "svg"))]
mod svg_tests {
    use crate::render::svg::Color as SvgColor;
    use crate::{EcLevel, QrCode, Version};

    #[test]
    fn test_annex_i_qr_as_svg() {
        let code = QrCode::new(b"01234567").unwrap();
        let image = code.render::<SvgColor>().build();
        let expected = include_str!("../docs/images/test_annex_i_qr_as_svg.svg");
        assert_eq!(&image, expected);
    }

    #[test]
    fn test_annex_i_micro_qr_as_svg() {
        let code = QrCode::with_version(b"01234567", Version::Micro(2), EcLevel::L).unwrap();
        let image = code
            .render()
            .min_dimensions(200, 200)
            .dark_color(SvgColor("#800000"))
            .light_color(SvgColor("#ffff80"))
            .build();
        let expected = include_str!("../docs/images/test_annex_i_micro_qr_as_svg.svg");
        assert_eq!(&image, expected);
    }
}

#[cfg(all(test, feature = "eps"))]
mod eps_tests {
    use crate::render::eps::Color as EpsColor;
    use crate::{EcLevel, QrCode, Version};

    #[test]
    fn test_annex_i_qr_as_eps() {
        let code = QrCode::new(b"01234567").unwrap();
        let image = code.render::<EpsColor>().build();
        let expected = include_str!("../docs/images/test_annex_i_qr_as_eps.eps");
        assert_eq!(&image, expected);
    }

    #[test]
    fn test_annex_i_micro_qr_as_eps() {
        let code = QrCode::with_version(b"01234567", Version::Micro(2), EcLevel::L).unwrap();
        let image = code
            .render()
            .min_dimensions(200, 200)
            .dark_color(EpsColor([0.5, 0.0, 0.0]))
            .light_color(EpsColor([1.0, 1.0, 0.5]))
            .build();
        let expected = include_str!("../docs/images/test_annex_i_micro_qr_as_eps.eps");
        assert_eq!(&image, expected);
    }
}

#[cfg(all(test, feature = "pic"))]
mod pic_tests {
    use crate::render::pic::Color as PicColor;
    use crate::{EcLevel, QrCode, Version};

    #[test]
    fn test_annex_i_qr_as_pic() {
        let code = QrCode::new(b"01234567").unwrap();
        let image = code.render::<PicColor>().build();
        let expected = include_str!("../docs/images/test_annex_i_qr_as_pic.pic");
        assert_eq!(&image, expected);
    }

    #[test]
    fn test_annex_i_micro_qr_as_pic() {
        let code = QrCode::with_version(b"01234567", Version::Micro(2), EcLevel::L).unwrap();
        let image = code.render::<PicColor>().min_dimensions(1, 1).build();
        let expected = include_str!("../docs/images/test_annex_i_micro_qr_as_pic.pic");
        assert_eq!(&image, expected);
    }
}
