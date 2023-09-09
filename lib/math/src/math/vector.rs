use std::{
    mem::size_of,
    ops::{Add, AddAssign, Div, Mul, MulAssign, Sub},
};

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, Encode, Decode, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub struct vec2_base<T: Copy + Clone> {
    pub x: T,
    pub y: T,
}

impl<T: Copy + Clone> vec2_base<T>
where
    T: std::default::Default,
{
    pub fn new(x: T, y: T) -> vec2_base<T> {
        vec2_base::<T> { x: x, y: y }
    }

    pub fn r(&mut self) -> &mut T {
        &mut self.x
    }
    pub fn g(&mut self) -> &mut T {
        &mut self.y
    }

    pub fn u(&mut self) -> &mut T {
        &mut self.x
    }
    pub fn v(&mut self) -> &mut T {
        &mut self.y
    }
}

impl<T: Copy + Clone + PartialEq<T>> PartialEq<vec2_base<T>> for vec2_base<T> {
    fn eq(&self, other: &vec2_base<T>) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl<T: Copy + Clone + Mul<Output = T>> Mul<T> for vec2_base<T> {
    type Output = vec2_base<T>;

    fn mul(self, rhs: T) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl<T: Copy + Clone + Div<Output = T>> Div<T> for vec2_base<T> {
    type Output = vec2_base<T>;

    fn div(self, rhs: T) -> Self::Output {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

impl<T: Copy + Clone + Add<Output = T>> Add<vec2_base<T>> for vec2_base<T> {
    type Output = vec2_base<T>;

    fn add(self, rhs: vec2_base<T>) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl<T: Copy + Clone + AddAssign<T>> AddAssign<vec2_base<T>> for vec2_base<T> {
    fn add_assign(&mut self, rhs: vec2_base<T>) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl<T: Copy + Clone + MulAssign<T>> MulAssign<T> for vec2_base<T> {
    fn mul_assign(&mut self, rhs: T) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl<T: Copy + Clone + Sub<Output = T>> Sub<vec2_base<T>> for vec2_base<T> {
    type Output = vec2_base<T>;

    fn sub(self, rhs: vec2_base<T>) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, Encode, Decode, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub struct vec3_base<T: Copy + Clone> {
    pub x: T,
    pub y: T,
    pub z: T,
}

impl<T: Copy + Clone> vec3_base<T> {
    pub fn dot(a: &Self, b: &Self) -> T
    where
        T: Copy + std::ops::Mul<T, Output = T> + std::ops::Add<T, Output = T>,
    {
        return a.x * b.x + a.y * b.y;
    }

    pub fn length(self) -> T
    where
        T: Copy + std::ops::Mul<T, Output = T> + std::ops::Add<T, Output = T> + num_traits::Float,
    {
        return Self::dot(&self, &self).sqrt();
    }

    pub fn normalize(self) -> Self
    where
        T: Default
            + Copy
            + std::ops::Mul<T, Output = T>
            + std::ops::Div<T, Output = T>
            + std::ops::Add<T, Output = T>
            + num_traits::Float,
    {
        let divisor = self.length();
        if divisor == T::zero() {
            Self::default()
        } else {
            let l = T::one() / divisor;
            Self {
                x: self.x * l,
                y: self.y * l,
                z: self.z * l,
            }
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, Encode, Decode, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub struct vec4_base<T: Copy + Clone> {
    pub x: T,
    pub y: T,
    pub z: T,
    pub w: T,
}

impl<T: Copy + Clone> vec4_base<T>
where
    T: std::default::Default,
{
    pub fn new(x: T, y: T, z: T, w: T) -> Self {
        Self {
            x: x,
            y: y,
            z: z,
            w: w,
        }
    }

    pub fn r(&self) -> T {
        self.x
    }
    pub fn g(&self) -> T {
        self.y
    }
    pub fn b(&self) -> T {
        self.z
    }
    pub fn a(&self) -> T {
        self.w
    }

    pub fn set_r(&mut self, val: T) {
        self.x = val;
    }
    pub fn set_g(&mut self, val: T) {
        self.y = val;
    }
    pub fn set_b(&mut self, val: T) {
        self.z = val;
    }
    pub fn set_a(&mut self, val: T) {
        self.w = val;
    }
}

#[allow(non_camel_case_types)]
pub type vec2 = vec2_base<f32>;
#[allow(non_camel_case_types)]
pub type dvec2 = vec2_base<f64>;
#[allow(non_camel_case_types)]
pub type vec3 = vec3_base<f32>;
#[allow(non_camel_case_types)]
pub type vec4 = vec4_base<f32>;
#[allow(non_camel_case_types)]
pub type ivec4 = vec4_base<i32>;
#[allow(non_camel_case_types)]
pub type ubvec4 = vec4_base<u8>;

pub fn read_i32_le(data: &[u8]) -> i32 {
    i32::from_le_bytes([data[0], data[1], data[2], data[3]])
}

impl ivec4 {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let (x, rest) = data.split_at(size_of::<i32>());
        let x = read_i32_le(x);

        let (y, rest) = rest.split_at(size_of::<i32>());
        let y = read_i32_le(y);

        let (z, rest) = rest.split_at(size_of::<i32>());
        let z = read_i32_le(z);

        let (w, _rest) = rest.split_at(size_of::<i32>());
        let w = read_i32_le(w);

        Self {
            x: x,
            y: y,
            z: z,
            w: w,
        }
    }
}
