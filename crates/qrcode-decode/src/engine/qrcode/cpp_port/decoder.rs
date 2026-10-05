// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
/*
* Copyright 2016 Nu-book Inc.
* Copyright 2016 ZXing authors
*/
// SPDX-License-Identifier: Apache-2.0

use crate::engine::Exceptions;
use crate::engine::common::cpp_essentials::{DecoderResult, StructuredAppendInfo};
use crate::engine::common::reedsolomon::{PredefinedGenericGF, ReedSolomonDecoder};
use crate::engine::common::{
    AIFlag, BitMatrix, BitSource, CharacterSet, ECIStringBuilder, Eci, Result, SymbologyIdentifier,
};
use crate::engine::qrcode::common::{ErrorCorrectionLevel, Mode, Version};
use crate::engine::qrcode::cpp_port::bitmatrix_parser::{ReadCodewords, ReadFormatInformation, ReadVersion};
use crate::engine::qrcode::decoder::DataBlock;

/**
* <p>Given data and error-correction codewords received, possibly corrupted by errors, attempts to
* correct the errors in-place using Reed-Solomon error correction.</p>
*
* @param codewordBytes data and error correction codewords
* @param numDataCodewords number of codewords that are data bytes
* @return false if error correction fails
*/
pub fn CorrectErrors(codewordBytes: &mut [u8], numDataCodewords: u32) -> Result<bool> {
    let numECCodewords = ((codewordBytes.len() as u32) - numDataCodewords) as i32;
    let rs = ReedSolomonDecoder::new(PredefinedGenericGF::QrCodeField256.into());

    rs.decode(codewordBytes, numECCodewords)?;

    Ok(true)
}

/**
* See specification GBT 18284-2000
*/
pub fn DecodeHanziSegment(bits: &mut BitSource, count: u32, result: &mut ECIStringBuilder) -> Result<()> {
    let mut count = count;

    // Each character will require 2 bytes, decode as GB2312
    // There is no ECI value for GB2312, use GB18030 which is a superset
    result.switch_encoding(CharacterSet::GB18030, false);
    result.reserve(2 * count as usize);

    while count > 0 {
        // Each 13 bits encodes a 2-byte character
        let twoBytes = bits.readBits(13)?;
        let mut assembledTwoBytes = ((twoBytes / 0x060) << 8) | (twoBytes % 0x060);
        if assembledTwoBytes < 0x00A00 {
            // In the 0xA1A1 to 0xAAFE range
            assembledTwoBytes += 0x0A1A1;
        } else {
            // In the 0xB0A1 to 0xFAFE range
            assembledTwoBytes += 0x0A6A1;
        }
        *result += ((assembledTwoBytes >> 8) & 0xFF) as u8;
        *result += (assembledTwoBytes & 0xFF) as u8;
        count -= 1;
    }
    Ok(())
}

pub fn DecodeKanjiSegment(bits: &mut BitSource, count: u32, result: &mut ECIStringBuilder) -> Result<()> {
    let mut count = count;
    // Each character will require 2 bytes. Read the characters as 2-byte pairs
    // and decode as Shift_JIS afterwards
    result.switch_encoding(CharacterSet::Shift_JIS, false);
    result.reserve(2 * count as usize);

    while count > 0 {
        // Each 13 bits encodes a 2-byte character
        let twoBytes = bits.readBits(13)?;
        let mut assembledTwoBytes = ((twoBytes / 0x0C0) << 8) | (twoBytes % 0x0C0);
        if assembledTwoBytes < 0x01F00 {
            // In the 0x8140 to 0x9FFC range
            assembledTwoBytes += 0x08140;
        } else {
            // In the 0xE040 to 0xEBBF range
            assembledTwoBytes += 0x0C140;
        }
        *result += (assembledTwoBytes >> 8) as u8;
        *result += (assembledTwoBytes) as u8;
        count -= 1;
    }
    Ok(())
}

pub fn DecodeByteSegment(bits: &mut BitSource, count: u32, result: &mut ECIStringBuilder) -> Result<()> {
    result.switch_encoding(CharacterSet::Unknown, false);
    result.reserve(count as usize);

    for _i in 0..count {
        // for (int i = 0; i < count; i++)
        *result += (bits.readBits(8)?) as u8;
    }
    Ok(())
}

