//! UTF-8 rendering, with various pixel densities.

use crate::render::{Canvas as RenderCanvas, Color, Pixel};

//{{{ Shared macro for bit-packed canvas

/// Generates a `Canvas` implementation for Unicode renderers that pack multiple
/// vertical pixels into a single `u8` cell. The `into_image` method processes
/// `ROW_GROUP` rows at a time with zero intermediate allocations.
macro_rules! impl_bit_canvas {
    ($canvas:ident, $pixel:ident, $row_group:expr, $col_step:expr, $encode:expr) => {
        #[doc(hidden)]
        pub struct $canvas {
            canvas: Vec<u8>,
            width: u32,
            dark_pixel: u8,
        }

        impl RenderCanvas for $canvas {
            type Pixel = $pixel;
            type Image = String;

            fn new(width: u32, height: u32, dark_pixel: $pixel, light_pixel: $pixel) -> Self {
                let a = vec![light_pixel.value(); (width * height) as usize];
                $canvas { width, canvas: a, dark_pixel: dark_pixel.value() }
            }

            fn draw_dark_pixel(&mut self, x: u32, y: u32) {
                self.canvas[(x + y * self.width) as usize] = self.dark_pixel;
            }

            fn into_image(self) -> String {
                let w = self.width as usize;
                let data = &self.canvas;
                let row_group = $row_group;
                let empty: &[u8] = &[];
                let rows: Vec<&[u8]> = data.chunks_exact(w).collect();
                let col_step: usize = $col_step;
                let mut out = String::with_capacity(rows.len() / row_group * (w / col_step + 1));

                for group in rows.chunks(row_group) {
                    let actual = group.len();
                    for col in (0..w).step_by(col_step) {
                        if actual == row_group {
                            out.push_str($encode(group, col));
                        } else {
                            let mut padded: [&[u8]; $row_group] = [empty; $row_group];
                            for i in 0..actual {
                                padded[i] = group[i];
                            }
                            out.push_str($encode(&padded, col));
                        }
                    }
                    out.push('\n');
                }
                if out.ends_with('\n') {
                    out.pop();
                }
                out
            }
        }
    };
}

//}}}
//{{{ Dense1x2 — half-block, 2 rows per character

const CODEPAGE: [&str; 4] = [" ", "\u{2584}", "\u{2580}", "\u{2588}"];

/// Unicode renderer packing 2 vertical pixels per character using half-block
/// elements (U+2580–U+2588). Use with `QrCode::render::<Dense1x2>()`.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Dense1x2 {
    /// A dark module.
    Dark,
    /// A light module.
    Light,
}

impl Pixel for Dense1x2 {
    type Image = String;
    type Canvas = Canvas1x2;
    fn default_unit_size() -> (u32, u32) {
        (1, 1)
    }
    fn default_color(color: Color) -> Dense1x2 {
        color.select(Dense1x2::Dark, Dense1x2::Light)
    }
}

impl Dense1x2 {
    const fn value(self) -> u8 {
        match self {
            Dense1x2::Dark => 1,
            Dense1x2::Light => 0,
        }
    }
}

fn encode_1x2(rows: &[&[u8]], col: usize) -> &'static str {
    let top = rows[0].get(col).copied().unwrap_or(0);
    let bot = rows[1].get(col).copied().unwrap_or(0);
    CODEPAGE[usize::from(top * 2 + bot)]
}

