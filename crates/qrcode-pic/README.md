# qrcode-pic

`qrcode-pic` is the pure Rust PIC/troff rendering backend for
[`qrcode-rs`](https://crates.io/crates/qrcode-rs).

```toml
[dependencies]
qrcode-pic = "2.0"
```

Most applications can enable the backend through the facade crate:

```toml
[dependencies]
qrcode-rs = { version = "2.0", features = ["pic"] }
```

## Features

| Feature | Purpose |
| --- | --- |
| `std` | Opts into the standard library. Disabled by default. |

Use this crate directly when integrating QR output into troff/PIC document
toolchains or other narrow rendering pipelines.
