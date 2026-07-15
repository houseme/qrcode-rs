# Changelog

## [Unreleased]

### Added

- **1.x → 2.0 migration guide** (`MIGRATION-1.x-to-2.0.md`) with API mapping,
  recommended migration order, and compatibility-test commands.
- **`compat-1x` feature** — a transitional compatibility contract that keeps
  the 1.x facade constructors, render chain, indexing, and payload helpers
  covered while 2.0 internals move into split workspace crates.
- **`qrcode-compat` workspace crate** — local-only migration harness for the
  dedicated dependency-name path; publishing is deferred until the facade crate
  itself is bumped to 2.0.0.

### Notes / deferred

- `compat-1x` is intended as a migration bridge, not the long-term 2.0 style.
  New code should prefer `QrCode::builder`, `module_view`, streaming APIs, and
  direct split-crate dependencies where narrower layering is useful.

## [1.6.0] - 2026-07-14

### Added

- **Structured Append bit-stream parser** (`qrcode_rs::decode::sa_parse`) — the
  pure-core, decoder-agnostic decode counterpart to
  `StructuredAppend::encode`. `parse_sa_datastream(bits, version)` reads the
  20-bit Structured Append header (mode `0011`, the symbol-sequence indicator,
  the parity byte) and decodes the Numeric / Alphanumeric / Byte / Kanji
  segments, returning `SaSymbolData { position, total, parity, data }`. Feed it
  the bytes any decoder recovers, then
  [`reassemble`](crate::structured_append::reassemble) the symbols.
- `SaError::NotStructuredAppend` and `SaError::MalformedStream` (both
  `#[non_exhaustive]`, non-breaking).

### Fixed

- **`RqrrDecoder` EC-level mapping** — `rqrr` stores the raw QR
  format-information EC bits (`M=00, L=01, H=10, Q=11`), not a sequential
  index, so `RqrrDecoder::decode` was returning the wrong `EcLevel` on
  `DecodedQrCode` (e.g. an M-level symbol read back as L). Now correct, with a
  round-trip assertion in `tests/decode_roundtrip.rs`.

### Notes / deferred

- **`rqrr` cannot decode Structured Append data** — it reaches the `0011` mode
  and returns `UnknownDataType`. The pure-core parser recovers the data from a
  byte stream, but `rqrr`'s public API can't supply the corrected,
  correctly-oriented stream (it reads the crate's rendered symbols mirrored,
  and its `MirroredGrid` / `codestream_ecc` are private), so a full
  encode→render→decode round-trip isn't wired through the bundled decoder.
  `tests/sa_roundtrip.rs` instead asserts `rqrr` *recognizes* the symbols
  (`UnknownDataType`), proving they are well-formed. A native SA-aware decoder
  (image → grid → Reed-Solomon → modes) is deferred to v2.0.0.

## [1.5.0] - 2026-07-13

### Added

- **Structured Append** (`qrcode_rs::structured_append`) — full ISO/IEC 18004
  §7.4 support: split one payload across 2..=16 QR symbols. The
  `StructuredAppend` builder computes the 20-bit header — mode `0011`, an 8-bit
  symbol-sequence indicator (high nibble = position, low = total), and the XOR
  parity of the original message — and encodes each symbol at the smallest
  fitting version. `QrCode::structured_append(payload, symbols, ec)` is a thin
  convenience, and the symmetric, decoder-agnostic `reassemble(&[SaSymbol])`
  recombines decoded symbols (validating uniform total/parity and a complete
  position set).
- `Bits::push_structured_append_header(position, total, parity)` low-level
  primitive. A position/total of 16 wraps to nibble `0` (the only 4-bit
  encoding).
- `QrError::InvalidStructuredAppend { value }` (`#[non_exhaustive]`, with a
  `suggestion()`).
- `SaError` (`#[non_exhaustive]`, no_std + alloc compatible).
- `examples/structured_append.rs` rewritten as a real multi-symbol demo.

### Notes / deferred

- **rqrr cannot decode Structured Append symbols** — it matches data modes
  `0/1/2/4/7/8` and returns `UnknownDataType` for mode `3`. The v1.4.0-style
  encode→rqrr→decode round-trip is therefore not possible for Structured
  Append; verification is bit-level + structural + the split-rule↔`reassemble`
  symmetry, all pure-core. A future `quircs` adapter or native SA decoder
  would close the gap.
