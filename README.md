Qrcode-rs
===========

[![Build](https://github.com/houseme/qrcode-rs/workflows/Build/badge.svg)](https://github.com/houseme/qrcode-rs/actions?query=workflow%3ABuild)
[![crates.io](https://img.shields.io/crates/v/qrcode-rs.svg)](https://crates.io/crates/qrcode-rs)
[![docs.rs](https://docs.rs/qrcode-rs/badge.svg)](https://docs.rs/qrcode-rs/)
[![License](https://img.shields.io/crates/l/qrcode-rs)](./LICENSE-APACHE.txt)
[![Crates.io](https://img.shields.io/crates/d/qrcode-rs)](https://crates.io/crates/qrcode-rs)

QR code and Micro QR code encoder in Rust. [Documentation](https://docs.rs/qrcode-rs).


Cargo.toml
----------

```toml
[dependencies]
qrcode-rs = "0.1"
```

The default settings will depend on the `image` crate. If you don't need image generation capability, disable the `default-features`:

```toml
[dependencies]
qrcode-rs = { version = "0.1", default-features = false }
```

Example
-------

## Image generation

```rust
use qrcode_rs::QrCode;
use image::Luma;

fn main() {
    // Encode some data into bits.
    let code = QrCode::new(b"01234567").unwrap();

    // Render the bits into an image.
    let image = code.render::<Luma<u8>>().build();

    // Save the image.
    image.save("/tmp/qrcode.png").unwrap();
}
```

Generates this image:

![Output](docs/images/test_annex_i_qr_as_image.png)

## String generation

```rust
use qrcode_rs::QrCode;

fn main() {
    let code = QrCode::new(b"Hello").unwrap();
    let string = code.render::<char>()
        .quiet_zone(false)
        .module_dimensions(2, 1)
        .build();
    println!("{}", string);
}
```

Generates this output:

```none
##############    ########  ##############
##          ##          ##  ##          ##
##  ######  ##  ##  ##  ##  ##  ######  ##
##  ######  ##  ##  ##      ##  ######  ##
##  ######  ##  ####    ##  ##  ######  ##
##          ##  ####  ##    ##          ##
##############  ##  ##  ##  ##############
                ##  ##
##  ##########    ##  ##    ##########
      ##        ##    ########    ####  ##
    ##########    ####  ##  ####  ######
    ##    ##  ####  ##########    ####
  ######    ##########  ##    ##        ##
                ##      ##    ##  ##
##############    ##  ##  ##    ##  ####
##          ##  ##  ##        ##########
##  ######  ##  ##    ##  ##    ##    ##
##  ######  ##  ####  ##########  ##
##  ######  ##  ####    ##  ####    ##
##          ##    ##  ########  ######
##############  ####    ##      ##    ##
```

## SVG generation

```rust
use qrcode_rs::{QrCode, Version, EcLevel};
use qrcode_rs::render::svg;

fn main() {
    let code = QrCode::with_version(b"01234567", Version::Micro(2), EcLevel::L).unwrap();
    let image = code.render()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#800000"))
        .light_color(svg::Color("#ffff80"))
        .build();
    println!("{}", image);
}
```

Generates this SVG:

[![Output](docs/images/test_annex_i_micro_qr_as_svg.svg)](docs/images/test_annex_i_micro_qr_as_svg.svg)

## Unicode string generation

```rust
use qrcode_rs::QrCode;
use qrcode_rs::render::unicode;

fn main() {
    let code = QrCode::new("mow mow").unwrap();
    let image = code.render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Light)
        .light_color(unicode::Dense1x2::Dark)
        .build();
    println!("{}", image);
}
```

Generates this output:

```text
█████████████████████████████
█████████████████████████████
████ ▄▄▄▄▄ █ ▀▀▀▄█ ▄▄▄▄▄ ████
████ █   █ █▀ ▀ ▀█ █   █ ████
████ █▄▄▄█ ██▄  ▀█ █▄▄▄█ ████
████▄▄▄▄▄▄▄█ ▀▄▀ █▄▄▄▄▄▄▄████
████▄▀ ▄▀ ▄ █▄█  ▀ ▀█ █▄ ████
████▄██▄▄▀▄▄▀█▄ ██▀▀█▀▄▄▄████
█████▄▄▄█▄▄█  ▀▀▄█▀▀▀▄█▄▄████
████ ▄▄▄▄▄ █   ▄▄██▄ ▄ ▀▀████
████ █   █ █▀▄▄▀▄▄ ▄▄▄▄ ▄████
████ █▄▄▄█ █▄  █▄▀▄▀██▄█▀████
████▄▄▄▄▄▄▄█▄████▄█▄██▄██████
█████████████████████████████
▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀
```

## License
Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE.txt) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT.txt) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.


### Thanks to [Kennytm](https://github.com/kennytm), this package is based on [`qrcode-rust`](https://github.com/kennytm/qrcode-rust).