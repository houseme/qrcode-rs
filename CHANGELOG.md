# Changelog

## [0.4.0] - 2026-07-01

### Added

- `qrencodes` CLI: a full command-line generator behind the new opt-in `cli`
  feature (`clap`-based, zero-dependency library by default). Supports all
  output formats (`string`, `unicode`, `ansi`, `svg`, `png`, `eps`, `pic`,
  `html`, `pdf`), error-correction level, QR/Micro version, module size,
  quiet zone, `--dark`/`--light` colors, `--invert`, `--unicode-mode`, stdin
  input, and `--batch` (one QR per line)
- `QrCode::builder(data)` builder with `.ec_level()`, `.version()`, `.micro()`,
  `.encoding_mode()` and `.build()`, delegating to the existing encoders
- Convenience constructors: `QrCode::for_url`, `for_wifi`, `for_vcard`,
  `for_text`
- Module iterators: `QrCode::rows()` (yielding `Row`) and
  `QrCode::dark_modules()` (yielding `(x, y)` coordinates), zero-allocation
- `FromStr` implementations for `EcLevel`, `Version` (`1..=40`, `M1..M4`) and
  `Mode`, plus a public `EnumParseError` type
- New examples: `encode_kanji`, `encode_eci`, `encode_fnc1`, `custom_colors`,
  `error_handling`, `cli_tool`, `structured_append`
- `QrError` re-exported at the crate root

### Changed

- `QrError` is now `#[non_exhaustive]` and its variants carry structured,
  `Copy` context: `InvalidVersion { version, ec_level }`,
  `InvalidEciDesignator { value }`, `InvalidCharacter { position, byte }`.
  `Display` messages now include this context. (Variant shapes changed — a
  source-breaking change permitted at 0.x.)
- The `image` feature now enables the PNG codec (`image/png`) so that
  `encode_to_format(ImageFormat::Png)` and the CLI `-f png` actually work

### Fixed

- `render::colors` module doc example no longer requires the `svg` feature,
  so `cargo test --no-default-features` is clean

### Notes

- WASM playground and `validate` subcommand (roadmap §5) are deferred to their
  scheduled versions (v1.1.0 / v1.4.0); i18n error messages are deferred in
  favor of the enriched English `Display`

## [0.3.0] - 2026-06-11

### Added

- HTML table and CSS Grid renderer (`html` feature) with `Mode::Table` and `Mode::Grid` modes
- SVG `inject_attributes()` for adding custom attributes to the root `<svg>` element
- SVG `round_corners()` post-processing function for rounded module rendering
- SVG `animate()` post-processing function with `ScanLine`, `FadeIn`, `Pulse` presets
- `Dense2x2` Unicode renderer using quadrant block elements (U+2596–U+259F) for 2×2 pixel packing
- `Dense3x2` Unicode renderer using sextant characters (U+1FB00–U+1FB3F) for 3×2 pixel packing
- `Braille` Unicode renderer using Braille characters (U+2800–U+28FF) for 2×4 pixel packing
- ANSI TrueColor terminal renderer (`ansi` module) with 24-bit color and color-change optimization
- PDF vector renderer (`pdf` feature) with direct PDF generation, no external dependencies
- `render::colors` module with hex/RGB/RGBA/CSS color conversion utilities
- `Srgba` unified color type with hex/CSS/ANSI/EPS-PDF conversions and `lerp()` interpolation
- `overlay_logo()` function for embedding logos into QR code images (alpha-blended, auto-resized)
- `encode_to_format()` function for encoding QR images to JPEG, WebP, BMP, TIFF, GIF formats
- `apply_gradient_background()` function with vertical/horizontal/diagonal gradient support
- Smart sizing presets: `for_web()`, `for_print(dpi)`, `for_social(platform)` on Renderer builder

### Changed

- SVG path optimization: merged horizontally adjacent rectangles into single path segments

## [0.2.1] - 2026-06-11

### Changed

- Excluded roadmap files from version control (kept locally only)
- Version bump to 0.2.1

## [0.2.0] - 2026-06-11

### Added

- EPS (Encapsulated PostScript) renderer (`eps` feature)
- PIC (troff) renderer (`pic` feature)
- `QrCode::new_micro()` and `QrCode::micro_with_error_correction_level()` for automatic Micro QR encoding
- `bits::encode_auto_micro()` for automatic Micro QR version selection
- `const fn` annotations on `QrCode::version()`, `QrCode::error_correction_level()`, `QrCode::width()`, `Version::width()`, `Bits::new()`
- Module-level documentation for bits, canvas, ec, optimize, render modules
- `docs/index.md` project documentation page
- Boundary tests: v40 encoding, M4 encoding, empty input, data-too-long, ECI designators, all versions
- Integration test suite in `tests/` directory

### Fixed

- Micro QR Version 3/L half codeword encoding bug (absorbed from upstream PR #90)
- Panic in `Bits::push_terminator` when padding calculation underflows (absorbed from upstream PR #91)

### Changed

- SVG output optimized with path commands for smaller file size (absorbed from upstream PR #74)
- Replaced `Box<dyn Fn>` heap allocation with stack closure in `compute_finder_penalty_score`
- Updated `rustfmt.toml` edition to 2024
- CI matrix: nightly + stable only, removed MSRV constraint

### Removed

- Deprecated methods: `QrCode::to_vec()`, `QrCode::into_vec()`, `Canvas::to_bools()`, `Renderer::module_size()`, `Renderer::min_width()`, `Renderer::to_image()`
- `bench` feature and all benchmark code (incompatible with stable Rust)
- `rust-version` field from Cargo.toml
- `Cargo.lock` entry from `.gitignore`
