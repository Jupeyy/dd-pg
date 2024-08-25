use math::math::{
    distance,
    vector::{ffixed, fvec2, vec2},
};

pub fn in_radius(pos1: &fvec2, pos2: &vec2, radius: f32) -> bool {
    distance(&vec2::new(pos1.x.to_num(), pos1.y.to_num()), pos2) < radius
}

pub fn rotate(center: &fvec2, rotation: ffixed, points: &mut [fvec2]) {
    let c = ffixed::from_num(rotation.to_num::<f64>().cos());
    let s = ffixed::from_num(rotation.to_num::<f64>().sin());

    for point in points.iter_mut() {
        let x = point.x - center.x;
        let y = point.y - center.y;
        *point = fvec2 {
            x: x * c - y * s + center.x,
            y: x * s + y * c + center.y,
        };
    }
}
