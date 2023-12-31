use std::fmt::Debug;

use anyhow::anyhow;
use graphics_types::{
    commands::RenderSpriteInfo,
    rendering::{GlVertex, SVertex},
};
use pool::mt_datatypes::PoolVec;

#[derive(Debug, Copy, Clone)]
pub enum GraphicsStreamedUniformDataType {
    Sprites(usize),
    Arbitrary {
        element_size: usize,
        element_count: usize,
    },
    None,
}

impl GraphicsStreamedUniformDataType {
    pub fn as_mut(&mut self) -> &mut usize {
        match self {
            GraphicsStreamedUniformDataType::Sprites(count) => count,
            GraphicsStreamedUniformDataType::Arbitrary { element_count, .. } => element_count,
            GraphicsStreamedUniformDataType::None => {
                panic!("this should not happen and indicates a bug in the implementation.")
            }
        }
    }
}

/// only allows to get either of the memebers
#[derive(Debug)]
pub struct GraphicsStreamedUniformData {
    sprites: &'static mut [RenderSpriteInfo],
    raw: &'static mut [u8],
    used_count: GraphicsStreamedUniformDataType,
}

impl GraphicsStreamedUniformData {
    pub fn new(sprites: &'static mut [RenderSpriteInfo], raw: &'static mut [u8]) -> Self {
        Self {
            sprites,
            raw,
            used_count: GraphicsStreamedUniformDataType::None,
        }
    }
}

pub struct GraphicsStreamedUniformDataSpritesBorrowMut<'a> {
    pub sprites: &'a mut [RenderSpriteInfo],
    pub used_count: &'a mut usize,
}

pub struct GraphicsArbitraryUniformDataSpritesBorrowMut<'a> {
    pub raw: &'a mut [u8],
    pub used_count: &'a mut usize,
}

#[derive(Debug)]
pub struct GraphicsStreamedData {
    vertices: &'static mut [GlVertex],

    /// number of vertices used
    num_vertices: usize,

    uniform_buffers: PoolVec<GraphicsStreamedUniformData>,
    /// number of uniform instances used
    num_uniforms: usize,
}

impl GraphicsStreamedData {
    pub fn new(
        vertices: &'static mut [GlVertex],

        uniform_buffers: PoolVec<GraphicsStreamedUniformData>,
    ) -> Self {
        Self {
            vertices,
            uniform_buffers,

            num_uniforms: 0,
            num_vertices: 0,
        }
    }
}

pub trait GraphicsStreamDataInterface: Debug {
    fn vertices(&self) -> &[SVertex];
    fn vertices_mut(&mut self) -> &mut [SVertex];
    fn vertices_count(&self) -> usize;
    fn vertices_count_mut(&mut self) -> &mut usize;
    fn vertices_and_count(&self) -> (&[SVertex], &usize);
    fn vertices_and_count_mut(&mut self) -> (&mut [SVertex], &mut usize);

    /// returns the instance id, used in latter functions
    fn allocate_uniform_instance(&mut self) -> anyhow::Result<usize>;
    fn get_sprites_uniform_instance(
        &mut self,
        instance: usize,
    ) -> GraphicsStreamedUniformDataSpritesBorrowMut;
    fn get_arbitrary_uniform_instance(
        &mut self,
        instance: usize,
        size_of_el: usize,
    ) -> GraphicsArbitraryUniformDataSpritesBorrowMut;

    fn uniform_instance_count(&self) -> usize;
    fn uniform_used_count_of_instance(
        &self,
        instance_index: usize,
    ) -> GraphicsStreamedUniformDataType;

    fn set_from_graphics_streamed_data(&mut self, streamed_data: GraphicsStreamedData);
}

impl GraphicsStreamDataInterface for GraphicsStreamedData {
    fn vertices(&self) -> &[SVertex] {
        self.vertices
    }

    fn vertices_mut(&mut self) -> &mut [SVertex] {
        self.vertices
    }

    fn vertices_count(&self) -> usize {
        self.num_vertices
    }

    fn vertices_count_mut(&mut self) -> &mut usize {
        &mut self.num_vertices
    }

    fn vertices_and_count(&self) -> (&[SVertex], &usize) {
        (&self.vertices, &self.num_vertices)
    }

    fn vertices_and_count_mut(&mut self) -> (&mut [SVertex], &mut usize) {
        (&mut self.vertices, &mut self.num_vertices)
    }

    fn allocate_uniform_instance(&mut self) -> anyhow::Result<usize> {
        if self.num_uniforms < self.uniform_buffers.len() {
            let index = self.num_uniforms;
            self.num_uniforms += 1;
            Ok(index)
        } else {
            Err(anyhow!("out of uniform instances"))
        }
    }

    fn get_sprites_uniform_instance(
        &mut self,
        instance: usize,
    ) -> GraphicsStreamedUniformDataSpritesBorrowMut {
        let uniform_instance = &mut self.uniform_buffers[instance];
        uniform_instance.used_count = GraphicsStreamedUniformDataType::Sprites(0);

        GraphicsStreamedUniformDataSpritesBorrowMut {
            sprites: uniform_instance.sprites,
            used_count: uniform_instance.used_count.as_mut(),
        }
    }

    fn get_arbitrary_uniform_instance(
        &mut self,
        instance: usize,
        size_of_el: usize,
    ) -> GraphicsArbitraryUniformDataSpritesBorrowMut {
        let uniform_instance = &mut self.uniform_buffers[instance];
        uniform_instance.used_count = GraphicsStreamedUniformDataType::Arbitrary {
            element_count: 0,
            element_size: size_of_el,
        };

        GraphicsArbitraryUniformDataSpritesBorrowMut {
            raw: uniform_instance.raw,
            used_count: uniform_instance.used_count.as_mut(),
        }
    }

    fn uniform_instance_count(&self) -> usize {
        self.num_uniforms
    }

    fn uniform_used_count_of_instance(
        &self,
        instance_index: usize,
    ) -> GraphicsStreamedUniformDataType {
        self.uniform_buffers[instance_index].used_count
    }

    fn set_from_graphics_streamed_data(&mut self, streamed_data: GraphicsStreamedData) {
        *self = streamed_data;
    }
}
