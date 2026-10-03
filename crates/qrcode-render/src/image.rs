#![cfg(feature = "image")]
//! Image rendering via the [`image`] crate (PNG, JPEG, …).
//!
//! `QrCode::render::<image::Rgba<u8>>()` (or `Luma<u8>`, `Rgb<u8>`, …) produces
//! an `image::ImageBuffer`, which can be saved or encoded with the `image` API.
use crate::{Canvas, Pixel, RenderError, StyledPixel};
use qrcode_core::Color;

use image::{DynamicImage, GenericImageView, ImageBuffer, Luma, LumaA, Primitive, Rgb, Rgba};

macro_rules! impl_pixel_for_image_pixel {
    ($p:ident<$s:ident>: $c:pat => $d:expr) => {
        impl<$s> Pixel for $p<$s>
        where
            $s: Primitive + 'static,
            $p<$s>: image::Pixel<Subpixel = $s>,
        {
            type Image = ImageBuffer<Self, Vec<$s>>;
            type Canvas = (Self, Self::Image);

            fn default_color(color: Color) -> Self {
                match color.select($s::zero(), $s::max_value()) {
                    $c => $p($d),
                }
            }
        }
    };
}

impl_pixel_for_image_pixel! { Luma<S>: p => [p] }
impl_pixel_for_image_pixel! { LumaA<S>: p => [p, S::max_value()] }
impl_pixel_for_image_pixel! { Rgb<S>: p => [p, p, p] }
impl_pixel_for_image_pixel! { Rgba<S>: p => [p, p, p, S::max_value()] }

impl StyledPixel for Rgb<u8> {
    fn from_hex(hex: &str) -> Self {
        let (r, g, b) = crate::colors::hex_to_rgb(hex).unwrap_or((0, 0, 0));
        Rgb([r, g, b])
    }
}

impl StyledPixel for Rgba<u8> {
    fn from_hex(hex: &str) -> Self {
        let (r, g, b) = crate::colors::hex_to_rgb(hex).unwrap_or((0, 0, 0));
        Rgba([r, g, b, 255])
    }
}

impl<P: image::Pixel + 'static> Canvas for (P, ImageBuffer<P, Vec<P::Subpixel>>) {
    type Pixel = P;
    type Image = ImageBuffer<P, Vec<P::Subpixel>>;

    fn validate_dimensions(width: u32, height: u32, _dark_pixel: &P, _light_pixel: &P) -> Result<(), RenderError> {
        // Match ImageBuffer's row-first length checks, including empty images on 32-bit targets.
        let bytes = (width as usize)
            .checked_mul(usize::from(P::CHANNEL_COUNT))
            .and_then(|row_samples| row_samples.checked_mul(height as usize))
            .and_then(|samples| samples.checked_mul(core::mem::size_of::<P::Subpixel>()))
            .ok_or(RenderError::OutputTooLarge)?;
        crate::check_buffer_size(bytes)
    }

    fn new(width: u32, height: u32, dark_pixel: P, light_pixel: P) -> Self {
        if let Err(error) = Self::validate_dimensions(width, height, &dark_pixel, &light_pixel) {
            panic!("image canvas dimensions are too large: {error}");
        }
        (dark_pixel, ImageBuffer::from_pixel(width, height, light_pixel))
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        self.1.put_pixel(x, y, self.0);
    }

    fn draw_dark_rect(&mut self, left: u32, top: u32, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        let (image_width, image_height) = self.1.dimensions();
        assert!(
            left < image_width && top < image_height && width <= image_width - left && height <= image_height - top,
            "rectangle exceeds image dimensions"
        );

        let channels = self.0.channels();
        let channel_count = usize::from(P::CHANNEL_COUNT);
        let row_stride = image_width as usize * channel_count;
        let row_width = width as usize * channel_count;
        let row_left = left as usize * channel_count;
        let data: &mut [P::Subpixel] = &mut self.1;

        if channel_count == 3 && width >= 8 && height > 1 {
            let first_start = top as usize * row_stride + row_left;
            let (first_rows, remaining_rows) = data.split_at_mut(first_start + row_stride);
            let first_row = &mut first_rows[first_start..first_start + row_width];
            for pixel in first_row.chunks_exact_mut(channel_count) {
                pixel.copy_from_slice(channels);
            }

            // The split keeps source and destination rows disjoint without allocating a template.
            for row in remaining_rows.chunks_mut(row_stride).take(height as usize - 1) {
                row[..row_width].copy_from_slice(first_row);
            }
            return;
        }

        // Each row is contiguous, so avoid recomputing and checking every pixel's coordinates.
        for y in top..top + height {
            let start = y as usize * row_stride + row_left;
            let row = &mut data[start..start + row_width];
            for pixel in row.chunks_exact_mut(channel_count) {
                pixel.copy_from_slice(channels);
            }
        }
    }

    fn into_image(self) -> ImageBuffer<P, Vec<P::Subpixel>> {
        self.1
    }
}

