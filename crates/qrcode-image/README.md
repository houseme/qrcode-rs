# qrcode-image

`qrcode-image` is the image backend for [`qrcode-rs`](https://crates.io/crates/qrcode-rs).
It exposes the image pixel implementations, PNG/JPEG encoding helper, logo
overlay, and gradient background helper from the shared `qrcode-render`
implementation.

The crate has no default features. Enable `image` for raster rendering:

```toml
[dependencies]
qrcode-core = "2.0"
qrcode-image = { version = "2.0", features = ["image"] }
```

```rust
use qrcode_core::Color;
use qrcode_image::{Luma, Renderer};

let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
let rendered = Renderer::<Luma<u8>>::new(&modules, 2, 1).build();
assert_eq!(rendered.dimensions(), (32, 32));
```

The `std` feature is available independently, matching the feature shape of
`qrcode-render`; `image` implies `std` and enables PNG/JPEG support.
