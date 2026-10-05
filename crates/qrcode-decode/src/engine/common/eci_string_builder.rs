// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
/*
 * Copyright 2022 ZXing authors
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

// package com.google.zxing.common;

// import com.google.zxing.FormatException;

// import java.nio.charset.Charset;
// import java.nio.charset.StandardCharsets;

use std::collections::HashSet;

use super::{CharacterSet, Eci};

/**
 * Raw byte content and ECI position bookkeeping for QR decoding
 *
 * @author Alex Geller
 */
#[derive(Default, PartialEq, Eq, Debug, Clone)]
pub struct ECIStringBuilder {
    pub has_eci: bool,
    bytes: Vec<u8>,
    pub(crate) eci_positions: Vec<(Eci, usize, usize)>, // (Eci, start, end)
    pub symbology: SymbologyIdentifier,
    eci_list: HashSet<Eci>,
}

impl ECIStringBuilder {
    #[cfg(test)]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Transfers the original payload allocation to the caller.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /**
     * Appends {@code value} as a byte value
     *
     * @param value byte to append
     */
    pub fn append_byte(&mut self, value: u8) {
        self.bytes.push(value)
    }

    /**
     * Appends the characters in {@code value} as bytes values
     *
     * @param value string to append
     */
    pub fn append_string(&mut self, value: &str) {
        if !value.is_ascii() {
            self.append_eci(Eci::UTF8);
        }
        self.bytes.extend_from_slice(value.as_bytes());
    }

    /**
     * Appends ECI value to output.
     *
     * @param value ECI value to append, as an int
     * @throws FormatException on invalid ECI value
     */
    pub fn append_eci(&mut self, eci: Eci) {
        if !self.has_eci && eci != Eci::ISO8859_1 {
            self.has_eci = true;
        }

        if self.has_eci {
            if let Some(last) = self.eci_positions.last_mut() {
                last.2 = self.bytes.len()
            }

            self.eci_positions.push((eci, self.bytes.len(), 0));

            self.eci_list.insert(eci);

            if self.eci_list.len() == 1 && (self.eci_list.contains(&Eci::Unknown)) {
                self.has_eci = false;
                self.eci_positions.clear();
            }
        }
    }

    /// Change the current encoding characterset, finding an eci to do so
    pub fn switch_encoding(&mut self, charset: CharacterSet, is_eci: bool) {
        //self.append_eci(Eci::from(charset))
        if is_eci && !self.has_eci {
            self.eci_positions.clear();
        }
        if is_eci || !self.has_eci
        //{self.eci_positions.push_back({eci, Size(bytes)});}
        {
            // self.append_eci(Eci::from(charset))
            if let Some(last) = self.eci_positions.last_mut() {
                last.2 = self.bytes.len()
            }

            self.eci_positions.push((Eci::from(charset), self.bytes.len(), 0));
        }

        self.has_eci |= is_eci;
    }

    /// Reserve an additional number of bytes for storage
    pub fn reserve(&mut self, additional: usize) {
        self.bytes.reserve(additional);
    }

    /**
     * @return true iff nothing has been appended
     */
    pub fn is_empty(&mut self) -> bool {
        self.bytes.is_empty()
    }
}

impl std::ops::AddAssign<u8> for ECIStringBuilder {
    fn add_assign(&mut self, rhs: u8) {
        self.append_byte(rhs)
    }
}

impl std::ops::AddAssign<String> for ECIStringBuilder {
    fn add_assign(&mut self, rhs: String) {
        self.append_string(&rhs)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AIFlag {
    None,
    GS1,
    AIM,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct SymbologyIdentifier {
    //char code = 0, modifier = 0, eciModifierOffset = 0;
    pub code: u8,
    pub modifier: u8,
    pub eciModifierOffset: u8,
    pub aiFlag: AIFlag,
    // AIFlag aiFlag = AIFlag::None;

    // std::string toString(bool hasECI = false) const
    // {
    // 	return code ? ']' + std::string(1, code) + static_cast<char>(modifier + eciModifierOffset * hasECI) : std::string();
    // }
}

impl Default for SymbologyIdentifier {
    fn default() -> Self {
        Self { code: 0, modifier: 0, eciModifierOffset: 0, aiFlag: AIFlag::None }
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::{CharacterSet, ECIStringBuilder};

    #[test]
    fn into_bytes_preserves_payload_pointer_and_capacity() {
        for payload in [b"".as_slice(), b"\0\xffA\x1d".as_slice()] {
            let mut content = ECIStringBuilder::default();
            content.reserve(128);
            content.switch_encoding(CharacterSet::UTF8, true);
            for byte in payload {
                content.append_byte(*byte);
            }
            let pointer = content.bytes.as_ptr();
            let capacity = content.bytes.capacity();
            let bytes = content.into_bytes();
            assert_eq!(bytes, payload);
            assert_eq!(bytes.as_ptr(), pointer);
            assert_eq!(bytes.capacity(), capacity);
        }
    }
}
