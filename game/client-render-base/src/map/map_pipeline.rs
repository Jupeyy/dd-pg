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
    SubRenderPassAttributes,
};
use graphics_types::{
    commands::{
        AllCommands, CommandsRender, CommandsRenderMod, SColorf, GRAPHICS_DEFAULT_UNIFORM_SIZE,
        GRAPHICS_MAX_UNIFORM_RENDER_COUNT,
    },
    rendering::{ColorRGBA, GlColorf, State, StateTexture, StateTexture2dArray},
};
use hiarc::Hiarc;
use math::math::vector::{usvec2, vec2};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use pool::{mixed_datatypes::StringPool, mixed_pool::Pool, mt_datatypes::PoolVec};
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

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandRenderTileLayer {
    pub state: State,
    pub texture_index: StateTexture2dArray,
    pub color: SColorf, // the color of the whole tilelayer -- already enveloped

    // the char offset of all indices that should be rendered, and the amount of renders
    pub indices_offsets: PoolVec<usize>,
    pub draw_count: PoolVec<usize>,

    pub indices_draw_num: usize,
    pub buffer_object_index: u128,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandRenderBorderTile {
    pub state: State,
    pub texture_index: StateTexture2dArray,
    pub color: SColorf, // the color of the whole tilelayer -- already enveloped
    pub indices_offset: usize, // you should use the command buffer data to allocate vertices for this command
    pub draw_num: usize,
    pub buffer_object_index: u128,
    pub buffer_object_offset: usize,

    pub offset: vec2,
    pub scale: vec2,
}

#[repr(C)]
#[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
pub struct QuadRenderInfo {
    pub color: ColorRGBA,
    pub offsets: vec2,
    pub rotation: f32,
    // allows easier upload for uniform buffers because of the alignment requirements
    pub padding: f32,
}

impl QuadRenderInfo {
    pub fn new(color: ColorRGBA, offsets: vec2, rotation: f32) -> Self {
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

#[derive(Debug, Hiarc)]
pub struct MapPipeline {
    pipe_name_offset: u64,
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

pub type SUniformTileGVertColor = ColorRGBA;

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

impl MapPipeline {
    pub fn new() -> Box<dyn BackendCustomPipeline> {
        Box::new(Self {
            pipe_name_offset: 0,
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
                format: BackendVertexFormat::UbVec4,
                offset: (std::mem::size_of::<usvec2>()) as u32,
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
            std::mem::size_of::<usvec2>() + std::mem::size_of::<u8>() * 4
        } else {
            std::mem::size_of::<usvec2>()
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
                offset: (std::mem::size_of::<f32>() * 2) as u32,
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
            std::mem::size_of::<f32>() * 2 + std::mem::size_of::<u8>() * 4
        } else {
            std::mem::size_of::<f32>() * 2
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
                offset: (std::mem::size_of::<f32>() * 4 + std::mem::size_of::<u8>() * 4) as u32,
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

        let stride = (std::mem::size_of::<f32>() * 4
            + std::mem::size_of::<u8>() * 4
            + (if is_textured {
                std::mem::size_of::<f32>() * 2
            } else {
                0
            })) as BackendDeviceSize;
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
            cmd.indices_draw_num,
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
                render_execute_manager
                    .set_color_attachment_as_texture(0, address_mode_index as u64);
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

    #[must_use]
    fn render_tile_layer(
        &self,
        render_manager: &mut dyn BackendRenderInterface,
        state: &State,
        texture_index: &StateTexture2dArray,
        is_border: bool,
        color: &GlColorf,
        scale: &vec2,
        off: &vec2,
        indices_draw_num: usize,
        indices_offsets: &[usize],
        draw_counts: &[usize],
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

        let draw_count: usize = indices_draw_num;
        for i in 0..draw_count {
            let index_offset =
                (indices_offsets[i] / std::mem::size_of::<u32>()) as BackendDeviceSize;

            render_manager.draw_indexed(draw_counts[i] as u32, 1, index_offset as u32, 0, 0);
        }

        Ok(())
    }

    #[must_use]
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
            cmd.indices_draw_num,
            &cmd.indices_offsets,
            &cmd.draw_count,
        )
    }

    #[must_use]
    fn cmd_render_border_tile(
        &self,
        render_manager: &mut dyn BackendRenderInterface,
        cmd: &CommandRenderBorderTile,
    ) -> anyhow::Result<()> {
        let scale = cmd.scale;
        let off = cmd.offset;
        let draw_num = cmd.draw_num * 6;
        self.render_tile_layer(
            render_manager,
            &cmd.state,
            &cmd.texture_index,
            true,
            &cmd.color,
            &scale,
            &off,
            1,
            &[cmd.indices_offset],
            &[draw_num],
        )
    }

    #[must_use]
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

        let mut push_constant_vertex = UniformQuadGPos::default();
        push_constant_vertex.pos = m;
        push_constant_vertex.quad_offset = cmd.quad_offset as i32;

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

            render_manager.draw_indexed((real_draw_count * 6) as u32, 1, index_offset as u32, 0, 0);

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
            &cmd,
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
            &cmd,
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
        f: &dyn Fn(&mut [&mut StateTexture], &mut [&mut StateTexture2dArray], &mut [&mut u128]),
    ) {
        let (mut command, _) = bincode::serde::decode_from_slice::<CommandsRenderMap, _>(
            cmd,
            bincode::config::standard(),
        )
        .unwrap();
        match &mut command {
            CommandsRenderMap::TileLayer(cmd) => f(
                &mut [],
                &mut [&mut cmd.texture_index],
                &mut [&mut cmd.buffer_object_index],
            ),
            CommandsRenderMap::BorderTile(cmd) => f(
                &mut [],
                &mut [&mut cmd.texture_index],
                &mut [&mut cmd.buffer_object_index],
            ),
            CommandsRenderMap::QuadLayer(cmd) => f(
                &mut [&mut cmd.texture_index],
                &mut [],
                &mut [&mut cmd.buffer_object_index],
            ),
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

#[derive(Debug, Hiarc)]
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
        color: &ColorRGBA,
        offsets: PoolVec<usize>,
        indiced_vertex_draw_num: PoolVec<usize>,
        num_indices_offset: usize,
    ) {
        if num_indices_offset == 0 {
            return;
        }

        // add the VertexArrays and draw
        let cmd = CommandRenderTileLayer {
            state: *state,
            texture_index: texture.into(),
            indices_draw_num: num_indices_offset,
            buffer_object_index: buffer_object_index.get_index_unsafe(),
            color: *color,

            indices_offsets: offsets,
            draw_count: indiced_vertex_draw_num,
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
        color: &ColorRGBA,
        index_buffer_offset: usize,
        offset: &vec2,
        scale: &vec2,
        draw_num: usize,
    ) {
        if draw_num == 0 {
            return;
        }
        // Draw a border tile a lot of times
        let cmd = CommandRenderBorderTile {
            state: *state,
            texture_index: texture.into(),
            draw_num: draw_num,

            buffer_object_index: buffer_object_index.get_index_unsafe(),
            buffer_object_offset,

            color: *color,

            indices_offset: index_buffer_offset,

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
            quad_num: quad_num,
            quad_offset: quad_offset,
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
