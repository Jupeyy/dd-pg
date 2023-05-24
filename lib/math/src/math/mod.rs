use self::vector::vec2;

pub mod vector;

pub const PI: f32 = 3.1415926535897932384626433;

pub fn mix<T, TB>(a: &T, b: &T, amount: TB) -> T
where
    T: std::ops::Sub<T, Output = T>
        + std::ops::Add<T, Output = T>
        + std::ops::Mul<TB, Output = T>
        + Copy,
{
    return *a + (*b - *a) * amount;
}

pub fn blend<T, TB>(a: &T, b: &T, one: TB, amount: TB) -> T
where
    T: std::ops::Sub<T, Output = T>
        + std::ops::Add<T, Output = T>
        + std::ops::Mul<TB, Output = T>
        + Copy,
    TB: Copy + std::ops::Sub<TB, Output = TB>,
{
    return *a * (one - amount) + (*b) * amount;
}

const FXP_SCALE: i32 = 1 << 10;
pub fn fx2f(v: i32) -> f32 {
    return (v as f32) / (FXP_SCALE as f32);
}

pub fn dot(a: &vec2, b: &vec2) -> f32 {
    return a.x * b.x + a.y * b.y;
}

pub fn length(a: &vec2) -> f32 {
    return (dot(a, a)).sqrt();
}

pub fn normalize(v: &vec2) -> vec2 {
    let divisor = length(v);
    if divisor == 0.0 {
        return vec2 { x: 0.0, y: 0.0 };
    }
    let l = 1.0 / divisor;
    return vec2 {
        x: v.x * l,
        y: v.y * l,
    };
}

pub fn distance(a: &vec2, b: &vec2) -> f32 {
    return length(&(*a - *b));
}

pub fn closest_point_on_line(
    line_point_a: &vec2,
    line_point_b: &vec2,
    target_point: &vec2,
    out_pos: &mut vec2,
) -> bool {
    let seg_ab = *line_point_b - *line_point_a;
    let squared_magnitude_ab = dot(&seg_ab, &seg_ab);
    if squared_magnitude_ab > 0.0 {
        let ap = *target_point - *line_point_a;
        let ap_dot_ab = dot(&ap, &seg_ab);
        let t = ap_dot_ab / squared_magnitude_ab;
        *out_pos = *line_point_a + seg_ab * t.clamp(0.0, 1.0);
        return true;
    }
    false
}

pub fn round_to_int(f: f32) -> i32 {
    return if f > 0.0 {
        (f + 0.5) as i32
    } else {
        (f - 0.5) as i32
    };
}
