// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
/*
 * Copyright 2009 ZXing authors
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

// import com.google.zxing.Binarizer;
// import com.google.zxing.LuminanceSource;
// import com.google.zxing.NotFoundException;

use once_cell::sync::OnceCell;

use crate::engine::common::Result;
use crate::engine::{Binarizer, Exceptions, LuminanceSource};

use super::BitMatrix;

const LUMINANCE_BITS: usize = 5;
const LUMINANCE_SHIFT: usize = 8 - LUMINANCE_BITS;
const LUMINANCE_BUCKETS: usize = 1 << LUMINANCE_BITS;

/// Whole-image global histogram thresholding for small QR frames.
pub struct GlobalHistogramBinarizer<LS: LuminanceSource> {
    source: LS,
    black_matrix: OnceCell<BitMatrix>,
}

impl<LS: LuminanceSource> Binarizer for GlobalHistogramBinarizer<LS> {
    type Source = LS;

    fn get_luminance_source(&self) -> &Self::Source {
        &self.source
    }

    fn get_black_matrix(&self) -> Result<&BitMatrix> {
        self.black_matrix.get_or_try_init(|| Self::build_black_matrix(&self.source))
    }
}

impl<LS: LuminanceSource> GlobalHistogramBinarizer<LS> {
    pub fn new(source: LS) -> Self {
        Self { source, black_matrix: OnceCell::new() }
    }

    fn build_black_matrix(source: &LS) -> Result<BitMatrix> {
        // let source = source.getLuminanceSource();
        let width = source.get_width();
        let height = source.get_height();
        let mut matrix = BitMatrix::new(width as u32, height as u32)?;

        // Quickly calculates the histogram by sampling four rows from the image. This proved to be
        // more robust on the blackbox tests than sampling a diagonal as we used to do.
        // self.initArrays(width);
        let mut localBuckets = [0; LUMINANCE_BUCKETS]; //self.buckets.clone();
        for y in 1..5 {
            // for (int y = 1; y < 5; y++) {
            let row = height * y / 5;
            let localLuminances =
                source.get_row(row).ok_or(Exceptions::index_out_of_bounds_with("row out of bounds"))?;
            let right = (width * 4) / 5;
            for pixel in &localLuminances[(width / 5)..right] {
                // for x in (width / 5)..right {
                // let pixel = localLuminances[x];
                localBuckets[(pixel >> LUMINANCE_SHIFT) as usize] += 1;
            }
        }
        let blackPoint = Self::estimateBlackPoint(&localBuckets)?;

        // We delay reading the entire image luminance until the black point estimation succeeds.
        // Although we end up reading four rows twice, it is consistent with our motto of
        // "fail quickly" which is necessary for continuous scanning.
        let localLuminances = source.get_matrix();
        for y in 0..height {
            let offset = y * width;
            for x in 0..width {
                let pixel = localLuminances[offset + x];
                if (pixel as u32) < blackPoint {
                    matrix.set(x as u32, y as u32);
                }
            }
        }

        Ok(matrix)
    }

    fn estimateBlackPoint<const BUCKET_COUNT: usize>(buckets: &[u32; BUCKET_COUNT]) -> Result<u32> {
        // Find the tallest peak in the histogram.
        let mut maxBucketCount = 0;
        let mut firstPeak = 0;
        let mut firstPeakSize = 0;
        for (x, &bucket) in buckets.iter().enumerate() {
            if bucket > firstPeakSize {
                firstPeak = x;
                firstPeakSize = bucket;
            }
            if bucket > maxBucketCount {
                maxBucketCount = bucket;
            }
        }

        // Find the second-tallest peak which is somewhat far from the tallest peak.
        let mut secondPeak = 0;
        let mut secondPeakScore = 0;
        for (x, bucket) in buckets.iter().enumerate() {
            let distanceToBiggest = (x as i32 - firstPeak as i32).unsigned_abs();
            // Encourage more distant second peaks by multiplying by square of distance.
            let score = *bucket * distanceToBiggest * distanceToBiggest;
            if score > secondPeakScore {
                secondPeak = x;
                secondPeakScore = score;
            }
        }

        // Make sure firstPeak corresponds to the black peak.
        if firstPeak > secondPeak {
            std::mem::swap(&mut firstPeak, &mut secondPeak);
        }

        // If there is too little contrast in the image to pick a meaningful black point, throw rather
        // than waste time trying to decode the image, and risk false positives.
        if secondPeak - firstPeak <= BUCKET_COUNT / 16 {
            return Err(Exceptions::not_found_with("secondPeak - firstPeak <= numBuckets / 16 "));
        }

        // Find a valley between them that is low and closer to the white peak.
        let mut bestValley = secondPeak as isize - 1;
        let mut bestValleyScore = -1;

        let mut x = secondPeak as isize;
        while x > firstPeak as isize {
            let fromFirst = x - firstPeak as isize;
            let score =
                fromFirst * fromFirst * (secondPeak as isize - x) * (maxBucketCount - buckets[x as usize]) as isize;
            if score as i32 > bestValleyScore {
                bestValley = x;
                bestValleyScore = score as i32;
            }
            x -= 1;
        }

        Ok((bestValley as u32) << LUMINANCE_SHIFT)
    }
}
