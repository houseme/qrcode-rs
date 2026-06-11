//! ANSI terminal color rendering.
//!
//! Renders QR codes using 24-bit TrueColor ANSI escape codes with half-block
//! characters. Each character represents 2 vertical pixels with independent
//! foreground and background colors.
//!
//! # Example
//!
//! ```
//! use qrcode_rs::QrCode;
//! use qrcode_rs::render::ansi::Color;
//!
//! let code = QrCode::new(b"Hello").unwrap();
//! // Dark modules in black, light modules in white.
//! let text = code.render::<Color>().build();
//! println!("{}", text);
//!
//! // Custom colors: dark blue on light gray.
//! let text = code.render::<Color>()
//!     .dark_color(Color::new(0, 51, 102))
//!     .light_color(Color::new(224, 224, 224))
//!     .build();
//! println!("{}", text);
//! ```

use crate::render::{Canvas as RenderCanvas, Pixel};
use crate::types::Color as ModuleColor;

/// An ANSI TrueColor (24-bit) pixel.
///
/// Each `Color` stores an RGB value that will be rendered using ANSI escape
/// codes in the terminal.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Color {
    /// Creates a new ANSI color from RGB components.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// ANSI escape sequence for this color as a foreground color.
    fn fg_ansi(self) -> String {
        format!("\x1b[38;2;{};{};{}m", self.r, self.g, self.b)
    }

    /// ANSI escape sequence for this color as a background color.
    fn bg_ansi(self) -> String {
        format!("\x1b[48;2;{};{};{}m", self.r, self.g, self.b)
    }
}

impl Pixel for Color {
    type Image = String;
    type Canvas = CanvasAnsi;

    fn default_unit_size() -> (u32, u32) {
        (1, 1)
    }

    fn default_color(color: ModuleColor) -> Self {
        match color {
            ModuleColor::Dark => Color::new(0, 0, 0),
            ModuleColor::Light => Color::new(255, 255, 255),
        }
    }
}

/// Canvas for ANSI terminal rendering.
///
/// Uses Unicode half-block characters (▀ U+2580) where the foreground color
/// paints the top half and the background color paints the bottom half.
/// This yields 2 vertical pixels per character.
pub struct CanvasAnsi {
    canvas: Vec<u8>,
    width: u32,
    dark_pixel: u8,
    dark_color: Color,
    light_color: Color,
}

impl RenderCanvas for CanvasAnsi {
    type Pixel = Color;
    type Image = String;

    fn new(width: u32, height: u32, dark_pixel: Color, light_pixel: Color) -> Self {
        CanvasAnsi {
            canvas: vec![0u8; (width * height) as usize],
            width,
            dark_pixel: 1,
            dark_color: dark_pixel,
            light_color: light_pixel,
        }
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        self.canvas[(x + y * self.width) as usize] = self.dark_pixel;
    }

    fn into_image(self) -> String {
        let w = self.width as usize;
        let dark = 1u8;
        let reset = "\x1b[0m";

        self.canvas
            .chunks_exact(w)
            .collect::<Vec<&[u8]>>()
            .chunks(2)
            .map(|rows| {
                let top_row = rows[0];
                let bot_row = rows.get(1).map_or(&[][..], |r| *r);

                let mut line = String::with_capacity(w * 40);
                let mut last_fg = None;
                let mut last_bg = None;

                for col in 0..w {
                    let top = top_row.get(col).copied().unwrap_or(0);
                    let bot = bot_row.get(col).copied().unwrap_or(0);

                    let (fg, bg) = if top == dark && bot == dark {
                        (self.dark_color, self.dark_color)
                    } else if top == dark && bot != dark {
                        (self.dark_color, self.light_color)
                    } else if top != dark && bot == dark {
                        (self.light_color, self.dark_color)
                    } else {
                        (self.light_color, self.light_color)
                    };

                    // Only emit escape codes when colors change.
                    if last_bg != Some(bg) {
                        line.push_str(&bg.bg_ansi());
                        last_bg = Some(bg);
                    }
                    if last_fg != Some(fg) {
                        line.push_str(&fg.fg_ansi());
                        last_fg = Some(fg);
                    }

                    if top == dark && bot == dark {
                        line.push('█');
                    } else if top == dark {
                        line.push('▀');
                    } else if bot == dark {
                        line.push('▄');
                    } else {
                        line.push(' ');
                    }
                }

                line.push_str(reset);
                line
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderer;

    #[test]
    fn test_ansi_all_dark() {
        let colors = vec![ModuleColor::Dark; 4];
        let image: String = Renderer::<Color>::new(&colors, 2, 0).module_dimensions(1, 1).build();
        // Should contain the full-block character and ANSI codes.
        assert!(image.contains('█'));
        assert!(image.contains("\x1b["));
        assert!(image.contains("\x1b[0m"));
    }

    #[test]
    fn test_ansi_all_light() {
        let colors = vec![ModuleColor::Light; 4];
        let image: String = Renderer::<Color>::new(&colors, 2, 0).module_dimensions(1, 1).build();
        assert!(image.contains(' '));
        assert!(image.contains("\x1b[0m"));
    }

    #[test]
    fn test_ansi_mixed() {
        let colors = vec![ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
        let image: String = Renderer::<Color>::new(&colors, 2, 0).module_dimensions(1, 1).build();
        // Dark on top, light on bottom → '▀' with dark fg, light bg.
        assert!(image.contains('▀'));
    }

    #[test]
    fn test_ansi_custom_colors() {
        let code = crate::QrCode::new(b"Hi").unwrap();
        let image = code
            .render::<Color>()
            .dark_color(Color::new(0, 51, 102))
            .light_color(Color::new(224, 224, 224))
            .module_dimensions(1, 1)
            .build();
        // Should contain the custom RGB values.
        assert!(image.contains("0;51;102"));
        assert!(image.contains("224;224;224"));
    }

    #[test]
    fn test_ansi_color_optimization() {
        // Consecutive same-colored pixels should not emit redundant escape codes.
        let colors = vec![ModuleColor::Dark; 16]; // 4x4 all dark
        let image: String = Renderer::<Color>::new(&colors, 4, 0).module_dimensions(1, 1).build();
        let lines: Vec<&str> = image.split('\n').collect();
        assert_eq!(lines.len(), 2);
        // All '█' chars, same fg/bg — only 3 escape sequences per line (fg + bg + reset).
        for line in &lines {
            let esc_count = line.matches("\x1b[").count();
            assert_eq!(esc_count, 3);
        }
    }
}
