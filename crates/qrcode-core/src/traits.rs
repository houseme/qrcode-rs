//! Core extension traits for encoding, rendering, and module-grid storage.
//!
//! These traits are intentionally small so the facade crate and future split
//! crates can share the same abstraction layer without pulling renderer or image
//! dependencies into `qrcode-core`.

use crate::types::Color;

/// Encodes raw input bytes into a concrete output type.
///
/// Implementations can produce a full QR code, an intermediate bit stream, or a
/// third-party symbol type. The input is borrowed to keep the trait usable in
/// `no_std + alloc` environments without requiring an owned buffer.
pub trait Encoder {
    /// The successful encoding output.
    type Output;

    /// The encoding error type.
    type Error;

    /// Encodes `input`.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] when the implementation cannot encode the input.
    fn encode(&self, input: &[u8]) -> Result<Self::Output, Self::Error>;
}

/// Renders a module-grid source into a concrete output type.
///
/// The `Code` parameter is usually a type implementing [`ModuleStorage`], such
/// as the facade crate's `QrCode`, but may also be a third-party borrowed view.
pub trait Renderer<Code: ModuleStorage + ?Sized> {
    /// The rendered output.
    type Output;

    /// The rendering error type.
    type Error;

    /// Renders `code`.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] when rendering fails.
    fn render(&self, code: &Code) -> Result<Self::Output, Self::Error>;
}

/// Read/write access to a QR module grid.
///
/// Coordinates are zero-based and exclude any quiet zone. Implementations should
/// store modules in row-major order when exposing [`modules`](Self::modules).
pub trait ModuleStorage {
    /// Returns the color at `(x, y)`.
    ///
    /// # Panics
    ///
    /// Implementations may panic when `x >= width()` or `y >= height()`.
    fn get(&self, x: usize, y: usize) -> Color;

    /// Sets the color at `(x, y)`.
    ///
    /// # Panics
    ///
    /// Implementations may panic when `x >= width()` or `y >= height()`.
    fn set(&mut self, x: usize, y: usize, color: Color);

    /// Returns the number of modules per row.
    fn width(&self) -> usize;

    /// Returns the number of module rows.
    fn height(&self) -> usize;

    /// Returns all modules in row-major order.
    fn modules(&self) -> &[Color];

    /// Returns whether this storage has no modules.
    fn is_empty(&self) -> bool {
        self.width() == 0 || self.height() == 0
    }
}
