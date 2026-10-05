//! Image decoding for normal and Micro QR symbols with the optional pure Rust
//! QR engine derived from rxing 0.9.3. Structured Append headers are retained
//! with their payloads.

use alloc::vec::Vec;
use core::fmt;

use crate::engine::common::cpp_essentials::{ConcentricPattern, DecoderResult, StructuredAppendInfo};
use crate::engine::common::{BitMatrix, DetectorRXingResult, HybridBinarizer};
use crate::engine::qrcode::common::{ErrorCorrectionLevel, FormatInformation, Version as BackendVersion};
use crate::engine::qrcode::cpp_port::decoder::{CorrectErrors, Decode, DecodeBitStream};
use crate::engine::qrcode::cpp_port::detector::{GenerateFinderPatternSets, SampleMQR, SampleQR};
use crate::engine::{Binarizer, Exceptions, Luma8Source};
use qrcode_core::{EcLevel, Version};

use crate::{DecodedQrCode, GrayPixels, GrayPixelsError, QrDecoder};

/// Error returned by the private QR engine.
///
/// This type is owned by `qrcode-decode`. It has a distinct Rust type identity
/// from the external crate's `rxing::Exceptions`.
pub use crate::engine::Exceptions as BackendError;

mod finder;

/// Limits and detection settings for [`RxingDecoder::scan_with_options`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanOptions {
    /// Maximum number of input pixels. Defaults to 16,777,216.
    pub max_pixels: u64,
    /// Maximum accepted finder patterns during discovery. Defaults to 512.
    ///
    /// Discovery stops as soon as another distinct pattern exceeds this limit.
    pub max_finder_patterns: usize,
    /// Maximum retained candidate results. Defaults to 256.
    ///
    /// Intermediate candidates have a separate checked budget of eight times
    /// this value. Either limit produces an error instead of truncating results.
    pub max_results: usize,
    /// Search more image rows for small symbols. Defaults to `true`.
    pub try_harder: bool,
    /// Decode the opposite black/white polarity. Defaults to `false`.
    ///
    /// This makes one copy of the binarized bit matrix and flips its bits;
    /// it does not allocate a second grayscale frame or retry normal polarity.
    pub inverted: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self { max_pixels: 16_777_216, max_finder_patterns: 512, max_results: 256, try_harder: true, inverted: false }
    }
}

/// A checked Structured Append header recovered from an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuredAppendHeader {
    position: u8,
    total: u8,
    parity: u8,
}

impl StructuredAppendHeader {
    /// The symbol's one-based position (`1..=total`).
    #[must_use]
    pub const fn position(self) -> u8 {
        self.position
    }

    /// The number of symbols in the sequence (`2..=16`).
    #[must_use]
    pub const fn total(self) -> u8 {
        self.total
    }

    /// The full message's XOR parity byte, shared by all sequence symbols.
    #[must_use]
    pub const fn parity(self) -> u8 {
        self.parity
    }
}

/// One successfully decoded normal or Micro QR symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSymbol {
    decoded: DecodedQrCode,
    structured_append: Option<StructuredAppendHeader>,
}

impl ScanSymbol {
    /// The decoded bytes, version, and error-correction level.
    #[must_use]
    pub fn decoded(&self) -> &DecodedQrCode {
        &self.decoded
    }

    /// Takes the decoded QR value without copying its payload.
    #[must_use]
    pub fn into_decoded(self) -> DecodedQrCode {
        self.decoded
    }

    /// The Structured Append header, if this symbol belongs to a sequence.
    #[must_use]
    pub const fn structured_append(&self) -> Option<StructuredAppendHeader> {
        self.structured_append
    }
}

