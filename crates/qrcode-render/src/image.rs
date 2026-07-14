#![cfg(feature = "image")]
//! Image rendering via the [`image`] crate (PNG, JPEG, …).
//!
//! `QrCode::render::<image::Rgba<u8>>()` (or `Luma<u8>`, `Rgb<u8>`, …) produces
//! an `image::ImageBuffer`, which can be saved or encoded with the `image` API.
use crate::{Canvas, Pixel, StyledPixel};
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

    fn new(width: u32, height: u32, dark_pixel: P, light_pixel: P) -> Self {
        (dark_pixel, ImageBuffer::from_pixel(width, height, light_pixel))
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        self.1.put_pixel(x, y, self.0);
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
    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut result = rgba.clone();

    let sc = gradient.start_color.0;
    let ec = gradient.end_color.0;

    for y in 0..h {
        for x in 0..w {
            let pixel = *rgba.get_pixel(x, y);
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
                result.put_pixel(x, y, Rgba([nr, ng, nb, na]));
            }
        }
    }

    DynamicImage::ImageRgba8(result)
}

#[cfg(test)]
mod render_tests {
    use crate::Renderer;
    use image::{GenericImageView, ImageBuffer, Luma, Rgba};
    use qrcode_core::Color;

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