- Uniform-version mode (all symbols sharing one QR version) is deferred;
  v1.5.0 uses per-symbol auto-version (spec-compliant and more compact).

## [1.4.0] - 2026-07-12

### Added

- **Content parsers** (`qrcode_rs::parse`) — the symmetric decode-side of the
  `for_*` encoders, operating on caller-provided `&str`/`&[u8]` (encoding is
  one-way): `WifiConfig::parse` (+ `WifiSecurity`), `VCard::parse` (tolerant of
  vCard 2.1/3.0/4.0, line folding, and property params), and `Gs1Result::parse`
  with a curated GS1 application-identifier table including the `310n`–`369n`
  measure families. `for_wifi`/`for_vcard` now share their wire-format logic with
  the parsers, so encode↔parse round-trips by construction.
- **Decode bridge** (`qrcode_rs::decode`): a `QrDecoder` trait, `DecodedQrCode`,
  and `GrayPixels` (a borrowed grayscale view that keeps the trait decoupled from
  the `image` crate, preserving the zero-dependency core), plus an opt-in
  `rqrr`-backed adapter (`RqrrDecoder`, behind the `decode-rqrr` feature). The
  encode → render → decode round-trip is verified end-to-end in tests.
- `parse::ParseError` (`#[non_exhaustive]`, no_std + alloc compatible).

### Notes / deferred

- Data Matrix / Aztec encoders, the unified `Barcode` interface, and
  cross-format benchmarks are deferred (each DM/Aztec encoder is major
  algorithmic work; better as companion crates or later versions).
- The image preprocessor / `QrLocator` and a `quircs` adapter are deferred
  (image-intensive, and a second adapter — `rqrr` already validates the trait).

## [1.3.0] - 2026-07-02

### Added

- **`QrError::suggestion()`** — per-variant actionable fix hints (every current variant has one).
- **`QrCode::analyze() -> Analysis`** — diagnostic stats: dark ratio, functional-module count, and data-module count (computed from `colors()` / `is_functional()`; combine with `info()` for version/capacity).
- **`log` facade** (opt-in `log` feature, no_std-safe): `debug!`/`info!` records at the `QrCode::with_bits` encode chokepoint.

### Notes / deferred

- `tracing` spans, `metrics` counters, encode-trace replay, visual debug render, retention-dependent error fields, and test-utils (Mock renderer / `random_seed` — encoding is already deterministic) are deferred (observability sinks needing a consumer / image-heavy / need data retention).

## [1.2.0] - 2026-07-01

### Added

- **Batch encoding**: `QrCode::batch(inputs, ec_level) -> QrResult<Vec<QrCode>>` — encodes many inputs at once, short-circuiting on the first error (no new deps; ~32ms/1000 short codes).
- **Style templates**: `QrTemplate` (serde-derivable: dark/light hex colors, module size, quiet zone) with presets (`minimal`, `dark_mode`, `high_contrast`, `corporate`), applied via `Renderer::template` for `StyledPixel` backends.
- `render::StyledPixel` trait (`from_hex`) implemented for the image RGB/RGBA, EPS, PDF, and ANSI backends.
- `examples/batch_template.rs`.

### Notes / deferred

- `par_batch` (rayon), batch render-to-files / grid / packaging, streaming / CSV / JSON data sources, brand customization (logo / captions / color-scheme), and template inheritance / TOML-YAML loading are deferred (external crates / image-heavy / couple to deferred features).
- Templates don't cover the borrowing `svg`/`html` backends (their color borrows the input) — apply those colors manually.

## [1.1.0] - 2026-07-01

### Added

- **Serde serialization** (opt-in `serde` feature): `Serialize`/`Deserialize` on `Color`, `EcLevel`, `Version`, `Mode`, `QrError`, and `Info`, plus a `QrCodeData` wrapper with `QrCode::to_serializable()` / `from_serializable()`. JSON round-trip tested (`tests/serde.rs`).
- **`no_std` + `alloc` support**: a default-on `std` feature; with it disabled the library builds as `#![no_std]` + `alloc` (`cargo build --no-default-features --features svg`). The `image` and `cli` features imply `std`.

### Changed

- Library internals use `core::` / `alloc::` instead of `std::` throughout (no behavior change for std users).

### Notes / deferred

