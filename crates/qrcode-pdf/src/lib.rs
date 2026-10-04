//! PDF rendering support.
//!
//! # Example
//!
//! ```
//! use qrcode_core::Color as ModuleColor;
//! use qrcode_pdf::Color;
//! use qrcode_render::Renderer;
//!
//! let modules = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
//! let pdf_data = Renderer::<Color>::new(&modules, 2, 1).build();
//! println!("PDF size: {} bytes", pdf_data.len());
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
use qrcode_render::colors::{CmykColor as SharedCmykColor, ColorSpace, RgbColor};
use qrcode_render::{Canvas as RenderCanvas, Pixel, StyledPixel};

const MAX_STREAM_PREALLOC: usize = 8 * 1024 * 1024;

fn stream_capacity(width: u32, height: u32, bytes_per_rect: usize) -> usize {
    (width as usize).saturating_mul(height as usize).saturating_mul(bytes_per_rect).min(MAX_STREAM_PREALLOC)
}

fn normalized_component(value: f64) -> f64 {
    if value.is_nan() || value < 0.0 {
        0.0
    } else if value > 1.0 {
        1.0
    } else {
        value
    }
}

/// A PDF color (`[R, G, B]`).
///
/// Rendering clips components to `0.0..=1.0`, including infinities. NaN is
/// rendered as `0.0`. Values already in range, including signed zero, are kept.
#[derive(Copy, Clone, Default, PartialEq, PartialOrd)]
pub struct Color(pub [f64; 3]);

/// A PDF CMYK color (`[C, M, Y, K]`).
///
/// Rendering uses the same component normalization as [`Color`].
#[derive(Copy, Clone, Default, PartialEq, PartialOrd)]
pub struct CmykColor(pub [f64; 4]);

impl Pixel for Color {
    type Canvas = Canvas;
    type Image = Vec<u8>;

    fn default_color(color: ModuleColor) -> Self {
        Self(color.select(Default::default(), [1.0; 3]))
    }
}

impl StyledPixel for Color {
    fn from_hex(hex: &str) -> Self {
        let (r, g, b) = qrcode_render::colors::hex_to_rgb(hex).unwrap_or((0, 0, 0));
        Self([r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0])
    }
}

impl From<RgbColor> for Color {
    fn from(color: RgbColor) -> Self {
        Self(color.to_array())
    }
}

impl Pixel for CmykColor {
    type Canvas = CmykCanvas;
    type Image = Vec<u8>;

    fn default_color(color: ModuleColor) -> Self {
        Self(color.select([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 0.0]))
    }
}

impl StyledPixel for CmykColor {
    fn from_hex(hex: &str) -> Self {
        let (r, g, b) = qrcode_render::colors::hex_to_rgb(hex).unwrap_or((0, 0, 0));
        SharedCmykColor::from_rgb(RgbColor::new(r, g, b)).into()
    }
}

impl From<SharedCmykColor> for CmykColor {
    fn from(color: SharedCmykColor) -> Self {
        Self(color.to_array())
    }
}

#[doc(hidden)]
pub struct Canvas {
    stream: String,
    width: u32,
    height: u32,
    fg_r: f64,
    fg_g: f64,
    fg_b: f64,
    foreground_prefix: Option<String>,
    has_flushed: bool,
    pending_left: u32,
    pending_bottom: u32,
    pending_width: u32,
    pending_height: u32,
    has_pending: bool,
}

#[doc(hidden)]
pub struct CmykCanvas {
    stream: String,
    width: u32,
    height: u32,
    fg_c: f64,
    fg_m: f64,
    fg_y: f64,
    fg_k: f64,
    foreground_prefix: Option<String>,
    has_flushed: bool,
    pending_left: u32,
    pending_bottom: u32,
    pending_width: u32,
    pending_height: u32,
    has_pending: bool,
}

impl Canvas {
    fn flush_pending(&mut self) {
        if self.has_pending {
            if self.has_flushed {
                let prefix = self
                    .foreground_prefix
                    .get_or_insert_with(|| format!("{} {} {} rg ", self.fg_r, self.fg_g, self.fg_b));
                self.stream.push_str(prefix);
                writeln!(
                    self.stream,
                    "{} {} {} {} re f",
                    self.pending_left, self.pending_bottom, self.pending_width, self.pending_height
                )
                .unwrap();
            } else {
                // Empty and single-rectangle canvases need no prefix allocation.
                writeln!(
                    self.stream,
                    "{} {} {} rg {} {} {} {} re f",
                    self.fg_r,
                    self.fg_g,
                    self.fg_b,
                    self.pending_left,
                    self.pending_bottom,
                    self.pending_width,
                    self.pending_height
                )
                .unwrap();
                self.has_flushed = true;
            }
            self.has_pending = false;
        }
    }
}

