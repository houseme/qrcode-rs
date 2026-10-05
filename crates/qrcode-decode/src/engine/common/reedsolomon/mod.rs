/*
 * Copyright 2007 ZXing authors
 * Licensed under the Apache License, Version 2.0.
 * http://www.apache.org/licenses/LICENSE-2.0
 */
// SPDX-License-Identifier: Apache-2.0

pub type GenericGFRef = &'static GenericGF;
const QR_CODE_FIELD_256: GenericGF = GenericGF::new(0x011D, 256, 0);
pub enum PredefinedGenericGF {
    QrCodeField256,
}
impl From<PredefinedGenericGF> for GenericGFRef {
    fn from(value: PredefinedGenericGF) -> Self {
        match value {
            PredefinedGenericGF::QrCodeField256 => &QR_CODE_FIELD_256,
        }
    }
}
mod generic_gf;
pub use generic_gf::*;
mod generic_gf_poly;
pub use generic_gf_poly::*;
mod reedsolomon_decoder;
pub use reedsolomon_decoder::*;
