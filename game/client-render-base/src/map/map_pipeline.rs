use std::ops::DerefMut;

use graphics::handles::{
    backend::backend::GraphicsBackendHandle,
    buffer_object::buffer_object::BufferObject,
    texture::texture::{TextureType, TextureType2dArray},
};
use graphics_backend_traits::plugin::{
    BackendCustomPipeline, BackendDeviceSize, BackendPipelineLayout, BackendPushConstant,
    BackendRenderExecuteInterface, BackendRenderInterface, BackendResourceDescription,
    BackendShaderStage, BackendVertexFormat, BackendVertexInputAttributeDescription,
    GraphicsBufferObjectAccess, GraphicsBufferObjectAccessAndRewrite, GraphicsObjectRewriteFunc,
    GraphicsUniformAccessAndRewrite, SubRenderPassAttributes,
};
use graphics_types::{
    commands::{
        AllCommands, CommandsRender, CommandsRenderMod, SColorf, GRAPHICS_DEFAULT_UNIFORM_SIZE,
        GRAPHICS_MAX_UNIFORM_RENDER_COUNT,
    },
    rendering::{ColorRgba, GlColorf, State, StateTexture, StateTexture2dArray},
};
use hiarc::Hiarc;
use math::math::vector::{ubvec2, ubvec4, usvec2, vec2};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use pool::{
    mixed_datatypes::StringPool, mixed_pool::Pool, mt_datatypes::PoolVec, mt_pool::Pool as MtPool,
};
use serde::{Deserialize, Serialize};

const MOD_NAME: &str = "internal::Map";

pub const GRAPHICS_MAX_QUADS_RENDER_COUNT: usize = (GRAPHICS_MAX_UNIFORM_RENDER_COUNT
    * GRAPHICS_DEFAULT_UNIFORM_SIZE)
    / std::mem::size_of::<QuadRenderInfo>();

#[derive(Debug, FromPrimitive, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u64)]
pub enum MapPipelineNames {
    TilePipeline,
    TileBorderPipeline,
    QuadPipeline,
}