pub fn ToAlphaNumericChar(value: u32) -> Result<char> {
    let value = value as usize;
    /**
     * See ISO 18004:2006, 6.4.4 Table 5
     */
    const ALPHANUMERIC_CHARS: [char; 45] = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
        'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', ' ', '$', '%', '*', '+', '-', '.', '/',
        ':',
    ];

    if value >= (ALPHANUMERIC_CHARS.len()) {
        return Err(Exceptions::index_out_of_bounds_with("oAlphaNumericChar: out of range"));
    }

    Ok(ALPHANUMERIC_CHARS[value])
}

pub fn DecodeAlphanumericSegment(bits: &mut BitSource, count: u32, result: &mut ECIStringBuilder) -> Result<()> {
    let mut count = count;

    // Read two characters at a time
    let mut buffer = Vec::new();

    while count > 1 {
        let nextTwoCharsBits = bits.readBits(11)?;
        buffer.push(ToAlphaNumericChar(nextTwoCharsBits / 45)?);
        buffer.push(ToAlphaNumericChar(nextTwoCharsBits % 45)?);
        count -= 2;
    }
    if count == 1 {
        // special case: one character left
        buffer.push(ToAlphaNumericChar(bits.readBits(6)?)?);
    }
    // See section 6.4.8.1, 6.4.8.2
    if result.symbology.aiFlag != AIFlag::None {
        // Compact escapes in place. The write position never passes the read
        // position, so each original character is examined at most once.
        let mut read = 0;
        let mut write = 0;
        while read < buffer.len() {
            let ch = buffer[read];
            buffer[write] = if ch == '%' {
                if buffer.get(read + 1) == Some(&'%') {
                    read += 1;
                    '%'
                } else {
                    char::from(0x1D)
                }
            } else {
                ch
            };
            read += 1;
            write += 1;
        }
        buffer.truncate(write);
    }

    result.switch_encoding(CharacterSet::ISO8859_1, false);
    *result += buffer.iter().collect::<String>();

    Ok(())
}

pub fn DecodeNumericSegment(bits: &mut BitSource, count: u32, result: &mut ECIStringBuilder) -> Result<()> {
    let mut count = count;

    result.switch_encoding(CharacterSet::ISO8859_1, false);
    result.reserve(count as usize);

    while count > 0 {
        let n = std::cmp::min(count, 3);
        let nDigits = bits.readBits(1 + 3 * n as usize)?; // read 4, 7 or 10 bits into 1, 2 or 3 digits
        result.append_string(&crate::engine::common::cpp_essentials::util::ToString(nDigits as usize, n as usize)?);
        count -= n;
    }

    Ok(())
}

pub fn ParseECIValue(bits: &mut BitSource) -> Result<Eci> {
    let firstByte = bits.readBits(8)?;
    if (firstByte & 0x80) == 0 {
        // just one byte
        return Ok(Eci::from(firstByte & 0x7F));
    }
    if (firstByte & 0xC0) == 0x80 {
        // two bytes
        let secondByte = bits.readBits(8)?;
        return Ok(Eci::from(((firstByte & 0x3F) << 8) | secondByte));
    }
    if (firstByte & 0xE0) == 0xC0 {
        // three bytes
        let secondThirdBytes = bits.readBits(16)?;
        return Ok(Eci::from(((firstByte & 0x1F) << 16) | secondThirdBytes));
    }
    Err(Exceptions::format_with("ParseECIValue: invalid value"))
}

/**
 * QR codes encode mode indicators and terminator codes into a constant bit length of 4.
 * Micro QR codes have terminator codes that vary in bit length but are always longer than
 * the mode indicators.
 * M1 - 0 length mode code, 3 bits terminator code
 * M2 - 1 bit mode code, 5 bits terminator code
 * M3 - 2 bit mode code, 7 bits terminator code
 * M4 - 3 bit mode code, 9 bits terminator code
 * IsTerminator peaks into the bit stream to see if the current position is at the start of
 * a terminator code.  If true, then the decoding can finish. If false, then the decoding
 * can read off the next mode code.
 *
 * See ISO 18004:2015, 7.4.1 Table 2
 *
 * @param bits the stream of bits that might have a terminator code
 * @param version the QR or micro QR code version
 */
