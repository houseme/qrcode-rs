# qrcode-decode

`qrcode-decode` contains decoder-facing contracts for `qrcode-rs`: grayscale
pixel views, the `QrDecoder` trait, decoded-symbol metadata, Structured Append
bitstream parsing, and optional `rqrr` and `rxing` image adapters.

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
| `rxing` | Enables the pure Rust Normal/Micro QR scanner and Structured Append metadata; implies `std`. |

`qrcode-decode` is intentionally adapter-oriented. It does not make `rqrr`
mandatory for users that only need decoder traits or Structured Append parsing.

Use `GrayPixels::try_new` to validate dimensions and the exact grayscale buffer
length before decoding, and `try_get` for optional pixel access. The rqrr adapter
returns `InvalidGridSize` for empty or malformed views before preparing an image.

`rxing::RxingDecoder::scan` returns each sampled candidate's success or error.
Successful `ScanSymbol` values retain original payload bytes, version, EC level,
and a checked Structured Append header. `QrDecoder::decode` is strict: any
retained candidate failure returns an error. `ScanOptions` controls input,
finder/candidate limits and explicit inverted polarity; no result is silently
truncated. Normal and Micro QR share one binarization and finder search.

The `rxing` feature uses a private QR engine derived from rxing 0.9.3. It retains
raw-byte handling and ECI metadata without text conversion. The external rxing
crate is not a dependency. Use `core::str::from_utf8` on decoded bytes when
appropriate, or transfer the buffer without copying through
`DecodedQrCode::into_data`.

The 2.2 scanner API exposes `rxing::BackendError` for engine failures. This type
belongs to `qrcode-decode` and has a different Rust type identity from the
external crate's `rxing::Exceptions`. Callers matching the experimental scanner
error variant must use `qrcode_decode::rxing::BackendError`.

The facade's `structured_append::reassemble_decoded` accepts borrowed selected
fragments and validates complete positions, matching headers and payload XOR.
Keep every independently scanned fragment; parity collisions do not identify
groups, and duplicates must not be silently removed.
