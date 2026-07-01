//! PDF rendering support.
//!
//! # Example
//!
//! ```
//! use qrcode_rs::QrCode;
//! use qrcode_rs::render::pdf;
//!
//! let code = QrCode::new(b"Hello").unwrap();
//! let pdf_data = code.render::<pdf::Color>().build();
//! println!("PDF size: {} bytes", pdf_data.len());
//! ```

#![cfg(feature = "pdf")]

use core::fmt::Write;

use crate::render::{Canvas as RenderCanvas, Pixel};
use crate::types::Color as ModuleColor;

/// A PDF color (`[R, G, B]`).
///
/// Each value must be in the range of 0.0 to 1.0.
#[derive(Copy, Clone, Default, PartialEq, PartialOrd)]
pub struct Color(pub [f64; 3]);

impl Pixel for Color {
    type Canvas = Canvas;
    type Image = Vec<u8>;

    fn default_color(color: ModuleColor) -> Self {
        Self(color.select(Default::default(), [1.0; 3]))
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
    pending_left: u32,
    pending_bottom: u32,
    pending_width: u32,
    pending_height: u32,
    has_pending: bool,
}

impl Canvas {
    fn flush_pending(&mut self) {
        if self.has_pending {
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
            self.has_pending = false;
        }
    }
}

impl RenderCanvas for Canvas {
    type Pixel = Color;
    type Image = Vec<u8>;

    fn new(width: u32, height: u32, dark_pixel: Color, _light_pixel: Color) -> Self {
        Canvas {
            stream: String::new(),
            width,
            height,
            fg_r: dark_pixel.0[0],
            fg_g: dark_pixel.0[1],
            fg_b: dark_pixel.0[2],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderer;
    use crate::types::Color as ModuleColor;

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
    }
}
