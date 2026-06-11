# Changelog

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
