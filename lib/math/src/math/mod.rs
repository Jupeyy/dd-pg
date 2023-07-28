use rand::SeedableRng;

use self::vector::{vec2, vec2_base};

pub mod vector;

pub const PId: f64 = 3.1415926535897932384626433;
pub const PI: f32 = PId as f32;

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

pub fn lerp<T, TB>(a: &T, b: &T, amount: TB) -> T
where
    T: std::ops::Sub<T, Output = T>
        + std::ops::Add<T, Output = T>
        + std::ops::Mul<TB, Output = T>
        + Copy,
    TB: Copy + std::ops::Sub<TB, Output = TB> + num_traits::Float,
{
    return blend::<T, TB>(a, b, TB::one(), amount);
}

const FXP_SCALE: i32 = 1 << 10;
pub fn fx2f(v: i32) -> f32 {
    return (v as f32) / (FXP_SCALE as f32);
}

pub fn dot<T>(a: &vec2_base<T>, b: &vec2_base<T>) -> T
where
    T: Copy + std::ops::Mul<T, Output = T> + std::ops::Add<T, Output = T>,
{
    return a.x * b.x + a.y * b.y;
}

pub fn length<T>(a: &vec2_base<T>) -> T
where
    T: Copy + std::ops::Mul<T, Output = T> + std::ops::Add<T, Output = T> + num_traits::Float,
{
    return (dot(a, a)).sqrt();
}

pub fn normalize<T>(v: &vec2_base<T>) -> vec2_base<T>
where
    T: Default
        + Copy
        + std::ops::Mul<T, Output = T>
        + std::ops::Div<T, Output = T>
        + std::ops::Add<T, Output = T>
        + num_traits::Float,
{
    let divisor = length(v);
    if divisor == T::zero() {
        vec2_base::<T>::default()
    } else {
        let l = T::one() / divisor;
        vec2_base::<T>::new(v.x * l, v.y * l)
    }
}

pub fn normalize_pre_length<T>(v: &vec2_base<T>, len: T) -> vec2_base<T>
where
    T: Default
        + Copy
        + std::ops::Mul<T, Output = T>
        + std::ops::Add<T, Output = T>
        + num_traits::Float,
{
    if len == T::zero() {
        vec2_base::<T>::default()
    } else {
        vec2_base::<T>::new(v.x / len, v.y / len)
    }
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

pub fn angle(a: &vec2) -> f32 {
    if a.x == 0.0 && a.y == 0.0 {
        return 0.0;
    } else if a.x == 0.0 {
        return if a.y < 0.0 { -PI / 2.0 } else { PI / 2.0 };
    }
    let mut result = (a.y / a.x).atan();
    if a.x < 0.0 {
        result = result + PI;
    }
    return result;
}

pub fn random_float() -> f32 {
    let mut r = rand::rngs::StdRng::seed_from_u64(0);
    rand::Rng::gen_range(&mut r, 0.0..=1.0)
}
