// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
use crate::engine::common::{BitMatrix, DetectorRXingResult};

pub struct QRCodeDetectorResult {
    bit_source: BitMatrix,
}

impl QRCodeDetectorResult {
    pub fn new(bit_source: BitMatrix) -> Self {
        Self { bit_source }
    }
}

impl DetectorRXingResult for QRCodeDetectorResult {
    fn getBits(&self) -> &BitMatrix {
        &self.bit_source
    }
}
