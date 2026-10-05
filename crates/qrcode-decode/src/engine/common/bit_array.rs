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

// package com.google.zxing.common;

// import java.util.Arrays;

use std::{cmp, fmt};

use num::traits::ops::overflowing::OverflowingSub;

use crate::engine::Exceptions;
use crate::engine::common::Result;

type BaseType = super::BitFieldBaseType;
const BASE_BITS: usize = super::BIT_FIELD_BASE_BITS;
const SHIFT_BITS: usize = super::BIT_FIELD_SHIFT_BITS;

/**
 * <p>A simple, fast array of bits, represented compactly by an array of ints internally.</p>
 *
 * @author Sean Owen
 */
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BitArray {
    bits: Vec<BaseType>,
    size: usize,
    reversed: Option<Vec<BaseType>>,
}

impl BitArray {
    pub const fn new() -> Self {
        Self { bits: Vec::new(), size: 0, reversed: None }
    }

    pub fn with_size(size: usize) -> Self {
        Self { bits: makeArray(size), size, reversed: None }
    }

    /// For testing only
    #[cfg(test)]
    pub const fn with_initial_values(bits: Vec<BaseType>, size: usize) -> Self {
        Self { bits, size, reversed: None }
    }

    pub const fn get_size(&self) -> usize {
        self.size
    }

    pub const fn getSizeInBytes(&self) -> usize {
        self.size.div_ceil(8)
    }

    #[inline]
    /**
     * @param i bit to get
     * @return true iff bit i is set
     */
    pub fn get(&self, i: usize) -> bool {
        (self.bits[i / BASE_BITS] & (1 << (i & SHIFT_BITS))) != 0
    }

    pub fn try_get(&self, i: usize) -> Option<bool> {
        if (i / BASE_BITS) >= self.bits.len() { None } else { Some(self.get(i)) }
    }

    /**
     * Sets bit i.
     *
     * @param i bit to set
     */
    pub fn set(&mut self, i: usize) {
        self.reversed = None;
        self.bits[i / BASE_BITS] |= 1 << (i & SHIFT_BITS);
    }

    /**
     * Sets bit i.
     *
     * @param i bit to set
     */
    pub fn unset(&mut self, i: usize) {
        self.reversed = None;
        // self.bits[i / BASE_BITS] |= 0 << (i & SHIFT_BITS);
        self.bits[i / BASE_BITS] &= !(1 << (i & SHIFT_BITS));
    }

    /**
     * Flips bit i.
     *
     * @param i bit to set
     */
    pub fn flip(&mut self, i: usize) {
        self.reversed = None;
        self.bits[i / BASE_BITS] ^= 1 << (i & SHIFT_BITS);
    }

    /**
     * @param from first bit to check
     * @return index of first bit that is set, starting from the given index, or size if none are set
     *  at or beyond this given index
     * @see #getNextUnset(int)
     */
    pub fn getNextSet(&self, from: usize) -> usize {
        if from >= self.size {
            return self.size;
        }
        let mut bitsOffset = from / BASE_BITS;
        let mut currentBits = self.bits[bitsOffset];
        // mask off lesser bits first
        currentBits &= !((1 << (from % BASE_BITS)) - 1);
        while currentBits == 0 {
            bitsOffset += 1;
            if bitsOffset == self.bits.len() {
                return self.size;
            }
            currentBits = self.bits[bitsOffset];
        }
        let result = (bitsOffset * BASE_BITS) + currentBits.trailing_zeros() as usize;
        cmp::min(result, self.size)
    }

    /**
     * @param from index to start looking for unset bit
     * @return index of next unset bit, or {@code size} if none are unset until the end
     * @see #getNextSet(int)
     */
    pub fn getNextUnset(&self, from: usize) -> usize {
        if from >= self.size {
            return self.size;
        }
        let mut bitsOffset = from / BASE_BITS;
        let mut currentBits = !self.bits[bitsOffset];
        // mask off lesser bits first
        currentBits &= !((1 << (from % BASE_BITS)) - 1);
        while currentBits == 0 {
            bitsOffset += 1;
            if bitsOffset == self.bits.len() {
                return self.size;
            }
            currentBits = !self.bits[bitsOffset];
        }
        let result = (bitsOffset * BASE_BITS) + currentBits.trailing_zeros() as usize;
        cmp::min(result, self.size)
    }

    /**
     * Sets a block of 32 bits, starting at bit i.
     *
     * @param i first bit to set
     * @param newBits the new value of the next 32 bits. Note again that the least-significant bit
     * corresponds to bit i, the next-least-significant to i+1, and so on.
     */
    pub fn setBulk(&mut self, i: usize, newBits: BaseType) {
        self.reversed = None;
        let bits = if i % BASE_BITS != 0 { newBits << (i % BASE_BITS) } else { newBits };
        self.bits[i / BASE_BITS] = bits;
    }

    /**
     * Clears all bits (sets to false).
     */
    #[inline]
    pub fn clear(&mut self) {
        self.reversed = None;
        self.bits.fill(0);
    }

