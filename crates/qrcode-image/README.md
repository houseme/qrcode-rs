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

For an existing borrowed module grid, the high-level helpers apply common
output settings and return fallible results:

```rust
use qrcode_core::{Color, ModuleView};
use qrcode_image::{encode_png, RenderOptions};

let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
let source = ModuleView::new(&modules, 2).expect("square module grid");
let png = encode_png(&source, RenderOptions::default().module_size(4, 4))?;
# Ok::<(), qrcode_image::EncodeError>(())
```

`render_rgba`, `render_luma`, and `render_dynamic` provide the corresponding
in-memory image forms. Dimension arithmetic is checked before allocating an
image, so oversized module or quiet-zone settings return an error.
Module dimensions must be non-zero; invalid options return
`ImageRenderError::InvalidModuleDimensions` instead of being silently
clamped. Options can also be checked ahead of time with
`RenderOptions::validate()`.

The `std` feature is available independently, matching the feature shape of
`qrcode-render`; `image` implies `std` and enables PNG/JPEG support.