/// Overlays a logo onto the center of a QR code image.
///
/// The logo is automatically resized to fit within the specified ratio of the
/// QR code's dimensions. A white padding margin is added around the logo to
/// ensure scannability.
///
/// Use `image::DynamicImage::from(qr_image)` to convert an `ImageBuffer` to
/// `DynamicImage` if needed.
///
/// # Arguments
///
/// * `qr_image` - The rendered QR code as a `DynamicImage`.
/// * `logo` - The logo image to overlay (any format the `image` crate supports).
/// * `size_ratio` - Maximum logo size as a fraction of the QR code size (0.0–0.5).
///   Recommended: 0.2–0.3. Values above 0.35 may make the QR code unscannable.
///
/// # Example
///
/// ```no_run
/// use qrcode_core::Color;
/// use qrcode_render::{Renderer, image::overlay_logo};
/// use image::{Rgb, DynamicImage, open};
///
/// let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
/// let qr = DynamicImage::ImageRgb8(
///     Renderer::<Rgb<u8>>::new(&modules, 2, 4).min_dimensions(300, 300).build(),
/// );
/// let logo = open("logo.png").unwrap();
/// let final_image = overlay_logo(&qr, &logo, 0.25);
/// final_image.save("qr_with_logo.png").unwrap();
/// ```
pub fn overlay_logo(qr_image: &DynamicImage, logo: &DynamicImage, size_ratio: f32) -> DynamicImage {
    let (qr_w, qr_h) = qr_image.dimensions();
    let ratio = size_ratio.clamp(0.05, 0.5);

    // Convert QR image to RGBA8 for compositing.
    let mut result = qr_image.to_rgba8();

    // Calculate target logo size (with padding margin).
    let max_logo_dim = ((qr_w.min(qr_h) as f32 * ratio) as u32).max(1);
    let padding = (max_logo_dim as f32 * 0.1) as u32;
    let logo_target = max_logo_dim.saturating_sub(2 * padding).max(1);

    // Resize logo preserving aspect ratio.
    let logo_resized = logo.resize(logo_target, logo_target, image::imageops::FilterType::Lanczos3);
    let (lw, lh) = logo_resized.dimensions();

    // Center position.
    let x_off = (qr_w.saturating_sub(lw)) / 2;
    let y_off = (qr_h.saturating_sub(lh)) / 2;

    // Draw white background behind the logo area.
    let bg_x = x_off.saturating_sub(padding);
    let bg_y = y_off.saturating_sub(padding);
    let bg_w = (lw + 2 * padding).min(qr_w - bg_x);
    let bg_h = (lh + 2 * padding).min(qr_h - bg_y);
    let white = Rgba([255u8, 255, 255, 255]);
    for py in bg_y..bg_y + bg_h {
        for px in bg_x..bg_x + bg_w {
            result.put_pixel(px, py, white);
        }
    }

    // Composite logo onto the QR code with alpha blending.
    let logo_rgba = logo_resized.to_rgba8();
    for py in 0..lh {
        for px in 0..lw {
            let src = logo_rgba.get_pixel(px, py);
            let [sr, sg, sb, sa] = src.0;
            if sa == 0 {
                continue;
            }
            let dst = *result.get_pixel(x_off + px, y_off + py);
            let [dr, dg, db, _da] = dst.0;
            let af = sa as f32 / 255.0;
            let inv = 1.0 - af;
            let r = (dr as f32 * inv + sr as f32 * af) as u8;
            let g = (dg as f32 * inv + sg as f32 * af) as u8;
            let b = (db as f32 * inv + sb as f32 * af) as u8;
            result.put_pixel(x_off + px, y_off + py, Rgba([r, g, b, 255]));
        }
    }

    DynamicImage::ImageRgba8(result)
}

/// Re-exports the `image` crate's `ImageFormat` for convenience.
pub use image::ImageFormat;

