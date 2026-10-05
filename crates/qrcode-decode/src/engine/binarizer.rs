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

use crate::engine::{
    LuminanceSource,
    common::{BitMatrix, Result},
};

/// Whole-image thresholding contract used by the private QR scanner.
pub trait Binarizer {
    type Source: LuminanceSource;

    fn get_luminance_source(&self) -> &Self::Source;
    fn get_black_matrix(&self) -> Result<&BitMatrix>;
}
