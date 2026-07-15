//! Core extension traits for encoding, rendering, and module-grid storage.
//!
//! These traits are intentionally small so the facade crate and future split
//! crates can share the same abstraction layer without pulling renderer or image
//! dependencies into `qrcode-core`.

use crate::types::{Color, EcLevel, Version};

/// Borrowed row-major view over a read-only QR module grid.
///
/// `ModuleView` is useful for adapters and tests that already have a module
/// slice and need to pass it through the shared [`ModuleSource`] abstraction
/// without allocating or implementing a bespoke wrapper type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModuleView<'a> {
    modules: &'a [Color],
    width: usize,
    height: usize,
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
        Some(Self { modules, width, height: width })
    }

    /// Creates a rectangular module view from a row-major module slice.
    ///
    /// This keeps the same zero-copy storage contract as [`ModuleView::new`],
    /// but allows callers to borrow a contiguous range of full rows.
    ///
    /// Returns `None` when either dimension is zero or when
    /// `modules.len() != width * height`.
    #[must_use]
    pub const fn new_rect(modules: &'a [Color], width: usize, height: usize) -> Option<Self> {
        if width == 0 || height == 0 || modules.len() != width * height {
            return None;
        }
        Some(Self { modules, width, height })
    }

    /// Returns a zero-copy view over a contiguous range of full rows.
    ///
    /// The returned view keeps the same width and borrows a sub-slice of the
    /// original row-major module slice.
    #[must_use]
    pub fn row_range(&self, start: usize, end: usize) -> Option<Self> {
        if start >= end || end > self.height {
            return None;
        }
        let height = end - start;
        let start = start * self.width;
        let end = end * self.width;
        Some(Self { modules: &self.modules[start..end], width: self.width, height })
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
        self.height
    }

    fn modules(&self) -> &[Color] {
        self.modules
    }
}

/// Borrowed QR symbol with module-grid data and QR metadata.
///
/// This is the zero-copy counterpart to an owned QR code. It carries the same
/// metadata expected by [`QrSymbol`] while borrowing the row-major module slice
/// from an existing symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QrCodeRef<'a> {
    modules: &'a [Color],
    version: Version,
    ec_level: EcLevel,
    width: usize,
}

impl<'a> QrCodeRef<'a> {
    /// Creates a borrowed QR symbol from row-major module data.
    ///
    /// Returns `None` when `width == 0` or `modules.len() != width * width`.
    #[must_use]
    pub const fn new(modules: &'a [Color], width: usize, version: Version, ec_level: EcLevel) -> Option<Self> {
        if width == 0 || modules.len() != width * width {
            return None;
        }
        Some(Self { modules, version, ec_level, width })
    }

    /// Returns a read-only module view over the same borrowed modules.
    #[must_use]
    pub const fn module_view(self) -> ModuleView<'a> {
        ModuleView { modules: self.modules, width: self.width, height: self.width }
    }
}

impl ModuleSource for QrCodeRef<'_> {
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

