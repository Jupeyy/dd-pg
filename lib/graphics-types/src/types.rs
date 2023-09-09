use std::fmt::Debug;

use bincode::{BorrowDecode, Decode, Encode};
use math::math::vector::vec2;
use num_derive::FromPrimitive;

#[derive(Debug)]
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

#[derive(Debug, Copy, Clone, Encode, Decode)]
pub struct WindowProps {
    pub canvas_width: f64,
    pub canvas_height: f64,

    pub window_width: u32,
    pub window_height: u32,
}

#[derive(Debug, FromPrimitive)]
pub enum ImageFormat {
    Rgb = 0,
    Rgba = 1,
    SingleComponent = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Encode, Decode)]
pub enum DrawModes {
    Quads = 1,
    Lines = 2,
    Triangles = 3,
}

pub trait GraphicsBackendMemoryStaticCleaner: Debug + Send + Sync {
    fn destroy(&self, mem: &'static mut [u8]);
}

#[derive(Debug)]
pub struct GraphicsBackendMemoryStatic {
    pub mem: Option<&'static mut [u8]>,
    pub deallocator: Option<Box<dyn GraphicsBackendMemoryStaticCleaner>>,
}

impl Drop for GraphicsBackendMemoryStatic {
    fn drop(&mut self) {
        if let Some(deallocator) = self.deallocator.take() {
            deallocator.destroy(self.mem.take().unwrap());
        }
    }
}

#[derive(Debug)]
pub enum GraphicsBackendMemory {
    Static(GraphicsBackendMemoryStatic),
    Vector(Vec<u8>),
}

impl Encode for GraphicsBackendMemory {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        match self {
            GraphicsBackendMemory::Static { .. } => {
                panic!("encoding static data is unsafe, leaks memory and is not wanted")
            }
            GraphicsBackendMemory::Vector(data) => {
                let conf = *encoder.config();
                bincode::encode_into_writer(data, encoder.writer(), conf)
            }
        }
    }
}

impl Decode for GraphicsBackendMemory {
    fn decode<D: bincode::de::Decoder>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let conf = *decoder.config();
        let res = bincode::decode_from_reader::<Vec<u8>, _, _>(decoder.reader(), conf)?;
        Ok(Self::Vector(res))
    }
}

impl<'de> BorrowDecode<'de> for GraphicsBackendMemory {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Self::decode(decoder)
    }
}

impl GraphicsBackendMemory {
    pub fn copy_from_slice(&mut self, slice: &[u8]) {
        match self {
            GraphicsBackendMemory::Static(GraphicsBackendMemoryStatic { mem, .. }) => {
                mem.as_mut().unwrap().copy_from_slice(slice)
            }
            GraphicsBackendMemory::Vector(mem) => mem.copy_from_slice(slice),
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        match self {
            GraphicsBackendMemory::Static(GraphicsBackendMemoryStatic { mem, .. }) => {
                mem.as_ref().unwrap()
            }
            GraphicsBackendMemory::Vector(mem) => mem.as_slice(),
        }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        match self {
            GraphicsBackendMemory::Static(GraphicsBackendMemoryStatic { mem, .. }) => {
                mem.as_mut().unwrap()
            }
            GraphicsBackendMemory::Vector(mem) => mem.as_mut_slice(),
        }
    }

    pub fn len(&self) -> usize {
        self.as_slice().len()
    }
}
