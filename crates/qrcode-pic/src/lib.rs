//! PIC rendering support.
//!
//! # Example
//!
//! ```
//! use qrcode_core::Color as ModuleColor;
//! use qrcode_pic::Color;
//! use qrcode_render::Renderer;
//!
//! let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
//! let pic = Renderer::<Color>::new(&modules, 2, 1).build();
//! println!("{pic}");
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::{
    borrow::ToOwned,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use core::fmt::Write;

use qrcode_core::Color as ModuleColor;
use qrcode_render::{Canvas as RenderCanvas, Pixel};

/// A PIC color.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Color;

impl Pixel for Color {
    type Canvas = Canvas;
    type Image = String;

    fn default_color(_color: ModuleColor) -> Self {
        Self
    }
}

#[doc(hidden)]
pub struct Canvas {
    pic: String,
}

impl RenderCanvas for Canvas {
    type Pixel = Color;
    type Image = String;

    fn new(width: u32, height: u32, _dark_pixel: Color, _light_pixel: Color) -> Self {
        Self {
            pic: format!(
                concat!(
                    "maxpswid={w};maxpsht={h};movewid=0;moveht=1;boxwid=1;boxht=1\n",
                    "define p {{ box wid $3 ht $4 fill 1 thickness 0.1 with .nw at $1,-$2 }}\n",
                    "box wid maxpswid ht maxpsht with .nw at 0,0\n",
                ),
                w = width,
                h = height
            ),
        }
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        self.draw_dark_rect(x, y, 1, 1);
    }

    fn draw_dark_rect(&mut self, left: u32, top: u32, width: u32, height: u32) {
        writeln!(self.pic, "p({left},{top},{width},{height})").unwrap();
    }

    fn into_image(self) -> String {
        self.pic
    }
}

#[cfg(test)]
mod tests {
    use super::Color;

    #[test]
    fn pic_renderer_outputs_header_and_dark_rects() {
        let modules =
            [qrcode_core::Color::Dark, qrcode_core::Color::Light, qrcode_core::Color::Light, qrcode_core::Color::Dark];

        let pic =
            qrcode_render::Renderer::<Color>::new(&modules, 2, 1).quiet_zone(false).module_dimensions(1, 1).build();

        assert!(pic.starts_with("maxpswid=2;maxpsht=2;"));
        assert!(pic.contains("define p"));
        assert!(pic.contains("p(0,0,1,1)"));
        assert!(pic.contains("p(1,1,1,1)"));
    }
}