impl RenderCanvas for Canvas {
    type Pixel = Color;
    type Image = Vec<u8>;

    fn new(width: u32, height: u32, dark_pixel: Color, light_pixel: Color) -> Self {
        let dark_pixel = Color(dark_pixel.0.map(normalized_component));
        let light_pixel = Color(light_pixel.0.map(normalized_component));
        let mut stream = String::with_capacity(stream_capacity(width, height, 48));
        writeln!(stream, "{} {} {} rg 0 0 {width} {height} re f", light_pixel.0[0], light_pixel.0[1], light_pixel.0[2])
            .unwrap();
        Canvas {
            stream,
            width,
            height,
            fg_r: dark_pixel.0[0],
            fg_g: dark_pixel.0[1],
            fg_b: dark_pixel.0[2],
            foreground_prefix: None,
            has_flushed: false,
            pending_left: 0,
            pending_bottom: 0,
            pending_width: 0,
            pending_height: 0,
            has_pending: false,
        }
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        self.draw_dark_rect(x, y, 1, 1);
    }

    fn draw_dark_rect(&mut self, left: u32, top: u32, width: u32, height: u32) {
        let bottom = self.height - top - height;
        if self.has_pending
            && bottom == self.pending_bottom
            && height == self.pending_height
            && left == self.pending_left + self.pending_width
        {
            self.pending_width += width;
        } else {
            self.flush_pending();
            self.pending_left = left;
            self.pending_bottom = bottom;
            self.pending_width = width;
            self.pending_height = height;
            self.has_pending = true;
        }
    }

    fn into_image(mut self) -> Vec<u8> {
        self.flush_pending();

        let w = self.width;
        let h = self.height;
        let stream_content = &self.stream;
        let stream_len = stream_content.len();

        let mut obj_offsets = Vec::with_capacity(5);
        let mut pos: usize = 0;

        // Header
        let header = "%PDF-1.4\n";
        pos += header.len();

        // Object 1: Catalog
        obj_offsets.push(pos);
        let obj1 = "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n";
        pos += obj1.len();

        // Object 2: Pages
        obj_offsets.push(pos);
        let obj2 = "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n";
        pos += obj2.len();

        // Object 3: Page
        obj_offsets.push(pos);
        let mut obj3 = String::new();
        write!(
            &mut obj3,
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] /Contents 4 0 R /Resources << >> >>\nendobj\n",
        )
        .unwrap();
        pos += obj3.len();

        // Object 4: Content stream
        obj_offsets.push(pos);
        let mut obj4_head = String::new();
        write!(&mut obj4_head, "4 0 obj\n<< /Length {stream_len} >>\nstream\n").unwrap();
        pos += obj4_head.len();
        pos += stream_len;
        let obj4_tail = "\nendstream\nendobj\n";
        pos += obj4_tail.len();

        // Cross-reference table
        let xref_pos = pos;
        let mut xref = String::new();
        xref.push_str("xref\n0 5\n");
        xref.push_str("0000000000 65535 f \n");
        for &off in &obj_offsets {
            writeln!(&mut xref, "{off:010} 00000 n ").unwrap();
        }

        // Trailer
        let mut trailer = String::new();
        write!(&mut trailer, "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n",).unwrap();

        // Assemble PDF
        let total = pos + xref.len() + trailer.len();
        let mut pdf = Vec::with_capacity(total);
        pdf.extend_from_slice(header.as_bytes());
        pdf.extend_from_slice(obj1.as_bytes());
        pdf.extend_from_slice(obj2.as_bytes());
        pdf.extend_from_slice(obj3.as_bytes());
        pdf.extend_from_slice(obj4_head.as_bytes());
        pdf.extend_from_slice(stream_content.as_bytes());
        pdf.extend_from_slice(obj4_tail.as_bytes());
        pdf.extend_from_slice(xref.as_bytes());
        pdf.extend_from_slice(trailer.as_bytes());
        pdf
    }
}