#[derive(Debug, Hiarc, Clone, Copy, Serialize, Deserialize)]
pub struct TileLayerDrawInfo {
    pub quad_offset: usize,
    pub quad_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandRenderTileLayer {
    pub state: State,
    pub texture_index: StateTexture2dArray,
    pub color: SColorf, // the color of the whole tilelayer -- already enveloped

    pub draws: PoolVec<TileLayerDrawInfo>,

    pub buffer_object_index: u128,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandRenderBorderTile {
    pub state: State,
    pub texture_index: StateTexture2dArray,
    pub color: SColorf, // the color of the whole tilelayer -- already enveloped

    pub draw: TileLayerDrawInfo,

    pub buffer_object_index: u128,
    pub buffer_object_offset: usize,

    pub offset: vec2,
    pub scale: vec2,
}

#[repr(C)]
#[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
pub struct QuadRenderInfo {
    pub color: ColorRgba,
    pub offsets: vec2,
    pub rotation: f32,
    // allows easier upload for uniform buffers because of the alignment requirements
    pub padding: f32,
}

impl QuadRenderInfo {
    pub fn new(color: ColorRgba, offsets: vec2, rotation: f32) -> Self {
        Self {
            color,
            offsets,
            rotation,
            padding: 0.0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandRenderQuadLayer {
    pub state: State,
    pub texture_index: StateTexture,

    pub buffer_object_index: u128,
    pub quad_info_uniform_instance: usize,
    pub quad_num: usize,
    pub quad_offset: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CommandsRenderMap {
    TileLayer(CommandRenderTileLayer),   // render a tilelayer
    BorderTile(CommandRenderBorderTile), // render one tile multiple times
    QuadLayer(CommandRenderQuadLayer),   // render a quad layer
}

#[derive(Default)]
#[repr(C)]
pub struct UniformTileGPos {
    pub pos: [f32; 4 * 2],
}

#[derive(Default)]
#[repr(C)]
pub struct UniformTileGPosBorder {
    pub base: UniformTileGPos,
    pub offset: vec2,
    pub scale: vec2,
}

pub type SUniformTileGVertColor = ColorRgba;

#[derive(Default)]
#[repr(C)]
pub struct UniformTileGVertColorAlign {
    pub pad: [f32; (64 - 48) / 4],
}

#[derive(Default)]
#[repr(C)]
pub struct UniformQuadGPos {
    pub pos: [f32; 4 * 2],
    pub quad_offset: i32,
}

const TILE_LAYER_VERTEX_SIZE: usize = std::mem::size_of::<usvec2>();
const TILE_LAYER_TEXTURED_VERTEX_SIZE: usize =
    TILE_LAYER_VERTEX_SIZE + std::mem::size_of::<ubvec2>();

const TILE_LAYER_BORDER_VERTEX_SIZE: usize = std::mem::size_of::<f32>() * 2;
const TILE_LAYER_BORDER_TEXTURED_VERTEX_SIZE: usize =
    TILE_LAYER_BORDER_VERTEX_SIZE + std::mem::size_of::<ubvec4>();

const QUAD_LAYER_VERTEX_SIZE: usize =
    std::mem::size_of::<f32>() * 4 + std::mem::size_of::<u8>() * 4;
const QUAD_LAYER_TEXTURED_VERTEX_SIZE: usize =
    QUAD_LAYER_VERTEX_SIZE + std::mem::size_of::<f32>() * 2;

#[derive(Debug, Hiarc)]
pub struct MapPipeline {
    pipe_name_offset: u64,
    accesses_pool: MtPool<Vec<GraphicsBufferObjectAccess>>,
}

impl MapPipeline {
    pub fn new_boxed() -> Box<dyn BackendCustomPipeline> {
        Box::new(Self {
            pipe_name_offset: 0,
            accesses_pool: MtPool::with_capacity(32),
        })
    }

    fn tile_pipeline_layout(has_sampler: bool) -> BackendPipelineLayout {
        let mut attribute_descriptors: Vec<BackendVertexInputAttributeDescription> =
            Default::default();
        attribute_descriptors.push(BackendVertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: BackendVertexFormat::UsVec2,
            offset: 0,
        });
        if has_sampler {
            attribute_descriptors.push(BackendVertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: BackendVertexFormat::UbVec2,
                offset: TILE_LAYER_VERTEX_SIZE as u32,
            });
        }

        let set_layouts = [BackendResourceDescription::Fragment2DArrayTexture].to_vec();

        let vert_push_constant_size = std::mem::size_of::<UniformTileGPos>();

        let frag_push_constant_size = std::mem::size_of::<SUniformTileGVertColor>();

        let push_constants = [
            BackendPushConstant {
                stage_flags: BackendShaderStage::VERTEX,
                offset: 0,
                size: vert_push_constant_size as u32,
            },
            BackendPushConstant {
                stage_flags: BackendShaderStage::FRAGMENT,
                offset: (std::mem::size_of::<UniformTileGPosBorder>()
                    + std::mem::size_of::<UniformTileGVertColorAlign>())
                    as u32,
                size: frag_push_constant_size as u32,
            },
        ]
        .to_vec();
        let stride = if has_sampler {
            TILE_LAYER_TEXTURED_VERTEX_SIZE
        } else {
            TILE_LAYER_VERTEX_SIZE
        };
        BackendPipelineLayout {
            vertex_attributes: attribute_descriptors,
            descriptor_layouts: set_layouts,
            push_constants,
            stride: stride as BackendDeviceSize,
            geometry_is_line: false,
        }
    }

    fn border_tile_pipeline_layout(has_sampler: bool) -> BackendPipelineLayout {
        let mut attribute_descriptors: Vec<BackendVertexInputAttributeDescription> =
            Default::default();
        attribute_descriptors.push(BackendVertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: BackendVertexFormat::Vec2,
            offset: 0,
        });
        if has_sampler {
            attribute_descriptors.push(BackendVertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: BackendVertexFormat::UbVec4,
                offset: TILE_LAYER_BORDER_VERTEX_SIZE as u32,
            });
        }

        let set_layouts = [BackendResourceDescription::Fragment2DArrayTexture].to_vec();

        let vert_push_constant_size = std::mem::size_of::<UniformTileGPosBorder>();

        let frag_push_constant_size = std::mem::size_of::<SUniformTileGVertColor>();

        let push_constants = [
            BackendPushConstant {
                stage_flags: BackendShaderStage::VERTEX,
                offset: 0,
                size: vert_push_constant_size as u32,
            },
            BackendPushConstant {
                stage_flags: BackendShaderStage::FRAGMENT,
                offset: (std::mem::size_of::<UniformTileGPosBorder>()
                    + std::mem::size_of::<UniformTileGVertColorAlign>())
                    as u32,
                size: frag_push_constant_size as u32,
            },
        ]
        .to_vec();
        let stride = if has_sampler {
            TILE_LAYER_BORDER_TEXTURED_VERTEX_SIZE
        } else {
            TILE_LAYER_BORDER_VERTEX_SIZE
        };
        BackendPipelineLayout {
            vertex_attributes: attribute_descriptors,
            descriptor_layouts: set_layouts,
            push_constants,
            stride: stride as BackendDeviceSize,
            geometry_is_line: false,
        }
    }

    fn quad_pipeline_layout(is_textured: bool) -> BackendPipelineLayout {
        let mut attribute_descriptors: Vec<BackendVertexInputAttributeDescription> =
            Default::default();
        attribute_descriptors.push(BackendVertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: BackendVertexFormat::Vec4,
            offset: 0,
        });
        attribute_descriptors.push(BackendVertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: BackendVertexFormat::UbVec4Normalized,
            offset: (std::mem::size_of::<f32>() * 4) as u32,
        });
        if is_textured {
            attribute_descriptors.push(BackendVertexInputAttributeDescription {
                location: 2,
                binding: 0,
                format: BackendVertexFormat::Vec2,
                offset: QUAD_LAYER_VERTEX_SIZE as u32,
            });
        }

        let mut set_layouts: Vec<BackendResourceDescription> = Default::default();
        if is_textured {
            set_layouts.push(BackendResourceDescription::Fragment2DTexture);
            set_layouts.push(BackendResourceDescription::VertexFragmentUniformBuffer);
        } else {
            set_layouts.push(BackendResourceDescription::VertexFragmentUniformBuffer);
        }

        let push_constant_size = std::mem::size_of::<UniformQuadGPos>();

        let push_constants = [BackendPushConstant {
            stage_flags: BackendShaderStage::VERTEX,
            offset: 0,
            size: push_constant_size as u32,
        }]
        .to_vec();

        let stride = if is_textured {
            QUAD_LAYER_TEXTURED_VERTEX_SIZE
        } else {
            QUAD_LAYER_VERTEX_SIZE
        } as BackendDeviceSize;
        BackendPipelineLayout {
            vertex_attributes: attribute_descriptors,
            descriptor_layouts: set_layouts,
            push_constants,
            stride,
            geometry_is_line: false,
        }
    }

