use palette::FromColor;

use crate::math::vector::ubvec4;

pub fn legacy_color_to_rgba(legacy_color_code: i32, ignore_alpha: bool) -> ubvec4 {
    let a = (legacy_color_code >> 24) & 0xFF;
    let x = ((legacy_color_code >> 16) & 0xFF) as f64 / 255.0;
    let y = ((legacy_color_code >> 8) & 0xFF) as f64 / 255.0;
    let z = ((legacy_color_code >> 0) & 0xFF) as f64 / 255.0;

    let hsv = palette::Hsl::new(x * 360.0, y, z);
    let mut rgb = palette::rgb::LinSrgb::from_color(hsv);

    // clamp
    rgb.red = rgb.red.clamp(0.0, 1.0);
    rgb.blue = rgb.blue.clamp(0.0, 1.0);
    rgb.green = rgb.green.clamp(0.0, 1.0);

    ubvec4::new(
        (rgb.red * 255.0) as u8,
        (rgb.green * 255.0) as u8,
        (rgb.blue * 255.0) as u8,
        if ignore_alpha { 255 } else { a as u8 },
    )
}