impl CmykCanvas {
    fn flush_pending(&mut self) {
        if self.has_pending {
            if self.has_flushed {
                let prefix = self
                    .foreground_prefix
                    .get_or_insert_with(|| format!("{} {} {} {} k ", self.fg_c, self.fg_m, self.fg_y, self.fg_k));
                self.stream.push_str(prefix);
                writeln!(
                    self.stream,
                    "{} {} {} {} re f",
                    self.pending_left, self.pending_bottom, self.pending_width, self.pending_height
                )
                .unwrap();
            } else {
                writeln!(
                    self.stream,
                    "{} {} {} {} k {} {} {} {} re f",
                    self.fg_c,
                    self.fg_m,
                    self.fg_y,
                    self.fg_k,
                    self.pending_left,
                    self.pending_bottom,
                    self.pending_width,
                    self.pending_height
                )
                .unwrap();
                self.has_flushed = true;
            }
            self.has_pending = false;
        }
    }
}

impl RenderCanvas for CmykCanvas {
    type Pixel = CmykColor;
    type Image = Vec<u8>;

    fn new(width: u32, height: u32, dark_pixel: CmykColor, light_pixel: CmykColor) -> Self {
        let dark_pixel = CmykColor(dark_pixel.0.map(normalized_component));
        let light_pixel = CmykColor(light_pixel.0.map(normalized_component));
        let mut stream = String::with_capacity(stream_capacity(width, height, 56));
        writeln!(
            stream,
            "{} {} {} {} k 0 0 {width} {height} re f",
            light_pixel.0[0], light_pixel.0[1], light_pixel.0[2], light_pixel.0[3]
        )
        .unwrap();
        CmykCanvas {
            stream,
            width,
            height,
            fg_c: dark_pixel.0[0],
            fg_m: dark_pixel.0[1],
            fg_y: dark_pixel.0[2],
            fg_k: dark_pixel.0[3],
            foreground_prefix: None,
            has_flushed: false,
            pending_left: 0,
            pending_bottom: 0,
            pending_width: 0,
            pending_height: 0,
            has_pending: false,
        }
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        self.draw_dark_rect(x, y, 1, 1);
    }

    fn draw_dark_rect(&mut self, left: u32, top: u32, width: u32, height: u32) {
        let bottom = self.height - top - height;
        if self.has_pending
            && bottom == self.pending_bottom
            && height == self.pending_height
            && left == self.pending_left + self.pending_width
        {
            self.pending_width += width;
        } else {
            self.flush_pending();
            self.pending_left = left;
            self.pending_bottom = bottom;
            self.pending_width = width;
            self.pending_height = height;
            self.has_pending = true;
        }
    }

    fn into_image(mut self) -> Vec<u8> {
        self.flush_pending();

        let w = self.width;
        let h = self.height;
        let stream_content = &self.stream;
        let stream_len = stream_content.len();

        let mut obj_offsets = Vec::with_capacity(5);
        let mut pos: usize = 0;

        let header = "%PDF-1.4\n";
        pos += header.len();

        obj_offsets.push(pos);
        let obj1 = "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n";
        pos += obj1.len();

        obj_offsets.push(pos);
        let obj2 = "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n";
        pos += obj2.len();

        obj_offsets.push(pos);
        let mut obj3 = String::new();
        write!(
            &mut obj3,
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] /Contents 4 0 R /Resources << >> >>\nendobj\n",
        )
        .unwrap();
        pos += obj3.len();

        obj_offsets.push(pos);
        let mut obj4_head = String::new();
        write!(&mut obj4_head, "4 0 obj\n<< /Length {stream_len} >>\nstream\n").unwrap();
        pos += obj4_head.len();
        pos += stream_len;
        let obj4_tail = "\nendstream\nendobj\n";
        pos += obj4_tail.len();

        let xref_pos = pos;
        let mut xref = String::new();
        xref.push_str("xref\n0 5\n");
        xref.push_str("0000000000 65535 f \n");
        for &off in &obj_offsets {
            writeln!(&mut xref, "{off:010} 00000 n ").unwrap();
        }

        let mut trailer = String::new();
        write!(&mut trailer, "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n",).unwrap();