    fn render_tile_layer_fill_execute_buffer(
        render_execute_manager: &mut dyn BackendRenderExecuteInterface,
        draw_calls: usize,
        state: &State,
        texture_index: &StateTexture2dArray,
        buffer_object_index: u128,
        buffer_object_offset: usize,
    ) {
        render_execute_manager
            .set_vertex_buffer_with_offset(buffer_object_index, buffer_object_offset);

        match texture_index {
            StateTexture2dArray::Texture(texture_index) => {
                render_execute_manager.set_texture_3d(0, *texture_index);
            }
            StateTexture2dArray::None => {
                // nothing to do
            }
        }

        render_execute_manager.uses_index_buffer();

        render_execute_manager.estimated_render_calls(draw_calls as u64);

        render_execute_manager.exec_buffer_fill_dynamic_states(state);
    }

    fn cmd_render_tile_layer_fill_execute_buffer(
        render_execute_manager: &mut dyn BackendRenderExecuteInterface,
        cmd: &CommandRenderTileLayer,
    ) {
        Self::render_tile_layer_fill_execute_buffer(
            render_execute_manager,
            cmd.draws.len(),
            &cmd.state,
            &cmd.texture_index,
            cmd.buffer_object_index,
            0,
        );
    }

    fn cmd_render_border_tile_fill_execute_buffer(
        render_execute_manager: &mut dyn BackendRenderExecuteInterface,
        cmd: &CommandRenderBorderTile,
    ) {
        Self::render_tile_layer_fill_execute_buffer(
            render_execute_manager,
            1,
            &cmd.state,
            &cmd.texture_index,
            cmd.buffer_object_index,
            cmd.buffer_object_offset,
        );
    }

