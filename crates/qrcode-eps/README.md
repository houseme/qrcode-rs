# qrcode-eps

`qrcode-eps` is the pure Rust Encapsulated PostScript renderer for
[`qrcode-rs`](https://crates.io/crates/qrcode-rs).

```toml
[dependencies]
qrcode-eps = "2.0"
```

Most applications can enable the backend through the facade crate:

```toml
[dependencies]
qrcode-rs = { version = "2.0", features = ["eps"] }
```

## Features

| Feature | Purpose |
| --- | --- |
| `std` | Opts into the standard library. Disabled by default. |

Use this crate directly for print or prepress workflows that only need the EPS
backend plus the shared core/render contracts.