/// Invalid inputs, resource limits, and individual QR decoding failures.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// A configured resource limit is zero or cannot be represented.
    InvalidOptions(&'static str),
    /// An input dimension is zero or exceeds 32,768 pixels.
    InvalidDimensions {
        /// Input width.
        width: u32,
        /// Input height.
        height: u32,
    },
    /// The grayscale view does not match its declared dimensions.
    InvalidImage(GrayPixelsError),
    /// The input exceeds the configured pixel budget.
    PixelLimit {
        /// Input pixel count.
        actual: u64,
        /// Configured maximum.
        max: u64,
    },
    /// The image contains more finder patterns than allowed.
    FinderLimit {
        /// Observed finder-pattern count when discovery stopped. This is a
        /// lower bound on the image's full count.
        actual: usize,
        /// Configured maximum.
        max: usize,
    },
    /// The intermediate sampled-candidate budget was exceeded.
    CandidateLimit {
        /// Configured intermediate maximum.
        max: usize,
    },
    /// The retained results exceed the configured maximum.
    ResultLimit {
        /// Retained candidate count.
        actual: usize,
        /// Configured maximum.
        max: usize,
    },
    /// Sampled geometry or decoded metadata is inconsistent.
    InvalidMetadata(&'static str),
    /// Detection, error correction, or payload decoding failed in the backend.
    Backend(BackendError),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOptions(option) => write!(f, "invalid QR scan option: {option}"),
            Self::InvalidDimensions { width, height } => {
                write!(f, "QR scan dimensions must be 1..=32768: {width}x{height}")
            }
            Self::InvalidImage(error) => error.fmt(f),
            Self::PixelLimit { actual, max } => {
                write!(f, "QR scan has {actual} pixels, exceeding the {max} pixel limit")
            }
            Self::FinderLimit { actual, max } => {
                write!(f, "QR scan found {actual} finder patterns, exceeding the {max} limit")
            }
            Self::CandidateLimit { max } => write!(f, "QR scan exceeds the {max} intermediate candidate limit"),
            Self::ResultLimit { actual, max } => write!(f, "QR scan has {actual} results, exceeding the {max} limit"),
            Self::InvalidMetadata(field) => write!(f, "invalid decoded QR metadata: {field}"),
            Self::Backend(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for DecodeError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::InvalidImage(error) => Some(error),
            Self::Backend(error) => Some(error),
            _ => None,
        }
    }
}

/// A pure Rust normal/Micro QR decoder with per-candidate scan results.
#[derive(Debug, Default, Clone, Copy)]
pub struct RxingDecoder;

impl RxingDecoder {
    /// Creates a decoder with the default scan limits.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Scans using [`ScanOptions::default`], retaining independent sampled-grid
    /// failures alongside successful symbols. A valid image without candidates
    /// returns an empty vector. Candidate order is backend-defined.
    ///
    /// # Errors
    ///
    /// Returns an outer error for invalid inputs, resource limits, or failure to
    /// prepare an image. Each confirmed sampled-grid failure is an inner error;
    /// its partial payload bytes are never exposed as a successful symbol.
    pub fn scan(&self, image: GrayPixels<'_>) -> Result<Vec<Result<ScanSymbol, DecodeError>>, DecodeError> {
        self.scan_with_options(image, ScanOptions::default())
    }

    /// Scans with explicit limits and polarity. Normal and Micro symbols share
    /// one binarization and finder search. A successful symbol owns its finder
    /// patterns, suppressing duplicate geometries and overlapping false errors.
    /// Distinct symbols are retained even when their payloads are identical.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::scan`]. Limits never silently truncate
    /// the result set; all option and input checks precede image allocation.
    pub fn scan_with_options(
        &self,
        image: GrayPixels<'_>,
        options: ScanOptions,
    ) -> Result<Vec<Result<ScanSymbol, DecodeError>>, DecodeError> {
        let pending_limit = validate_input(image, options)?;
        let source =
            Luma8Source::new_with_slice(image.data, image.width(), image.height()).map_err(DecodeError::Backend)?;
        let binarizer = HybridBinarizer::new(source);
        let original = match binarizer.get_black_matrix() {
            Ok(matrix) => matrix,
            Err(Exceptions::NotFoundException(_)) => return Ok(Vec::new()),
            Err(error) => return Err(DecodeError::Backend(error)),
        };
        let inverted;
        let matrix = if options.inverted {
            inverted = {
                let mut copy = original.clone();
                copy.flip_self();
                copy
            };
            &inverted
        } else {
            original
        };

        let mut finders = finder::find_bounded(matrix, options.try_harder, options.max_finder_patterns)?;
        let sets = GenerateFinderPatternSets(&mut finders);
        let mut good_finders = Vec::new();
        let mut pending = Vec::new();
        for set in sets {
            let ownership = FinderOwnership::normal([set.bl, set.tl, set.tr]);
            if ownership.overlaps(&good_finders) {
                continue;
            }
            let Ok(sampled) = SampleQR(matrix, &set) else { continue };
            let result = Decode(sampled.getBits())
                .map_err(DecodeError::Backend)
                .and_then(|decoded| scan_symbol(decoded, sampled.getBits()));
            retain_candidate(&mut pending, &mut good_finders, ownership, result, pending_limit)?;
        }
        for finder in finders {
            let ownership = FinderOwnership::micro(finder);
            if ownership.overlaps(&good_finders) {
                continue;
            }
            let Ok(sampled) = SampleMQR(matrix, finder) else { continue };
            let result = decode_micro(sampled.getBits()).and_then(|decoded| scan_symbol(decoded, sampled.getBits()));
            retain_candidate(&mut pending, &mut good_finders, ownership, result, pending_limit)?;
        }
        if pending.len() > options.max_results {
            return Err(DecodeError::ResultLimit { actual: pending.len(), max: options.max_results });
        }
        Ok(pending.into_iter().map(|candidate| candidate.result).collect())
    }
}

