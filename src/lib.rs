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
#![deny(clippy::uninlined_format_args, clippy::manual_range_contains, clippy::semicolon_if_nothing_returned)]
#![allow(
    clippy::must_use_candidate, // This is just annoying.
)]

pub mod bits;
pub mod canvas;
mod cast;
pub mod ec;
pub mod optimize;
pub mod render;
pub mod types;
pub use crate::types::{Color, EcLevel, Mode, QrError, QrResult, Version};

use crate::cast::As;
use crate::render::{Pixel, Renderer};
use std::iter::FusedIterator;
use std::ops::Index;

/// The encoded QR code symbol.
#[derive(Clone)]
pub struct QrCode {
    content: Vec<Color>,
    version: Version,
    ec_level: EcLevel,
    width: usize,
}

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
        let data = bits.into_bytes();
        let (encoded_data, ec_data) = ec::construct_codewords(&data, version, ec_level)?;
        let mut canvas = canvas::Canvas::new(version, ec_level);
        canvas.draw_all_functional_patterns();
        canvas.draw_data(&encoded_data, &ec_data);
        let canvas = canvas.apply_best_mask();
        Ok(Self { content: canvas.into_colors(), version, ec_level, width: version.width().as_usize() })
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
        let mut payload = String::from("WIFI:T:");
        payload.push_str(auth);
        payload.push_str(";S:");
        push_escaped_wifi(&mut payload, ssid);
        payload.push_str(";P:");
        push_escaped_wifi(&mut payload, password);
        payload.push_str(";;");
        Self::new(payload)
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
        let vcard = format!("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:{name}\r\nTEL:{phone}\r\nEMAIL:{email}\r\nEND:VCARD\r\n");
        Self::new(vcard)
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
}

/// Backslash-escapes the characters that are special in a WiFi QR payload.
fn push_escaped_wifi(out: &mut String, s: &str) {
    for c in s.chars() {
        if matches!(c, ';' | ',' | '"' | '\\' | ':') {
            out.push('\\');
        }
        out.push(c);
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

    /// Hints the encoding [`Mode`] (e.g. [`Mode::Byte`]). Best-effort: it is
    /// honored only when a [`version`](Self::version) is also set; otherwise
    /// automatic mode optimization is used.
    #[must_use]
    pub fn encoding_mode(mut self, mode: Mode) -> Self {
        self.mode_hint = Some(mode);
        self
    }

    /// Builds the [`QrCode`].
    ///
    /// # Errors
    ///
    /// Propagates any [`QrError`](crate::QrError) from the underlying encoder
    /// (e.g. data too long, or an incompatible version / error-correction
    /// combination).
    pub fn build(self) -> QrResult<QrCode> {
        if let Some(version) = self.version {
            if let Some(mode) = self.mode_hint {
                return QrCode::with_mode(self.data, version, self.ec_level, mode);
            }
            return QrCode::with_version(self.data, version, self.ec_level);
        }
        if self.micro {
            return QrCode::micro_with_error_correction_level(self.data, self.ec_level);
        }
        QrCode::with_error_correction_level(self.data, self.ec_level)
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
    fn wifi_escape_helper() {
        let mut out = String::new();
        super::push_escaped_wifi(&mut out, "a;b,c\"d\\e:f");
        assert_eq!(out, "a\\;b\\,c\\\"d\\\\e\\:f");
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
