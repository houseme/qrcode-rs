# qrcode-render

`qrcode-render` contains the shared QR rendering traits and built-in text,
Unicode, ANSI, color, and template helpers used by
[`qrcode-rs`](https://crates.io/crates/qrcode-rs).

Most users should render through the facade crate:

```toml
[dependencies]
qrcode-rs = "2.0"
```

Depend on `qrcode-render` directly when implementing a renderer backend,
building a plugin, or keeping rendering code separate from the high-level
encoder facade.

```toml
[dependencies]
qrcode-render = "2.0"
```

## Features

| Feature | Purpose |
| --- | --- |
| `std` | Opts into the standard library. Disabled by default. |
| `image` | Enables the image backend implementation and PNG/JPEG support. |

## Companion Backends

Format-specific vector backends live in separate crates:
`qrcode-image`, `qrcode-svg`, `qrcode-eps`, `qrcode-pic`, `qrcode-html`, and
`qrcode-pdf`. Use `qrcode-image` when you want the image backend as a focused
dependency; its API re-exports this crate's image implementation.