impl QrDecoder for RxingDecoder {
    type Error = DecodeError;

    fn decode(&self, image: GrayPixels<'_>) -> Result<Vec<DecodedQrCode>, Self::Error> {
        self.scan(image)?.into_iter().map(|candidate| candidate.map(ScanSymbol::into_decoded)).collect()
    }
}

fn validate_input(image: GrayPixels<'_>, options: ScanOptions) -> Result<usize, DecodeError> {
    if options.max_pixels == 0 {
        return Err(DecodeError::InvalidOptions("max_pixels must be positive"));
    }
    if options.max_finder_patterns == 0 {
        return Err(DecodeError::InvalidOptions("max_finder_patterns must be positive"));
    }
    let pending_limit =
        options.max_results.checked_mul(8).filter(|&limit| limit > 0).ok_or(DecodeError::InvalidOptions(
            "max_results must be positive and its candidate budget representable",
        ))?;
    if image.width() == 0 || image.height() == 0 || image.width() > 32_768 || image.height() > 32_768 {
        return Err(DecodeError::InvalidDimensions { width: image.width(), height: image.height() });
    }
    GrayPixels::try_new(image.width(), image.height(), image.data).map_err(DecodeError::InvalidImage)?;
    let pixels = u64::from(image.width()) * u64::from(image.height());
    if pixels > options.max_pixels {
        return Err(DecodeError::PixelLimit { actual: pixels, max: options.max_pixels });
    }
    Ok(pending_limit)
}

struct FinderOwnership {
    patterns: [ConcentricPattern; 3],
    count: usize,
}

impl FinderOwnership {
    fn normal(patterns: [ConcentricPattern; 3]) -> Self {
        Self { patterns, count: 3 }
    }

    fn micro(pattern: ConcentricPattern) -> Self {
        Self { patterns: [pattern; 3], count: 1 }
    }

    fn overlaps(&self, good: &[ConcentricPattern]) -> bool {
        self.patterns[..self.count].iter().any(|pattern| good.contains(pattern))
    }
}

struct Candidate {
    ownership: FinderOwnership,
    result: Result<ScanSymbol, DecodeError>,
}

fn retain_candidate(
    pending: &mut Vec<Candidate>,
    good: &mut Vec<ConcentricPattern>,
    ownership: FinderOwnership,
    result: Result<ScanSymbol, DecodeError>,
    limit: usize,
) -> Result<(), DecodeError> {
    if result.is_ok() {
        good.extend_from_slice(&ownership.patterns[..ownership.count]);
        pending.retain(|candidate| candidate.result.is_ok() || !candidate.ownership.overlaps(good));
    }
    if pending.len() >= limit {
        return Err(DecodeError::CandidateLimit { max: limit });
    }
    pending.push(Candidate { ownership, result });
    Ok(())
}

fn scan_symbol(decoded: DecoderResult<bool>, bits: &BitMatrix) -> Result<ScanSymbol, DecodeError> {
    if let Some(error) = decoded.error() {
        return Err(DecodeError::Backend(error.clone()));
    }
    if !decoded.isValid() || bits.width() != bits.height() {
        return Err(DecodeError::InvalidMetadata("invalid symbol geometry or payload"));
    }
    let number = decoded.versionNumber();
    let version = if bits.width() < 21 {
        if !(1..=4).contains(&number) || bits.width() != number * 2 + 9 {
            return Err(DecodeError::InvalidMetadata("Micro QR version does not match sampled dimensions"));
        }
        Version::Micro(number as i16)
    } else {
        if !(1..=40).contains(&number) || bits.width() != number * 4 + 17 {
            return Err(DecodeError::InvalidMetadata("QR version does not match sampled dimensions"));
        }
        Version::Normal(number as i16)
    };
    let ec_level = match decoded.ecLevel() {
        "L" => EcLevel::L,
        "M" => EcLevel::M,
        "Q" => EcLevel::Q,
        "H" => EcLevel::H,
        _ => return Err(DecodeError::InvalidMetadata("unknown error-correction level")),
    };
    qrcode_core::bits::data_capacity_bits(version, ec_level)
        .map_err(|_| DecodeError::InvalidMetadata("unsupported version and error-correction level"))?;
    let structured_append = checked_append_header(decoded.structuredAppend())?;
    // Move raw content only after every payload and metadata check succeeded.
    Ok(ScanSymbol {
        decoded: DecodedQrCode::new(decoded.into_content().into_bytes(), version, ec_level),
        structured_append,
    })
}

