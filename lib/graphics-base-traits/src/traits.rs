use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use anyhow::anyhow;
use graphics_types::{
    commands::{GRAPHICS_DEFAULT_UNIFORM_SIZE, GRAPHICS_MAX_UNIFORM_RENDER_COUNT},
    rendering::{GlVertex, SVertex},
};
use hiarc::{hiarc_safer_rc_refcell, Hiarc};
use pool::mt_datatypes::PoolVec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize)]
pub enum GraphicsStreamedUniformDataType {
    Arbitrary {
        element_size: usize,
        element_count: usize,
    },
    None,
}

impl GraphicsStreamedUniformDataType {
    pub fn count_mut(&mut self) -> &mut usize {
        match self {
            GraphicsStreamedUniformDataType::Arbitrary { element_count, .. } => element_count,
            GraphicsStreamedUniformDataType::None => {
                panic!("this should not happen and indicates a bug in the implementation.")
            }
        }
    }
}

#[derive(Debug, Hiarc)]
pub enum GraphicsStreamedUniformRawData {
    Raw(&'static mut [u8]),
    Vector(Vec<u8>),
}

impl Deref for GraphicsStreamedUniformRawData {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match self {
            GraphicsStreamedUniformRawData::Raw(r) => r,
            GraphicsStreamedUniformRawData::Vector(r) => r,
        }
    }
}

impl DerefMut for GraphicsStreamedUniformRawData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            GraphicsStreamedUniformRawData::Raw(r) => r,
            GraphicsStreamedUniformRawData::Vector(r) => r,
        }
    }
}

impl Serialize for GraphicsStreamedUniformRawData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let self_slice: &[u8] = match self {
            GraphicsStreamedUniformRawData::Raw(r) => r,
            GraphicsStreamedUniformRawData::Vector(r) => r,
        };

        <&[u8]>::serialize(&self_slice, serializer)
    }
}

impl<'de> Deserialize<'de> for GraphicsStreamedUniformRawData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let self_slice = <Vec<u8>>::deserialize(deserializer)?;

        Ok(Self::Vector(self_slice.to_vec()))
    }
}

/// only allows to get either of the memebers
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct GraphicsStreamedUniformData {
    raw: GraphicsStreamedUniformRawData,
    used_count: GraphicsStreamedUniformDataType,
}

impl GraphicsStreamedUniformData {
    pub fn new(raw: GraphicsStreamedUniformRawData) -> Self {
        Self {
            raw,
            used_count: GraphicsStreamedUniformDataType::None,
        }
    }

    pub fn raw_as<T>(&mut self) -> &mut [T]
    where
        T: Sized,
    {
        assert!(
            std::mem::size_of::<T>()
                < GRAPHICS_MAX_UNIFORM_RENDER_COUNT * GRAPHICS_DEFAULT_UNIFORM_SIZE
        );
        unsafe {
            std::slice::from_raw_parts_mut::<T>(
                self.raw.as_ptr() as *mut _,
                (GRAPHICS_MAX_UNIFORM_RENDER_COUNT * GRAPHICS_DEFAULT_UNIFORM_SIZE)
                    / std::mem::size_of::<T>(),
            )
        }
    }
}

#[derive(Debug, Hiarc)]
pub enum GraphicsStreamVertices {
    Static(&'static mut [GlVertex]),
    Vec(Vec<GlVertex>),
}

impl Deref for GraphicsStreamVertices {
    type Target = [GlVertex];

    fn deref(&self) -> &Self::Target {
        match self {
            GraphicsStreamVertices::Static(v) => v,
            GraphicsStreamVertices::Vec(v) => v.as_slice(),
        }
    }
}

impl DerefMut for GraphicsStreamVertices {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            GraphicsStreamVertices::Static(v) => v,
            GraphicsStreamVertices::Vec(v) => v.as_mut_slice(),
        }
    }
}

#[hiarc_safer_rc_refcell(sync_send_wrapper)]
#[derive(Debug, Hiarc)]
pub struct GraphicsStreamedData {
    vertices: GraphicsStreamVertices,

    /// number of vertices used
    num_vertices: usize,

    uniform_buffers: PoolVec<GraphicsStreamedUniformData>,
    /// number of uniform instances used
    num_uniforms: usize,
}

#[hiarc_safer_rc_refcell]
impl GraphicsStreamedData {
    pub fn new(
        vertices: GraphicsStreamVertices,

        uniform_buffers: PoolVec<GraphicsStreamedUniformData>,
    ) -> Self {
        Self {
            vertices,
            uniform_buffers,

            num_uniforms: 0,
            num_vertices: 0,
        }
    }

    pub fn used_vertices_as_vec(&self) -> Vec<SVertex> {
        self.vertices[0..self.num_vertices].to_vec()
    }

    pub fn vertices_count(&self) -> usize {
        self.num_vertices
    }