pub fn IsEndOfStream(bits: &mut BitSource, version: &Version) -> Result<bool> {
    let bitsRequired = Mode::get_terminator_bit_length(version); //super::qr_codec_mode::TerminatorBitsLength(version);
    let bitsAvailable = std::cmp::min(bits.available(), bitsRequired as usize);
    Ok(bitsAvailable == 0 || bits.peak_bits(bitsAvailable)? == 0)
}

/**
* <p>QR Codes can encode text as bits in one of several modes, and can use multiple modes
* in one QR Code. This method decodes the bits back into text.</p>
*
* <p>See ISO 18004:2006, 6.4.3 - 6.4.7</p>
*/
// ZXING_EXPORT_TEST_ONLY
pub fn DecodeBitStream(bytes: &[u8], version: &Version, ecLevel: ErrorCorrectionLevel) -> Result<DecoderResult<bool>> {
    let mut bits = BitSource::new(bytes);
    let mut result = ECIStringBuilder::default();
    // Error error;
    result.symbology = SymbologyIdentifier { code: b'Q', modifier: b'1', eciModifierOffset: 1, aiFlag: AIFlag::None }; //{'Q', '1', 1};
    let mut structuredAppend = StructuredAppendInfo::default();
    let modeBitLength = Mode::get_codec_mode_bits_length(version);

    if version.isModel1() {
        bits.readBits(4)?; /* Model 1 is leading with 4 0-bits -> drop them */
    }

    let res = (|| {
        while !IsEndOfStream(&mut bits, version)? {
            let mode: Mode = if modeBitLength == 0 {
                Mode::NUMERIC // MicroQRCode version 1 is always NUMERIC and modeBitLength is 0
            } else {
                Mode::CodecModeForBits(bits.readBits(modeBitLength as usize)?, Some(version.qr_type))?
            };

            match mode {
                Mode::FNC1_FIRST_POSITION => {
                    //				if (!result.empty()) // uncomment to enforce specification
                    //					throw FormatError("GS1 Indicator (FNC1 in first position) at illegal position");
                    result.symbology.modifier = b'3';
                    result.symbology.aiFlag = AIFlag::GS1; // In Alphanumeric mode undouble doubled '%' and treat single '%' as <GS>
                }
                Mode::FNC1_SECOND_POSITION => {
                    if !result.is_empty() {
                        return Err(Exceptions::format_with(
                            "AIM Application Indicator (FNC1 in second position) at illegal position",
                        ));
                        // throw FormatError("AIM Application Indicator (FNC1 in second position) at illegal position");
                    }
                    result.symbology.modifier = b'5'; // As above
                    // ISO/IEC 18004:2015 7.4.8.3 AIM Application Indicator (FNC1 in second position), "00-99" or "A-Za-z"
                    let appInd = bits.readBits(8)?;
                    if appInd < 100
                    // "00-09"
                    {
                        result += crate::engine::common::cpp_essentials::util::ToString(appInd as usize, 2)?;
                    } else if (165..=190).contains(&appInd) || (197..=222).contains(&appInd)
                    // "A-Za-z"
                    {
                        result += (appInd - 100) as u8;
                    } else {
                        return Err(Exceptions::format_with("Invalid AIM Application Indicator"));
                        // throw FormatError("Invalid AIM Application Indicator");
                    }
                    result.symbology.aiFlag = AIFlag::AIM; // see also above
                }
                Mode::STRUCTURED_APPEND => {
                    // sequence number and parity is added later to the result metadata
                    // Read next 4 bits of index, 4 bits of symbol count, and 8 bits of parity data, then continue
                    structuredAppend.index = bits.readBits(4)? as i32;
                    structuredAppend.count = bits.readBits(4)? as i32 + 1;
                    structuredAppend.id = (bits.readBits(8)?).to_string(); //std::to_string(bits.readBits(8));
                }
                Mode::ECI => {
                    // Count doesn't apply to ECI
                    result.switch_encoding(ParseECIValue(&mut bits)?.into(), true);
                }
                Mode::HANZI => {
                    // First handle Hanzi mode which does not start with character count
                    // chinese mode contains a sub set indicator right after mode indicator
                    let subset = bits.readBits(4)?;
                    if subset != 1
                    // GB2312_SUBSET is the only supported one right now
                    {
                        return Err(Exceptions::format_with("Unsupported HANZI subset"));
                        // throw FormatError("Unsupported HANZI subset");
                    }
                    let count = bits.readBits(mode.CharacterCountBits(version) as usize)?;
                    DecodeHanziSegment(&mut bits, count, &mut result)?;
                }
                _ => {
                    // "Normal" QR code modes:
                    // How many characters will follow, encoded in this mode?
                    let count = bits.readBits(mode.CharacterCountBits(version) as usize)?;
                    match mode {
                        Mode::NUMERIC => DecodeNumericSegment(&mut bits, count, &mut result)?,
                        Mode::ALPHANUMERIC => DecodeAlphanumericSegment(&mut bits, count, &mut result)?,
                        Mode::BYTE => DecodeByteSegment(&mut bits, count, &mut result)?,
                        Mode::KANJI => DecodeKanjiSegment(&mut bits, count, &mut result)?,
                        _ => return Err(Exceptions::format_with("Invalid CodecMode")), //throw FormatError("Invalid CodecMode");
                    };
                }
            }
        }
        Ok(())
    })();

    Ok(DecoderResult::with_eci_string_builder(result)
        .withError(res.err())
        .withEcLevel(ecLevel.to_string())
        .withVersionNumber(version.getVersionNumber())
        .withStructuredAppend(structuredAppend)
        .withIsModel1(version.isModel1()))
}

