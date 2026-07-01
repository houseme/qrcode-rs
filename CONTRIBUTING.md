# Contributing to qrcode-rs

Thanks for your interest in improving `qrcode-rs`! This guide covers the
development setup, the quality bar every change must meet, and the commit/PR
conventions used in this repository.

## Getting started

- **Rust toolchain**: stable **1.85 or newer** (the crate uses edition 2024;
  declared via `rust-version` in `Cargo.toml`).
- **Build & test**:

  ```bash
  cargo build --all-features
  cargo test --all-features
  ```

## Project layout

```
src/
  lib.rs        # QrCode, builder, iterators, Info, convenience methods
  bits.rs       # bit-level encoding (Numeric/Alphanumeric/Byte/Kanji, ECI, FNC1)
  canvas.rs     # module matrix construction + masking
  ec.rs         # Reed-Solomon error correction
  optimize.rs   # optimal mode segmentation
  types.rs      # Color, EcLevel, Version, Mode, QrError
  render/       # backends: string, unicode, ansi, svg, image, eps, pic, html, pdf
  bin/          # qrencodes CLI (behind the `cli` feature)
benches/        # criterion benchmarks (encoding, rendering)
examples/       # runnable examples
tests/          # integration tests
docs/           # roadmap & planning (local; see below)
```

## Feature flags

The **library is zero-dependency by default** — the core encoder pulls in no
external crates. Optional capabilities are feature-gated:

| Feature   | Enables                                  |
|-----------|------------------------------------------|
| `image`   | PNG/JPEG/… rendering via the `image` crate (includes the PNG codec) |
| `svg`     | SVG renderer                             |
| `eps`     | Encapsulated PostScript renderer         |
| `pic`     | troff PIC renderer                       |
| `html`    | HTML table / CSS Grid renderer           |
| `pdf`     | vector PDF renderer (no external dep)    |
| `cli`     | the `qrencodes` binary (`clap`-based)    |

`default = ["image", "svg", "pic", "eps", "html", "pdf"]`. When adding a new
dependency, prefer a feature gate or a hand-written implementation to honor the
zero-dependency principle.

## Quality bar

Every PR must pass:

```bash
cargo test --all-features
cargo test --no-default-features          # the lib must still build without default features
cargo clippy --all-features --all-targets -- -D warnings
cargo fmt -- --check
cargo doc --all-features --no-deps        # warning/error-free (#![deny(missing_docs)] is enforced)
```

All public API must be documented (`#![deny(missing_docs)]` is set in
`src/lib.rs`). New features should include unit tests and/or doctests, and an
example where useful.

## Commit conventions

- Use **conventional-commit** prefixes: `feat:`, `fix:`, `perf:`, `docs:`,
  `refactor:`, `test:`, `style:`, `chore:`, optionally scoped
  (`feat(v0.5.0): …`).
- Make **small, focused commits** — one logical change each.
- End every commit message with exactly this trailer and no other
  `Co-Authored-By`:

  ```
  Co-Authored-By: heihutu <heihutu@gmail.com>
  ```

## Pull requests

1. Open a PR against `main` (this project develops on `main`).
2. Ensure the quality bar above is green.
3. Update `CHANGELOG.md` and, where relevant, the roadmap docs.
4. For breaking changes (only on a major-version bump), describe the migration
   path.

## Releases

Releases follow [SemVer](https://semver.org). The flow:

1. Update `version` in `Cargo.toml`.
2. Add a `## [X.Y.Z]` section to `CHANGELOG.md` (group by Added / Changed /
   Fixed / Notes, list deferrals explicitly).
3. Tick the relevant roadmap checklist (`docs/roadmap/v*.md`) and update
   `docs/roadmap.md` (current version + status).
4. Verify the quality bar, then `cargo package` (and `cargo publish` from a
   machine with crates.io credentials).

## Roadmap & local docs

The roadmap lives in `docs/roadmap/`. Note that `docs/*` (except
`docs/images/` and `docs/index.md`) are **gitignored and kept locally** —
planning notes and per-version implementation plans live there and are not
version-controlled. Trust `Cargo.toml`'s `version` and `CHANGELOG.md` for the
true release state, not the roadmap status markers (which can lag).
