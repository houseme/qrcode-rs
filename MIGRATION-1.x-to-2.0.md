# Migrating from qrcode-rs 1.x to 2.0

This guide tracks the intended 2.0 migration path while the internals move from
one facade crate into smaller workspace crates. The public 1.x facade remains
available behind the `compat-1x` feature so applications can upgrade first and
then adopt the more explicit 2.0 APIs incrementally.

## Quick Path

Enable the compatibility feature during the first upgrade:

```toml
[dependencies]
qrcode-rs = { version = "2.0", features = ["compat-1x"] }
```

Then migrate one call site at a time. The compatibility layer is transitional:
it keeps 1.x-shaped constructors, render chains, indexing, and payload helpers
available while new code can use the builder, trait-based module views, and
split render/parse/decode crates.

The workspace also contains a thin `qrcode-compat` crate for local migration
testing. It re-exports the facade with `compat-1x` enabled, but it remains
`publish = false` because the published migration path is the facade crate plus
the `compat-1x` feature. Use a workspace path dependency when testing it inside
this repository:

```toml
[dependencies]
qrcode-compat = { path = "crates/qrcode-compat" }
```

## API Map

| 1.x usage | 2.0-preferred usage | Notes |
| --- | --- | --- |
| `QrCode::new(data)` | `QrCode::builder(data).build()` | `new` stays available in `compat-1x`; builder reads better once options grow. |
| `QrCode::with_error_correction_level(data, ec)` | `QrCode::builder(data).ec_level(ec).build()` | Same encoder path. |
| `QrCode::with_version(data, version, ec)` | `QrCode::builder(data).version(version).ec_level(ec).build()` | Keeps fixed-version behavior explicit. |
| `QrCode::new_micro(data)` | `QrCode::builder(data).micro(true).build()` | Use `.version(Version::Micro(_))` when a specific Micro QR version is required. |
| `code.render::<P>().dark_color(...).build()` | `code.render_builder::<P>().dark_color(...).build()` | `render` remains; `render_builder` is the explicit 2.0 naming. |
| `code.colors()` / `code[(x, y)]` | `code.module_view()` / `ModuleSource` | Borrowed module views avoid allocation and work across renderers. |
| `QrCode::for_wifi(...)` / `for_vcard(...)` / `for_gs1(...)` | Same facade helpers, plus `qrcode_rs::parse::*` for parsing | Helpers stay facade-level; parsing logic lives in `qrcode-parse`. |
| `qrcode_rs::decode::*` | `qrcode-decode` or facade re-export | Use the split crate when depending only on decode contracts. |
| `qrcode_rs::render::*` | `qrcode-render` plus backend crates | Backend crates are useful for plugin authors or minimal builds. |
| `QrCode::batch(inputs, ec)` | `QrCode::stream_with_error_correction_level(inputs, ec)` | Streaming keeps memory bounded for large batches. |

## Feature Flags

The default feature set continues to enable the common renderers. For migration
work, the important flags are:

- `compat-1x`: keeps the 1.x facade API available while 2.0 internals are split.
- `std`: opt into the standard library; disable default features for
  `no_std + alloc` core usage.
- `svg`, `image`, `eps`, `pic`, `html`, `pdf`: enable renderer backends.
- `decode-rqrr`: enables the bundled `rqrr` decoder adapter.
- `async`: enables Tokio-backed `render_async`.

## Recommended Migration Order

1. Enable `compat-1x` and run your existing tests unchanged.
2. Replace constructor call sites with `QrCode::builder(...)` where options are
   already being passed.
3. Replace allocation-heavy module access with `module_view()` or APIs that
   accept `ModuleSource`.
4. For large batches, move from `batch` to `stream` or
   `stream_with_error_correction_level`.
5. If your library only needs one layer, depend on the split crate directly
   (`qrcode-core`, `qrcode-render`, `qrcode-parse`, `qrcode-decode`, or a
   renderer backend) instead of the full facade.
6. If you used the workspace-local `qrcode-compat` crate, switch imports back
   to the facade or split crates once no 1.x-only paths remain.
7. Remove `compat-1x` once no 1.x-only paths remain.

## Compatibility Checks

Run the compatibility test target while migrating:

```bash
cargo test --test compat_1x --features compat-1x
cargo test --test compat_1x --no-default-features --features compat-1x
cargo test -p qrcode-compat
cargo test -p qrcode-compat --no-default-features
```

For the full local quality bar, also run:

```bash
cargo test --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

## Known Limits

The compatibility feature preserves public facade usage, not private internals.
Code that reached into undocumented implementation details should migrate to one
of the public traits (`ModuleSource`, `ModuleStorage`, `QrSymbol`) or the split
workspace crates.
