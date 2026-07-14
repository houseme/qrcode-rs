//! Zero-dependency QR code encoding core (`no_std` + `alloc`) for `qrcode-rs`.
//!
//! Holds the encoding primitive layer — bit-stream construction ([`bits::Bits`]),
//! mode optimization ([`optimize`]), Reed–Solomon error correction ([`ec`]),
//! and module-grid canvas drawing ([`canvas`]) — plus the shared types
//! ([`types`]) and checked-cast helpers ([`cast`]), with no external crate
//! dependencies. The `qrcode-rs` facade crate depends on this and re-exports
//! its public surface; embedders wanting only the encoder (no rendering or
//! image dependencies) can depend on `qrcode-core` directly.

#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]
#![deny(missing_docs)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod bits;
pub mod canvas;
pub mod cast;
pub mod ec;
pub mod optimize;
pub mod traits;
pub mod types;

pub use cast::{As, Truncate};
pub use traits::{Builder, Encoder, ModuleSource, ModuleStorage, ModuleView, QrSymbol, Renderer};
pub use types::{Color, EcLevel, Mode, QrError, QrResult, Version};