fn checked_append_header(info: &StructuredAppendInfo) -> Result<Option<StructuredAppendHeader>, DecodeError> {
    if info.index == -1 && info.count == -1 && info.id.is_empty() {
        return Ok(None);
    }
    if !(2..=16).contains(&info.count) || info.index < 0 || info.index >= info.count {
        return Err(DecodeError::InvalidMetadata("Structured Append position or count"));
    }
    let parity = info.id.parse::<u8>().map_err(|_| DecodeError::InvalidMetadata("Structured Append parity"))?;
    Ok(Some(StructuredAppendHeader { position: info.index as u8 + 1, total: info.count as u8, parity }))
}

// Micro data extraction is a direct stage, not a retry of the normal decoder.
// The final M1/M3 data word has four transmitted high bits and four implicit
// zero bits. Assemble it in that form before computing RS syndromes.
fn decode_micro(bits: &BitMatrix) -> Result<DecoderResult<bool>, DecodeError> {
    let side = bits.width();
    if side != bits.height() || ![11, 13, 15, 17].contains(&side) {
        return Err(DecodeError::InvalidMetadata("invalid Micro QR dimensions"));
    }
    let number = (side - 9) / 2;
    let mut format_bits = 0_u32;
    for x in 1..=8 {
        format_bits = (format_bits << 1) | u32::from(bits.get(x, 8));
    }
    for y in (1..=7).rev() {
        format_bits = (format_bits << 1) | u32::from(bits.get(8, y));
    }
    let format = FormatInformation::DecodeMQR(format_bits);
    if !format.isValid() || format.microVersion != number || format.data_mask > 3 {
        return Err(DecodeError::InvalidMetadata("invalid Micro QR format information"));
    }
    let ec_level = match format.error_correction_level {
        ErrorCorrectionLevel::L => EcLevel::L,
        ErrorCorrectionLevel::M => EcLevel::M,
        ErrorCorrectionLevel::Q => EcLevel::Q,
        ErrorCorrectionLevel::H => EcLevel::H,
        ErrorCorrectionLevel::Invalid => return Err(DecodeError::InvalidMetadata("invalid Micro QR correction level")),
    };
    let core_version = Version::Micro(number as i16);
    let capacity = qrcode_core::bits::data_capacity_bits(core_version, ec_level)
        .map_err(|_| DecodeError::InvalidMetadata("unsupported Micro QR correction level"))?;
    let version = BackendVersion::Micro(number).map_err(DecodeError::Backend)?;
    let blocks = version.getECBlocksForLevel(format.error_correction_level);
    let Some(block) = blocks.getECBlocks().first() else {
        return Err(DecodeError::InvalidMetadata("missing Micro QR correction block"));
    };
    let data_words = block.getDataCodewords() as usize;
    let total_words = version.getTotalCodewords() as usize;
    if blocks.getNumBlocks() != 1
        || data_words != capacity.div_ceil(8)
        || total_words != data_words + blocks.getECCodewordsPerBlock() as usize
        || data_words == 0
        || total_words > 24
    {
        return Err(DecodeError::InvalidMetadata("inconsistent Micro QR correction block"));
    }
    let half_word = number == 1 || number == 3;
    let mut words = [0_u8; 24];
    let mut word_count = 0;
    let mut current = 0_u8;
    let mut bit_count = 0;
    let mut reading_up = true;
    for right in (1..side).rev().step_by(2) {
        for row in 0..side {
            let y = if reading_up { side - 1 - row } else { row };
            for column in 0..2 {
                let x = right - column;
                if qrcode_core::canvas::is_functional(core_version, side as i16, x as i16, y as i16) {
                    continue;
                }
                let masked = if format.isMirrored { bits.get(y, x) } else { bits.get(x, y) };
                let dark = masked ^ micro_mask(format.data_mask, x, y);
                current = (current << 1) | u8::from(dark);
                bit_count += 1;
                let word_bits = if half_word && word_count == data_words - 1 { 4 } else { 8 };
                if bit_count == word_bits {
                    if word_count >= total_words {
                        return Err(DecodeError::InvalidMetadata("too many Micro QR codeword bits"));
                    }
                    words[word_count] = if word_bits == 4 { current << 4 } else { current };
                    word_count += 1;
                    current = 0;
                    bit_count = 0;
                }
            }
        }
        reading_up = !reading_up;
    }
    if word_count != total_words || bit_count != 0 {
        return Err(DecodeError::InvalidMetadata("incomplete Micro QR codewords"));
    }
    if !CorrectErrors(&mut words[..total_words], data_words as u32).map_err(DecodeError::Backend)? {
        return Err(DecodeError::Backend(Exceptions::CHECKSUM));
    }
    if half_word && words[data_words - 1] & 0x0f != 0 {
        return Err(DecodeError::InvalidMetadata("non-zero Micro QR padding bits"));
    }
    DecodeBitStream(&words[..data_words], version, format.error_correction_level)
        .map(|decoded| decoded.withIsMirrored(format.isMirrored))
        .map_err(DecodeError::Backend)
}