    fn cmd_render_quad_layer_fill_execute_buffer(
        render_execute_manager: &mut dyn BackendRenderExecuteInterface,
        cmd: &CommandRenderQuadLayer,
    ) {
        render_execute_manager.set_vertex_buffer(cmd.buffer_object_index);

        let address_mode_index = render_execute_manager.get_address_mode_index(&cmd.state);
        match cmd.texture_index {
            StateTexture::Texture(texture_index) => {
                render_execute_manager.set_texture(0, texture_index, address_mode_index);
            }
            StateTexture::ColorAttachmentOfPreviousPass => {
                render_execute_manager.set_color_attachment_as_texture(0, address_mode_index);
            }
            StateTexture::ColorAttachmentOfOffscreen(offscreen_id) => {
                render_execute_manager.set_offscreen_attachment_as_texture(
                    offscreen_id,
                    0,
                    address_mode_index,
                );
            }
            StateTexture::None => {
                // nothing to do
            }
        }

        render_execute_manager.uses_stream_uniform_buffer(
            0,
            cmd.quad_info_uniform_instance as u64,
            1,
        );

        render_execute_manager.uses_index_buffer();

        render_execute_manager.estimated_render_calls(
            (((cmd.quad_num - 1) / GRAPHICS_MAX_QUADS_RENDER_COUNT) + 1) as u64,
        );

        render_execute_manager.exec_buffer_fill_dynamic_states(&cmd.state);
    }

    fn render_tile_layer(
        &self,
        render_manager: &mut dyn BackendRenderInterface,
        state: &State,
        texture_index: &StateTexture2dArray,
        is_border: bool,
        color: &GlColorf,
        scale: &vec2,
        off: &vec2,
        draws: &[TileLayerDrawInfo],
    ) -> anyhow::Result<()> {
        let mut m: [f32; 4 * 2] = Default::default();
        render_manager.get_state_matrix(state, &mut m);

        render_manager.bind_pipeline_2d_array_texture(
            state,
            texture_index,
            if !is_border {
                SubRenderPassAttributes::Additional(
                    MapPipelineNames::TilePipeline as u64 + self.pipe_name_offset,
                )
            } else {
                SubRenderPassAttributes::Additional(
                    MapPipelineNames::TileBorderPipeline as u64 + self.pipe_name_offset,
                )
            },
        );

        render_manager.bind_vertex_buffer();

        if render_manager.is_textured() {
            render_manager.bind_texture_descriptor_sets(0, 0);
        }

        let mut vertex_push_constants = UniformTileGPosBorder::default();
        let mut vertex_push_constant_size: usize = std::mem::size_of::<UniformTileGPos>();
        let frag_push_constant_size: usize = std::mem::size_of::<SUniformTileGVertColor>();

        vertex_push_constants.base.pos = m;

        let frag_push_constants: SUniformTileGVertColor = *color;

        if is_border {
            vertex_push_constants.scale = *scale;
            vertex_push_constants.offset = *off;
            vertex_push_constant_size = std::mem::size_of::<UniformTileGPosBorder>();
        }

        render_manager.push_constants(BackendShaderStage::VERTEX, 0, unsafe {
            std::slice::from_raw_parts(
                (&vertex_push_constants) as *const _ as *const u8,
                vertex_push_constant_size,
            )
        });
        render_manager.push_constants(
            BackendShaderStage::FRAGMENT,
            (std::mem::size_of::<UniformTileGPosBorder>()
                + std::mem::size_of::<UniformTileGVertColorAlign>()) as u32,
            unsafe {
                std::slice::from_raw_parts(
                    &frag_push_constants as *const _ as *const u8,
                    frag_push_constant_size,
                )
            },
        );

        render_manager.bind_index_buffer(0);

        for draw in draws {
            let index_offset = draw.quad_offset.checked_mul(6).unwrap() as BackendDeviceSize;

            render_manager.draw_indexed(
                draw.quad_count.checked_mul(6).unwrap() as u32,
                1,
                index_offset as u32,
                0,
                0,
            );
        }

        Ok(())
    }