impl_bit_canvas!(Canvas1x2, Dense1x2, 2, 1, encode_1x2 as fn(&[&[u8]], usize) -> &'static str);

//}}}
//{{{ Dense2x2 — quadrant blocks (U+2596–U+259F), 2×2 per character

/// The 16 quadrant characters.
/// Bit layout: bit 0 = top-left, bit 1 = top-right, bit 2 = bottom-left, bit 3 = bottom-right.
const QUADRANT: [&str; 16] = [
    " ",        // 0b0000
    "\u{2598}", // 0b0001 top-left
    "\u{259D}", // 0b0010 top-right
    "\u{2580}", // 0b0011 top
    "\u{2596}", // 0b0100 bottom-left
    "\u{258C}", // 0b0101 left
    "\u{259E}", // 0b0110 anti-diagonal
    "\u{259B}", // 0b0111 all except bottom-right
    "\u{2597}", // 0b1000 bottom-right
    "\u{259A}", // 0b1001 diagonal
    "\u{2590}", // 0b1010 right
    "\u{259C}", // 0b1011 all except bottom-left
    "\u{2584}", // 0b1100 bottom
    "\u{2599}", // 0b1101 all except top-right
    "\u{259F}", // 0b1110 all except top-left
    "\u{2588}", // 0b1111 full
];

/// Unicode renderer packing a 2×2 block of pixels per character using
/// quadrant elements (U+2596–U+259F).
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Dense2x2 {
    /// A dark module.
    Dark,
    /// A light module.
    Light,
}

impl Pixel for Dense2x2 {
    type Image = String;
    type Canvas = Canvas2x2;
    fn default_unit_size() -> (u32, u32) {
        (1, 1)
    }
    fn default_color(color: Color) -> Dense2x2 {
        color.select(Dense2x2::Dark, Dense2x2::Light)
    }
}

impl Dense2x2 {
    const fn value(self) -> u8 {
        match self {
            Dense2x2::Dark => 1,
            Dense2x2::Light => 0,
        }
    }
}

fn encode_2x2(rows: &[&[u8]], col: usize) -> &'static str {
    let tl = rows[0][col] & 1;
    let tr = rows[0].get(col + 1).copied().unwrap_or(0) & 1;
    let bl = rows[1].get(col).copied().unwrap_or(0) & 1;
    let br = rows[1].get(col + 1).copied().unwrap_or(0) & 1;
    QUADRANT[(tl | (tr << 1) | (bl << 2) | (br << 3)) as usize]
}

impl_bit_canvas!(Canvas2x2, Dense2x2, 2, 2, encode_2x2 as fn(&[&[u8]], usize) -> &'static str);

//}}}
//{{{ Braille (U+2800–U+28FF), 2×4 dots per character

/// UTF-8 rendering using Braille characters (U+2800–U+28FF).
///
/// Each character encodes a 2×4 grid of 8 dots, yielding 8 pixels per
/// character — the highest density among Unicode renderers.
///
/// # Example
///
/// ```
/// use qrcode_rs::QrCode;
/// use qrcode_rs::render::unicode::Braille;
///
/// let code = QrCode::new(b"Hello").unwrap();
/// let text = code.render::<Braille>().module_dimensions(1, 1).build();
/// println!("{}", text);
/// ```
/// Unicode renderer packing a 2×4 block of pixels per character using Braille
/// patterns (U+2800–U+28FF) — the densest text output.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Braille {
    /// A dark module.
    Dark,
    /// A light module.
    Light,
}

impl Pixel for Braille {
    type Image = String;
    type Canvas = CanvasBraille;
    fn default_unit_size() -> (u32, u32) {
        (1, 1)
    }
    fn default_color(color: Color) -> Braille {
        color.select(Braille::Dark, Braille::Light)
    }
}

impl Braille {
    const fn value(self) -> u8 {
        match self {
            Braille::Dark => 1,
            Braille::Light => 0,
        }
    }
}

/// Precomputed UTF-8 encodings for all 256 Braille code points (U+2800–U+28FF).
const BRAILLE_UTF8: [[u8; 3]; 256] = {
    let mut table = [[0u8; 3]; 256];
    let mut i = 0usize;
    while i < 256 {
        let cp = 0x2800u32 + i as u32;
        table[i][0] = ((cp >> 12) & 0x0F) as u8 | 0xE0;
        table[i][1] = ((cp >> 6) & 0x3F) as u8 | 0x80;
        table[i][2] = (cp & 0x3F) as u8 | 0x80;
        i += 1;
    }
    table
};

