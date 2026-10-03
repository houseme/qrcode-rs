//! String rendering support.

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::{
    borrow::ToOwned,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use crate::{Canvas as RenderCanvas, Pixel, RenderError, check_buffer_size, checked_area};
use qrcode_core::Color;

/// A renderable character or string fragment used by the plain-text renderer.
///
/// Implemented for `char` and `&'static str` so a [`Renderer`](crate::Renderer)
/// can be driven with either.
pub trait Element: Copy {
    /// Returns the default element for a dark or light module.
    fn default_color(color: Color) -> Self;
    /// The UTF-8 byte count appended by [`Element::append_to_string`].
    ///
    /// Implementations must report this accurately for output-size budgeting.
    fn strlen(self) -> usize;
    /// Appends this element to `string`.
    fn append_to_string(self, string: &mut String);
}

impl Element for char {
    fn default_color(color: Color) -> Self {
        color.select('\u{2588}', ' ')
    }

    fn strlen(self) -> usize {
        self.len_utf8()
    }

    fn append_to_string(self, string: &mut String) {
        string.push(self);
    }
}

impl Element for &str {
    fn default_color(color: Color) -> Self {
        color.select("\u{2588}", " ")
    }

    fn strlen(self) -> usize {
        self.len()
    }

    fn append_to_string(self, string: &mut String) {
        string.push_str(self);
    }
}

#[doc(hidden)]
pub struct Canvas<P: Element> {
    buffer: Vec<P>,
    width: usize,
    dark_pixel: P,
    dark_byte_len: usize,
    capacity: usize,
}

fn layout<P: Element>(width: u32, height: u32, dark: P, light: P) -> Result<(usize, usize), RenderError> {
    let area = checked_area(width, height)?;
    let newlines = if area == 0 { 0 } else { height as usize - 1 };
    let buffer_bytes = area.checked_mul(core::mem::size_of::<P>()).ok_or(RenderError::OutputTooLarge)?;
    let output_bytes = area
        .checked_mul(dark.strlen().max(light.strlen()))
        .and_then(|bytes| bytes.checked_add(newlines))
        .ok_or(RenderError::OutputTooLarge)?;
    check_buffer_size(buffer_bytes.checked_add(output_bytes).ok_or(RenderError::OutputTooLarge)?)?;
    let capacity = area
        .checked_mul(light.strlen())
        .and_then(|bytes| bytes.checked_add(newlines))
        .ok_or(RenderError::OutputTooLarge)?;
    Ok((area, capacity))
}

impl<P: Element> Pixel for P {
    type Image = String;
    type Canvas = Canvas<Self>;

    fn default_unit_size() -> (u32, u32) {
        (1, 1)
    }

    fn default_color(color: Color) -> Self {
        <Self as Element>::default_color(color)
    }
}

impl<P: Element> RenderCanvas for Canvas<P> {
    type Pixel = P;
    type Image = String;

    fn new(width: u32, height: u32, dark_pixel: P, light_pixel: P) -> Self {
        let (area, capacity) = layout(width, height, dark_pixel, light_pixel).unwrap_or_else(|error| panic!("{error}"));
        Self {
            buffer: vec![light_pixel; area],
            width: width as usize,
            dark_pixel,
            dark_byte_len: dark_pixel.strlen(),
            capacity,
        }
    }

    fn validate_dimensions(width: u32, height: u32, dark: &P, light: &P) -> Result<(), RenderError> {
        layout(width, height, *dark, *light).map(|_| ())
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        let pixel = &mut self.buffer[x as usize + y as usize * self.width];
        self.capacity = self.capacity - pixel.strlen() + self.dark_byte_len;
        *pixel = self.dark_pixel;
    }

    fn into_image(self) -> String {
        let mut result = String::with_capacity(self.capacity);
        for (i, pixel) in self.buffer.into_iter().enumerate() {
            if i != 0 && i % self.width == 0 {
                result.push('\n');
            }
            pixel.append_to_string(&mut result);
        }
        result
    }
}

#[test]
fn test_render_to_string() {
    use crate::Renderer;

    let colors = &[Color::Dark, Color::Light, Color::Light, Color::Dark];
    let image: String = Renderer::<char>::new(colors, 2, 1).build();
    assert_eq!(&image, "    \n \u{2588}  \n  \u{2588} \n    ");

    let image2 = Renderer::new(colors, 2, 1).light_color("A").dark_color("!B!").module_dimensions(2, 2).build();

    assert_eq!(
        &image2,
        "AAAAAAAA\n\
         AAAAAAAA\n\
         AA!B!!B!AAAA\n\
         AA!B!!B!AAAA\n\
         AAAA!B!!B!AA\n\
         AAAA!B!!B!AA\n\
         AAAAAAAA\n\
         AAAAAAAA"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MAX_BUFFER_BYTES, Renderer};

    #[test]
    fn character_budget_includes_buffer_and_output() {
        let width = (MAX_BUFFER_BYTES / 5) as u32;
        assert!(Canvas::<char>::validate_dimensions(width, 1, &'#', &' ').is_ok());
        assert_eq!(Canvas::<char>::validate_dimensions(width + 1, 1, &'#', &' '), Err(RenderError::OutputTooLarge));
        assert_eq!(Canvas::<char>::validate_dimensions(65_536, 65_536, &'#', &' '), Err(RenderError::OutputTooLarge));
    }

    #[test]
    fn long_utf8_fragments_are_rejected_before_allocating() {
        let fragment = "💖".repeat(1024);
        let modules = [Color::Dark];
        let result = Renderer::new(&modules, 1, 0)
            .dark_color(fragment.as_str())
            .light_color("")
            .module_dimensions(1024, 1024)
            .try_build();
        assert_eq!(result, Err(RenderError::OutputTooLarge));
    }

    #[test]
    fn repainting_shorter_pixels_keeps_the_actual_output_capacity() {
        let mut canvas = Canvas::new(2, 2, "", "long");
        for _ in 0..128 {
            canvas.draw_dark_pixel(0, 0);
        }
        assert_eq!(canvas.capacity, 13);
        assert_eq!(canvas.into_image(), "long\nlonglong");
    }

    #[test]
    fn repainting_multibyte_characters_counts_the_replaced_pixel_once() {
        let mut canvas = Canvas::new(2, 2, '😀', ' ');
        for _ in 0..128 {
            canvas.draw_dark_pixel(1, 1);
        }
        assert_eq!(canvas.capacity, 8);
        assert_eq!(canvas.into_image(), "  \n 😀");
    }

    #[test]
    fn empty_canvases_produce_empty_text() {
        for (width, height) in [(0, 0), (0, u32::MAX), (u32::MAX, 0)] {
            assert_eq!(Canvas::new(width, height, '█', ' ').into_image(), "");
        }
    }
}
