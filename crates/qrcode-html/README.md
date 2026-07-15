# qrcode-html

`qrcode-html` is the pure Rust HTML renderer for
[`qrcode-rs`](https://crates.io/crates/qrcode-rs). It supports table and
CSS-grid style output for web or document embedding.

```toml
[dependencies]
qrcode-html = "2.0"
```

Most applications can enable the backend through the facade crate:

```toml
[dependencies]
qrcode-rs = { version = "2.0", features = ["html"] }
```

## Features

| Feature | Purpose |
| --- | --- |
| `std` | Opts into the standard library. Disabled by default. |

Use this crate directly when you need only HTML rendering plus the shared
core/render contracts.
