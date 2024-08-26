use std::fmt::Debug;

use hiarc::Hiarc;
use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};

use crate::commands::TexFlags;

#[derive(Debug, Clone, Copy)]
pub enum GraphicsMemoryAllocationType {
    Texture {
        width: usize,
        height: usize,
        depth: usize,
        is_3d_tex: bool,
        flags: TexFlags,
    },
    Buffer {
        required_size: usize,
    },
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
            canvas_width,
            canvas_height,
            window_width,
            window_height,
            refresh_rate,
            red,
            green,
            blue,
            format,
        }
    }
}

#[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowProps {
    pub canvas_width: f64,
    pub canvas_height: f64,

    pub window_width: u32,
    pub window_height: u32,
}

#[derive(Debug, Hiarc, FromPrimitive, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Rgb = 0,
    Rgba = 1,
    SingleComponent = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DrawModes {
    Quads = 1,
    Lines = 2,
    Triangles = 3,
}

pub trait GraphicsBackendMemoryStaticCleaner: Debug + Send + Sync {
    fn destroy(&self, mem: &'static mut [u8]);
}

#[derive(Debug, Hiarc)]
pub struct GraphicsBackendMemoryStatic {
    pub mem: Option<&'static mut [u8]>,
    #[hiarc_skip_unsafe]
    pub deallocator: Option<Box<dyn GraphicsBackendMemoryStaticCleaner>>,
}

impl Drop for GraphicsBackendMemoryStatic {
    fn drop(&mut self) {
        if let Some(deallocator) = self.deallocator.take() {
            deallocator.destroy(self.mem.take().unwrap());
        }
    }
}

#[derive(Debug, Hiarc)]
pub enum GraphicsBackendMemory {
    Static(GraphicsBackendMemoryStatic),
    Vector(Vec<u8>),
}

impl Serialize for GraphicsBackendMemory {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            GraphicsBackendMemory::Static { .. } => {
                panic!("encoding static data is unsafe, leaks memory and is not wanted")
            }
            GraphicsBackendMemory::Vector(data) => serde::Serialize::serialize(data, serializer),
        }
    }
}

impl<'de> Deserialize<'de> for GraphicsBackendMemory {
    /*fn decode<D: bincode::de::Decoder>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let conf = *decoder.config();
        let res = bincode::decode_from_reader::<Vec<u8>, _, _>(decoder.reader(), conf)?;
        Ok(Self::Vector(res))
    }*/
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self::Vector(<Vec<u8>>::deserialize(deserializer)?))
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
