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

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
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
pub use crate::types::{Color, EcLevel, QrResult, Version};

use crate::cast::As;
use crate::render::{Pixel, Renderer};
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

    /// Converts the QR code to a vector of booleans. Each entry represents the
    /// color of the module, with "true" means dark and "false" means light.
    #[deprecated(since = "0.2.0", note = "use `to_colors()` instead")]
    pub fn to_vec(&self) -> Vec<bool> {
        self.content.iter().map(|c| *c != Color::Light).collect()
    }

    /// Converts the QR code to a vector of booleans. Each entry represents the
    /// color of the module, with "true" means dark and "false" means light.
    #[deprecated(since = "0.2.0", note = "use `into_colors()` instead")]
    pub fn into_vec(self) -> Vec<bool> {
        self.content.into_iter().map(|c| c != Color::Light).collect()
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

impl Index<(usize, usize)> for QrCode {
    type Output = Color;

    fn index(&self, (x, y): (usize, usize)) -> &Color {
        let index = y * self.width + x;
        &self.content[index]
    }
}

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
mod boundary_tests {
    use crate::bits::{self, Bits};
    use crate::{EcLevel, QrCode, Version};

    #[test]
    fn test_max_version_qr_v40() {
        // Version 40-L can hold 2953 bytes
        let data: Vec<u8> = (0..2953).map(|i| (i % 256) as u8).collect();
        let code = QrCode::with_error_correction_level(&data, EcLevel::L).unwrap();
        assert_eq!(code.version(), Version::Normal(40));
        assert_eq!(code.width(), 177);
    }

    #[test]
    fn test_max_version_qr_v40_high_ec() {
        // Version 40-H can hold 1273 bytes
        let data: Vec<u8> = (0..1273).map(|i| (i % 256) as u8).collect();
        let code = QrCode::with_error_correction_level(&data, EcLevel::H).unwrap();
        assert_eq!(code.version(), Version::Normal(40));
    }

    #[test]
    fn test_max_micro_qr_m4() {
        // M4-L can hold 15 bytes
        let code = QrCode::with_version(b"Hello, world!!!", Version::Micro(4), EcLevel::L).unwrap();
        assert_eq!(code.width(), 17);
    }

    #[test]
    fn test_empty_input() {
        // Empty data should succeed (0-length is valid for QR encoding)
        let result = QrCode::new(b"");
        assert!(result.is_ok());
    }

    #[test]
    fn test_long_input_auto_version() {
        // A medium-length string should auto-select an appropriate version
        let data = b"This is a test string for automatic version selection in QR code encoding.";
        let code = QrCode::new(data).unwrap();
        assert!(code.version().width() >= 21);
    }

    #[test]
    fn test_data_too_long_standard_qr() {
        // Definitely exceeds v40-L capacity
        let data: Vec<u8> = (0..5000).map(|i| (i % 256) as u8).collect();
        let result = QrCode::with_error_correction_level(&data, EcLevel::L);
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_auto_micro_numeric() {
        let code = QrCode::new_micro(b"0123456789").unwrap();
        assert!(code.version().is_micro());
    }

    #[test]
    fn test_encode_auto_micro_too_long() {
        // 100 numeric digits exceeds M4 capacity (35 digits)
        let data: Vec<u8> = (0..100).map(|i| b'0' + (i % 10)).collect();
        let result = QrCode::new_micro(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_eci_designator() {
        // Test ECI designator 9 (ISO-8859-1)
        let mut bits = Bits::new(Version::Normal(1));
        bits.push_eci_designator(9).unwrap();
        bits.push_byte_data(b"test").unwrap();
        bits.push_terminator(EcLevel::L).unwrap();
        assert!(QrCode::with_bits(bits, EcLevel::L).is_ok());
    }

    #[test]
    fn test_eci_designator_range() {
        let mut bits = Bits::new(Version::Normal(1));
        assert!(bits.push_eci_designator(0).is_ok());

        let mut bits = Bits::new(Version::Normal(1));
        assert!(bits.push_eci_designator(999999).is_ok());

        let mut bits = Bits::new(Version::Normal(1));
        assert!(bits.push_eci_designator(1000000).is_err());
    }

    #[test]
    fn test_all_versions_encode() {
        for v in 1..=40 {
            let version = Version::Normal(v);
            let result = QrCode::with_version(b"A", version, EcLevel::L);
            assert!(result.is_ok(), "Version {v} should encode 1 byte");
        }
    }

    #[test]
    fn test_all_micro_versions_encode() {
        for v in 1..=4 {
            let version = Version::Micro(v);
            let result = QrCode::with_version(b"0", version, EcLevel::L);
            if v == 1 {
                let _ = result;
            } else {
                assert!(result.is_ok(), "Micro version M{v} should encode 1 digit");
            }
        }
    }

    #[test]
    fn test_encode_auto_function() {
        let bits = bits::encode_auto(b"Hello, World!", EcLevel::M).unwrap();
        let code = QrCode::with_bits(bits, EcLevel::M).unwrap();
        assert!(code.width() >= 21);
    }

    #[test]
    fn test_encode_auto_micro_function() {
        let bits = bits::encode_auto_micro(b"12345", EcLevel::L).unwrap();
        let code = QrCode::with_bits(bits, EcLevel::L).unwrap();
        assert!(code.version().is_micro());
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
