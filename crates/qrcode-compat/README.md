# qrcode-compat

`qrcode-compat` is a workspace-local migration harness for `qrcode-rs` 2.0. It
re-exports the facade crate with the `compat-1x` feature enabled so the 1.x API
shape can be tested while internals are split across smaller crates.

This crate is intentionally not published to crates.io.

```toml
[dependencies]
qrcode-compat = { path = "crates/qrcode-compat" }
```

For published dependencies, use the facade crate directly:

```toml
[dependencies]
qrcode-rs = { version = "2.0", features = ["compat-1x"] }
```

Remove `compat-1x` once call sites have moved to the builder, module-view,
streaming, and split-crate APIs.
