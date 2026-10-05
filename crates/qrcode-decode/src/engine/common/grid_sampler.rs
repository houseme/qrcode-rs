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

// import com.google.zxing.NotFoundException;

use super::{BitMatrix, PerspectiveTransform};
use crate::engine::{Point, common::Result};
/// Samples the bounded module regions supplied by the QR detector.
pub trait GridSampler {
    fn sample_grid(
        &self,
        image: &BitMatrix,
        dimensionX: u32,
        dimensionY: u32,
        controls: &[SamplerControl],
    ) -> Result<(BitMatrix, [Point; 4])>;
}
pub struct SamplerControl {
    pub p0: Point,
    pub p1: Point,
    pub transform: PerspectiveTransform,
}
