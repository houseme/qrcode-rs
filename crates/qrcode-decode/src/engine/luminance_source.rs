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

use std::borrow::Cow;

/// Immutable row-major luminance access used by whole-image QR binarization.
pub trait LuminanceSource {
    fn get_row(&self, y: usize) -> Option<Cow<'_, [u8]>>;
    fn get_matrix(&self) -> Cow<'_, [u8]>;
    fn get_width(&self) -> usize;
    fn get_height(&self) -> usize;
}