    fn cmd_render_tile_layer(
        &self,
        render_manager: &mut dyn BackendRenderInterface,
        cmd: &CommandRenderTileLayer,
    ) -> anyhow::Result<()> {
        let scale = vec2::default();
        let off = vec2::default();
        self.render_tile_layer(
            render_manager,
            &cmd.state,
            &cmd.texture_index,
            false,
            &cmd.color,
            &scale,
            &off,
            &cmd.draws,
        )
    }

    fn cmd_render_border_tile(
        &self,
        render_manager: &mut dyn BackendRenderInterface,
        cmd: &CommandRenderBorderTile,
    ) -> anyhow::Result<()> {
        let scale = cmd.scale;
        let off = cmd.offset;
        self.render_tile_layer(
            render_manager,
            &cmd.state,
            &cmd.texture_index,
            true,
            &cmd.color,
            &scale,
            &off,
            &[cmd.draw],
        )
    }

    fn cmd_render_quad_layer(
        &self,
        render_manager: &mut dyn BackendRenderInterface,
        cmd: &CommandRenderQuadLayer,
    ) -> anyhow::Result<()> {
        let mut m: [f32; 4 * 2] = Default::default();
        render_manager.get_state_matrix(&cmd.state, &mut m);

        render_manager.bind_pipeline(
            &cmd.state,
            &cmd.texture_index,
            SubRenderPassAttributes::Additional(
                MapPipelineNames::QuadPipeline as u64 + self.pipe_name_offset,
            ),
        );

        render_manager.bind_vertex_buffer();

        render_manager.bind_index_buffer(0);

        if render_manager.is_textured() {
            render_manager.bind_texture_descriptor_sets(0, 0);
        }

        let push_constant_vertex = UniformQuadGPos {
            pos: m,
            quad_offset: cmd.quad_offset as i32,
        };

        render_manager.push_constants(BackendShaderStage::VERTEX, 0, unsafe {
            std::slice::from_raw_parts(
                &push_constant_vertex as *const UniformQuadGPos as *const u8,
                std::mem::size_of::<UniformQuadGPos>(),
            )
        });

        let mut draw_count = cmd.quad_num;
        let mut render_offset: usize = 0;

        while draw_count > 0 {
            let real_draw_count = if draw_count > GRAPHICS_MAX_QUADS_RENDER_COUNT {
                GRAPHICS_MAX_QUADS_RENDER_COUNT
            } else {
                draw_count
            };

            let index_offset = (cmd.quad_offset + render_offset) * 6;

            render_manager
                .bind_uniform_descriptor_sets(if render_manager.is_textured() { 2 } else { 0 }, 0);

            if render_offset > 0 {
                let quad_offset: i32 = (cmd.quad_offset + render_offset) as i32;
                render_manager.push_constants(
                    BackendShaderStage::VERTEX,
                    (std::mem::size_of::<UniformQuadGPos>() - std::mem::size_of::<i32>()) as u32,
                    unsafe {
                        std::slice::from_raw_parts(
                            &quad_offset as *const i32 as *const u8,
                            std::mem::size_of::<i32>(),
                        )
                    },
                );
            }

            render_manager.draw_indexed(
                real_draw_count.checked_mul(6).unwrap().try_into().unwrap(),
                1,
                index_offset as u32,
                0,
                0,
            );

            render_offset += real_draw_count;
            draw_count -= real_draw_count;
        }

        Ok(())
    }
}

impl BackendCustomPipeline for MapPipeline {
    fn pipe_name(&self) -> String {
        MOD_NAME.into()
    }

    fn pipeline_count(&self) -> u64 {
        3
    }

