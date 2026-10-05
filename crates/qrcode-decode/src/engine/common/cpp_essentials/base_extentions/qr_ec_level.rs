// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
/*
* Copyright 2016 Nu-book Inc.
* Copyright 2016 ZXing authors
*/
// SPDX-License-Identifier: Apache-2.0

use crate::engine::qrcode::common::ErrorCorrectionLevel;

impl ErrorCorrectionLevel {
    pub fn ECLevelFromBitsSigned(bits: i8, isMicro: bool) -> Self {
        if isMicro {
            let LEVEL_FOR_BITS: [ErrorCorrectionLevel; 8] = [
                ErrorCorrectionLevel::L,
                ErrorCorrectionLevel::L,
                ErrorCorrectionLevel::M,
                ErrorCorrectionLevel::L,
                ErrorCorrectionLevel::M,
                ErrorCorrectionLevel::L,
                ErrorCorrectionLevel::M,
                ErrorCorrectionLevel::Q,
            ];
            return LEVEL_FOR_BITS[bits as usize & 0x07];
        }
        let LEVEL_FOR_BITS: [ErrorCorrectionLevel; 4] =
            [ErrorCorrectionLevel::M, ErrorCorrectionLevel::L, ErrorCorrectionLevel::H, ErrorCorrectionLevel::Q];
        LEVEL_FOR_BITS[bits as usize & 0x3]
    }

    pub fn ECLevelFromBits(bits: u8, isMicro: bool) -> Self {
        Self::ECLevelFromBitsSigned(bits as i8, isMicro)
    }
}
