use bincode::{Decode, Encode};
use math::math::vector::vec2;
use num_derive::FromPrimitive;

pub enum GraphicsMemoryAllocationType {
    Texture,
    Buffer,
}

#[derive(Debug)]
pub struct VideoMode {
    pub canvas_width: i32,
    pub canvas_height: i32,
    pub window_width: i32,
    pub window_height: i32,
    pub refresh_rate: u32,
    pub red: u32,
    pub green: u32,
    pub blue: u32,
    pub format: u32,
}

impl VideoMode {
    pub const fn new(
        canvas_width: i32,
        canvas_height: i32,
        window_width: i32,
        window_height: i32,
        refresh_rate: u32,
        red: u32,
        green: u32,
        blue: u32,
        format: u32,
    ) -> VideoMode {
        VideoMode {
            canvas_width: canvas_width,
            canvas_height: canvas_height,
            window_width: window_width,
            window_height: window_height,
            refresh_rate: refresh_rate,
            red: red,
            green: green,
            blue: blue,
            format: format,
        }
    }
}

#[repr(C)]
pub struct CQuadItem {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl CQuadItem {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> CQuadItem {
        CQuadItem {
            x,
            y,
            width: w,
            height: h,
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Triangle {
    pub vertices: [vec2; 3],
}

impl Triangle {
    pub fn new(vertices: &[vec2; 3]) -> Self {
        Self {
            vertices: *vertices,
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Line {
    pub vertices: [vec2; 2],
}

impl Line {
    pub fn new(vertices: &[vec2; 2]) -> Self {
        Self {
            vertices: *vertices,
        }
    }
}

#[derive(Copy, Clone, Default)]
pub struct WindowProps {
    pub canvas_width: u32,
    pub canvas_height: u32,

    pub window_width: u32,
    pub window_height: u32,
}

#[derive(FromPrimitive)]
pub enum ImageFormat {
    Rgb = 0,
    Rgba = 1,
    SingleComponent = 2,
}

#[derive(PartialEq, Encode, Decode)]
pub enum DrawModes {
    Quads = 1,
    Lines = 2,
    Triangles = 3,
}
