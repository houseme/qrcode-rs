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

Select table or CSS Grid through the normal renderer builder:

```rust
use qrcode_rs::{QrCode, render::html::{Color, GridColor}};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let code = QrCode::new("https://example.com")?;
    let table = code.render::<Color>().try_build()?;
    let grid = code.render::<GridColor>()
        .dark_color(GridColor("#123456"))
        .light_color(GridColor("#ffffff"))
        .module_dimensions(2, 3)
        .quiet_zone(true)
        .try_build()?;
    println!("{table}\n{grid}");
    Ok(())
}
```

`Color` retains the default table output. `GridColor` selects `Mode::Grid`
without constructing a canvas manually. Both use the same escaping and output
budget checks, and both accept the shared renderer's geometry options.

With direct `qrcode-core` and `qrcode-render` dependencies, a borrowed source
works through the same entry point:

```rust
use qrcode_core::{Color as ModuleColor, ModuleView};
use qrcode_html::GridColor;
use qrcode_render::Renderer;

fn render_grid() -> Result<String, qrcode_render::RenderError> {
    let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
    let source = ModuleView::new(&modules, 2).expect("the four modules form a 2x2 grid");
    Renderer::<GridColor>::try_from_source(&source, 1)?.try_build()
}
```

Source grids must be non-empty and square. The low-level `Canvas::set_mode`
API remains available for callers that already draw their own pixels.

## Features

| Feature | Purpose |
| --- | --- |
| `std` | Opts into the standard library. Disabled by default. |

Use this crate directly when you need only HTML rendering plus the shared
core/render contracts.