        let total = pos + xref.len() + trailer.len();
        let mut pdf = Vec::with_capacity(total);
        pdf.extend_from_slice(header.as_bytes());
        pdf.extend_from_slice(obj1.as_bytes());
        pdf.extend_from_slice(obj2.as_bytes());
        pdf.extend_from_slice(obj3.as_bytes());
        pdf.extend_from_slice(obj4_head.as_bytes());
        pdf.extend_from_slice(stream_content.as_bytes());
        pdf.extend_from_slice(obj4_tail.as_bytes());
        pdf.extend_from_slice(xref.as_bytes());
        pdf.extend_from_slice(trailer.as_bytes());
        pdf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qrcode_core::Color as ModuleColor;
    use qrcode_render::{Renderer, StyledPixel};

    fn content_stream(pdf: &[u8]) -> &str {
        core::str::from_utf8(pdf).unwrap().split_once("stream\n").unwrap().1.split_once("\nendstream").unwrap().0
    }

    fn legacy_stream<const N: usize>(
        width: u32,
        height: u32,
        dark: [f64; N],
        light: [f64; N],
        rects: &[(u32, u32, u32, u32)],
    ) -> String {
        fn emit<const N: usize>(out: &mut String, color: [f64; N], rect: (u32, u32, u32, u32)) {
            let (left, bottom, width, height) = rect;
            match N {
                3 => writeln!(out, "{} {} {} rg {left} {bottom} {width} {height} re f", color[0], color[1], color[2]),
                4 => writeln!(
                    out,
                    "{} {} {} {} k {left} {bottom} {width} {height} re f",
                    color[0], color[1], color[2], color[3]
                ),
                _ => unreachable!(),
            }
            .unwrap();
        }
        let mut output = String::new();
        emit(&mut output, light, (0, 0, width, height));
        let mut pending: Option<(u32, u32, u32, u32)> = None;
        for &(left, top, width, rect_height) in rects {
            let bottom = height - top - rect_height;
            if let Some((old_left, old_bottom, old_width, old_height)) = pending {
                if old_bottom == bottom && old_height == rect_height && left == old_left + old_width {
                    pending = Some((old_left, bottom, old_width + width, rect_height));
                    continue;
                }
                emit(&mut output, dark, (old_left, old_bottom, old_width, old_height));
            }
            pending = Some((left, bottom, width, rect_height));
        }
        if let Some(rect) = pending {
            emit(&mut output, dark, rect);
        }
        output
    }

    // Build the original four-object document in one buffer, recording actual
    // offsets as objects are appended rather than sharing production assembly.
    fn legacy_pdf(width: u32, height: u32, stream: &str) -> Vec<u8> {
        let objects = [
            "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".into(),
            "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".into(),
            format!(
                "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Contents 4 0 R /Resources << >> >>\nendobj\n"
            ),
            format!("4 0 obj\n<< /Length {} >>\nstream\n{stream}\nendstream\nendobj\n", stream.len()),
        ];
        let mut output = String::from("%PDF-1.4\n");
        let mut offsets = [0; 4];
        for (index, object) in objects.iter().enumerate() {
            offsets[index] = output.len();
            output.push_str(object);
        }
        let xref = output.len();
        output.push_str("xref\n0 5\n0000000000 65535 f \n");
        for offset in offsets {
            writeln!(output, "{offset:010} 00000 n ").unwrap();
        }
        write!(output, "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").unwrap();
        output.into_bytes()
    }

    #[test]
    fn lazy_rgb_and_cmyk_prefixes_keep_complete_legacy_pdf_bytes() {
        let rects = [(1, 2, 3, 4), (4, 2, 2, 4), (10, 2, 2, 4), (10, 8, 2, 3), (0, 0, 1, 1), (31, 23, 1, 1)];
        for count in 0..=rects.len() {
            for (dark, light) in [([0.0; 3], [1.0; 3]), ([-0.0, 0.12345, 1.0], [0.25, 0.5, 0.75])] {
                let mut canvas = Canvas::new(32, 24, Color(dark), Color(light));
                for &(left, top, width, height) in &rects[..count] {
                    canvas.draw_dark_rect(left, top, width, height);
                }
                let expected = legacy_pdf(32, 24, &legacy_stream(32, 24, dark, light, &rects[..count]));
                assert_eq!(canvas.into_image(), expected, "RGB rects {count}");
            }
            for (dark, light) in
                [([0.0, 0.0, 0.0, 1.0], [0.0; 4]), ([-0.0, 0.12345, 0.25, 0.5], [0.75, 0.5, 0.25, 0.0])]
            {
                let mut canvas = CmykCanvas::new(32, 24, CmykColor(dark), CmykColor(light));
                for &(left, top, width, height) in &rects[..count] {
                    canvas.draw_dark_rect(left, top, width, height);
                }
                let expected = legacy_pdf(32, 24, &legacy_stream(32, 24, dark, light, &rects[..count]));
                assert_eq!(canvas.into_image(), expected, "CMYK rects {count}");
            }
        }
    }

