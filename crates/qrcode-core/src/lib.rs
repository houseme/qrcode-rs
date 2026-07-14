//! Zero-dependency QR code encoding core (`no_std` + `alloc`) for `qrcode-rs`.
//!
//! Holds the encoding primitive layer — bit-stream construction, mode
//! optimization, Reed–Solomon error correction, and module-grid canvas drawing
//! — with no external crate dependencies. The `qrcode-rs` facade crate depends
//! on this and re-exports its public surface, so most users depend on
//! `qrcode-rs` directly; `qrcode-core` is for embedders who want only the
//! encoder with no rendering or image dependencies.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

#[cfg(not(feature = "std"))]
extern crate alloc;
