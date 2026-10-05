// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
/*
 * Copyright 2007 ZXing authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use super::ErrorCorrectionLevel;

pub const FORMAT_INFO_MASK_QR: u32 = 0x5412;
pub const FORMAT_INFO_MASK_MODEL2: u32 = FORMAT_INFO_MASK_QR;

/**
 * <p>Encapsulates a QR Code's format information, including the data mask used and
 * error correction level.</p>
 *
 * @author Sean Owen
 * @see DataMask
 * @see ErrorCorrectionLevel
 */
#[derive(Hash, Eq, PartialEq, Debug)]
pub struct FormatInformation {
    pub hammingDistance: u32,
    pub error_correction_level: ErrorCorrectionLevel,
    pub data_mask: u8,
    pub microVersion: u32,
    pub isMirrored: bool,

    pub mask: u32,     // = 0
    pub data: u32,     // = 255
    pub bitsIndex: u8, // = 255;
}

impl Default for FormatInformation {
    fn default() -> Self {
        Self {
            hammingDistance: 255,
            error_correction_level: ErrorCorrectionLevel::Invalid,
            data_mask: Default::default(),
            microVersion: 0,
            isMirrored: false,
            mask: 0,
            data: 255,
            bitsIndex: 255,
        }
    }
}

impl FormatInformation {
    pub fn numBitsDiffering(a: u32, b: u32) -> u32 {
        (a ^ b).count_ones()
    }
}
