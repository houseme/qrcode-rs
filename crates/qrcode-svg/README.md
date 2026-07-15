# qrcode-svg

`qrcode-svg` is the pure Rust SVG rendering backend for
[`qrcode-rs`](https://crates.io/crates/qrcode-rs). It provides SVG color types,
renderer integration, and small post-processing helpers such as ARIA labeling.

```toml
[dependencies]
qrcode-svg = "2.0"
```

Most applications can enable the backend through the facade crate:

```toml
[dependencies]
qrcode-rs = { version = "2.0", features = ["svg"] }
```

## Features

| Feature | Purpose |
| --- | --- |
| `std` | Opts into the standard library. Disabled by default. |

Use this crate directly when building a renderer plugin or when a library wants
SVG output without depending on every facade default backend.