impl QrSymbol for QrCodeRef<'_> {
    fn version(&self) -> Version {
        self.version
    }

    fn error_correction_level(&self) -> EcLevel {
        self.ec_level
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

/// Builds a configured value into its final output.
///
/// This small trait gives encoders, renderers, and future plugin factories a
/// shared builder contract without forcing them into one concrete builder type.
pub trait Builder {
    /// The successfully built value.
    type Output;

    /// The build error type.
    type Error;

    /// Consumes the builder and returns its output.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] when the configured value cannot be built.
    fn build(self) -> Result<Self::Output, Self::Error>;
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

    /// Returns row `y` as a contiguous row-major slice.
    ///
    /// # Panics
    ///
    /// Panics when `y >= height()` or when [`modules`](Self::modules) does not
    /// contain a complete row-major grid.
    fn row(&self, y: usize) -> &[Color] {
        let width = self.width();
        let start = y * width;
        &self.modules()[start..start + width]
    }

    /// Returns whether this storage has no modules.
    fn is_empty(&self) -> bool {
        self.width() == 0 || self.height() == 0
    }
}

/// Read-only QR symbol metadata plus module-grid access.
///
/// `QrSymbol` is the higher-level counterpart to [`ModuleSource`]: renderers
/// and adapters can use it when they need both the module grid and QR-specific
/// metadata such as [`Version`] and [`EcLevel`].
pub trait QrSymbol: ModuleSource {
    /// Returns the encoded QR or Micro QR version.
    fn version(&self) -> Version;

    /// Returns the encoded error-correction level.
    fn error_correction_level(&self) -> EcLevel;

    /// Returns the default quiet-zone width in modules for this symbol.
    ///
    /// Normal QR symbols use four modules. Micro QR symbols use two modules.
    fn quiet_zone(&self) -> u32 {
        if self.version().is_micro() { 2 } else { 4 }
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
    use super::{Builder, Encoder, ModuleSource, ModuleStorage, ModuleView, QrCodeRef, QrSymbol, Renderer};
    use crate::{Color, EcLevel, Version};
    use core::convert::Infallible;

    struct DummySymbol {
        version: Version,
        modules: [Color; 1],
    }

    impl ModuleSource for DummySymbol {
        fn get(&self, _x: usize, _y: usize) -> Color {
            self.modules[0]
        }

        fn width(&self) -> usize {
            1
        }

        fn height(&self) -> usize {
            1
        }

        fn modules(&self) -> &[Color] {
            &self.modules
        }
    }

    impl QrSymbol for DummySymbol {
        fn version(&self) -> Version {
            self.version
        }

        fn error_correction_level(&self) -> EcLevel {
            EcLevel::M
        }
    }

    struct DummyBuilder {
        value: u8,
    }

    impl Builder for DummyBuilder {
        type Output = u8;
        type Error = ();

        fn build(self) -> Result<Self::Output, Self::Error> {
            Ok(self.value)
        }
    }

    struct DummyEncoder;

    impl Encoder for DummyEncoder {
        type Output = usize;
        type Error = Infallible;

        fn encode(&self, input: &[u8]) -> Result<Self::Output, Self::Error> {
            Ok(input.len())
        }
    }

    struct DummyRenderer {
        dark: char,
        light: char,
    }

    impl<C: ModuleSource + ?Sized> Renderer<C> for DummyRenderer {
        type Output = String;
        type Error = Infallible;

        fn render(&self, code: &C) -> Result<Self::Output, Self::Error> {
            let mut out = String::new();
            for y in 0..code.height() {
                for x in 0..code.width() {
                    out.push(match code.get(x, y) {
                        Color::Dark => self.dark,
                        Color::Light => self.light,
                    });
                }
            }
            Ok(out)
        }
    }

    struct DummyStorage {
        modules: [Color; 4],
        width: usize,
    }

    impl ModuleStorage for DummyStorage {
        fn get(&self, x: usize, y: usize) -> Color {
            self.modules[y * self.width + x]
        }

        fn set(&mut self, x: usize, y: usize, color: Color) {
            self.modules[y * self.width + x] = color;
        }

        fn width(&self) -> usize {
            self.width
        }

        fn height(&self) -> usize {
            self.modules.len() / self.width
        }

        fn modules(&self) -> &[Color] {
            &self.modules
        }
    }

    #[test]
    fn module_view_reads_row_major_modules() {
        let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
        let view = ModuleView::new(&modules, 2).unwrap();

        assert_eq!(view.width(), 2);
        assert_eq!(view.height(), 2);
        assert_eq!(view.modules(), modules);
        assert_eq!(view.row(1), &[Color::Light, Color::Dark]);
        assert_eq!(view.get(0, 0), Color::Dark);
        assert_eq!(view.get(1, 1), Color::Dark);
    }

    #[test]
    fn module_view_reads_rectangular_row_major_modules() {
        let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark, Color::Dark, Color::Light];
        let view = ModuleView::new_rect(&modules, 3, 2).unwrap();

        assert_eq!(view.width(), 3);
        assert_eq!(view.height(), 2);
        assert_eq!(view.row(1), &[Color::Dark, Color::Dark, Color::Light]);
    }

    #[test]
    fn module_view_row_range_borrows_contiguous_rows() {
        let modules = [
            Color::Dark,
            Color::Light,
            Color::Light,
            Color::Dark,
            Color::Dark,
            Color::Light,
            Color::Light,
            Color::Dark,
            Color::Dark,
        ];
        let view = ModuleView::new(&modules, 3).unwrap();
        let rows = view.row_range(1, 3).unwrap();

        assert_eq!(rows.width(), 3);
        assert_eq!(rows.height(), 2);
        assert_eq!(rows.modules(), &modules[3..9]);
        assert!(view.row_range(2, 2).is_none());
        assert!(view.row_range(2, 4).is_none());
    }

    #[test]
    fn module_view_rejects_non_square_input() {
        let modules = [Color::Dark, Color::Light, Color::Dark];

        assert!(ModuleView::new(&modules, 2).is_none());
        assert!(ModuleView::new(&modules, 0).is_none());
    }

    #[test]
    fn module_view_rejects_invalid_rectangular_input() {
        let modules = [Color::Dark, Color::Light, Color::Dark];

        assert!(ModuleView::new_rect(&modules, 2, 2).is_none());
        assert!(ModuleView::new_rect(&modules, 0, 2).is_none());
        assert!(ModuleView::new_rect(&modules, 2, 0).is_none());
    }

    #[test]
    fn qr_code_ref_exposes_borrowed_symbol_metadata() {
        let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
        let symbol = QrCodeRef::new(&modules, 2, Version::Normal(3), EcLevel::Q).unwrap();

        assert_eq!(symbol.width(), 2);
        assert_eq!(symbol.height(), 2);
        assert_eq!(symbol.modules(), modules);
        assert_eq!(symbol.get(0, 0), Color::Dark);
        assert_eq!(symbol.version(), Version::Normal(3));
        assert_eq!(symbol.error_correction_level(), EcLevel::Q);
        assert_eq!(symbol.quiet_zone(), 4);
    }

    #[test]
    fn qr_code_ref_rejects_invalid_module_geometry() {
        let modules = [Color::Dark, Color::Light, Color::Dark];

        assert!(QrCodeRef::new(&modules, 2, Version::Normal(1), EcLevel::M).is_none());
        assert!(QrCodeRef::new(&modules, 0, Version::Normal(1), EcLevel::M).is_none());
    }

    #[test]
    fn qr_code_ref_module_view_reuses_borrowed_modules() {
        let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
        let symbol = QrCodeRef::new(&modules, 2, Version::Micro(1), EcLevel::L).unwrap();
        let view = symbol.module_view();

        assert_eq!(view.modules(), modules);
        assert_eq!(view.get(1, 1), Color::Dark);
        assert_eq!(symbol.quiet_zone(), 2);
    }

    #[test]
    fn qr_symbol_default_quiet_zone_for_normal_qr_is_four_modules() {
        let symbol = DummySymbol { version: Version::Normal(1), modules: [Color::Dark] };

        assert_eq!(symbol.quiet_zone(), 4);
    }

    #[test]
    fn qr_symbol_default_quiet_zone_for_micro_qr_is_two_modules() {
        let symbol = DummySymbol { version: Version::Micro(1), modules: [Color::Dark] };

        assert_eq!(symbol.quiet_zone(), 2);
    }

    #[test]
    fn builder_trait_builds_configured_output() {
        let result = DummyBuilder { value: 7 }.build();

        assert_eq!(result, Ok(7));
    }

    #[test]
    fn encoder_trait_accepts_third_party_implementations() {
        let output = DummyEncoder.encode(b"hello").unwrap();

        assert_eq!(output, 5);
    }

    #[test]
    fn renderer_trait_accepts_third_party_implementations() {
        let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
        let view = ModuleView::new(&modules, 2).unwrap();
        let renderer = DummyRenderer { dark: '#', light: '.' };

        assert_eq!(renderer.render(&view).unwrap(), "#..#");
    }

    #[test]
    fn module_storage_blanket_impl_provides_module_source() {
        let mut storage = DummyStorage { modules: [Color::Light; 4], width: 2 };
        storage.set(1, 0, Color::Dark);
        storage.set(0, 1, Color::Dark);

        assert_eq!(ModuleSource::width(&storage), 2);
        assert_eq!(ModuleSource::height(&storage), 2);
        assert_eq!(ModuleSource::get(&storage, 1, 0), Color::Dark);
        assert_eq!(<DummyStorage as ModuleSource>::row(&storage, 1), &[Color::Dark, Color::Light]);
        assert_eq!(ModuleSource::modules(&storage), &[Color::Light, Color::Dark, Color::Dark, Color::Light]);
    }
}
