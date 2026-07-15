# qrcode-pdf

`qrcode-pdf` is the pure Rust vector PDF rendering backend for
[`qrcode-rs`](https://crates.io/crates/qrcode-rs). It does not depend on an
external PDF library.

```toml
[dependencies]
qrcode-pdf = "2.0"
```

Most applications can enable the backend through the facade crate:

```toml
[dependencies]
qrcode-rs = { version = "2.0", features = ["pdf"] }
```

## Features

| Feature | Purpose |
| --- | --- |
| `std` | Opts into the standard library. Disabled by default. |

Use this crate directly for document-generation workflows that only need the PDF
backend plus the shared core/render contracts.