    /**
     * Efficient method to check if a range of bits is set, or not set.
     *
     * @param start start of range, inclusive.
     * @param end end of range, exclusive
     * @param value if true, checks that bits in range are set, otherwise checks that they are not set
     * @return true iff all bits are set or not set in range, according to value argument
     * @throws IllegalArgumentException if end is less than start or the range is not contained in the array
     */
    pub fn isRange(&self, start: usize, end: usize, value: bool) -> Result<bool> {
        let mut end = end;
        if end < start || end > self.size {
            return Err(Exceptions::ILLEGAL_ARGUMENT);
        }
        if end == start {
            return Ok(true); // empty range matches
        }
        end -= 1; // will be easier to treat this as the last actually set bit -- inclusive
        let firstInt = start / BASE_BITS;
        let lastInt = end / BASE_BITS;
        for i in firstInt..=lastInt {
            //for (int i = firstInt; i <= lastInt; i++) {
            let firstBit = if i > firstInt { 0 } else { start & SHIFT_BITS };
            let lastBit = if i < lastInt { SHIFT_BITS } else { end & SHIFT_BITS };
            // Ones from firstBit to lastBit, inclusive
            let (mask, _): (BaseType, _) = (2 << lastBit).overflowing_sub(&(1 << firstBit));
            // let mask: u128 = (2 << lastBit) - (1 << firstBit);

            // Return false if we're looking for 1s and the masked bits[i] isn't all 1s (that is,
            // equals the mask, or we're looking for 0s and the masked portion is not all 0s
            if (self.bits[i] & mask as BaseType) != (if value { mask as BaseType } else { 0 }) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /**
     * Appends the least-significant bits, from value, in order from most-significant to
     * least-significant. For example, appending 6 bits from 0x000001E will append the bits
     * 0, 1, 1, 1, 1, 0 in that order.
     *
     * @param value {@code int} containing bits to append
     * @param numBits bits from value to append
     */
    pub fn xor(&mut self, other: &BitArray) -> Result<()> {
        self.reversed = None;
        if self.size != other.size {
            return Err(Exceptions::illegal_argument_with("Sizes don't match"));
        }
        for (lhs, rhs) in self.bits.iter_mut().zip(other.bits.iter()) {
            //for (int i = 0; i < bits.length; i++) {
            // The last int could be incomplete (i.e. not have 32 bits in
            // it) but there is no problem since 0 XOR 0 == 0.
            *lhs ^= rhs;
        }
        Ok(())
    }

    /**
     * @return underlying array of ints. The first element holds the first 32 bits, and the least
     *         significant bit is bit 0.
     */
    pub fn getBitArray(&self) -> &[BaseType] {
        &self.bits
    }

    /**
     * Reverses all bits in the array.
     */
    pub fn reverse(&mut self) {
        if self.size == 0 {
            return;
        }
        // check if we've already done the rever operation once
        if self.reversed.is_some() {
            self.bits = self.reversed.replace(self.bits.clone()).unwrap();
            return;
        }

        // first we save off the current version as the reversed version
        self.reversed = Some(self.bits.clone());

        // reverse all int's first
        let len = (self.size - 1) / BASE_BITS;
        let oldBitsLen = len + 1;

        self.bits[..oldBitsLen].reverse();
        self.bits[..oldBitsLen].iter_mut().for_each(|bit_store| *bit_store = bit_store.reverse_bits());
        self.bits[oldBitsLen..].fill(0);

        // now correct the int's if the bit size isn't a multiple of 32
        if self.size != oldBitsLen * BASE_BITS {
            let leftOffset = oldBitsLen * BASE_BITS - self.size;
            let mut currentInt = self.bits[0] >> leftOffset;
            for i in 1..oldBitsLen {
                //for (int i = 1; i < oldBitsLen; i++) {
                let nextInt = self.bits[i];
                currentInt |= nextInt << (BASE_BITS - leftOffset);
                self.bits[i - 1] = currentInt;
                currentInt = nextInt >> leftOffset;
            }
            self.bits[oldBitsLen - 1] = currentInt;
        }
    }
}

impl fmt::Display for BitArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut _str = String::with_capacity(self.size + (self.size / 8) + 1);
        for i in 0..self.size {
            //for (int i = 0; i < size; i++) {
            if (i & 0x07) == 0 {
                _str.push(' ');
            }
            _str.push_str(if self.get(i) { "X" } else { "." });
        }
        write!(f, "{_str}")
    }
}

impl Default for BitArray {
    fn default() -> Self {
        Self::new()
    }
}

impl From<BitArray> for Vec<bool> {
    fn from(value: BitArray) -> Self {
        Self::from(&value)
    }
}

impl From<&BitArray> for Vec<bool> {
    fn from(value: &BitArray) -> Self {
        let mut array = vec![false; value.size];

        for (pixel, element) in array.iter_mut().enumerate().take(value.size) {
            *element = value.get(pixel);
        }

        array
    }
}

fn makeArray(size: usize) -> Vec<BaseType> {
    vec![0; size.div_ceil(BASE_BITS)]
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    #[test]
    fn empty_reverse_is_a_noop_and_nonempty_reverse_roundtrips() {
        let mut empty = BitArray::new();
        empty.reverse();
        empty.reverse();
        assert_eq!(empty.get_size(), 0);
        assert!(empty.getBitArray().is_empty());
        let mut row = BitArray::with_size(BASE_BITS + 3);
        row.set(0);
        row.set(BASE_BITS + 1);
        row.reverse();
        assert!(row.get(1));
        assert!(row.get(BASE_BITS + 2));
        row.reverse();
        assert!(row.get(0));
        assert!(row.get(BASE_BITS + 1));
    }
    #[test]
    fn bulk_shift_uses_offset_within_the_word() {
        let mut row = BitArray::with_size(BASE_BITS * 2);
        row.setBulk(BASE_BITS, 3);
        assert!(row.get(BASE_BITS));
        assert!(row.get(BASE_BITS + 1));
        row.setBulk(BASE_BITS + 1, 1);
        assert!(!row.get(BASE_BITS));
        assert!(row.get(BASE_BITS + 1));
    }
}
