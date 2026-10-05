//! Shared primitives used by the private QR scanner.
pub type Result<T, E = crate::engine::Exceptions> = std::result::Result<T, E>;
pub type BitFieldBaseType = usize;
pub const BIT_FIELD_BASE_BITS: usize = BitFieldBaseType::BITS as usize;
pub const BIT_FIELD_SHIFT_BITS: usize = BIT_FIELD_BASE_BITS - 1;

pub trait DetectorRXingResult {
    fn getBits(&self) -> &BitMatrix;
}

mod bit_array;
pub use bit_array::*;
mod bit_matrix;
pub use bit_matrix::*;
mod bit_source;
pub use bit_source::*;
mod character_set;
pub use character_set::*;
mod eci;
pub use eci::*;
mod eci_string_builder;
pub use eci_string_builder::*;
mod global_histogram_binarizer;
pub use global_histogram_binarizer::*;
mod hybrid_binarizer;
pub use hybrid_binarizer::*;
mod perspective_transform;
pub use perspective_transform::*;
mod grid_sampler;
pub use grid_sampler::*;
mod default_grid_sampler;
pub use default_grid_sampler::*;
mod quad;
pub use quad::*;
pub mod cpp_essentials;
pub mod reedsolomon;