    fn pipeline_names(&mut self, name_of_first: u64) {
        self.pipe_name_offset = name_of_first;
    }

    fn pipe_layout_of(&self, name: u64, is_textured: bool) -> BackendPipelineLayout {
        let name = MapPipelineNames::from_u64(name - self.pipe_name_offset).unwrap();
        match name {
            MapPipelineNames::TilePipeline => Self::tile_pipeline_layout(is_textured),
            MapPipelineNames::TileBorderPipeline => Self::border_tile_pipeline_layout(is_textured),
            MapPipelineNames::QuadPipeline => Self::quad_pipeline_layout(is_textured),
        }
    }

    fn pipe_shader_names(&self, name: u64, is_textured: bool) -> Option<(String, String)> {
        let name = MapPipelineNames::from_u64(name - self.pipe_name_offset).unwrap();
        match name {
            MapPipelineNames::TilePipeline => {
                if is_textured {
                    Some((
                        "shader/vulkan/tile_textured.vert.spv".into(),
                        "shader/vulkan/tile_textured.frag.spv".into(),
                    ))
                } else {
                    Some((
                        "shader/vulkan/tile.vert.spv".into(),
                        "shader/vulkan/tile.frag.spv".into(),
                    ))
                }
            }
            MapPipelineNames::TileBorderPipeline => {
                if is_textured {
                    Some((
                        "shader/vulkan/tile_border_textured.vert.spv".into(),
                        "shader/vulkan/tile_border_textured.frag.spv".into(),
                    ))
                } else {
                    Some((
                        "shader/vulkan/tile_border.vert.spv".into(),
                        "shader/vulkan/tile_border.frag.spv".into(),
                    ))
                }
            }
            MapPipelineNames::QuadPipeline => {
                if is_textured {
                    Some((
                        "shader/vulkan/quad_textured.vert.spv".into(),
                        "shader/vulkan/quad_textured.frag.spv".into(),
                    ))
                } else {
                    Some((
                        "shader/vulkan/quad.vert.spv".into(),
                        "shader/vulkan/quad.frag.spv".into(),
                    ))
                }
            }
        }
    }

    fn fill_exec_buffer(
        &self,
        cmd: &PoolVec<u8>,
        render_execute: &mut dyn BackendRenderExecuteInterface,
    ) {
        let (command, _) = bincode::serde::decode_from_slice::<CommandsRenderMap, _>(
            cmd,
            bincode::config::standard(),
        )
        .unwrap();
        match command {
            CommandsRenderMap::TileLayer(cmd) => {
                Self::cmd_render_tile_layer_fill_execute_buffer(render_execute, &cmd);
            }
            CommandsRenderMap::BorderTile(cmd) => {
                Self::cmd_render_border_tile_fill_execute_buffer(render_execute, &cmd);
            }
            CommandsRenderMap::QuadLayer(cmd) => {
                Self::cmd_render_quad_layer_fill_execute_buffer(render_execute, &cmd);
            }
        }
    }

    fn render(
        &self,
        cmd: &PoolVec<u8>,
        render: &mut dyn graphics_backend_traits::plugin::BackendRenderInterface,
    ) -> anyhow::Result<()> {
        let (command, _) = bincode::serde::decode_from_slice::<CommandsRenderMap, _>(
            cmd,
            bincode::config::standard(),
        )
        .unwrap();
        match command {
            CommandsRenderMap::TileLayer(cmd) => self.cmd_render_tile_layer(render, &cmd),
            CommandsRenderMap::BorderTile(cmd) => self.cmd_render_border_tile(render, &cmd),
            CommandsRenderMap::QuadLayer(cmd) => self.cmd_render_quad_layer(render, &cmd),
        }
    }