- WASM bindings, framework integrations (Axum/Actix/Rocket/Leptos/Yew), interop with other QR crates, and GBK/Big5 charsets are deferred (separate companion crates / external infra / algorithmic work).

## [1.0.0] - 2026-07-01

🎉 **First stable release.** The public API is now frozen; future breaking
changes will require a new major version.

### Stable-release work

- **API freeze & full documentation**: every public item is now documented, and
  `#![deny(missing_docs)]` is enforced at the crate root so the standard holds
  going forward. `cargo doc --all-features` is warning/error-free.
- **`Info` is `#[non_exhaustive]`** — metadata fields may grow in 1.x without a
  breaking change (`QrError` was already `#[non_exhaustive]`).
- **MSRV declared**: `rust-version = "1.85"` (edition 2024).
- **`CONTRIBUTING.md`** added: dev setup, feature flags, the quality bar,
  commit conventions, and the release flow.
- **Packaging verified**: `cargo package` is clean.

### Notes / deferred (maintainer / external infrastructure)

- Stabilization items needing external tooling or credentials remain the
  maintainer's responsibility: `cargo-tarpaulin` coverage reporting,
  `cargo-fuzz` / `cargo miri`, cross-platform/WASM CI, the actual
  `cargo publish` to crates.io, and the GitHub Release.
- No behavioral change versus 0.6.0; this release codifies API stability.

## [0.6.0] - 2026-07-01

### Added

- Criterion benchmark suite (`benches/encoding.rs`, `benches/rendering.rs`; dev-dependency only — the library stays zero-dependency). Encoding: short/medium/long payloads + v1/v10/v20/v30/v40; rendering: string/svg/image/eps
- `QrCode::colors() -> &[Color]` — zero-copy borrow of the module slice (complements `to_colors()` / `into_colors()`)
- `Color` is now `#[repr(u8)]` with explicit discriminants (`Light = 0`, `Dark = 1`) — deterministic 1-byte layout, FFI-friendly, makes `Color as u8` sound
- Compile-time guarantee (and docs) that `QrCode: Send + Sync`

### Changed (Performance)

- `ec::create_error_correction_code` allocates its buffer once (`Vec::with_capacity`) instead of `to_vec()` + `resize()` realloc — bench: `encode/long` ~6.06ms → ~5.68ms (~6%), no regression elsewhere
- EPS renderer preallocates its output buffer (was the only renderer that didn't) — bench: `render/eps` ~15.5µs → ~14.7µs

### Notes

- Deferred (external deps conflict with zero-dependency): `SmallVec`, `bumpalo` arena, `rayon` parallel (a future `parallel` feature).
- Deferred (breaking/complex): 1-bit-per-module storage, `unsafe new_unchecked`, DP segmentation.
- Deferred (infra): CI bench regression detection, memory profiling, compile-time/monomorphization opts.

## [0.5.0] - 2026-07-01

### Added

- `QrCode::for_gs1(data)` — GS1 / FNC1-first-position convenience constructor (smallest fitting version, medium error correction)
- `QrCode::info() -> Info` metadata struct (version, ec_level, width, module_count, max_allowed_errors, data_capacity_bytes); backed by a new public `bits::data_capacity_bits(version, ec_level)`
- `QrCode::alt_text(data)` and `alt_text_custom(data, f)` — accessible alt-text generation (URL-aware: "linking to …" vs "containing: …")
- Builder `.force_mode(M)` (alias of `.encoding_mode(M)`) now works without a pinned version — auto-selects the smallest fitting version for the forced mode
- `render::svg::aria_label(svg, label)` and `render::html::aria_label(html, label)` — inject `role="img"` + `aria-label` for screen readers
- `render::html::inject_attributes(html, attrs)` — arbitrary attributes on the QR container (`<table>`/`<div>`)
- Examples: `encode_gs1`, `alt_text`, `accessible_svg`

### Fixed

- `render::svg::inject_attributes` now targets the `<svg>` root element instead of the `<?xml ?>` declaration (attributes were previously misplaced between the XML prolog and `<svg>`)

### Notes

- Deferred to later versions (need external decoder/scanner verification or a larger refactor): Structured Append (§1), custom Finder/Alignment patterns (§2), encoding stats (§4.2), and `Info` fields requiring input/mask retention (encoding_modes, mask_pattern, remaining_capacity)

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