fn encode_braille(rows: &[&[u8]], col: usize) -> &'static str {
    let d1 = rows[0].get(col).copied().unwrap_or(0) & 1;
    let d2 = rows[1].get(col).copied().unwrap_or(0) & 1;
    let d3 = rows[2].get(col).copied().unwrap_or(0) & 1;
    let d4 = rows[0].get(col + 1).copied().unwrap_or(0) & 1;
    let d5 = rows[1].get(col + 1).copied().unwrap_or(0) & 1;
    let d6 = rows[2].get(col + 1).copied().unwrap_or(0) & 1;
    let d7 = rows[3].get(col).copied().unwrap_or(0) & 1;
    let d8 = rows[3].get(col + 1).copied().unwrap_or(0) & 1;

    let bits = d1 | (d2 << 1) | (d3 << 2) | (d4 << 3) | (d5 << 4) | (d6 << 5) | (d7 << 6) | (d8 << 7);
    // SAFETY: BRAILLE_UTF8[bits] is valid UTF-8 for U+2800+bits.
    unsafe { std::str::from_utf8_unchecked(&BRAILLE_UTF8[bits as usize]) }
}

impl_bit_canvas!(CanvasBraille, Braille, 4, 2, encode_braille as fn(&[&[u8]], usize) -> &'static str);

//}}}
//{{{ Dense3x2 — sextant characters (U+1FB00–U+1FB3F), 3×2 per character

/// UTF-8 rendering using Unicode sextant characters (U+1FB00–U+1FB3F).
///
/// Each character encodes a 3×2 grid of 6 cells, yielding 6 pixels per
/// character — between Dense2x2 (4 px) and Braille (8 px) in density.
///
/// Bit layout (per Unicode sextant specification):
///
/// ```text
/// bit0 bit3     row0[col] row0[col+1]
/// bit1 bit4  =  row1[col] row1[col+1]
/// bit2 bit5     row2[col] row2[col+1]
/// ```
///
/// # Example
///
/// ```
/// use qrcode_rs::QrCode;
/// use qrcode_rs::render::unicode::Dense3x2;
///
/// let code = QrCode::new(b"Hello").unwrap();
/// let text = code.render::<Dense3x2>().module_dimensions(1, 1).build();
/// println!("{}", text);
/// ```
/// Unicode renderer packing a 3×2 block of pixels per character using
/// sextant elements (U+1FB00–U+1FB3F).
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Dense3x2 {
    /// A dark module.
    Dark,
    /// A light module.
    Light,
}

impl Pixel for Dense3x2 {
    type Image = String;
    type Canvas = Canvas3x2;
    fn default_unit_size() -> (u32, u32) {
        (1, 1)
    }
    fn default_color(color: Color) -> Dense3x2 {
        color.select(Dense3x2::Dark, Dense3x2::Light)
    }
}

impl Dense3x2 {
    const fn value(self) -> u8 {
        match self {
            Dense3x2::Dark => 1,
            Dense3x2::Light => 0,
        }
    }
}

/// Precomputed UTF-8 encodings for all 64 sextant code points (U+1FB00–U+1FB3F).
/// Each entry is 4 bytes (these are supplementary plane characters).
const SEXTANT_UTF8: [[u8; 4]; 64] = {
    let mut table = [[0u8; 4]; 64];
    let mut i = 0usize;
    while i < 64 {
        let cp = 0x1FB00u32 + i as u32;
        table[i][0] = 0xF0u8 | ((cp >> 18) & 0x07) as u8;
        table[i][1] = 0x80u8 | ((cp >> 12) & 0x3F) as u8;
        table[i][2] = 0x80u8 | ((cp >> 6) & 0x3F) as u8;
        table[i][3] = 0x80u8 | (cp & 0x3F) as u8;
        i += 1;
    }
    table
};

/// Encodes a 3×2 block of pixels into a sextant character.
/// Pattern 0 (all light) maps to ASCII space for visual consistency.
fn encode_3x2(rows: &[&[u8]], col: usize) -> &'static str {
    let d0 = rows[0].get(col).copied().unwrap_or(0) & 1;
    let d1 = rows[1].get(col).copied().unwrap_or(0) & 1;
    let d2 = rows[2].get(col).copied().unwrap_or(0) & 1;
    let d3 = rows[0].get(col + 1).copied().unwrap_or(0) & 1;
    let d4 = rows[1].get(col + 1).copied().unwrap_or(0) & 1;
    let d5 = rows[2].get(col + 1).copied().unwrap_or(0) & 1;

    let bits = d0 | (d1 << 1) | (d2 << 2) | (d3 << 3) | (d4 << 4) | (d5 << 5);
    if bits == 0 {
        " "
    } else {
        // SAFETY: SEXTANT_UTF8[bits] is valid UTF-8 for U+1FB00+bits (bits > 0).
        unsafe { std::str::from_utf8_unchecked(&SEXTANT_UTF8[bits as usize]) }
    }
}

