# qrcode-rs

A QR code and Micro QR code encoder library for Rust.

## Features

- **Standard QR Code** (Version 1-40) with all error correction levels (L, M, Q, H)
- **Micro QR Code** (Version M1-M4) for compact encoding
- **Multiple output formats**: PNG/JPEG (via `image` crate), SVG, EPS, PIC, Unicode, plain text
- **Automatic optimization**: selects the smallest version and optimal data mode segmentation
- **Customizable rendering**: colors, module dimensions, quiet zone, minimum/maximum size

## Quick Start

```rust
use qrcode_rs::QrCode;

// Encode data into a QR code
let code = QrCode::new(b"https://example.com").unwrap();

// Render as a string
let string = code.render()
    .dark_color('#')
    .light_color(' ')
    .build();
println!("{}", string);

// Render as an image (requires `image` feature)
// let image = code.render::<image::Luma<u8>>().build();
// image.save("qrcode.png").unwrap();
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `image` | Yes | PNG/JPEG rendering via the `image` crate |
| `svg`   | Yes | SVG vector rendering |
| `eps`   | Yes | Encapsulated PostScript rendering |
| `pic`   | Yes | PIC (troff) rendering |

## Modules

- [`qrcode_rs::QrCode`](https://docs.rs/qrcode-rs/latest/qrcode_rs/struct.QrCode.html) — Main entry point for encoding
- [`qrcode_rs::bits`](https://docs.rs/qrcode-rs/latest/qrcode_rs/bits/index.html) — Bit-level data encoding
- [`qrcode_rs::canvas`](https://docs.rs/qrcode-rs/latest/qrcode_rs/canvas/index.html) — QR code canvas and masking
- [`qrcode_rs::ec`](https://docs.rs/qrcode-rs/latest/qrcode_rs/ec/index.html) — Reed-Solomon error correction
- [`qrcode_rs::optimize`](https://docs.rs/qrcode-rs/latest/qrcode_rs/optimize/index.html) — Data mode segmentation optimizer
- [`qrcode_rs::render`](https://docs.rs/qrcode-rs/latest/qrcode_rs/render/index.html) — Rendering pipeline and Pixel trait

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0) or [MIT License](https://opensource.org/licenses/MIT) at your option.