pub fn Decode(bits: &BitMatrix) -> Result<DecoderResult<bool>> {
    // This entry point serves the adapter's normal Model 2 symbols. Micro QR
    // codewords are extracted and checked by the adapter before DecodeBitStream.
    if !Version::HasValidSizeType(bits, crate::engine::qrcode::cpp_port::Type::Model2) {
        return Err(Exceptions::format_with("Invalid symbol size"));
    }
    let Ok(formatInfo) = ReadFormatInformation(bits) else {
        return Err(Exceptions::format_with("Invalid format information"));
    };
    if !formatInfo.isValid() || formatInfo.qr_type() != crate::engine::qrcode::cpp_port::Type::Model2 {
        return Err(Exceptions::format_with("Invalid format information"));
    }

    let Ok(pversion) = ReadVersion(bits, formatInfo.qr_type()) else {
        return Err(Exceptions::format_with("Invalid version"));
    };
    let version = pversion;

    // Read codewords
    let codewords = ReadCodewords(bits, version, &formatInfo)?;
    if codewords.is_empty() {
        return Err(Exceptions::format_with("Failed to read codewords"));
    }

    // Separate into data blocks
    let dataBlocks: Vec<DataBlock> = DataBlock::getDataBlocks(&codewords, version, formatInfo.error_correction_level)?;
    if dataBlocks.is_empty() {
        return Err(Exceptions::format_with("Failed to get data blocks"));
    }

    // Count total number of data bytes
    let op = |totalBytes, dataBlock: &DataBlock| totalBytes + dataBlock.getNumDataCodewords();
    let totalBytes = dataBlocks.iter().fold(0, op); // std::accumulate(std::begin(dataBlocks), std::end(dataBlocks), int{}, op);
    let mut resultBytes = vec![0u8; totalBytes as usize];
    let mut resultIterator = 0; //resultBytes.begin();

    // Error-correct and copy data blocks together into a stream of bytes
    let mut codewordBytes: Vec<u8> = Vec::new();
    for dataBlock in dataBlocks.iter() {
        codewordBytes.clear();
        codewordBytes.extend_from_slice(dataBlock.getCodewords());
        let numDataCodewords = dataBlock.getNumDataCodewords() as usize;

        if !CorrectErrors(&mut codewordBytes, numDataCodewords as u32)? {
            return Err(Exceptions::CHECKSUM);
        }

        // resultIterator = std::copy_n(codewordBytes.begin(), numDataCodewords, resultIterator);
        resultBytes[resultIterator..(resultIterator + numDataCodewords)]
            .copy_from_slice(&codewordBytes[..numDataCodewords]);
        resultIterator += numDataCodewords;
    }

    // Decode the contents of that stream of bytes
    Ok(DecodeBitStream(&resultBytes, version, formatInfo.error_correction_level)?.withIsMirrored(formatInfo.isMirrored))
}