    #[test]
    fn lazy_prefixes_start_only_after_a_second_distinct_flush() {
        let mut rgb = Canvas::new(8, 2, Color([0.2, 0.4, 0.6]), Color([1.0; 3]));
        rgb.flush_pending();
        assert!(rgb.foreground_prefix.is_none() && !rgb.has_flushed);
        rgb.draw_dark_rect(0, 0, 1, 1);
        rgb.draw_dark_rect(1, 0, 1, 1);
        rgb.flush_pending();
        assert!(rgb.foreground_prefix.is_none() && rgb.has_flushed);
        rgb.draw_dark_rect(3, 0, 1, 1);
        rgb.flush_pending();
        assert_eq!(rgb.foreground_prefix.as_deref(), Some("0.2 0.4 0.6 rg "));

        let mut cmyk = CmykCanvas::new(8, 2, CmykColor([0.1, 0.2, 0.3, 0.4]), CmykColor([0.0; 4]));
        cmyk.flush_pending();
        assert!(cmyk.foreground_prefix.is_none() && !cmyk.has_flushed);
        cmyk.draw_dark_rect(0, 0, 1, 1);
        cmyk.draw_dark_rect(1, 0, 1, 1);
        cmyk.flush_pending();
        assert!(cmyk.foreground_prefix.is_none() && cmyk.has_flushed);
        cmyk.draw_dark_rect(3, 0, 1, 1);
        cmyk.flush_pending();
        assert_eq!(cmyk.foreground_prefix.as_deref(), Some("0.1 0.2 0.3 0.4 k "));
    }

    #[test]
    fn test_pdf_header() {
        let colors = vec![ModuleColor::Dark; 4];
        let pdf: Vec<u8> = Renderer::<Color>::new(&colors, 2, 0).module_dimensions(1, 1).build();
        assert!(pdf.starts_with(b"%PDF-1.4\n"));
        assert!(pdf.ends_with(b"%%EOF\n"));
    }

    #[test]
    fn test_pdf_contains_rect() {
        let colors = vec![ModuleColor::Dark; 4];
        let pdf: Vec<u8> = Renderer::<Color>::new(&colors, 2, 0).module_dimensions(1, 1).build();
        let content = String::from_utf8_lossy(&pdf);
        assert!(content.contains(" re f"));
        assert!(content.contains(" rg"));
    }

    #[test]
    fn test_pdf_xref() {
        let colors = vec![ModuleColor::Light, ModuleColor::Dark, ModuleColor::Dark, ModuleColor::Light];
        let pdf: Vec<u8> = Renderer::<Color>::new(&colors, 2, 1).module_dimensions(1, 1).build();
        let content = String::from_utf8_lossy(&pdf);
        assert!(content.contains("xref"));
        assert!(content.contains("trailer"));
        assert!(content.contains("startxref"));
    }

    #[test]
    fn test_pdf_empty_rects() {
        let colors = vec![ModuleColor::Light; 4];
        let pdf: Vec<u8> = Renderer::<Color>::new(&colors, 2, 0).module_dimensions(1, 1).build();
        // All-light produces no dark rects, but PDF is still valid.
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        assert_eq!(content_stream(&pdf), "1 1 1 rg 0 0 2 2 re f\n");
    }