fn micro_mask(mask: u8, x: u32, y: u32) -> bool {
    match mask {
        0 => y.is_multiple_of(2),
        1 => (y / 2 + x / 3).is_multiple_of(2),
        2 => ((x * y) % 2 + (x * y) % 3).is_multiple_of(2),
        3 => ((x + y) % 2 + (x * y) % 3).is_multiple_of(2),
        _ => unreachable!("checked Micro QR mask"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::LuminanceSource;
    use std::borrow::Cow;

    #[test]
    fn scan_symbol_transfers_payload_allocation_after_metadata_validation() {
        use crate::engine::common::ECIStringBuilder;
        let bits = BitMatrix::new(21, 21).unwrap();
        for payload in [b"".as_slice(), b"\0\xffA\x1d".as_slice()] {
            for append in
                [StructuredAppendInfo::default(), StructuredAppendInfo { index: 1, count: 2, id: "231".into() }]
            {
                let mut content = ECIStringBuilder::default();
                content.symbology.code = b'Q';
                content.reserve(128);
                for byte in payload {
                    content.append_byte(*byte);
                }
                let pointer = content.bytes().as_ptr();
                let decoded = DecoderResult::<bool>::with_eci_string_builder(content)
                    .withVersionNumber(1)
                    .withEcLevel("L".into())
                    .withStructuredAppend(append.clone());
                let symbol = scan_symbol(decoded, &bits).unwrap();
                assert_eq!(symbol.decoded().data(), payload);
                assert_eq!(symbol.structured_append(), checked_append_header(&append).unwrap());
                let bytes = symbol.into_decoded().into_data();
                assert_eq!(bytes.as_ptr(), pointer);
                assert!(bytes.capacity() >= 128);
            }
        }
    }

    #[test]
    fn scan_symbol_rejects_partial_payloads_and_invalid_metadata() {
        use crate::engine::common::ECIStringBuilder;
        let bits = BitMatrix::new(21, 21).unwrap();
        let mut content = ECIStringBuilder::default();
        content.symbology.code = b'Q';
        content.append_string("partial");
        let valid =
            DecoderResult::<bool>::with_eci_string_builder(content).withVersionNumber(1).withEcLevel("L".into());
        assert!(matches!(
            scan_symbol(valid.clone().withError(Some(Exceptions::FORMAT)), &bits),
            Err(DecodeError::Backend(_))
        ));
        assert!(matches!(scan_symbol(valid.clone().withVersionNumber(2), &bits), Err(DecodeError::InvalidMetadata(_))));
        assert!(matches!(
            scan_symbol(valid.clone().withEcLevel("?".into()), &bits),
            Err(DecodeError::InvalidMetadata(_))
        ));
        assert!(matches!(
            scan_symbol(valid.withStructuredAppend(StructuredAppendInfo { index: 0, count: 1, id: "0".into() }), &bits),
            Err(DecodeError::InvalidMetadata(_))
        ));
    }

    #[test]
    fn append_metadata_keeps_real_parity_count_and_one_based_position() {
        assert_eq!(checked_append_header(&StructuredAppendInfo::default()), Ok(None));
        let info = StructuredAppendInfo { index: 15, count: 16, id: "231".into() };
        let header = checked_append_header(&info).unwrap().unwrap();
        assert_eq!(header.position(), 16);
        assert_eq!(header.total(), 16);
        assert_eq!(header.parity(), 231);
        for info in [
            StructuredAppendInfo { index: -1, count: -1, id: "0".into() },
            StructuredAppendInfo { index: 0, count: 1, id: "0".into() },
            StructuredAppendInfo { index: 2, count: 2, id: "0".into() },
            StructuredAppendInfo { index: 0, count: 2, id: "256".into() },
        ] {
            assert!(checked_append_header(&info).is_err());
        }
    }

    #[test]
    fn limits_reject_bad_views_and_options_before_preparing_pixels() {
        let image = GrayPixels::new(2, 2, &[255; 4]);
        for options in [
            ScanOptions { max_pixels: 0, ..ScanOptions::default() },
            ScanOptions { max_finder_patterns: 0, ..ScanOptions::default() },
            ScanOptions { max_results: 0, ..ScanOptions::default() },
            ScanOptions { max_results: usize::MAX, ..ScanOptions::default() },
        ] {
            assert!(matches!(validate_input(image, options), Err(DecodeError::InvalidOptions(_))));
        }
        let options = ScanOptions { max_pixels: 3, ..ScanOptions::default() };
        assert!(matches!(validate_input(image, options), Err(DecodeError::PixelLimit { actual: 4, max: 3 })));
        assert!(matches!(
            validate_input(GrayPixels::new(32_769, 1, &[]), ScanOptions::default()),
            Err(DecodeError::InvalidDimensions { .. })
        ));
        assert!(matches!(
            validate_input(GrayPixels::new(2, 2, &[0]), ScanOptions::default()),
            Err(DecodeError::InvalidImage(_))
        ));
    }

    #[test]
    fn luminance_borrows_the_input_without_modifying_it() {
        let bytes = [1, 2, 3, 4];
        let source = Luma8Source::new_with_slice(&bytes, 2, 2).unwrap();
        assert!(matches!(source.get_matrix(), Cow::Borrowed(_)));
        assert_eq!(source.get_matrix().as_ptr(), bytes.as_ptr());
        assert_eq!(source.get_row(1).unwrap().as_ref(), &[3, 4]);
        assert_eq!(source.get_row(0).unwrap().as_ref(), &[1, 2]);
        assert_eq!(bytes, [1, 2, 3, 4]);
    }

    #[test]
    fn error_sources_retain_backend_and_view_errors() {
        use core::error::Error;
        let error = DecodeError::Backend(Exceptions::CHECKSUM);
        assert!(error.source().unwrap().downcast_ref::<Exceptions>().is_some());
        let error = DecodeError::InvalidImage(GrayPixelsError::BufferLengthMismatch { expected: 4, actual: 1 });
        assert!(error.source().unwrap().downcast_ref::<GrayPixelsError>().is_some());
    }

    #[test]
    fn confirmed_finders_remove_only_overlapping_errors_not_equal_payloads() {
        let first = ConcentricPattern { size: 1, ..ConcentricPattern::default() };
        let independent = ConcentricPattern { size: 2, ..ConcentricPattern::default() };
        let duplicate_payload = ConcentricPattern { size: 3, ..ConcentricPattern::default() };
        let mut pending = Vec::new();
        let mut good = Vec::new();
        for finder in [first, independent] {
            retain_candidate(
                &mut pending,
                &mut good,
                FinderOwnership::micro(finder),
                Err(DecodeError::Backend(Exceptions::CHECKSUM)),
                8,
            )
            .unwrap();
        }
        for finder in [first, duplicate_payload] {
            retain_candidate(
                &mut pending,
                &mut good,
                FinderOwnership::micro(finder),
                Ok(ScanSymbol {
                    decoded: DecodedQrCode::new(b"same".to_vec(), Version::Micro(2), EcLevel::L),
                    structured_append: None,
                }),
                8,
            )
            .unwrap();
        }
        assert_eq!(pending.len(), 3);
        assert!(pending[0].result.is_err());
        assert_eq!(pending[0].ownership.patterns[0], independent);
        assert_eq!(pending[1].result.as_ref().unwrap().decoded().data(), b"same");
        assert_eq!(pending[2].result.as_ref().unwrap().decoded().data(), b"same");
    }
}