// } // namespace ZXing::QRCode

#[cfg(test)]
mod format_guard_tests {
    use super::*;
    use crate::engine::qrcode::cpp_port::Type;
    use qrcode_core::bits::Bits;
    use qrcode_core::canvas::Canvas;
    use qrcode_core::{Color, EcLevel, Version as CoreVersion};

    fn normal_symbol() -> BitMatrix {
        let version = CoreVersion::Normal(1);
        let mut bits = Bits::new(version);
        bits.push_numeric_data(b"123").unwrap();
        bits.push_terminator(EcLevel::L).unwrap();
        let (data, correction) = qrcode_core::ec::construct_codewords(&bits.into_bytes(), version, EcLevel::L).unwrap();
        let mut canvas = Canvas::new(version, EcLevel::L);
        canvas.draw_all_functional_patterns();
        canvas.draw_data(&data, &correction);
        let colors = canvas.apply_best_mask().into_colors();
        let mut matrix = BitMatrix::new(21, 21).unwrap();
        for y in 0..21 {
            for x in 0..21 {
                matrix.set_bool(x, y, colors[(y * 21 + x) as usize] == Color::Dark);
            }
        }
        matrix
    }

    fn replace_format_copies(matrix: &mut BitMatrix, value: u32) {
        let first = [
            (0, 8),
            (1, 8),
            (2, 8),
            (3, 8),
            (4, 8),
            (5, 8),
            (7, 8),
            (8, 8),
            (8, 7),
            (8, 5),
            (8, 4),
            (8, 3),
            (8, 2),
            (8, 1),
            (8, 0),
        ];
        for (index, (x, y)) in first.into_iter().enumerate() {
            matrix.set_bool(x, y, value & (1 << (14 - index)) != 0);
        }
        // The second copy has the dark module between its seven high bits
        // and eight low bits. ReadFormatInformation removes that extra bit.
        let raw_second = ((value & 0x7f00) << 1) | 0x100 | (value & 0xff);
        let dimension = matrix.height();
        let second =
            ((dimension - 8)..dimension).rev().map(|y| (8, y)).chain(((dimension - 8)..dimension).map(|x| (x, 8)));
        for (index, (x, y)) in second.enumerate() {
            matrix.set_bool(x, y, raw_second & (1 << (15 - index)) != 0);
        }
    }

    #[test]
    fn normal_decode_accepts_valid_bch_and_rejects_invalid_bch_before_rs() {
        let normal = normal_symbol();
        assert!(ReadFormatInformation(&normal).unwrap().isValid());
        let decoded = Decode(&normal).unwrap();
        assert!(decoded.isValid());
        assert_eq!(decoded.content().bytes(), b"123");

        let mut malformed = normal.clone();
        replace_format_copies(&mut malformed, 0x001e);
        let format = ReadFormatInformation(&malformed).unwrap();
        assert_eq!(format.qr_type(), Type::Model2);
        assert_eq!(format.hammingDistance, 4);
        assert_eq!(Decode(&malformed).unwrap_err(), Exceptions::format_with("Invalid format information"));
    }

    #[test]
    fn normal_entry_rejects_legacy_format_and_non_normal_geometry() {
        let mut legacy = normal_symbol();
        replace_format_copies(&mut legacy, 0x2825);
        let format = ReadFormatInformation(&legacy).unwrap();
        assert!(format.isValid());
        assert_eq!(format.qr_type(), Type::Model1);
        assert_eq!(Decode(&legacy).unwrap_err(), Exceptions::format_with("Invalid format information"));
        for (width, height) in [(11, 11), (17, 17), (43, 7), (20, 20), (21, 25), (181, 181)] {
            assert_eq!(
                Decode(&BitMatrix::new(width, height).unwrap()).unwrap_err(),
                Exceptions::format_with("Invalid symbol size")
            );
        }
    }
}

