//! Core extension traits for encoding, rendering, and module-grid storage.
//!
//! These traits are intentionally small so the facade crate and future split
//! crates can share the same abstraction layer without pulling renderer or image
//! dependencies into `qrcode-core`.

use crate::types::Color;

/// Borrowed row-major view over a read-only QR module grid.
///
/// `ModuleView` is useful for adapters and tests that already have a module
/// slice and need to pass it through the shared [`ModuleSource`] abstraction
/// without allocating or implementing a bespoke wrapper type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModuleView<'a> {
    modules: &'a [Color],
    width: usize,
}

impl<'a> ModuleView<'a> {
    /// Creates a square module view from a row-major module slice.
    ///
    /// Returns `None` when `width == 0` or `modules.len() != width * width`.
    #[must_use]
    pub const fn new(modules: &'a [Color], width: usize) -> Option<Self> {
        if width == 0 || modules.len() != width * width {
            return None;
        }
        Some(Self { modules, width })
    }
}

impl ModuleSource for ModuleView<'_> {
    fn get(&self, x: usize, y: usize) -> Color {
        self.modules[y * self.width + x]
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.width
    }

    fn modules(&self) -> &[Color] {
        self.modules
    }
}

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
/// The `Code` parameter is usually a type implementing [`ModuleSource`], such
/// as the facade crate's `QrCode`, but may also be a third-party borrowed view.
pub trait Renderer<Code: ModuleSource + ?Sized> {
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

/// Read-only access to a QR module grid.
///
/// Coordinates are zero-based and exclude any quiet zone. Implementations should
/// store modules in row-major order when exposing [`modules`](Self::modules).
pub trait ModuleSource {
    /// Returns the color at `(x, y)`.
    ///
    /// # Panics
    ///
    /// Implementations may panic when `x >= width()` or `y >= height()`.
    fn get(&self, x: usize, y: usize) -> Color;

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

/// Read/write access to a QR module grid.
///
/// Rendering and inspection APIs should prefer [`ModuleSource`] when they only
/// need read access. This trait remains available for in-place mutation and
/// testing utilities.
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

impl<T: ModuleStorage + ?Sized> ModuleSource for T {
    fn get(&self, x: usize, y: usize) -> Color {
        ModuleStorage::get(self, x, y)
    }

    fn width(&self) -> usize {
        ModuleStorage::width(self)
    }

    fn height(&self) -> usize {
        ModuleStorage::height(self)
    }

    fn modules(&self) -> &[Color] {
        ModuleStorage::modules(self)
    }

    fn is_empty(&self) -> bool {
        ModuleStorage::is_empty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{ModuleSource, ModuleView};
    use crate::Color;

    #[test]
    fn module_view_reads_row_major_modules() {
        let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
        let view = ModuleView::new(&modules, 2).unwrap();

        assert_eq!(view.width(), 2);
        assert_eq!(view.height(), 2);
        assert_eq!(view.modules(), modules);
        assert_eq!(view.get(0, 0), Color::Dark);
        assert_eq!(view.get(1, 1), Color::Dark);
    }

    #[test]
    fn module_view_rejects_non_square_input() {
        let modules = [Color::Dark, Color::Light, Color::Dark];

        assert!(ModuleView::new(&modules, 2).is_none());
        assert!(ModuleView::new(&modules, 0).is_none());
    }
}