impl_bit_canvas!(Canvas3x2, Dense3x2, 3, 2, encode_3x2 as fn(&[&[u8]], usize) -> &'static str);

//}}}

#[test]
fn test_render_to_utf8_string() {
    use crate::render::Renderer;
    let colors = &[Color::Dark, Color::Light, Color::Light, Color::Dark];
    let image: String = Renderer::<Dense1x2>::new(colors, 2, 1).build();

    assert_eq!(&image, " ▄  \n  ▀ ");

    let image2 = Renderer::<Dense1x2>::new(colors, 2, 1).module_dimensions(2, 2).build();

    assert_eq!(&image2, "        \n  ██    \n    ██  \n        ");
}

#[test]
fn integration_render_utf8_1x2() {
    use crate::render::unicode::Dense1x2;
    use crate::{EcLevel, QrCode, Version};

    let code = QrCode::with_version(b"09876542", Version::Micro(2), EcLevel::L).unwrap();
    let image = code.render::<Dense1x2>().module_dimensions(1, 1).build();
    assert_eq!(
        image,
        String::new()
            + "                 \n"
            + "  █▀▀▀▀▀█ ▀ █ ▀  \n"
            + "  █ ███ █  ▀ █   \n"
            + "  █ ▀▀▀ █  ▀█ █  \n"
            + "  ▀▀▀▀▀▀▀ ▄▀▀ █  \n"
            + "  ▀█ ▀▀▀▀▀██▀▀▄  \n"
            + "  ▀███▄ ▀▀ █ ██  \n"
            + "  ▀▀▀ ▀ ▀▀ ▀  ▀  \n"
            + "                 "
    );
}

#[test]
fn integration_render_utf8_1x2_inverted() {
    use crate::render::unicode::Dense1x2;
    use crate::{EcLevel, QrCode, Version};

    let code = QrCode::with_version(b"12345678", Version::Micro(2), EcLevel::L).unwrap();
    let image = code
        .render::<Dense1x2>()
        .dark_color(Dense1x2::Light)
        .light_color(Dense1x2::Dark)
        .module_dimensions(1, 1)
        .build();
    assert_eq!(
        image,
        "█████████████████\n\
         ██ ▄▄▄▄▄ █▄▀▄█▄██\n\
         ██ █   █ █   █ ██\n\
         ██ █▄▄▄█ █▄▄██▀██\n\
         ██▄▄▄▄▄▄▄█▄▄▄▀ ██\n\
         ██▄ ▀ ▀ ▀▄▄  ████\n\
         ██▄▄▀▄█ ▀▀▀ ▀▄▄██\n\
         ██▄▄▄█▄▄█▄██▄█▄██\n\
         ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀"
    );
}

#[test]
fn test_dense2x2_basic() {
    use crate::render::Renderer;
    let colors = &[Color::Dark, Color::Light, Color::Light, Color::Dark];
    let image: String = Renderer::<Dense2x2>::new(colors, 2, 0).module_dimensions(1, 1).build();
    assert_eq!(&image, "\u{259A}");
}

#[test]
fn test_dense2x2_with_quiet_zone() {
    use crate::render::Renderer;
    let colors = &[Color::Dark, Color::Light, Color::Light, Color::Dark];
    let image: String = Renderer::<Dense2x2>::new(colors, 2, 1).build();
    assert!(image.chars().count() >= 1);
}

#[test]
fn test_dense2x2_all_dark() {
    use crate::render::Renderer;
    let colors = vec![Color::Dark; 4];
    let image: String = Renderer::<Dense2x2>::new(&colors, 2, 0).module_dimensions(1, 1).build();
    assert_eq!(&image, "\u{2588}");
}

#[test]
fn test_dense2x2_all_light() {
    use crate::render::Renderer;
    let colors = vec![Color::Light; 4];
    let image: String = Renderer::<Dense2x2>::new(&colors, 2, 0).module_dimensions(1, 1).build();
    assert_eq!(&image, " ");
}

#[test]
fn integration_render_utf8_2x2() {
    use crate::render::unicode::Dense2x2;
    use crate::{EcLevel, QrCode, Version};

    let code = QrCode::with_version(b"09876542", Version::Micro(2), EcLevel::L).unwrap();
    let image = code.render::<Dense2x2>().module_dimensions(1, 1).build();
    assert!(!image.is_empty());
    let dense1x2 = code.render::<Dense1x2>().module_dimensions(1, 1).build();
    assert!(image.len() < dense1x2.len());
}

#[test]
fn test_braille_all_dark() {
    use crate::render::Renderer;
    let colors = vec![Color::Dark; 16];
    let image: String = Renderer::<Braille>::new(&colors, 4, 0).module_dimensions(1, 1).build();
    assert_eq!(&image, "\u{28FF}\u{28FF}");
}

#[test]
fn test_braille_all_light() {
    use crate::render::Renderer;
    let colors = vec![Color::Light; 16];
    let image: String = Renderer::<Braille>::new(&colors, 4, 0).module_dimensions(1, 1).build();
    assert_eq!(&image, "\u{2800}\u{2800}");
}

#[test]
fn test_braille_top_left_dot() {
    use crate::render::Renderer;
    let mut colors = vec![Color::Light; 16];
    colors[0] = Color::Dark;
    let image: String = Renderer::<Braille>::new(&colors, 4, 0).module_dimensions(1, 1).build();
    assert_eq!(&image, "\u{2801}\u{2800}");
}

#[test]
fn test_braille_density() {
    use crate::render::unicode::{Braille, Dense1x2};
    use crate::{EcLevel, QrCode, Version};

    let code = QrCode::with_version(b"09876542", Version::Micro(2), EcLevel::L).unwrap();
    let braille = code.render::<Braille>().module_dimensions(1, 1).build();
    let dense1x2 = code.render::<Dense1x2>().module_dimensions(1, 1).build();
    assert!(braille.len() < dense1x2.len());
}

#[test]
fn test_dense3x2_all_dark() {
    use crate::render::Renderer;
    // 6×6 all dark = 2 row groups × 3 cols = 6 full sextant chars (pattern 63 = U+1FB3F)
    let colors = vec![Color::Dark; 36];
    let image: String = Renderer::<Dense3x2>::new(&colors, 6, 0).module_dimensions(1, 1).build();
    assert_eq!(&image, "\u{1FB3F}\u{1FB3F}\u{1FB3F}\n\u{1FB3F}\u{1FB3F}\u{1FB3F}");
}

#[test]
fn test_dense3x2_all_light() {
    use crate::render::Renderer;
    let colors = vec![Color::Light; 36];
    let image: String = Renderer::<Dense3x2>::new(&colors, 6, 0).module_dimensions(1, 1).build();
    assert_eq!(&image, "   \n   ");
}

#[test]
fn test_dense3x2_top_left_cell() {
    use crate::render::Renderer;
    // 6×6 grid with only (0,0) dark → bit0 set → pattern 1 = U+1FB01
    let mut colors = vec![Color::Light; 36];
    colors[0] = Color::Dark;
    let image: String = Renderer::<Dense3x2>::new(&colors, 6, 0).module_dimensions(1, 1).build();
    // First char: U+1FB01, rest are spaces
    assert!(image.starts_with('\u{1FB01}'));
}

#[test]
fn test_dense3x2_density() {
    use crate::render::unicode::{Dense1x2, Dense3x2};
    use crate::{EcLevel, QrCode, Version};

    let code = QrCode::with_version(b"09876542", Version::Micro(2), EcLevel::L).unwrap();
    let sextant = code.render::<Dense3x2>().module_dimensions(1, 1).build();
    let dense1x2 = code.render::<Dense1x2>().module_dimensions(1, 1).build();
    // Sextant should be smaller due to higher density (3 rows per char vs 2).
    assert!(sextant.len() < dense1x2.len());
}
