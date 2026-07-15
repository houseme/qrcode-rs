# qrcode-core

`qrcode-core` is the zero-dependency QR encoding core used by
[`qrcode-rs`](https://crates.io/crates/qrcode-rs). It provides the bitstream,
mode optimization, Reed-Solomon, canvas, core type, module-view, and plugin
contracts without pulling in renderer or image dependencies.

Most applications should depend on `qrcode-rs`. Depend on `qrcode-core`
directly when building a custom facade, renderer, plugin, or `no_std + alloc`
integration.

```toml
[dependencies]
qrcode-core = "2.0"
```

## Features

| Feature | Purpose |
| --- | --- |
| `std` | Opts into the standard library. Disabled by default. |
| `serde` | Enables serialization for core QR types. |
| `bench-internals` | Exposes benchmark-only internals. Not a stable public surface. |

## Relationship to qrcode-rs

The facade crate re-exports the common `qrcode-core` types, including
`EcLevel`, `Version`, `Mode`, `QrError`, `ModuleSource`, `ModuleView`,
`QrCodeRef`, `Renderer`, `Encoder`, and the plugin registry contracts.

Use this crate directly only when you want a narrower dependency graph than the
facade provides.