/// Encodes a QR code image into the specified format and writes to a byte vector.
///
/// This is a convenience wrapper around the `image` crate's format support.
/// Supported formats depend on the `image` crate's enabled features
/// (default: PNG, JPEG, GIF, BMP, TIFF, WebP).
///
/// # Example
///
/// ```no_run
/// use qrcode_core::Color;
/// use qrcode_render::{Renderer, image::{encode_to_format, ImageFormat}};
/// use image::{DynamicImage, Rgb};
///
/// let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
/// let img = DynamicImage::ImageRgb8(
///     Renderer::<Rgb<u8>>::new(&modules, 2, 4).min_dimensions(200, 200).build(),
/// );
/// let jpeg_bytes = encode_to_format(&img, ImageFormat::Jpeg).unwrap();
/// std::fs::write("qr.jpg", &jpeg_bytes).unwrap();
/// ```
pub fn encode_to_format(image: &DynamicImage, format: ImageFormat) -> image::ImageResult<Vec<u8>> {
    let mut buf = std::io::Cursor::new(Vec::new());
    image.write_to(&mut buf, format)?;
    Ok(buf.into_inner())
}

/// Direction of a gradient sweep.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GradientDirection {
    /// Top to bottom.
    Vertical,
    /// Left to right.
    Horizontal,
    /// Top-left to bottom-right.
    Diagonal,
}

/// A linear gradient defined by two endpoint colors.
#[derive(Copy, Clone, Debug)]
pub struct Gradient {
    /// Direction of the gradient across the image.
    pub direction: GradientDirection,
    /// Color at the gradient's start position.
    pub start_color: Rgba<u8>,
    /// Color at the gradient's end position.
    pub end_color: Rgba<u8>,
}

/// Applies a gradient tint to the light (background) pixels of a QR code image.
///
/// Dark (foreground) pixels are preserved. Light pixels are replaced with the
/// interpolated gradient color based on their position.
///
/// # Example
///
/// ```no_run
/// use qrcode_core::Color;
/// use qrcode_render::{Renderer, image::{apply_gradient_background, Gradient, GradientDirection}};
/// use image::{Rgb, DynamicImage, Rgba};
///
/// let modules = [Color::Dark, Color::Light, Color::Light, Color::Dark];
/// let qr = DynamicImage::ImageRgb8(
///     Renderer::<Rgb<u8>>::new(&modules, 2, 4).min_dimensions(200, 200).build(),
/// );
/// let gradient = Gradient {
///     direction: GradientDirection::Vertical,
///     start_color: Rgba([255, 200, 200, 255]),
///     end_color: Rgba([200, 200, 255, 255]),
/// };
/// let result = apply_gradient_background(&qr, &gradient);
/// result.save("qr_gradient.png").unwrap();
/// ```
pub fn apply_gradient_background(image: &DynamicImage, gradient: &Gradient) -> DynamicImage {
    let mut result = image.to_rgba8();
    let (w, h) = result.dimensions();

    let sc = gradient.start_color.0;
    let ec = gradient.end_color.0;

    for (x, y, pixel) in result.enumerate_pixels_mut() {
        let [r, g, b, a] = pixel.0;

        // Detect light pixels: high luminance and not fully transparent.
        let lum = (r as u32 + g as u32 + b as u32) / 3;
        if lum > 200 && a > 0 {
            let t = match gradient.direction {
                GradientDirection::Vertical => {
                    if h <= 1 {
                        0.0
                    } else {
                        y as f32 / (h - 1) as f32
                    }
                }
                GradientDirection::Horizontal => {
                    if w <= 1 {
                        0.0
                    } else {
                        x as f32 / (w - 1) as f32
                    }
                }
                GradientDirection::Diagonal => {
                    if w <= 1 || h <= 1 {
                        0.0
                    } else {
                        (x as f32 / (w - 1) as f32 + y as f32 / (h - 1) as f32) / 2.0
                    }
                }
            };
            let inv = 1.0 - t;
            let nr = (sc[0] as f32 * inv + ec[0] as f32 * t) as u8;
            let ng = (sc[1] as f32 * inv + ec[1] as f32 * t) as u8;
            let nb = (sc[2] as f32 * inv + ec[2] as f32 * t) as u8;
            let na = (sc[3] as f32 * inv + ec[3] as f32 * t) as u8;
            *pixel = Rgba([nr, ng, nb, na]);
        }
    }

    DynamicImage::ImageRgba8(result)
}

#[cfg(test)]
mod render_tests {
    use crate::{Canvas, MAX_BUFFER_BYTES, RenderError, Renderer};
    use image::{DynamicImage, GenericImageView, ImageBuffer, Luma, LumaA, Rgb, Rgba};
    use qrcode_core::Color;

