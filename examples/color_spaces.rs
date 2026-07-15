use qrcode_rs::render::colors::{CmykColor, ColorSpace, LabColor, RgbColor};

fn main() {
    let rgb = RgbColor::new(51, 102, 153);
    let cmyk = CmykColor::from_rgb(rgb);
    let lab = LabColor::from_rgb(rgb);

    println!("rgb: {:?}", rgb.to_array());
    println!("cmyk: {:?}", cmyk.to_array());
    println!("lab: L={:.2}, a={:.2}, b={:.2}", lab.l, lab.a, lab.b);
    println!("cmyk -> rgb: {:?}", cmyk.to_rgb());
    println!("lab -> rgb: {:?}", lab.to_rgb());
}
