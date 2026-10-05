// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
/*
* Copyright 2016 Nu-book Inc.
* Copyright 2016 ZXing authors
*/
// SPDX-License-Identifier: Apache-2.0

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum Type {
    Model1,
    Model2,
    Micro,
    RectMicro,
}

impl Type {
    pub const fn const_eq(a: Type, b: Type) -> bool {
        let (a, b) = (a as u8, b as u8);

        a == b
    }
}