    fn assert_image_buffer_budget<P>(dark: P, light: P, bytes_per_pixel: usize)
    where
        P: image::Pixel + 'static,
    {
        let validate = <(P, ImageBuffer<P, Vec<P::Subpixel>>) as Canvas>::validate_dimensions;
        let max_pixels = u32::try_from(MAX_BUFFER_BYTES / bytes_per_pixel).unwrap();
        assert_eq!(validate(max_pixels, 1, &dark, &light), Ok(()));
        assert_eq!(validate(1, max_pixels, &dark, &light), Ok(()));
        assert_eq!(validate(max_pixels + 1, 1, &dark, &light), Err(RenderError::OutputTooLarge));
        assert_eq!(validate(1, max_pixels + 1, &dark, &light), Err(RenderError::OutputTooLarge));
        assert_eq!(validate(65_536, 65_536, &dark, &light), Err(RenderError::OutputTooLarge));
        assert_eq!(validate(u32::MAX, u32::MAX, &dark, &light), Err(RenderError::OutputTooLarge));
        let wide_empty =
            if usize::BITS == 32 && P::CHANNEL_COUNT > 1 { Err(RenderError::OutputTooLarge) } else { Ok(()) };
        assert_eq!(validate(u32::MAX, 0, &dark, &light), wide_empty);
        assert_eq!(validate(0, u32::MAX, &dark, &light), Ok(()));
    }

    #[test]
    fn image_buffer_budget_accounts_for_channels_and_subpixel_sizes_without_allocating() {
        assert_image_buffer_budget(Luma([0u8]), Luma([255]), 1);
        assert_image_buffer_budget(LumaA([0u8, 255]), LumaA([255, 255]), 2);
        assert_image_buffer_budget(Rgb([0u8, 0, 0]), Rgb([255, 255, 255]), 3);
        assert_image_buffer_budget(Rgba([0u8, 0, 0, 255]), Rgba([255, 255, 255, 255]), 4);
        assert_image_buffer_budget(Luma([0u16]), Luma([65535]), 2);
        assert_image_buffer_budget(LumaA([0u16, 65535]), LumaA([65535, 65535]), 4);
        assert_image_buffer_budget(Rgb([0u16, 0, 0]), Rgb([65535, 65535, 65535]), 6);
        assert_image_buffer_budget(Rgba([0u16, 0, 0, 65535]), Rgba([65535, 65535, 65535, 65535]), 8);
        assert_image_buffer_budget(Luma([0.0f32]), Luma([1.0]), 4);
        assert_image_buffer_budget(LumaA([0.0f32, 1.0]), LumaA([1.0, 1.0]), 8);
        assert_image_buffer_budget(Rgb([0.0f32, 0.0, 0.0]), Rgb([1.0, 1.0, 1.0]), 12);
        assert_image_buffer_budget(Rgba([0.0f32, 0.0, 0.0, 1.0]), Rgba([1.0, 1.0, 1.0, 1.0]), 16);
        assert_image_buffer_budget(Luma([0.0f64]), Luma([1.0]), 8);
        assert_image_buffer_budget(LumaA([0.0f64, 1.0]), LumaA([1.0, 1.0]), 16);
        assert_image_buffer_budget(Rgb([0.0f64, 0.0, 0.0]), Rgb([1.0, 1.0, 1.0]), 24);
        assert_image_buffer_budget(Rgba([0.0f64, 0.0, 0.0, 1.0]), Rgba([1.0, 1.0, 1.0, 1.0]), 32);
    }

    #[test]
    #[should_panic(expected = "image canvas dimensions are too large")]
    fn direct_image_canvas_construction_rejects_oversized_dimensions_before_allocating() {
        let _ = <(Rgba<u8>, ImageBuffer<Rgba<u8>, Vec<u8>>) as Canvas>::new(
            65_536,
            65_536,
            Rgba([0, 0, 0, 255]),
            Rgba([255, 255, 255, 255]),
        );
    }

    fn assert_rectangle_matches_scalar<P>(
        image_width: u32,
        image_height: u32,
        rectangle: (u32, u32, u32, u32),
        dark: P,
        light: P,
    ) where
        P: image::Pixel + 'static,
        P::Subpixel: std::fmt::Debug,
    {
        let (left, top, width, height) = rectangle;
        let mut canvas = <(P, ImageBuffer<P, Vec<P::Subpixel>>) as Canvas>::new(image_width, image_height, dark, light);
        let mut expected = ImageBuffer::from_pixel(image_width, image_height, light);
        canvas.draw_dark_rect(left, top, width, height);
        for y in top..top + height {
            for x in left..left + width {
                expected.put_pixel(x, y, dark);
            }
        }
        assert_eq!(
            canvas.into_image().as_raw(),
            expected.as_raw(),
            "image {image_width}x{image_height}, rectangle ({left}, {top}, {width}, {height})"
        );
    }