    #[test]
    fn test_pdf_color_parses_hex_for_templates() {
        let color = Color::from_hex("#336699");

        assert!((color.0[0] - 0.2).abs() < f64::EPSILON);
        assert!((color.0[1] - 0.4).abs() < f64::EPSILON);
        assert!((color.0[2] - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pdf_cmyk_renderer_outputs_native_cmyk_operator() {
        let colors = vec![ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
        let pdf: Vec<u8> = Renderer::<CmykColor>::new(&colors, 2, 0)
            .dark_color(CmykColor([1.0, 0.0, 0.0, 0.25]))
            .module_dimensions(1, 1)
            .build();
        let content = String::from_utf8_lossy(&pdf);

        assert!(content.contains("1 0 0 0.25 k"));
        assert!(!content.contains(" rg"));
    }

    #[test]
    fn pdf_rgb_background_covers_light_modules_and_quiet_zone_before_dark_rectangles() {
        let colors = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
        let pdf = Renderer::<Color>::new(&colors, 2, 1)
            .dark_color(Color([1.0, 0.0, 0.0]))
            .light_color(Color([0.25, 0.5, 0.75]))
            .module_dimensions(2, 3)
            .build();

        assert_eq!(
            content_stream(&pdf),
            "0.25 0.5 0.75 rg 0 0 8 12 re f\n1 0 0 rg 2 6 2 3 re f\n1 0 0 rg 4 3 2 3 re f\n"
        );
    }

    #[test]
    fn pdf_cmyk_background_covers_light_modules_and_quiet_zone_before_dark_rectangles() {
        let colors = [ModuleColor::Dark, ModuleColor::Light, ModuleColor::Light, ModuleColor::Dark];
        let pdf = Renderer::<CmykColor>::new(&colors, 2, 1)
            .dark_color(CmykColor([1.0, 0.0, 0.0, 0.25]))
            .light_color(CmykColor([0.0, 0.25, 0.5, 0.0]))
            .module_dimensions(2, 3)
            .build();

        assert_eq!(
            content_stream(&pdf),
            "0 0.25 0.5 0 k 0 0 8 12 re f\n1 0 0 0.25 k 2 6 2 3 re f\n1 0 0 0.25 k 4 3 2 3 re f\n"
        );
    }

    #[test]
    fn pdf_cmyk_all_light_canvas_is_filled_with_configured_background() {
        let colors = [ModuleColor::Light; 4];
        let pdf = Renderer::<CmykColor>::new(&colors, 2, 0)
            .light_color(CmykColor([0.0, 0.25, 0.5, 0.0]))
            .module_dimensions(2, 3)
            .build();

        assert_eq!(content_stream(&pdf), "0 0.25 0.5 0 k 0 0 4 6 re f\n");
    }

    #[test]
    fn pdf_stream_preallocation_is_bounded() {
        assert_eq!(stream_capacity(u32::MAX, u32::MAX, 64), MAX_STREAM_PREALLOC);
        assert_eq!(stream_capacity(2, 3, 48), 288);
    }

    #[test]
    fn pdf_normalization_keeps_valid_components_bit_for_bit() {
        for value in [-0.0, 0.0, f64::from_bits(1), f64::MIN_POSITIVE, 0.2, 0.5, 1.0] {
            assert_eq!(normalized_component(value).to_bits(), value.to_bits());
        }
    }

    #[test]
    fn pdf_rgb_nonfinite_and_out_of_range_components_become_valid_operands() {
        let mut canvas =
            Canvas::new(8, 12, Color([f64::NAN, f64::NEG_INFINITY, f64::INFINITY]), Color([1.5, -0.5, 0.25]));
        canvas.draw_dark_rect(2, 3, 4, 5);
        assert_eq!(content_stream(&canvas.into_image()), "1 0 0.25 rg 0 0 8 12 re f\n0 0 1 rg 2 4 4 5 re f\n");
        let canvas = Canvas::new(1, 1, Color([0.0; 3]), Color([f64::NAN, f64::NEG_INFINITY, f64::INFINITY]));
        assert_eq!(content_stream(&canvas.into_image()), "0 0 1 rg 0 0 1 1 re f\n");
    }

    #[test]
    fn pdf_cmyk_nonfinite_and_out_of_range_components_become_valid_operands() {
        let mut canvas = CmykCanvas::new(
            8,
            12,
            CmykColor([f64::NAN, f64::NEG_INFINITY, f64::INFINITY, 2.0]),
            CmykColor([f64::NAN, 1.5, -0.5, 0.5]),
        );
        canvas.draw_dark_rect(2, 3, 4, 5);
        assert_eq!(content_stream(&canvas.into_image()), "0 1 0 0.5 k 0 0 8 12 re f\n0 0 1 1 k 2 4 4 5 re f\n");
    }

    #[test]
    fn pdf_signed_zero_stays_in_background_and_foreground_operands() {
        let mut canvas = Canvas::new(1, 1, Color([-0.0, 0.5, 1.0]), Color([1.0, -0.0, 0.5]));
        canvas.draw_dark_pixel(0, 0);
        assert_eq!(content_stream(&canvas.into_image()), "1 -0 0.5 rg 0 0 1 1 re f\n-0 0.5 1 rg 0 0 1 1 re f\n");
    }
}
