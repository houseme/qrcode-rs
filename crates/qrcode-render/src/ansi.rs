//! ANSI terminal color rendering.
//!
//! Renders QR codes using 24-bit TrueColor ANSI escape codes with half-block
//! characters. Each character represents 2 vertical pixels with independent
//! foreground and background colors.
//!
//! # Example
//!
//! ```
//! use qrcode_core::Color as ModuleColor;
//! use qrcode_render::{Renderer, ansi::Color};
//!
//! let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
//! // Dark modules in black, light modules in white.
//! let text = Renderer::<Color>::new(&modules, 2, 0).build();
//! println!("{}", text);
//!
//! // Custom colors: dark blue on light gray.
//! let text = Renderer::<Color>::new(&modules, 2, 0)
//!     .dark_color(Color::new(0, 51, 102))
//!     .light_color(Color::new(224, 224, 224))
//!     .build();
//! println!("{}", text);
//! ```

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::{
    borrow::ToOwned,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use crate::{Canvas as RenderCanvas, Pixel, RenderError, StyledPixel, check_buffer_size, checked_area};
use qrcode_core::Color as ModuleColor;

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

    fn escape_sequence(self, foreground: bool) -> EscapeSequence {
        let mut bytes = [0; 19];
        bytes[..7].copy_from_slice(if foreground { b"\x1b[38;2;" } else { b"\x1b[48;2;" });
        let mut len = 7;
        for component in [self.r, self.g, self.b] {
            if component >= 100 {
                bytes[len] = b'0' + component / 100;
                len += 1;
            }
            if component >= 10 {
                bytes[len] = b'0' + component / 10 % 10;
                len += 1;
            }
            bytes[len] = b'0' + component % 10;
            bytes[len + 1] = b';';
            len += 2;
        }
        bytes[len - 1] = b'm';
        EscapeSequence { bytes, len: len as u8 }
    }
}

struct EscapeSequence {
    bytes: [u8; 19],
    len: u8,
}

impl EscapeSequence {
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..usize::from(self.len)]).expect("ANSI escape sequences contain only ASCII")
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

impl StyledPixel for Color {
    fn from_hex(hex: &str) -> Self {
        let (r, g, b) = crate::colors::hex_to_rgb(hex).unwrap_or((0, 0, 0));
        Color::new(r, g, b)
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
    output_capacity: usize,
}

fn layout(width: u32, height: u32) -> Result<(usize, usize), RenderError> {
    let area = checked_area(width, height)?;
    if area == 0 {
        return Ok((0, 0));
    }
    let rows = (height as usize).div_ceil(2);
    // Two TrueColor escapes (19 bytes each), a UTF-8 block (3), and the row reset.
    let capacity = (width as usize)
        .checked_mul(41)
        .and_then(|bytes| bytes.checked_add(4))
        .and_then(|bytes| bytes.checked_mul(rows))
        .and_then(|bytes| bytes.checked_add(rows - 1))
        .ok_or(RenderError::OutputTooLarge)?;
    check_buffer_size(area.checked_add(capacity).ok_or(RenderError::OutputTooLarge)?)?;
    Ok((area, capacity))
}

impl RenderCanvas for CanvasAnsi {
    type Pixel = Color;
    type Image = String;

    fn new(width: u32, height: u32, dark_pixel: Color, light_pixel: Color) -> Self {
        let (area, output_capacity) = layout(width, height).unwrap_or_else(|error| panic!("{error}"));
        CanvasAnsi {
            canvas: vec![0u8; area],
            width,
            dark_pixel: 1,
            dark_color: dark_pixel,
            light_color: light_pixel,
            output_capacity,
        }
    }

    fn validate_dimensions(width: u32, height: u32, _dark: &Color, _light: &Color) -> Result<(), RenderError> {
        layout(width, height).map(|_| ())
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        self.canvas[x as usize + y as usize * self.width as usize] = self.dark_pixel;
    }

    fn into_image(self) -> String {
        let w = self.width as usize;
        if self.canvas.is_empty() {
            return String::new();
        }
        let dark = 1u8;
        let reset = "\x1b[0m";
        let row_count = self.canvas.len() / w;
        let mut out = String::with_capacity(self.output_capacity);
        // Both colors are fixed for this canvas. Encode each escape once on the
        // stack, preserving the existing heap-buffer budget.
        let dark_fg = self.dark_color.escape_sequence(true);
        let dark_bg = self.dark_color.escape_sequence(false);
        let light_fg = self.light_color.escape_sequence(true);
        let light_bg = self.light_color.escape_sequence(false);
        let foregrounds = [dark_fg.as_str(), light_fg.as_str()];
        let backgrounds = [dark_bg.as_str(), light_bg.as_str()];

        for group_start in (0..row_count).step_by(2) {
            if group_start > 0 {
                out.push('\n');
            }

            let top_start = group_start * w;
            let top_row = &self.canvas[top_start..top_start + w];
            let bot_row = if group_start + 1 < row_count {
                let bot_start = (group_start + 1) * w;
                &self.canvas[bot_start..bot_start + w]
            } else {
                &[][..]
            };

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

                if last_bg != Some(bg) {
                    out.push_str(backgrounds[usize::from(bot != dark)]);
                    last_bg = Some(bg);
                }
                if last_fg != Some(fg) {
                    out.push_str(foregrounds[usize::from(top != dark)]);
                    last_fg = Some(fg);
                }

                if top == dark && bot == dark {
                    out.push('█');
                } else if top == dark {
                    out.push('▀');
                } else if bot == dark {
                    out.push('▄');
                } else {
                    out.push(' ');
                }
            }

            out.push_str(reset);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Renderer;

    fn legacy_render(canvas: &[u8], width: usize, dark: Color, light: Color) -> String {
        use core::fmt::Write as _;

        if canvas.is_empty() {
            return String::new();
        }
        let rows = canvas.len() / width;
        let mut output = String::new();
        for y in (0..rows).step_by(2) {
            if y > 0 {
                output.push('\n');
            }
            let mut last_fg = None;
            let mut last_bg = None;
            for x in 0..width {
                let top = canvas[y * width + x] == 1;
                let bottom = y + 1 < rows && canvas[(y + 1) * width + x] == 1;
                let fg = if top { dark } else { light };
                let bg = if bottom { dark } else { light };
                if last_bg != Some(bg) {
                    write!(output, "\x1b[48;2;{};{};{}m", bg.r, bg.g, bg.b).unwrap();
                    last_bg = Some(bg);
                }
                if last_fg != Some(fg) {
                    write!(output, "\x1b[38;2;{};{};{}m", fg.r, fg.g, fg.b).unwrap();
                    last_fg = Some(fg);
                }
                output.push(match (top, bottom) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                });
            }
            output.push_str("\x1b[0m");
        }
        output
    }