    fn assert_rectangles_match_scalar<P>(dark: P, light: P)
    where
        P: image::Pixel + 'static,
        P::Subpixel: std::fmt::Debug,
    {
        for (image_width, image_height) in [(0, 0), (0, 5), (7, 0), (1, 5), (7, 1), (7, 5)] {
            for left in 0..=image_width {
                for top in 0..=image_height {
                    for width in 0..=image_width - left {
                        for height in 0..=image_height - top {
                            assert_rectangle_matches_scalar(
                                image_width,
                                image_height,
                                (left, top, width, height),
                                dark,
                                light,
                            );
                        }
                    }
                }
            }
        }
        for width in [1, 2, 7, 8, 32] {
            for height in [1, 8, 64] {
                for (left, top) in [(0, 0), (3, 2), (6, 4)] {
                    assert_rectangle_matches_scalar(width + 6, height + 4, (left, top, width, height), dark, light);
                }
            }
        }
    }

    #[test]
    fn rectangles_match_scalar_for_all_pixel_formats_and_primitives() {
        macro_rules! check_primitives {
            ($($subpixel:ty),+ $(,)?) => {
                $(
                    assert_rectangles_match_scalar(Luma([2 as $subpixel]), Luma([11 as $subpixel]));
                    assert_rectangles_match_scalar(
                        LumaA([2 as $subpixel, 3 as $subpixel]),
                        LumaA([11 as $subpixel, 12 as $subpixel]),
                    );
                    assert_rectangles_match_scalar(
                        Rgb([2 as $subpixel, 3 as $subpixel, 4 as $subpixel]),
                        Rgb([11 as $subpixel, 12 as $subpixel, 13 as $subpixel]),
                    );
                    assert_rectangles_match_scalar(
                        Rgba([2 as $subpixel, 3 as $subpixel, 4 as $subpixel, 5 as $subpixel]),
                        Rgba([11 as $subpixel, 12 as $subpixel, 13 as $subpixel, 14 as $subpixel]),
                    );
                )+
            };
        }
        check_primitives!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64);
    }

    fn assert_rectangle_bits<P>(dark: P, light: P, bits: impl Fn(P::Subpixel) -> u64 + Copy)
    where
        P: image::Pixel + 'static,
    {
        for width in [1, 2, 7, 8, 32] {
            for height in [1, 8, 64] {
                for (left, top) in [(0, 0), (3, 2), (6, 4)] {
                    let mut canvas =
                        <(P, ImageBuffer<P, Vec<P::Subpixel>>) as Canvas>::new(width + 6, height + 4, dark, light);
                    let mut expected = ImageBuffer::from_pixel(width + 6, height + 4, light);
                    canvas.draw_dark_rect(left, top, width, height);
                    // Repainting must also preserve NaN payloads and signed zero exactly.
                    canvas.draw_dark_rect(left, top, width, height);
                    for y in top..top + height {
                        for x in left..left + width {
                            expected.put_pixel(x, y, dark);
                        }
                    }
                    let actual = canvas.into_image().into_raw().into_iter().map(bits).collect::<Vec<_>>();
                    let expected = expected.into_raw().into_iter().map(bits).collect::<Vec<_>>();
                    assert_eq!(actual, expected, "rectangle ({left}, {top}, {width}, {height})");
                }
            }
        }
    }

    #[test]
    fn float_rectangle_copies_preserve_nan_payloads_and_signed_zero_bits() {
        macro_rules! check_float {
            ($float:ty, $dark_nan:expr, $signaling_nan:expr, $light_nan:expr, $bits:expr) => {{
                let dark =
                    [<$float>::from_bits($dark_nan), <$float>::from_bits($signaling_nan), -0.0, <$float>::INFINITY];
                let light = [<$float>::from_bits($light_nan), 0.0, <$float>::NEG_INFINITY, 1.0];
                let bits = $bits;
                assert_rectangle_bits(Luma([dark[0]]), Luma([light[0]]), bits);
                assert_rectangle_bits(LumaA([dark[0], dark[2]]), LumaA([light[0], light[1]]), bits);
                assert_rectangle_bits(Rgb([dark[0], dark[1], dark[2]]), Rgb([light[0], light[1], light[2]]), bits);
                assert_rectangle_bits(Rgba(dark), Rgba(light), bits);
            }};
        }
        check_float!(f32, 0x7fc0_0123, 0x7fa0_0067, 0xffc0_0456, |value: f32| u64::from(value.to_bits()));
        check_float!(f64, 0x7ff8_0000_0000_0123, 0x7ff0_0000_0000_0067, 0xfff8_0000_0000_0456, f64::to_bits);
    }

    #[test]
    fn rectangles_reject_out_of_bounds_ranges() {
        for (left, top, width, height) in [
            (7, 0, 1, 1),
            (0, 5, 1, 1),
            (6, 0, 2, 1),
            (0, 4, 1, 2),
            (u32::MAX, 0, 1, 1),
            (0, u32::MAX, 1, 1),
            (1, 0, u32::MAX, 1),
            (0, 1, 1, u32::MAX),
        ] {
            let result = std::panic::catch_unwind(|| {
                let mut canvas = (Luma([0u8]), ImageBuffer::from_pixel(7, 5, Luma([255])));
                canvas.draw_dark_rect(left, top, width, height);
            });
            assert!(result.is_err(), "rectangle ({left}, {top}, {width}, {height}) should panic");
        }
    }

    fn scalar_gradient(image: &DynamicImage, gradient: &super::Gradient) -> DynamicImage {
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        let mut result = rgba.clone();
        for y in 0..height {
            for x in 0..width {
                let [r, g, b, a] = rgba.get_pixel(x, y).0;
                let luminance = (u32::from(r) + u32::from(g) + u32::from(b)) / 3;
                if luminance <= 200 || a == 0 {
                    continue;
                }
                let t = match gradient.direction {
                    super::GradientDirection::Vertical if height > 1 => y as f32 / (height - 1) as f32,
                    super::GradientDirection::Horizontal if width > 1 => x as f32 / (width - 1) as f32,
                    super::GradientDirection::Diagonal if width > 1 && height > 1 => {
                        (x as f32 / (width - 1) as f32 + y as f32 / (height - 1) as f32) / 2.0
                    }
                    _ => 0.0,
                };
                let mut color = [0u8; 4];
                for (channel, value) in color.iter_mut().enumerate() {
                    *value = (gradient.start_color.0[channel] as f32 * (1.0 - t)
                        + gradient.end_color.0[channel] as f32 * t) as u8;
                }
                result.put_pixel(x, y, Rgba(color));
            }
        }
        DynamicImage::ImageRgba8(result)
    }

    #[test]
    fn gradient_matches_scalar_for_all_directions_dimensions_and_image_formats() {
        use super::{Gradient, GradientDirection, apply_gradient_background};

        let colors = [
            Rgba([255, 255, 255, 255]),
            Rgba([0, 0, 0, 255]),
            Rgba([200, 200, 200, 255]),
            Rgba([201, 200, 202, 1]),
            Rgba([255, 255, 255, 0]),
            Rgba([254, 247, 241, 127]),
            Rgba([0, 255, 255, 255]),
        ];
        for (width, height) in [(0, 0), (0, 3), (3, 0), (1, 1), (1, 7), (7, 1), (5, 3), (9, 7)] {
            let rgba = DynamicImage::ImageRgba8(ImageBuffer::from_fn(width, height, |x, y| {
                colors[((y * width + x) as usize) % colors.len()]
            }));
            let images = [
                rgba.clone(),
                DynamicImage::ImageLuma8(rgba.to_luma8()),
                DynamicImage::ImageLumaA8(rgba.to_luma_alpha8()),
                DynamicImage::ImageRgb8(rgba.to_rgb8()),
                DynamicImage::ImageLuma16(rgba.to_luma16()),
                DynamicImage::ImageLumaA16(rgba.to_luma_alpha16()),
                DynamicImage::ImageRgb16(rgba.to_rgb16()),
                DynamicImage::ImageRgba16(rgba.to_rgba16()),
                DynamicImage::ImageRgb32F(rgba.to_rgb32f()),
                DynamicImage::ImageRgba32F(rgba.to_rgba32f()),
            ];
            for image in images {
                for direction in
                    [GradientDirection::Vertical, GradientDirection::Horizontal, GradientDirection::Diagonal]
                {
                    let gradient = Gradient {
                        direction,
                        start_color: Rgba([23, 117, 251, 19]),
                        end_color: Rgba([250, 61, 9, 231]),
                    };
                    assert_eq!(
                        apply_gradient_background(&image, &gradient).as_rgba8().unwrap().as_raw(),
                        scalar_gradient(&image, &gradient).as_rgba8().unwrap().as_raw(),
                        "{direction:?}, {width}x{height}, {:?}",
                        image.color()
                    );
                }
            }
        }
    }

    #[test]
    fn test_render_luma8_unsized() {
        let image = Renderer::<Luma<u8>>::new(
            &[
                Color::Light,
                Color::Dark,
                Color::Dark,
                //
                Color::Dark,
                Color::Light,
                Color::Light,
                //
                Color::Light,
                Color::Dark,
                Color::Light,
            ],
            3,
            1,
        )
        .module_dimensions(1, 1)
        .build();

        #[rustfmt::skip]
            let expected = [
            255, 255, 255, 255, 255,
            255, 255,   0,   0, 255,
            255,   0, 255, 255, 255,
            255, 255,   0, 255, 255,
            255, 255, 255, 255, 255,
        ];
        assert_eq!(image.into_raw(), expected);
    }

    #[test]
    fn test_render_rgba_unsized() {
        let image = Renderer::<Rgba<u8>>::new(&[Color::Light, Color::Dark, Color::Dark, Color::Dark], 2, 1)
            .module_dimensions(1, 1)
            .build();

        #[rustfmt::skip]
            let expected: &[u8] = &[
            255,255,255,255, 255,255,255,255, 255,255,255,255, 255,255,255,255,
            255,255,255,255, 255,255,255,255,   0,  0,  0,255, 255,255,255,255,
            255,255,255,255,   0,  0,  0,255,   0,  0,  0,255, 255,255,255,255,
            255,255,255,255, 255,255,255,255, 255,255,255,255, 255,255,255,255,
        ];

        assert_eq!(image.into_raw(), expected);
    }

    #[test]
    fn test_render_resized_min() {
        let image = Renderer::<Luma<u8>>::new(&[Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 1)
            .min_dimensions(10, 10)
            .build();

        #[rustfmt::skip]
            let expected: &[u8] = &[
            255,255,255, 255,255,255, 255,255,255, 255,255,255,
            255,255,255, 255,255,255, 255,255,255, 255,255,255,
            255,255,255, 255,255,255, 255,255,255, 255,255,255,

            255,255,255,   0,  0,  0, 255,255,255, 255,255,255,
            255,255,255,   0,  0,  0, 255,255,255, 255,255,255,
            255,255,255,   0,  0,  0, 255,255,255, 255,255,255,

            255,255,255, 255,255,255,   0,  0,  0, 255,255,255,
            255,255,255, 255,255,255,   0,  0,  0, 255,255,255,
            255,255,255, 255,255,255,   0,  0,  0, 255,255,255,

            255,255,255, 255,255,255, 255,255,255, 255,255,255,
            255,255,255, 255,255,255, 255,255,255, 255,255,255,
            255,255,255, 255,255,255, 255,255,255, 255,255,255,
        ];

        assert_eq!(image.dimensions(), (12, 12));
        assert_eq!(image.into_raw(), expected);
    }

    #[test]
    fn test_render_resized_max() {
        let image = Renderer::<Luma<u8>>::new(&[Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 1)
            .max_dimensions(10, 5)
            .build();

        #[rustfmt::skip]
            let expected: &[u8] = &[
            255,255, 255,255, 255,255, 255,255,

            255,255,   0,  0, 255,255, 255,255,

            255,255, 255,255,   0,  0, 255,255,

            255,255, 255,255, 255,255, 255,255,
        ];

        assert_eq!(image.dimensions(), (8, 4));
        assert_eq!(image.into_raw(), expected);
    }

    #[test]
    fn test_overlay_logo() {
        use super::overlay_logo;
        use image::DynamicImage;

        // Create a small QR code image.
        let qr = Renderer::<Rgba<u8>>::new(&[Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 0)
            .module_dimensions(10, 10)
            .build();
        let qr_dyn = DynamicImage::ImageRgba8(qr);

        // Create a small red logo.
        let logo = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(4, 4, Rgba([255u8, 0, 0, 255])));

        let result = overlay_logo(&qr_dyn, &logo, 0.5);
        assert_eq!(result.dimensions(), (20, 20));

        // The center pixels should be the logo (red).
        let center = result.as_rgba8().unwrap().get_pixel(10, 10);
        assert_eq!(center.0[0], 255); // red channel
        assert_eq!(center.0[3], 255); // alpha
    }

    #[test]
    fn test_overlay_logo_small_ratio() {
        use super::overlay_logo;
        use image::DynamicImage;

        let qr = Renderer::<Rgba<u8>>::new(&[Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 0)
            .module_dimensions(100, 100)
            .build();
        let qr_dyn = DynamicImage::ImageRgba8(qr);
        let logo = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(4, 4, Rgba([0u8, 255, 0, 255])));

        let result = overlay_logo(&qr_dyn, &logo, 0.2);
        assert_eq!(result.dimensions(), (200, 200));
    }

    #[test]
    fn test_encode_to_format_png() {
        use super::{ImageFormat, encode_to_format};
        use image::DynamicImage;

        let img = Renderer::<Rgba<u8>>::new(&[Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 0)
            .module_dimensions(10, 10)
            .build();
        let dyn_img = DynamicImage::ImageRgba8(img);
        let png_bytes = encode_to_format(&dyn_img, ImageFormat::Png).unwrap();
        assert!(!png_bytes.is_empty());
        // PNG magic bytes
        assert_eq!(&png_bytes[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn test_encode_to_format_jpeg() {
        use super::{ImageFormat, encode_to_format};
        use image::DynamicImage;

        let img = Renderer::<Rgba<u8>>::new(&[Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 0)
            .module_dimensions(10, 10)
            .build();
        let dyn_img = DynamicImage::ImageRgba8(img);
        let jpeg_bytes = encode_to_format(&dyn_img, ImageFormat::Jpeg).unwrap();
        assert!(!jpeg_bytes.is_empty());
        // JPEG magic bytes (FF D8 FF)
        assert_eq!(&jpeg_bytes[..3], &[0xFF, 0xD8, 0xFF]);
    }

    #[test]
    fn test_for_web() {
        let img =
            Renderer::<Luma<u8>>::new(&[Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 1).for_web().build();
        // for_web sets min_dimensions(200, 200). With modules_count=2 + quiet_zone=2*4=8,
        // total modules = 10. Unit = 200/10 = 20. Image = 10*20 = 200.
        assert!(img.dimensions().0 >= 200);
        assert!(img.dimensions().1 >= 200);
    }

    #[test]
    fn test_for_print_300() {
        let img = Renderer::<Luma<u8>>::new(&[Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 1)
            .for_print(300)
            .build();
        assert!(img.dimensions().0 >= 300);
        assert!(img.dimensions().1 >= 300);
    }

    #[test]
    fn test_for_social_twitter() {
        let img = Renderer::<Luma<u8>>::new(&[Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 1)
            .for_social("twitter")
            .build();
        assert!(img.dimensions().0 >= 400);
    }

    #[test]
    fn test_for_social_instagram() {
        let img = Renderer::<Luma<u8>>::new(&[Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 1)
            .for_social("instagram")
            .build();
        assert!(img.dimensions().0 >= 1080);
    }

    #[test]
    fn test_gradient_vertical() {
        use super::{Gradient, GradientDirection, apply_gradient_background};
        use image::DynamicImage;

        let qr = Renderer::<Rgba<u8>>::new(&[Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 0)
            .module_dimensions(10, 10)
            .build();
        let qr_dyn = DynamicImage::ImageRgba8(qr);
        let gradient = Gradient {
            direction: GradientDirection::Vertical,
            start_color: Rgba([255, 0, 0, 255]),
            end_color: Rgba([0, 0, 255, 255]),
        };
        let result = apply_gradient_background(&qr_dyn, &gradient);
        assert_eq!(result.dimensions(), (20, 20));
        // Dark pixels should be preserved (black).
        let dark = result.as_rgba8().unwrap().get_pixel(0, 0);
        assert_eq!(dark.0[0], 0);
        assert_eq!(dark.0[1], 0);
        assert_eq!(dark.0[2], 0);
    }

    #[test]
    fn test_gradient_horizontal() {
        use super::{Gradient, GradientDirection, apply_gradient_background};
        use image::DynamicImage;

        let qr = Renderer::<Rgba<u8>>::new(&[Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 0)
            .module_dimensions(10, 10)
            .build();
        let qr_dyn = DynamicImage::ImageRgba8(qr);
        let gradient = Gradient {
            direction: GradientDirection::Horizontal,
            start_color: Rgba([255, 0, 0, 255]),
            end_color: Rgba([0, 0, 255, 255]),
        };
        let result = apply_gradient_background(&qr_dyn, &gradient);
        assert_eq!(result.dimensions(), (20, 20));
    }

    #[test]
    fn test_gradient_diagonal() {
        use super::{Gradient, GradientDirection, apply_gradient_background};
        use image::DynamicImage;

        let qr = Renderer::<Rgba<u8>>::new(&[Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 0)
            .module_dimensions(10, 10)
            .build();
        let qr_dyn = DynamicImage::ImageRgba8(qr);
        let gradient = Gradient {
            direction: GradientDirection::Diagonal,
            start_color: Rgba([255, 255, 0, 255]),
            end_color: Rgba([0, 255, 255, 255]),
        };
        let result = apply_gradient_background(&qr_dyn, &gradient);
        assert_eq!(result.dimensions(), (20, 20));
    }
}