    pub fn reset_vertices_count(&mut self) {
        self.num_vertices = 0;
    }

    pub fn max_vertices_len_and_cur_count(&self) -> (usize, usize) {
        (self.vertices.len(), self.num_vertices)
    }

    pub fn add_vertices(&mut self, add_vertices: &[SVertex]) {
        self.vertices[self.num_vertices..self.num_vertices + add_vertices.len()]
            .copy_from_slice(add_vertices);
        self.num_vertices += add_vertices.len();
    }

    pub fn is_full(&self, add_count: usize) -> bool {
        self.num_vertices + add_count >= self.vertices.len()
    }

    pub fn allocate_uniform_instance(&mut self) -> anyhow::Result<usize> {
        if self.num_uniforms < self.uniform_buffers.len() {
            let index = self.num_uniforms;
            self.num_uniforms += 1;
            Ok(index)
        } else {
            Err(anyhow!("out of uniform instances"))
        }
    }

    /// returns: uniform count, should flush
    pub fn uniform_info<T>(&mut self, instance: usize) -> (usize, bool)
    where
        T: Sized,
    {
        let uniform_instance = &mut self.uniform_buffers[instance];
        let count = match &mut uniform_instance.used_count {
            GraphicsStreamedUniformDataType::Arbitrary { element_count, .. } => *element_count,
            GraphicsStreamedUniformDataType::None => {
                uniform_instance.used_count = GraphicsStreamedUniformDataType::Arbitrary {
                    element_size: std::mem::size_of::<T>(),
                    element_count: 0,
                };
                0
            }
        };
        (count, count >= uniform_instance.raw_as::<T>().len())
    }

    /// returns: uniform count, should flush
    pub fn add_uniform<T>(&mut self, instance: usize, info: T) -> (usize, bool)
    where
        T: Sized,
    {
        let uniform_instance = &mut self.uniform_buffers[instance];

        let index = match &mut uniform_instance.used_count {
            GraphicsStreamedUniformDataType::Arbitrary { element_count, .. } => {
                let index = *element_count;
                *element_count += 1;
                index
            }
            GraphicsStreamedUniformDataType::None => {
                uniform_instance.used_count = GraphicsStreamedUniformDataType::Arbitrary {
                    element_size: std::mem::size_of::<T>(),
                    element_count: 1,
                };
                0
            }
        };

        uniform_instance.raw_as::<T>()[index] = info;
        self.uniform_info::<T>(instance)
    }

    pub fn uniform_instance_count(&self) -> usize {
        self.num_uniforms
    }

    pub fn uniform_is_full(&self, add_count: usize) -> bool {
        self.uniform_buffers.len() <= self.num_uniforms + add_count
    }

    /// intended for wasm API only
    pub fn reset_uniform_instances(&mut self) {
        for i in 0..self.num_uniforms {
            self.uniform_buffers[i].used_count = GraphicsStreamedUniformDataType::None;
        }
        self.num_uniforms = 0;
    }

    /// intended for wasm API only
    pub fn serialize_uniform_instances_as_vec(&self) -> Vec<Vec<u8>> {
        let mut res: Vec<Vec<u8>> = Default::default();
        for i in 0..self.num_uniforms {
            res.push(
                bincode::serde::encode_to_vec(
                    &self.uniform_buffers[i],
                    bincode::config::standard(),
                )
                .unwrap(),
            );
        }
        res
    }

    pub fn deserialize_uniform_instances_from_vec(&mut self, src: Vec<Vec<u8>>) {
        let start_index = self.num_uniforms;
        self.num_uniforms += src.len();
        for i in start_index..self.num_uniforms {
            let buffer = &mut self.uniform_buffers[i];
            let buf: GraphicsStreamedUniformData = bincode::serde::decode_from_slice(
                &src[i - start_index],
                bincode::config::standard(),
            )
            .unwrap()
            .0;
            buffer.used_count = buf.used_count;
            match &buffer.used_count {
                GraphicsStreamedUniformDataType::Arbitrary {
                    element_size,
                    element_count,
                } => {
                    let dst_slice: &mut [u8] = match &mut buffer.raw {
                        GraphicsStreamedUniformRawData::Raw(mem) => mem,
                        GraphicsStreamedUniformRawData::Vector(mem) => mem.as_mut(),
                    };
                    let src_slice: &[u8] = match &buf.raw {
                        GraphicsStreamedUniformRawData::Raw(mem) => mem,
                        GraphicsStreamedUniformRawData::Vector(mem) => &mem,
                    };
                    let size = element_size * element_count;
                    dst_slice[0..size].copy_from_slice(&src_slice[0..size]);
                }
                GraphicsStreamedUniformDataType::None => {}
            }
        }
    }

    pub fn uniform_used_count_of_instance(
        &self,
        instance_index: usize,
    ) -> GraphicsStreamedUniformDataType {
        self.uniform_buffers[instance_index].used_count
    }
}