    #[test]
    fn cached_escape_sequences_match_decimal_formatting_for_every_component_value() {
        for component in 0..=255 {
            for color in [Color::new(component, 0, 255), Color::new(0, component, 255), Color::new(0, 255, component)] {
                assert_eq!(
                    color.escape_sequence(true).as_str(),
                    format!("\x1b[38;2;{};{};{}m", color.r, color.g, color.b)
                );
                assert_eq!(
                    color.escape_sequence(false).as_str(),
                    format!("\x1b[48;2;{};{};{}m", color.r, color.g, color.b)
                );
            }
        }
    }

    #[test]
    fn cached_renderer_matches_independent_formatter_for_patterns_and_odd_rows() {
        let colors = [
            (Color::new(0, 0, 0), Color::new(255, 255, 255)),
            (Color::new(0, 9, 10), Color::new(99, 100, 255)),
            (Color::new(255, 100, 9), Color::new(10, 99, 0)),
            (Color::new(7, 128, 250), Color::new(7, 128, 250)),
        ];
        for width in [1, 2, 3, 7, 21] {
            for height in [1, 2, 3, 4, 7] {
                for (dark, light) in colors {
                    for pattern in 0..4 {
                        let mut canvas = CanvasAnsi::new(width, height, dark, light);
                        for y in 0..height {
                            for x in 0..width {
                                let is_dark = match pattern {
                                    0 => false,
                                    1 => true,
                                    2 => (x + y) % 2 == 0,
                                    _ => (x * 7 + y * 3) % 5 < 2,
                                };
                                if is_dark {
                                    canvas.draw_dark_pixel(x, y);
                                }
                            }
                        }
                        let expected = legacy_render(&canvas.canvas, width as usize, dark, light);
                        let capacity = canvas.output_capacity;
                        let actual = canvas.into_image();
                        assert_eq!(actual, expected, "width {width}, height {height}, pattern {pattern}");
                        assert!(actual.len() <= capacity);
                    }
                }
            }
        }
    }

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
        let colors = vec![ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
        let image = Renderer::<Color>::new(&colors, 2, 0)
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

    #[test]
    fn ansi_budget_counts_escape_sequences_and_row_reset() {
        let dark = Color::new(255, 254, 253);
        let light = Color::new(252, 251, 250);
        let width = ((crate::MAX_BUFFER_BYTES - 4) / 42) as u32;
        assert!(CanvasAnsi::validate_dimensions(width, 1, &dark, &light).is_ok());
        assert_eq!(CanvasAnsi::validate_dimensions(width + 1, 1, &dark, &light), Err(RenderError::OutputTooLarge));
        assert_eq!(CanvasAnsi::validate_dimensions(65_536, 65_536, &dark, &light), Err(RenderError::OutputTooLarge));
        let modules = [ModuleColor::Light];
        assert_eq!(
            Renderer::<Color>::new(&modules, 1, 0).module_dimensions(65_536, 65_536).try_build(),
            Err(RenderError::OutputTooLarge)
        );
    }

    #[test]
    fn ansi_empty_canvases_produce_empty_text() {
        for (width, height) in [(0, 0), (0, u32::MAX), (u32::MAX, 0)] {
            assert_eq!(CanvasAnsi::new(width, height, Color::new(0, 0, 0), Color::new(255, 255, 255)).into_image(), "");
        }
    }

    #[test]
    fn ansi_odd_rows_stay_within_estimated_output_bytes() {
        let mut canvas = CanvasAnsi::new(7, 3, Color::new(255, 254, 253), Color::new(252, 251, 250));
        for y in 0..3 {
            for x in 0..7 {
                if (x + y) % 2 == 0 {
                    canvas.draw_dark_pixel(x, y);
                }
            }
        }
        let capacity = canvas.output_capacity;
        let output = canvas.into_image();
        assert!(output.len() <= capacity);
        assert_eq!(output.lines().count(), 2);
        assert!(output.ends_with("\x1b[0m"));
    }
}