    fn rewrite_texture_and_buffer_object_indices(
        &self,
        cmd: &mut PoolVec<u8>,
        f: &dyn Fn(GraphicsObjectRewriteFunc),
    ) {
        let (mut command, _) = bincode::serde::decode_from_slice::<CommandsRenderMap, _>(
            cmd,
            bincode::config::standard(),
        )
        .unwrap();
        match &mut command {
            CommandsRenderMap::TileLayer(cmd) => f(GraphicsObjectRewriteFunc {
                textures: &mut [],
                buffer_objects: &mut [GraphicsBufferObjectAccessAndRewrite {
                    buffer_object_index: &mut cmd.buffer_object_index,
                    accesses: {
                        let mut accesses = self.accesses_pool.new();

                        cmd.draws.iter().for_each(|draw| {
                            accesses.push(GraphicsBufferObjectAccess::Quad {
                                quad_offset: draw.quad_offset,
                                quad_count: draw.quad_count,
                                buffer_byte_offset: 0,
                                vertex_byte_size: if cmd.texture_index.is_textured() {
                                    TILE_LAYER_TEXTURED_VERTEX_SIZE
                                } else {
                                    TILE_LAYER_VERTEX_SIZE
                                },
                                alignment: 2.try_into().unwrap(),
                            });
                        });

                        accesses
                    },
                }],
                textures_2d_array: &mut [&mut cmd.texture_index],
                uniform_instances: &mut [],
            }),
            CommandsRenderMap::BorderTile(cmd) => f(GraphicsObjectRewriteFunc {
                textures: &mut [],
                buffer_objects: &mut [GraphicsBufferObjectAccessAndRewrite {
                    buffer_object_index: &mut cmd.buffer_object_index,
                    accesses: {
                        let mut accesses = self.accesses_pool.new();

                        accesses.push(GraphicsBufferObjectAccess::Quad {
                            quad_offset: cmd.draw.quad_offset,
                            quad_count: cmd.draw.quad_count,
                            buffer_byte_offset: cmd.buffer_object_offset,
                            vertex_byte_size: if cmd.texture_index.is_textured() {
                                TILE_LAYER_BORDER_TEXTURED_VERTEX_SIZE
                            } else {
                                TILE_LAYER_BORDER_VERTEX_SIZE
                            },
                            alignment: 4.try_into().unwrap(),
                        });

                        accesses
                    },
                }],
                textures_2d_array: &mut [&mut cmd.texture_index],
                uniform_instances: &mut [],
            }),
            CommandsRenderMap::QuadLayer(cmd) => f(GraphicsObjectRewriteFunc {
                textures_2d_array: &mut [],
                buffer_objects: &mut [GraphicsBufferObjectAccessAndRewrite {
                    buffer_object_index: &mut cmd.buffer_object_index,
                    accesses: {
                        let mut accesses = self.accesses_pool.new();

                        accesses.push(GraphicsBufferObjectAccess::Quad {
                            quad_offset: cmd.quad_offset,
                            quad_count: cmd.quad_num,
                            buffer_byte_offset: 0,
                            vertex_byte_size: if cmd.texture_index.is_textured() {
                                QUAD_LAYER_TEXTURED_VERTEX_SIZE
                            } else {
                                QUAD_LAYER_VERTEX_SIZE
                            },
                            alignment: 4.try_into().unwrap(),
                        });

                        accesses
                    },
                }],
                textures: &mut [&mut cmd.texture_index],
                uniform_instances: &mut [GraphicsUniformAccessAndRewrite {
                    index: &mut cmd.quad_info_uniform_instance,
                    instance_count: cmd.quad_num,
                    single_instance_byte_size: std::mem::size_of::<QuadRenderInfo>(),
                }],
            }),
        }
        cmd.clear();
        bincode::serde::encode_into_std_write(
            command,
            cmd.deref_mut(),
            bincode::config::standard(),
        )
        .unwrap();
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct MapGraphics {
    backend_handle: GraphicsBackendHandle,
    mod_name: StringPool,
    cmd_pool: Pool<Vec<u8>>,
}

impl MapGraphics {
    pub fn new(backend_handle: &GraphicsBackendHandle) -> Self {
        let (mod_name, mod_name_sync_point) = Pool::with_capacity(32);
        let (cmd_pool, cmd_pool_sync_point) = Pool::with_capacity(32);
        backend_handle.add_sync_point(mod_name_sync_point);
        backend_handle.add_sync_point(cmd_pool_sync_point);
        Self {
            backend_handle: backend_handle.clone(),
            mod_name,
            cmd_pool,
        }
    }

    pub fn render_tile_layer(
        &self,
        state: &State,
        texture: TextureType2dArray,
        buffer_object_index: &BufferObject,
        color: &ColorRgba,
        draws: PoolVec<TileLayerDrawInfo>,
    ) {
        if draws.is_empty() {
            return;
        }

        // add the VertexArrays and draw
        let cmd = CommandRenderTileLayer {
            state: *state,
            texture_index: texture.into(),
            buffer_object_index: buffer_object_index.get_index_unsafe(),
            color: *color,

            draws,
        };

        let mut pooled_cmd = self.cmd_pool.new();
        bincode::serde::encode_into_std_write(
            CommandsRenderMap::TileLayer(cmd),
            pooled_cmd.deref_mut(),
            bincode::config::standard(),
        )
        .unwrap();
        let mut mod_name = self.mod_name.new();
        mod_name.push_str(MOD_NAME);
        self.backend_handle
            .add_cmd(AllCommands::Render(CommandsRender::Mod(
                CommandsRenderMod {
                    cmd: pooled_cmd,
                    mod_name,
                },
            )));

        // TODO m_pCommandBuffer->AddRenderCalls(NumIndicesOffset);
        // todo max indices group check!!
    }

    pub fn render_border_tiles(
        &self,
        state: &State,
        texture: TextureType2dArray,
        buffer_object_index: &BufferObject,
        buffer_object_offset: usize,
        color: &ColorRgba,
        offset: &vec2,
        scale: &vec2,
        quad_offset: usize,
        quad_count: usize,
    ) {
        if quad_count == 0 {
            return;
        }
        // Draw a border tile a lot of times
        let cmd = CommandRenderBorderTile {
            state: *state,
            texture_index: texture.into(),
            draw: TileLayerDrawInfo {
                quad_offset,
                quad_count,
            },

            buffer_object_index: buffer_object_index.get_index_unsafe(),
            buffer_object_offset,

            color: *color,

            offset: *offset,
            scale: *scale,
        };

        let mut pooled_cmd = self.cmd_pool.new();
        bincode::serde::encode_into_std_write(
            CommandsRenderMap::BorderTile(cmd),
            pooled_cmd.deref_mut(),
            bincode::config::standard(),
        )
        .unwrap();
        let mut mod_name = self.mod_name.new();
        mod_name.push_str(MOD_NAME);
        self.backend_handle
            .add_cmd(AllCommands::Render(CommandsRender::Mod(
                CommandsRenderMod {
                    cmd: pooled_cmd,
                    mod_name,
                },
            )));

        // TODO: m_pCommandBuffer->AddRenderCalls(1);
    }

    pub fn render_quad_layer(
        &self,
        state: &State,
        texture: TextureType,
        buffer_object_index: &BufferObject,
        quad_info_uniform_instance: usize,
        quad_num: usize,
        quad_offset: usize,
    ) {
        if quad_num == 0 {
            return;
        }

        // add the VertexArrays and draw
        let cmd = CommandRenderQuadLayer {
            state: *state,
            texture_index: texture.into(),
            quad_num,
            quad_offset,
            buffer_object_index: buffer_object_index.get_index_unsafe(),

            quad_info_uniform_instance,
        };

        let mut pooled_cmd = self.cmd_pool.new();
        let pooled_write: &mut Vec<_> = &mut pooled_cmd;
        bincode::serde::encode_into_std_write(
            CommandsRenderMap::QuadLayer(cmd),
            pooled_write,
            bincode::config::standard(),
        )
        .unwrap();
        let mut mod_name = self.mod_name.new();
        mod_name.push_str(MOD_NAME);
        self.backend_handle
            .add_cmd(AllCommands::Render(CommandsRender::Mod(
                CommandsRenderMod {
                    cmd: pooled_cmd,
                    mod_name,
                },
            )));

        // TODO m_pCommandBuffer->AddRenderCalls(((QuadNum - 1) / gs_GraphicsMaxQuadsRenderCount) + 1);
    }
}
