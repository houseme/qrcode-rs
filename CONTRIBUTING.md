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
  lib.rs        # qrcode-rs facade: QrCode, builder, streams, re-exports
  bin/          # qrencodes CLI (behind the `cli` feature)
crates/
  qrcode-core/  # encoding core, traits, module views, plugin contracts
  qrcode-render/# shared render traits, text/unicode/ansi/image helpers
  qrcode-parse/ # WiFi/vCard/GS1 payload parsers
  qrcode-decode/# decoder traits, grayscale views, rqrr adapter, SA parser
  qrcode-*/     # individual backend crates: svg, eps, pic, html, pdf
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
| `async`   | Tokio-backed async rendering helpers     |
| `decode-rqrr` | decoder adapter through `rqrr`        |
| `compat-1x` | transitional 1.x facade compatibility  |
| `cli`     | the `qrencodes` binary (`clap`-based)    |

`default = ["std", "image", "svg", "pic", "eps", "html", "pdf"]`. When adding
a new dependency, prefer a feature gate or a hand-written implementation to
honor the zero-dependency principle.

## Quality bar

Every PR must pass:

```bash
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo test --test proptest --all-features
cargo test --test differential --all-features
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo fmt --all --check
cargo doc --all-features --no-deps        # warning/error-free (#![deny(missing_docs)] is enforced)
```

All public API must be documented (`#![deny(missing_docs)]` is set in
`src/lib.rs`). New features should include unit tests and/or doctests, and an
example where useful.

## Property, differential, and fuzz testing

The v2 test architecture includes three layers beyond ordinary integration
tests:

```bash
cargo test --test proptest --all-features
cargo test --test proptest --no-default-features
cargo test --test differential --all-features
cargo test --test differential --no-default-features
```

The `fuzz/` workspace is a `cargo-fuzz` project with targets for core encoding,
fixed-version encoding, SVG rendering, structured payload parsing, and
structured append. Use a compile check for quick local validation:

```bash
cargo check --manifest-path fuzz/Cargo.toml --bins
```

For a short local fuzz smoke run, install `cargo-fuzz` and run a bounded target:

```bash
cargo install cargo-fuzz --locked
cargo fuzz run encode -- -runs=100
cargo fuzz run render_svg -- -runs=100
```

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
   For a major workspace release, keep `qrcode-rs` and all publishable split
   crates on the same version and update every path dependency version
   constraint.
2. Add a `## [X.Y.Z]` section to `CHANGELOG.md` (group by Added / Changed /
   Fixed / Notes, list deferrals explicitly).
3. Update crate READMEs and the root README when a published crate boundary,
   feature flag, or install snippet changes.
4. Tick the relevant roadmap checklist (`docs/roadmap/v*.md`) and update
   `docs/roadmap.md` (current version + status).
5. Verify the quality bar, then check package file lists:

   ```bash
   cargo package --workspace --allow-dirty --no-verify --list
   ```

   A full `cargo package --workspace` before anything has been published will
   fail for crates that depend on newly split workspace crates, because Cargo
   resolves publishable path dependencies through the registry. Use the file-list
   check before publishing, then publish in dependency order.

6. Publish split crates before the facade, skip workspace-local crates such as
   `qrcode-compat`, and wait for each version to appear in the crates.io index
   before publishing crates that depend on it:

   ```bash
   version=2.0.0
   for crate in \
     qrcode-core qrcode-render qrcode-parse qrcode-decode \
     qrcode-svg qrcode-eps qrcode-pic qrcode-html qrcode-pdf qrcode-rs
   do
     cargo publish --registry crates-io --package "$crate"
     until cargo info "${crate}@${version}" --registry crates-io >/dev/null 2>&1
     do
       sleep 10
     done
   done
   ```

## Roadmap & local docs

The roadmap lives in `docs/roadmap/`. Note that `docs/*` (except
`docs/images/` and `docs/index.md`) are **gitignored and kept locally** —
planning notes and per-version implementation plans live there and are not
version-controlled. Trust `Cargo.toml`'s `version` and `CHANGELOG.md` for the
true release state, not the roadmap status markers (which can lag).