#[cfg(test)]
mod fnc1_tests {
    use super::{DecodeBitStream, ErrorCorrectionLevel, Version};

    fn append_bits(bits: &mut Vec<bool>, value: usize, width: usize) {
        bits.extend((0..width).rev().map(|shift| value & (1 << shift) != 0));
    }

    fn fnc1_stream(second_position: bool, text: &str) -> Vec<u8> {
        fnc1_segments(second_position, &[text])
    }

    fn fnc1_segments(second_position: bool, segments: &[&str]) -> Vec<u8> {
        let mut bits = Vec::new();
        append_bits(&mut bits, if second_position { 0b1001 } else { 0b0101 }, 4);
        if second_position {
            append_bits(&mut bits, 0, 8); // AIM application indicator "00"
        }
        let alphabet = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";
        for text in segments {
            append_bits(&mut bits, 0b0010, 4);
            append_bits(&mut bits, text.len(), 9);
            let values: Vec<_> = text.bytes().map(|ch| alphabet.iter().position(|v| *v == ch).unwrap()).collect();
            let mut pairs = values.chunks_exact(2);
            for pair in &mut pairs {
                append_bits(&mut bits, pair[0] * 45 + pair[1], 11);
            }
            if let [value] = pairs.remainder() {
                append_bits(&mut bits, *value, 6);
            }
        }
        append_bits(&mut bits, 0, 4);
        bits.resize(bits.len().div_ceil(8) * 8, false);
        bits.chunks_exact(8).map(|byte| byte.iter().fold(0u8, |value, bit| (value << 1) | u8::from(*bit))).collect()
    }

    #[test]
    fn fnc1_percent_escapes_preserve_all_bytes_for_both_positions() {
        let version = Version::Model2(1).unwrap();
        for second_position in [false, true] {
            for (input, expected) in [
                ("%%", b"%".as_slice()),
                ("%%%", b"%\x1d".as_slice()),
                ("%%%%", b"%%".as_slice()),
                ("%", b"\x1d".as_slice()),
                ("A%", b"A\x1d".as_slice()),
            ] {
                let stream = fnc1_stream(second_position, input);
                let decoded = DecodeBitStream(&stream, version, ErrorCorrectionLevel::L).unwrap();
                assert!(decoded.isValid(), "{second_position:?} {input:?}: {:?}", decoded.error());
                let mut payload = if second_position { b"00".to_vec() } else { Vec::new() };
                payload.extend_from_slice(expected);
                assert_eq!(decoded.content().bytes(), payload, "{second_position:?} {input:?}");
            }
        }
    }

    #[test]
    fn fnc1_dense_and_mixed_segments_match_literal_escape_replacement() {
        let dense = "%".repeat(320);
        let mixed = "AB%%12%%%Z%34%%%%%".repeat(8);
        // V9-L holds these streams and still uses the 9-bit alpha count tier.
        let version = Version::Model2(9).unwrap();
        for second_position in [false, true] {
            for segments in
                [vec![dense.as_str()], vec![mixed.as_str(), "A1 $:-./ B"], vec!["%", "%", "%%", "%%%", "%%%%", "TAIL%"]]
            {
                let decoded =
                    DecodeBitStream(&fnc1_segments(second_position, &segments), version, ErrorCorrectionLevel::L)
                        .unwrap();
                assert!(decoded.isValid(), "{segments:?}: {:?}", decoded.error());
                let mut expected = if second_position { b"00".to_vec() } else { Vec::new() };
                for segment in &segments {
                    // QR alphanumeric input cannot contain NUL: it can mark an
                    // escaped percent independently of the in-place algorithm.
                    let literal = segment.replace("%%", "\0").replace('%', "\x1d").replace('\0', "%");
                    expected.extend_from_slice(literal.as_bytes());
                }
                assert_eq!(decoded.content().bytes(), expected);
            }
        }
    }
}
