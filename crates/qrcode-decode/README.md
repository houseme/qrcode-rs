# qrcode-decode

`qrcode-decode` contains decoder-facing contracts for `qrcode-rs`: grayscale
pixel views, the `QrDecoder` trait, decoded-symbol metadata, Structured Append
bitstream parsing, and the optional `rqrr` adapter.

```toml
[dependencies]
qrcode-decode = "2.0"
```

Use the facade crate when you want encoding, rendering, parsing, and decoding
from one dependency:

```toml
[dependencies]
qrcode-rs = { version = "2.0", features = ["decode-rqrr"] }
```

## Features

| Feature | Purpose |
| --- | --- |
| `std` | Opts into the standard library. Disabled by default. |
| `image` | Enables `GrayPixels` conversion from `image::GrayImage`. |
| `rqrr` | Enables the `RqrrDecoder` adapter. |

`qrcode-decode` is intentionally adapter-oriented. It does not make `rqrr`
mandatory for users that only need decoder traits or Structured Append parsing.
