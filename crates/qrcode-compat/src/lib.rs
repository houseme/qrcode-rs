//! 1.x compatibility facade for qrcode-rs 2.0 migrations.
//!
//! This crate re-exports the `qrcode-rs` facade with its `compat-1x` feature
//! enabled. It is intended for applications that want a dedicated dependency
//! name for the transitional API while gradually moving call sites to the 2.0
//! builder, module-view, streaming, and split-crate APIs.
//!
//! ```rust
//! use qrcode_compat::{EcLevel, QrCode, Version};
//!
//! let code = QrCode::with_version(b"01234567", Version::Normal(1), EcLevel::M).unwrap();
//! let rendered = code.render::<char>().quiet_zone(false).build();
//! assert_eq!(rendered.lines().count(), code.width());
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

pub use qrcode_rs::*;
