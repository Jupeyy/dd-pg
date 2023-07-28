use std::{
    alloc::System,
    cell::RefCell,
    str::FromStr,
    sync::{Arc, Mutex},
};

use anyhow::anyhow;
use arrayvec::ArrayString;

use base::counted_index::{CountedIndexDrop, CountedIndexGetIndexUnsafe};
use graphics_base::streaming::{
    rotate, DrawLines, DrawQuads, DrawScope, DrawScopeImpl, DrawTriangles,
};
use graphics_render_traits::{
    GraphicsInterface, GraphicsRenderGeometry, GraphicsRenderHandles, GraphicsRenderQuadContainer,
};
use graphics_traits::{GraphicsBackendBufferInterface, GraphicsSizeQuery, GraphicsStreamHandler};
use native::native::{Native, NativeImpl};
use num_traits::FromPrimitive;

use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use config::config::Config;

use math::math::vector::{ubvec4, vec2, vec4};
use pool::{mt_datatypes::PoolVec, mt_pool::Pool};
use thiserror::Error;

use crate::{
    backend::{BackendBuffer, GraphicsBackend},
    graphics_mt::GraphicsMultiThreaded,
    traits::{GraphicsLoadIOPipe, GraphicsLoadWhileIOPipe},
};

use graphics_types::{
    command_buffer::{
        AllCommands, BufferContainerIndex, BufferObjectIndex, CommandCopyBufferObject,
        CommandCreateBufferContainer, CommandCreateBufferObject, CommandDeleteBufferContainer,
        CommandDeleteBufferObject, CommandIndicesRequiredNumNotify, CommandRecreateBufferObject,
        CommandRender, CommandRenderBorderTile, CommandRenderBorderTileLine,
        CommandRenderQuadContainer, CommandRenderQuadContainerAsSpriteMultiple,
        CommandRenderQuadContainerEx, CommandRenderQuadLayer, CommandRenderTileLayer, CommandSwap,
        CommandTextureCreate, CommandTextureDestroy, CommandUpdateBufferContainer,
        CommandUpdateBufferObject, Commands, CommandsRender, GraphicsType, PrimType, RenderCommand,
        SAttribute, SBufferContainerInfo, SColor, SQuadRenderInfo, SRenderSpriteInfo, STexCoord,
        StreamDataMax, TexFlags, TexFormat,
    },
    rendering::{
        ColorRGBA, GlPoint, GlVertex, SVertex, State, TextureIndex, WriteVertexAttributes,
    },
    types::{
        CQuadItem, DrawModes, GraphicsBackendMemory, GraphicsMemoryAllocationType, ImageFormat,
        Line, QuadContainerIndex, Triangle, VideoMode, WindowProps,
    },
};

const FAKE_MODES: [VideoMode; 68] = [
    VideoMode::new(8192, 4320, 8192, 4320, 0, 8, 8, 8, 0),
    VideoMode::new(7680, 4320, 7680, 4320, 0, 8, 8, 8, 0),
    VideoMode::new(5120, 2880, 5120, 2880, 0, 8, 8, 8, 0),
    VideoMode::new(4096, 2160, 4096, 2160, 0, 8, 8, 8, 0),
    VideoMode::new(3840, 2160, 3840, 2160, 0, 8, 8, 8, 0),
    VideoMode::new(2560, 1440, 2560, 1440, 0, 8, 8, 8, 0),
    VideoMode::new(2048, 1536, 2048, 1536, 0, 8, 8, 8, 0),
    VideoMode::new(1920, 2400, 1920, 2400, 0, 8, 8, 8, 0),
    VideoMode::new(1920, 1440, 1920, 1440, 0, 8, 8, 8, 0),
    VideoMode::new(1920, 1200, 1920, 1200, 0, 8, 8, 8, 0),
    VideoMode::new(1920, 1080, 1920, 1080, 0, 8, 8, 8, 0),
    VideoMode::new(1856, 1392, 1856, 1392, 0, 8, 8, 8, 0),
    VideoMode::new(1800, 1440, 1800, 1440, 0, 8, 8, 8, 0),
    VideoMode::new(1792, 1344, 1792, 1344, 0, 8, 8, 8, 0),
    VideoMode::new(1680, 1050, 1680, 1050, 0, 8, 8, 8, 0),
    VideoMode::new(1600, 1200, 1600, 1200, 0, 8, 8, 8, 0),
    VideoMode::new(1600, 1000, 1600, 1000, 0, 8, 8, 8, 0),
    VideoMode::new(1440, 1050, 1440, 1050, 0, 8, 8, 8, 0),
    VideoMode::new(1440, 900, 1440, 900, 0, 8, 8, 8, 0),
    VideoMode::new(1400, 1050, 1400, 1050, 0, 8, 8, 8, 0),
    VideoMode::new(1368, 768, 1368, 768, 0, 8, 8, 8, 0),
    VideoMode::new(1280, 1024, 1280, 1024, 0, 8, 8, 8, 0),
    VideoMode::new(1280, 960, 1280, 960, 0, 8, 8, 8, 0),
    VideoMode::new(1280, 800, 1280, 800, 0, 8, 8, 8, 0),
    VideoMode::new(1280, 768, 1280, 768, 0, 8, 8, 8, 0),
    VideoMode::new(1152, 864, 1152, 864, 0, 8, 8, 8, 0),
    VideoMode::new(1024, 768, 1024, 768, 0, 8, 8, 8, 0),
    VideoMode::new(1024, 600, 1024, 600, 0, 8, 8, 8, 0),
    VideoMode::new(800, 600, 800, 600, 0, 8, 8, 8, 0),
    VideoMode::new(768, 576, 768, 576, 0, 8, 8, 8, 0),
    VideoMode::new(720, 400, 720, 400, 0, 8, 8, 8, 0),
    VideoMode::new(640, 480, 640, 480, 0, 8, 8, 8, 0),
    VideoMode::new(400, 300, 400, 300, 0, 8, 8, 8, 0),
    VideoMode::new(320, 240, 320, 240, 0, 8, 8, 8, 0),
    VideoMode::new(8192, 4320, 8192, 4320, 0, 5, 6, 5, 0),
    VideoMode::new(7680, 4320, 7680, 4320, 0, 5, 6, 5, 0),
    VideoMode::new(5120, 2880, 5120, 2880, 0, 5, 6, 5, 0),
    VideoMode::new(4096, 2160, 4096, 2160, 0, 5, 6, 5, 0),
    VideoMode::new(3840, 2160, 3840, 2160, 0, 5, 6, 5, 0),
    VideoMode::new(2560, 1440, 2560, 1440, 0, 5, 6, 5, 0),
    VideoMode::new(2048, 1536, 2048, 1536, 0, 5, 6, 5, 0),
    VideoMode::new(1920, 2400, 1920, 2400, 0, 5, 6, 5, 0),
    VideoMode::new(1920, 1440, 1920, 1440, 0, 5, 6, 5, 0),
    VideoMode::new(1920, 1200, 1920, 1200, 0, 5, 6, 5, 0),
    VideoMode::new(1920, 1080, 1920, 1080, 0, 5, 6, 5, 0),
    VideoMode::new(1856, 1392, 1856, 1392, 0, 5, 6, 5, 0),
    VideoMode::new(1800, 1440, 1800, 1440, 0, 5, 6, 5, 0),
    VideoMode::new(1792, 1344, 1792, 1344, 0, 5, 6, 5, 0),
    VideoMode::new(1680, 1050, 1680, 1050, 0, 5, 6, 5, 0),
    VideoMode::new(1600, 1200, 1600, 1200, 0, 5, 6, 5, 0),
    VideoMode::new(1600, 1000, 1600, 1000, 0, 5, 6, 5, 0),
    VideoMode::new(1440, 1050, 1440, 1050, 0, 5, 6, 5, 0),
    VideoMode::new(1440, 900, 1440, 900, 0, 5, 6, 5, 0),
    VideoMode::new(1400, 1050, 1400, 1050, 0, 5, 6, 5, 0),
    VideoMode::new(1368, 768, 1368, 768, 0, 5, 6, 5, 0),
    VideoMode::new(1280, 1024, 1280, 1024, 0, 5, 6, 5, 0),
    VideoMode::new(1280, 960, 1280, 960, 0, 5, 6, 5, 0),
    VideoMode::new(1280, 800, 1280, 800, 0, 5, 6, 5, 0),
    VideoMode::new(1280, 768, 1280, 768, 0, 5, 6, 5, 0),
    VideoMode::new(1152, 864, 1152, 864, 0, 5, 6, 5, 0),
    VideoMode::new(1024, 768, 1024, 768, 0, 5, 6, 5, 0),
    VideoMode::new(1024, 600, 1024, 600, 0, 5, 6, 5, 0),
    VideoMode::new(800, 600, 800, 600, 0, 5, 6, 5, 0),
    VideoMode::new(768, 576, 768, 576, 0, 5, 6, 5, 0),
    VideoMode::new(720, 400, 720, 400, 0, 5, 6, 5, 0),
    VideoMode::new(640, 480, 640, 480, 0, 5, 6, 5, 0),
    VideoMode::new(400, 300, 400, 300, 0, 5, 6, 5, 0),
    VideoMode::new(320, 240, 320, 240, 0, 5, 6, 5, 0),
];

enum CommandBuffersCount {
    Num = 2,
}

/**
 * This buffer is useful if you want to
 * allocate memory that is going to be used by the
 * graphics engine. It might optimize away expensive
 * heap allocations and copying.
 */
pub struct GraphicsRawMemoryBuffer<'a> {
    memory: Option<GraphicsBackendMemory>,
    graphics: Option<&'a mut Graphics>,
}

impl<'a> Drop for GraphicsRawMemoryBuffer<'a> {
    fn drop(&mut self) {
        let mut memory: Option<GraphicsBackendMemory> = None;
        let mut graphics: Option<&'a mut Graphics> = None;
        std::mem::swap(&mut memory, &mut self.memory);
        std::mem::swap(&mut graphics, &mut self.graphics);
        if let Some(graphics) = graphics {
            if let Some(memory) = memory {
                graphics.backend_handle.backend.mem_free(memory);
            }
        }
    }
}

/**
 * implements minimal graphics traits that are useful for
 * memory management from the backend
 */
pub struct StagingGraphics<'a> {
    memory: Option<GraphicsBackendMemory>,
    graphics: &'a mut Graphics,
}

impl<'a> StagingGraphics<'a> {
    pub fn load_texture(
        &mut self,
        width: usize,
        height: usize,
        format: i32,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<TextureIndex> {
        let mut memory: Option<GraphicsBackendMemory> = None;
        std::mem::swap(&mut memory, &mut self.memory);
        self.graphics.load_texture_impl(
            width,
            height,
            1,
            false,
            format,
            memory.unwrap(),
            store_format,
            flags,
            tex_name,
        )
    }
}

impl<'a> GraphicsRawMemoryBuffer<'a> {
    pub fn exec(mut self) -> StagingGraphics<'a> {
        let mut memory: Option<GraphicsBackendMemory> = None;
        let mut graphics: Option<&'a mut Graphics> = None;
        std::mem::swap(&mut memory, &mut self.memory);
        std::mem::swap(&mut graphics, &mut self.graphics);
        StagingGraphics {
            graphics: graphics.unwrap(),
            memory: memory,
        }
    }

    pub fn mem<'b>(&'b mut self) -> &'b mut GraphicsBackendMemory {
        self.memory.as_mut().unwrap()
    }
}

pub trait GraphicsTextureAllocations {
    fn load_texture_slow(
        &mut self,
        width: usize,
        height: usize,
        format: i32,
        data: Vec<u8>,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<TextureIndex>;
    fn load_texture_3d_slow(
        &mut self,
        width: usize,
        height: usize,
        depth: usize,
        format: i32,
        data: Vec<u8>,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<TextureIndex>;
}

#[derive(Default, Clone, Copy)]
#[repr(C)]
pub struct SQuad {
    vertices: [SVertex; 4],
}

impl SQuad {
    pub fn append_to_bytes_vec(&self, bytes_vec: &mut Vec<u8>) {
        self.vertices
            .iter()
            .for_each(|vert| vert.append_to_bytes_vec(bytes_vec));
    }

    /**
     * Creates a new quad with white color and texture coordinates to match a normal rect
     */
    pub fn new() -> Self {
        *Self::default()
            .with_color(&ubvec4 {
                x: 255,
                y: 255,
                z: 255,
                w: 255,
            })
            .with_tex(&[
                vec2 { x: 0.0, y: 0.0 },
                vec2 { x: 1.0, y: 0.0 },
                vec2 { x: 1.0, y: 1.0 },
                vec2 { x: 0.0, y: 1.0 },
            ])
    }

    pub fn from_rect(&mut self, x: f32, y: f32, width: f32, height: f32) -> &mut Self {
        self.vertices[0].pos.x = x;
        self.vertices[0].pos.y = y;

        self.vertices[1].pos.x = x + width;
        self.vertices[1].pos.y = y;

        self.vertices[2].pos.x = x + width;
        self.vertices[2].pos.y = y + height;

        self.vertices[3].pos.x = x;
        self.vertices[3].pos.y = y + height;

        self
    }

    pub fn from_width_and_height_centered(&mut self, width: f32, height: f32) -> &mut Self {
        let x = -width / 2.0;
        let y = -height / 2.0;

        self.from_rect(x, y, width, height)
    }

    pub fn from_size_centered(&mut self, size: f32) -> &mut Self {
        self.from_width_and_height_centered(size, size)
    }

    pub fn with_tex(&mut self, tex: &[vec2; 4]) -> &mut Self {
        self.vertices[0].tex = tex[0];
        self.vertices[1].tex = tex[1];
        self.vertices[2].tex = tex[2];
        self.vertices[3].tex = tex[3];

        self
    }

    /**
     * builds UV coordinates from 2 points
     */
    pub fn with_uv_from_points(&mut self, uv1: &vec2, uv2: &vec2) -> &mut Self {
        self.vertices[0].tex = *uv1;
        self.vertices[1].tex = vec2::new(uv2.x, uv1.y);
        self.vertices[2].tex = *uv2;
        self.vertices[3].tex = vec2::new(uv1.x, uv2.y);

        self
    }

    pub fn with_colors(&mut self, colors: &[ubvec4; 4]) -> &mut Self {
        self.vertices[0].color = colors[0];
        self.vertices[1].color = colors[1];
        self.vertices[2].color = colors[2];
        self.vertices[3].color = colors[3];

        self
    }

    pub fn with_color(&mut self, color: &ubvec4) -> &mut Self {
        self.with_colors(&[*color, *color, *color, *color])
    }

    pub fn with_rotation(&mut self, rotation: f32) -> &mut Self {
        let x = self.vertices[0].pos.x;
        let y = self.vertices[0].pos.y;
        let w = self.vertices[2].pos.x - self.vertices[0].pos.x;
        let h = self.vertices[2].pos.y - self.vertices[0].pos.y;

        let center = vec2 {
            x: x + w / 2.0,
            y: y + h / 2.0,
        };

        rotate(&center, rotation, &mut self.vertices);

        self
    }
}

pub enum QuadContainerRenderCount {
    Auto,
    Count(usize),
}

pub struct SQuadContainer {
    quads: Vec<SQuad>,

    quad_buffer_object_index: Option<BufferObjectIndex>,
    quad_buffer_container_index: Option<BufferContainerIndex>,

    next_free_index: Option<usize>,

    index: QuadContainerIndex,

    automatic_upload: bool,
}

impl SQuadContainer {
    pub fn quads_to_bytes(&self) -> Vec<u8> {
        let mut res: Vec<u8> = Vec::new();
        res.reserve(self.quads.len() * std::mem::size_of::<SQuad>());
        self.quads.iter().for_each(|quad| {
            quad.append_to_bytes_vec(&mut res);
        });
        res
    }
}

struct SVertexArrayInfo {
    // keep a reference to it, so we can free the ID
    associated_buffer_object_index: Option<BufferObjectIndex>,

    next_free_index: Option<usize>,
    index: BufferContainerIndex,
}

pub struct QuadContainerBuilder {
    automatic_upload: bool,
}

impl QuadContainerBuilder {
    pub fn new(atomatic_upload: bool) -> Self {
        Self {
            automatic_upload: atomatic_upload,
        }
    }

    pub fn build(&self, index: usize) -> SQuadContainer {
        SQuadContainer {
            quads: Vec::new(),
            quad_buffer_object_index: None,
            quad_buffer_container_index: None,
            next_free_index: None,
            automatic_upload: self.automatic_upload,

            index: QuadContainerIndex::new(index),
        }
    }
}

#[derive(Error, Debug)]
pub enum GraphicsBackendHandleError {
    #[error("TODO: Not yet implemented.")]
    BackendInitializationError,
}

pub struct GraphicsBackendHandle {
    backend_buffer: BackendBuffer,
    backend: GraphicsBackend,
}

impl GraphicsStreamHandler for GraphicsBackendHandle {
    fn backend_buffer_mut(&mut self) -> &mut dyn GraphicsBackendBufferInterface {
        &mut self.backend_buffer
    }

    fn flush_vertices(&mut self, state: &State, vertices_offset: usize, draw_mode: DrawModes) {
        let mut cmd = CommandRender::new();
        if self.flush_vertices_impl(state, draw_mode, vertices_offset, &mut cmd) {
            cmd.vertices_offset = vertices_offset;
            self.add_cmd(AllCommands::Render(CommandsRender::Render(cmd)));
        }
    }

    fn run_backend_buffer(&mut self) {
        self.backend.run_cmds(&mut self.backend_buffer);
    }

    fn add_cmd(&mut self, cmd: AllCommands) {
        self.backend_buffer.cmds.push(cmd);
    }
}

impl GraphicsBackendHandle {
    fn new() -> Self {
        Self {
            backend_buffer: BackendBuffer::default(),
            backend: GraphicsBackend::new(),
        }
    }

    fn load_io(&mut self, io_pipe: &mut GraphicsLoadIOPipe) {
        self.backend.load_io(io_pipe);
    }

    fn init_while_io(&mut self, pipe: &mut GraphicsLoadWhileIOPipe) {
        self.backend.init_while_io(pipe);
    }

    pub fn init(
        &mut self,
        io_batcher: &Arc<Mutex<TokIOBatcher>>,
    ) -> Result<(), GraphicsBackendHandleError> {
        let res = self.backend.init(io_batcher);
        match res {
            Ok(backend_buffer) => {
                self.backend_buffer = backend_buffer;
                Ok(())
            }
            Err(_) => Err(GraphicsBackendHandleError::BackendInitializationError),
        }
    }

    pub fn destroy(self) {
        self.backend.destroy()
    }

    fn flush_vertices_impl<T>(
        &mut self,
        state: &State,
        draw_mode: DrawModes,
        vertices_offset: usize,
        cmd: &mut T,
    ) -> bool
    where
        T: RenderCommand,
    {
        let prim_type: PrimType;
        let prim_count: usize;
        let num_verts: usize;

        num_verts = self.backend_buffer.num_vertices - vertices_offset;

        if num_verts == 0 {
            return false;
        }

        if draw_mode == DrawModes::Quads {
            prim_type = PrimType::Quads;
            prim_count = num_verts / 4;
        } else if draw_mode == DrawModes::Lines {
            prim_type = PrimType::Lines;
            prim_count = num_verts / 2;
        } else if draw_mode == DrawModes::Triangles {
            prim_type = PrimType::Triangles;
            prim_count = num_verts / 3;
        } else {
            return false;
        }

        cmd.set_state(*state);

        cmd.set_prim_type(prim_type);
        cmd.set_prim_count(prim_count);

        //TODO: m_pCommandBuffer->AddRenderCalls(1);
        return true;
    }

    fn quads_begin_from_draw_scope(draw_scope: DrawScope<4>) -> DrawQuads {
        DrawQuads::from_draw_scope(draw_scope)
    }
}

impl GraphicsRenderGeometry for GraphicsBackendHandle {
    fn lines_begin(&mut self) -> DrawLines {
        let vertices_offset = self.backend_buffer.num_vertices;
        DrawLines::new(self, vertices_offset)
    }

    fn triangles_begin(&mut self) -> DrawTriangles {
        let vertices_offset = self.backend_buffer.num_vertices;
        DrawTriangles::new(self, vertices_offset)
    }

    fn quads_begin(&mut self) -> DrawQuads {
        let vertices_offset = self.backend_buffer.num_vertices;
        DrawQuads::new(self, vertices_offset)
    }

    fn quads_tex_3d_begin(&mut self) -> DrawQuads {
        let vertices_offset = self.backend_buffer.num_vertices;
        DrawQuads::new(self, vertices_offset)
    }

    fn quad_scope_begin(&mut self) -> DrawScope<4> {
        let vertices_offset = self.backend_buffer.num_vertices;
        DrawScope::<4>::new(self, vertices_offset)
    }
}

pub struct GraphicsQuadContainerHandle {
    quad_containers: Vec<SQuadContainer>,
    first_free_quad_container: Option<usize>,
}

impl GraphicsQuadContainerHandle {
    pub fn new() -> Self {
        Self {
            quad_containers: Vec::new(),
            first_free_quad_container: None,
        }
    }

    fn is_quad_container_buffering_enabled() -> bool {
        true
    }

    pub fn create_quad_container(&mut self, builder: &QuadContainerBuilder) -> QuadContainerIndex {
        let index: usize;
        match self.first_free_quad_container {
            None => {
                index = self.quad_containers.len();
                self.quad_containers.push(builder.build(index));
            }
            Some(free_index) => {
                index = free_index;
                self.first_free_quad_container = self.quad_containers[index].next_free_index;
                self.quad_containers[index].next_free_index = None;
            }
        }

        return self.quad_containers[index].index.clone();
    }

    pub fn quad_container_change_automatic_upload(
        &mut self,
        container_index: &QuadContainerIndex,
        automatic_upload: bool,
    ) {
        let container = &mut self.quad_containers[container_index.get_index_unsafe()];
        container.automatic_upload = automatic_upload;
    }

    pub fn quad_container_upload(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        buffer_container_handle: &mut GraphicsBufferContainerHandle,
        buffer_object_handle: &mut GraphicsBufferObjectHandle,
        container_index: &QuadContainerIndex,
    ) {
        if Self::is_quad_container_buffering_enabled() {
            let container = &mut self.quad_containers[container_index.get_index_unsafe()];
            if !container.quads.is_empty() {
                match container.quad_buffer_object_index.as_ref() {
                    None => {
                        let upload_data_size = container.quads.len() * std::mem::size_of::<SQuad>();
                        container.quad_buffer_object_index =
                            Some(buffer_object_handle.create_buffer_object_slow(
                                backend_handle,
                                upload_data_size,
                                container.quads_to_bytes(),
                                0,
                            ));
                    }
                    Some(quad_buffer_object_index) => {
                        let upload_data_size = container.quads.len() * std::mem::size_of::<SQuad>();
                        buffer_object_handle.recreate_buffer_object_slow(
                            backend_handle,
                            quad_buffer_object_index,
                            upload_data_size,
                            container.quads_to_bytes(),
                            0,
                        );
                    }
                }

                if container.quad_buffer_container_index.is_none() {
                    let container_info = SBufferContainerInfo {
                        stride: std::mem::size_of::<SVertex>(),
                        vert_buffer_binding_index: container
                            .quad_buffer_object_index
                            .as_ref()
                            .unwrap()
                            .clone(),

                        attributes: vec![
                            SAttribute {
                                data_type_count: 2,
                                graphics_type: GraphicsType::Float,
                                normalized: false,
                                offset: 0,
                                func_type: 0,
                            },
                            SAttribute {
                                data_type_count: 2,
                                graphics_type: GraphicsType::Float,
                                normalized: false,
                                offset: (std::mem::size_of::<f32>() * 2),
                                func_type: 0,
                            },
                            SAttribute {
                                data_type_count: 4,
                                graphics_type: GraphicsType::UnsignedByte,
                                normalized: true,
                                offset: (std::mem::size_of::<f32>() * 2
                                    + std::mem::size_of::<f32>() * 2),
                                func_type: 0,
                            },
                        ],
                    };

                    container.quad_buffer_container_index = Some(
                        buffer_container_handle
                            .create_buffer_container(backend_handle, &container_info),
                    );
                }
            }
        }
    }

    /**
     * Returns the index of the first added quad
     */
    pub fn quad_container_add_quads(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        buffer_container_handle: &mut GraphicsBufferContainerHandle,
        buffer_object_handle: &mut GraphicsBufferObjectHandle,
        container_index: &QuadContainerIndex,
        quad_array: &[SQuad],
    ) -> usize {
        let container = &mut self.quad_containers[container_index.get_index_unsafe()];

        // TODO!: *4 -> *4 or *6 (if triangles) and also check for other add quads calls
        if container.quads.len() > quad_array.len() * 4 + StreamDataMax::MaxVertices as usize {
            panic!("quad count exceeded the maximum allowed number")
        }

        let ret_off = container.quads.len();

        container.quads.append(&mut quad_array.to_vec());

        if container.automatic_upload {
            self.quad_container_upload(
                backend_handle,
                buffer_container_handle,
                buffer_object_handle,
                container_index,
            );
        }

        return ret_off;
    }
    /*
    /**
     * Returns the index of the first added quad
     */
     fn QuadContainerAddQuads(&mut self, ContainerIndex: &QuadContainerIndex, pArray:& [CQuadItem]) -> usize
    {
        let Container = &mut self.m_vQuadContainers[ContainerIndex.unwrap()];

        if(Container.m_vQuads.len() > pArray.len()* 4 + StreamDataMax::MAX_VERTICES) {
           panic!("quad count exceeded the maximum allowed number")
        }

        let RetOff = Container.m_vQuads.len();

        for  i in 0 .. pArray.len()
        {
            Container.m_vQuads.push(Default::default());
            let mut Quad = Container.m_vQuads.last_mut().unwrap();

            Quad.m_aVertices[0].m_Pos.x = pArray[i].m_X;
            Quad.m_aVertices[0].m_Pos.y = pArray[i].m_Y;
            Quad.m_aVertices[0].m_Tex = m_aTexture[0];
            self.SetColor(&Quad.m_aVertices[0], 0);

            Quad.m_aVertices[1].m_Pos.x = pArray[i].m_X + pArray[i].m_Width;
            Quad.m_aVertices[1].m_Pos.y = pArray[i].m_Y;
            Quad.m_aVertices[1].m_Tex = m_aTexture[1];
            self.SetColor(&Quad.m_aVertices[1], 1);

            Quad.m_aVertices[2].m_Pos.x = pArray[i].m_X + pArray[i].m_Width;
            Quad.m_aVertices[2].m_Pos.y = pArray[i].m_Y + pArray[i].m_Height;
            Quad.m_aVertices[2].m_Tex = m_aTexture[2];
            self.SetColor(&Quad.m_aVertices[2], 2);

            Quad.m_aVertices[3].m_Pos.x = pArray[i].m_X;
            Quad.m_aVertices[3].m_Pos.y = pArray[i].m_Y + pArray[i].m_Height;
            Quad.m_aVertices[3].m_Tex = m_aTexture[3];
            self.SetColor(&Quad.m_aVertices[3], 3);

            if(self.m_Rotation != 0)
            {
                let Center = vec2 {
                x:  pArray[i].m_X + pArray[i].m_Width / 2.0,
                y:  pArray[i].m_Y + pArray[i].m_Height / 2.0
                };

                self.Rotate(Center, Quad.m_aVertices, 4);
            }
        }

        if(Container.m_AutomaticUpload) {
            self.QuadContainerUpload(ContainerIndex);
        }

        return RetOff;
    }

    /**
     * Returns the index of the first added quad
     */
     fn QuadContainerAddQuadsFreeform(&mut self,  ContainerIndex: &QuadContainerIndex, pArray: &[CFreeformItem]) -> usize
    {
        let Container = &mut self.m_vQuadContainers[ContainerIndex.unwrap()];

        if(Container.m_vQuads.len() > pArray.len()* 4 + StreamDataMax::MAX_VERTICES) {
           panic!("quad count exceeded the maximum allowed number")
        }

        let RetOff = Container.m_vQuads.len();

        for  i in 0 .. pArray.len()
        {
            Container.m_vQuads.push(Default::default());
            let mut Quad = Container.m_vQuads.last_mut().unwrap();

            Quad.m_aVertices[0].m_Pos.x = pArray[i].m_X0;
            Quad.m_aVertices[0].m_Pos.y = pArray[i].m_Y0;
            Quad.m_aVertices[0].m_Tex = m_aTexture[0];
            SetColor(&Quad.m_aVertices[0], 0);

            Quad.m_aVertices[1].m_Pos.x = pArray[i].m_X1;
            Quad.m_aVertices[1].m_Pos.y = pArray[i].m_Y1;
            Quad.m_aVertices[1].m_Tex = m_aTexture[1];
            SetColor(&Quad.m_aVertices[1], 1);

            Quad.m_aVertices[2].m_Pos.x = pArray[i].m_X3;
            Quad.m_aVertices[2].m_Pos.y = pArray[i].m_Y3;
            Quad.m_aVertices[2].m_Tex = m_aTexture[3];
            SetColor(&Quad.m_aVertices[2], 3);

            Quad.m_aVertices[3].m_Pos.x = pArray[i].m_X2;
            Quad.m_aVertices[3].m_Pos.y = pArray[i].m_Y2;
            Quad.m_aVertices[3].m_Tex = m_aTexture[2];
            SetColor(&Quad.m_aVertices[3], 2);
        }

        if(Container.m_AutomaticUpload) {
            self.QuadContainerUpload(ContainerIndex);
        }

        return RetOff;
    } */

    pub fn quad_container_reset(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        buffer_container_handle: &mut GraphicsBufferContainerHandle,
        buffer_object_handle: &mut GraphicsBufferObjectHandle,
        container_index: &QuadContainerIndex,
    ) {
        let container = &mut self.quad_containers[container_index.get_index_unsafe()];

        // clear the references to the buffer container & object first
        // so the indices are not counted as "in-use" anymore
        let mut quad_buffer_container_index = None;
        std::mem::swap(
            &mut quad_buffer_container_index,
            &mut container.quad_buffer_container_index,
        );
        container.quad_buffer_object_index = None;

        // then actually delete them
        if Self::is_quad_container_buffering_enabled() {
            if let Some(index) = quad_buffer_container_index {
                buffer_container_handle.delete_buffer_container(
                    backend_handle,
                    buffer_object_handle,
                    index,
                    true,
                );
            }
        }
        container.quads.clear();
    }

    pub fn delete_quad_container(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        buffer_container_handle: &mut GraphicsBufferContainerHandle,
        buffer_object_handle: &mut GraphicsBufferObjectHandle,
        container_index: QuadContainerIndex,
    ) {
        self.quad_container_reset(
            backend_handle,
            buffer_container_handle,
            buffer_object_handle,
            &container_index,
        );

        // also clear the container index
        let index = container_index.get_index_unsafe();
        container_index.drop_index_without_logic_unsafe();
        self.quad_containers[index].next_free_index = self.first_free_quad_container;
        self.first_free_quad_container = Some(index);
    }

    pub fn render_quad_container(
        &mut self,
        container_index: &QuadContainerIndex,
        quad_draw_num: &QuadContainerRenderCount,
        quad_scope: DrawScope<4>,
    ) {
        self.render_quad_container_ex(container_index, 0, quad_draw_num, quad_scope);
    }

    pub fn render_quad_container_ex(
        &mut self,
        container_index: &QuadContainerIndex,
        quad_offset: usize,
        quad_draw_count: &QuadContainerRenderCount,
        quad_scope: DrawScope<4>,
    ) {
        let container = &mut self.quad_containers[container_index.get_index_unsafe()];

        let quad_draw_num;
        match quad_draw_count {
            QuadContainerRenderCount::Auto => quad_draw_num = container.quads.len() - quad_offset,
            QuadContainerRenderCount::Count(count) => quad_draw_num = *count,
        }

        if container.quads.len() < quad_offset + quad_draw_num || quad_draw_num == 0 {
            return;
        }

        if Self::is_quad_container_buffering_enabled() {
            if container.quad_buffer_container_index.is_none() {
                return;
            }

            let mut cmd = CommandRenderQuadContainer::default();
            cmd.state = quad_scope.state;
            cmd.draw_num = quad_draw_num * 6;
            cmd.offset = quad_offset * 6 * std::mem::size_of::<u32>();
            cmd.buffer_container_index = container
                .quad_buffer_container_index
                .as_ref()
                .unwrap()
                .get_index_unsafe();

            quad_scope
                .backend_handle
                .add_cmd(AllCommands::Render(CommandsRender::QuadContainer(cmd)));

            // TODO: m_pCommandBuffer->AddRenderCalls(1);
        } else {
            let mut draw_quads = GraphicsBackendHandle::quads_begin_from_draw_scope(quad_scope);
            let prims = draw_quads.get_raw_handle(quad_draw_num);
            prims.iter_mut().enumerate().for_each(|(index, prim)| {
                *prim = container.quads[quad_offset + (index / 4)].vertices[index % 4]
            });
            drop(draw_quads);
        }
    }

    pub fn render_quad_container_ex2(
        &mut self,
        container_index: &QuadContainerIndex,
        quad_offset: usize,
        quad_draw_count: &QuadContainerRenderCount,
        x: f32,
        y: f32,
        scale_x: f32,
        scale_y: f32,
        mut quad_scope: DrawScope<4>,
    ) {
        let container = &mut self.quad_containers[container_index.get_index_unsafe()];

        if container.quads.len() < quad_offset + 1 {
            return;
        }

        let quad_draw_num = match quad_draw_count {
            QuadContainerRenderCount::Auto => container.quads.len() - quad_offset,
            QuadContainerRenderCount::Count(count) => *count,
        };

        if Self::is_quad_container_buffering_enabled() {
            if container.quad_buffer_container_index.is_none() {
                return;
            }

            let quad = &container.quads[quad_offset];
            let mut cmd = CommandRenderQuadContainerEx::default();

            quad_scope.wrap_clamp();

            let (canvas_x0, canvas_y0, canvas_x1, canvas_y1) = quad_scope.get_canvas_mapping();
            quad_scope.map_canvas(
                (canvas_x0 - x) / scale_x,
                (canvas_y0 - y) / scale_y,
                (canvas_x1 - x) / scale_x,
                (canvas_y1 - y) / scale_y,
            );
            cmd.state = quad_scope.state;
            quad_scope.map_canvas(canvas_x0, canvas_y0, canvas_x1, canvas_y1);

            cmd.draw_num = quad_draw_num * 6;
            cmd.offset = quad_offset * 6 * std::mem::size_of::<u32>();
            cmd.buffer_container_index = container
                .quad_buffer_container_index
                .as_ref()
                .unwrap()
                .get_index_unsafe();

            cmd.vertex_color.r = quad_scope.colors[0].r() as f32 / 255.0;
            cmd.vertex_color.g = quad_scope.colors[0].g() as f32 / 255.0;
            cmd.vertex_color.b = quad_scope.colors[0].b() as f32 / 255.0;
            cmd.vertex_color.a = quad_scope.colors[0].a() as f32 / 255.0;

            cmd.rotation = quad_scope.rotation;

            // rotate before positioning
            cmd.center.x = quad.vertices[0].get_pos().x
                + (quad.vertices[1].get_pos().x - quad.vertices[0].get_pos().x) / 2.0;
            cmd.center.y = quad.vertices[0].get_pos().y
                + (quad.vertices[2].get_pos().y - quad.vertices[0].get_pos().y) / 2.0;

            quad_scope
                .backend_handle
                .add_cmd(AllCommands::Render(CommandsRender::QuadContainerEx(cmd)));

            // TODO! m_pCommandBuffer->AddRenderCalls(1);
        } else {
            let rotation = quad_scope.rotation;
            let color = quad_scope.colors[0];
            let mut draw_quads = GraphicsBackendHandle::quads_begin_from_draw_scope(quad_scope);
            let verts = draw_quads.get_raw_handle(quad_draw_num);
            verts.iter_mut().enumerate().for_each(|(index, v)| {
                *v = container.quads[quad_offset + (index / 4)].vertices[index % 4];
            });
            for i in 0..quad_draw_num {
                for n in 0..4 {
                    verts[i * 4 + n].pos.x *= scale_x;
                    verts[i * 4 + n].pos.y *= scale_y;
                    verts[i * 4 + n].set_color(&color);
                }

                if rotation != 0.0 {
                    let center = vec2 {
                        x: verts[i * 4 + 0].pos.x
                            + (verts[i * 4 + 1].pos.x - verts[i * 4 + 0].pos.x) / 2.0,
                        y: verts[i * 4 + 0].pos.y
                            + (verts[i * 4 + 2].pos.y - verts[i * 4 + 0].pos.y) / 2.0,
                    };
                    rotate(&center, rotation, &mut verts[i * 4..i * 4 + 4]);
                }

                for n in 0..4 {
                    verts[i * 4 + n].pos.x += x;
                    verts[i * 4 + n].pos.y += y;
                }
            }
            draw_quads.wrap_clamp();
        }
    }

    pub fn render_quad_container_as_sprite_multiple(
        &mut self,
        container_index: &QuadContainerIndex,
        quad_offset: usize,
        quad_draw_count: &QuadContainerRenderCount,
        render_infos: PoolVec<SRenderSpriteInfo>,
        mut quad_scope: DrawScope<4>,
    ) {
        let container = &mut self.quad_containers[container_index.get_index_unsafe()];

        let draw_count;
        match quad_draw_count {
            QuadContainerRenderCount::Auto => draw_count = container.quads.len() - quad_offset,
            QuadContainerRenderCount::Count(count) => draw_count = *count,
        }
        if draw_count == 0 {
            return;
        }

        if Self::is_quad_container_buffering_enabled() {
            if container.quad_buffer_container_index.is_none() {
                return;
            }

            quad_scope.wrap_clamp();
            let quad = &container.quads[0];
            let mut cmd = CommandRenderQuadContainerAsSpriteMultiple {
                state: quad_scope.state,

                draw_num: 1 * 6,
                draw_count: draw_count,
                offset: quad_offset * 6 * std::mem::size_of::<u32>(),
                buffer_container_index: container
                    .quad_buffer_container_index
                    .as_ref()
                    .unwrap()
                    .get_index_unsafe(),

                vertex_color: ColorRGBA {
                    r: quad_scope.colors[0].r() as f32 / 255.0,
                    g: quad_scope.colors[0].g() as f32 / 255.0,
                    b: quad_scope.colors[0].b() as f32 / 255.0,
                    a: quad_scope.colors[0].a() as f32 / 255.0,
                },

                // rotate before positioning
                center: vec2::new(
                    quad.vertices[0].pos.x
                        + (quad.vertices[1].pos.x - quad.vertices[0].pos.x) / 2.0,
                    quad.vertices[0].pos.y
                        + (quad.vertices[2].pos.y - quad.vertices[0].pos.y) / 2.0,
                ),

                render_info: render_infos,
            };

            quad_scope.backend_handle.add_cmd(AllCommands::Render(
                CommandsRender::QuadContainerSpriteMultiple(cmd),
            ));

            // TODO! m_pCommandBuffer->AddRenderCalls(((DrawCount - 1) / gs_GraphicsMaxParticlesRenderCount) + 1);

            quad_scope.wrap_normal();
        } else {
            /* TODO!: for i in 0..DrawCount {
                quad_scope.rotation = pRenderInfo[i].m_Rotation;
                self.RenderQuadContainerAsSprite(
                    ContainerIndex,
                    QuadOffset,
                    pRenderInfo[i].m_Pos.x,
                    pRenderInfo[i].m_Pos.y,
                    pRenderInfo[i].m_Scale,
                    pRenderInfo[i].m_Scale,
                    quad_scope,
                );
            }*/
        }
    }
}

impl GraphicsRenderQuadContainer for GraphicsQuadContainerHandle {
    fn render_quad_container_as_sprite(
        &mut self,
        container_index: &QuadContainerIndex,
        quad_offset: usize,
        x: f32,
        y: f32,
        scale_x: f32,
        scale_y: f32,
        quad_scope: DrawScope<4>,
    ) {
        self.render_quad_container_ex2(
            container_index,
            quad_offset,
            &QuadContainerRenderCount::Count(1),
            x,
            y,
            scale_x,
            scale_y,
            quad_scope,
        );
    }
}

pub trait GraphicsHandlesTrait {
    fn get_handles(
        &mut self,
    ) -> (
        &mut GraphicsQuadContainerHandle,
        &mut GraphicsBackendHandle,
        &mut GraphicsBufferContainerHandle,
        &mut GraphicsBufferObjectHandle,
    );
}

pub trait GraphicsQuadContainerInterface
where
    Self: GraphicsHandlesTrait,
{
    fn get_quad_container_handle(&mut self) -> &mut GraphicsQuadContainerHandle;

    fn create_quad_container(&mut self, builder: &QuadContainerBuilder) -> QuadContainerIndex {
        self.get_quad_container_handle()
            .create_quad_container(builder)
    }

    fn quad_container_add_quads(
        &mut self,
        container_index: &QuadContainerIndex,
        quad_array: &[SQuad],
    ) -> usize {
        let (quad_cont_handle, backend_handle, buffer_cont_handle, buffer_obj_handle) =
            self.get_handles();
        quad_cont_handle.quad_container_add_quads(
            backend_handle,
            buffer_cont_handle,
            buffer_obj_handle,
            container_index,
            quad_array,
        )
    }

    fn quad_container_upload(&mut self, container_index: &QuadContainerIndex) {
        let (quad_cont_handle, backend_handle, buffer_container_handle, buffer_object_handle) =
            self.get_handles();
        quad_cont_handle.quad_container_upload(
            backend_handle,
            buffer_container_handle,
            buffer_object_handle,
            container_index,
        )
    }

    fn delete_quad_container(&mut self, container_index: QuadContainerIndex) {
        let (quad_cont_handle, backend_handle, buffer_container_handle, buffer_object_handle) =
            self.get_handles();
        quad_cont_handle.delete_quad_container(
            backend_handle,
            buffer_container_handle,
            buffer_object_handle,
            container_index,
        )
    }
}

pub struct GraphicsBufferObjectIndex {
    index: BufferObjectIndex,
    next_free_index: Option<usize>,
}

pub struct GraphicsBufferObjectHandle {
    buffer_object_indices: Vec<GraphicsBufferObjectIndex>,
    first_free_buffer_object_index: Option<usize>,
}

impl GraphicsBufferObjectHandle {
    pub fn new() -> Self {
        Self {
            buffer_object_indices: Vec::new(),
            first_free_buffer_object_index: None,
        }
    }

    pub fn create_buffer_object(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        _upload_data_size: usize,
        upload_data: GraphicsBackendMemory,
        create_flags: i32,
    ) -> BufferObjectIndex {
        let index;
        match self.first_free_buffer_object_index {
            None => {
                let new_index = self.buffer_object_indices.len();
                self.buffer_object_indices.push(GraphicsBufferObjectIndex {
                    index: BufferObjectIndex::new(new_index),
                    next_free_index: None,
                });
                index = new_index;
            }
            Some(new_index) => {
                index = new_index;
                self.first_free_buffer_object_index =
                    self.buffer_object_indices[new_index].next_free_index;
                self.buffer_object_indices[new_index].next_free_index = None;
            }
        }

        let mut cmd = CommandCreateBufferObject::default();
        cmd.buffer_index = index;
        cmd.flags = create_flags;
        cmd.upload_data = RefCell::new(Some(upload_data));

        backend_handle.add_cmd(AllCommands::Misc(Commands::CreateBufferObject(cmd)));

        self.buffer_object_indices[index].index.clone()
    }

    pub fn create_buffer_object_slow(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        upload_data_size: usize,
        upload_data: Vec<u8>,
        create_flags: i32,
    ) -> BufferObjectIndex {
        let mut buffer_mem = backend_handle
            .backend
            .mem_alloc(GraphicsMemoryAllocationType::Buffer, upload_data.len());
        buffer_mem.copy_from_slice(&upload_data);
        self.create_buffer_object(backend_handle, upload_data_size, buffer_mem, create_flags)
    }

    pub fn recreate_buffer_object(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        buffer_index: &BufferObjectIndex,
        _upload_data_size: usize,
        upload_data: GraphicsBackendMemory,
        create_flags: i32,
    ) {
        let mut cmd = CommandRecreateBufferObject::default();
        cmd.buffer_index = buffer_index.get_index_unsafe();
        cmd.flags = create_flags;
        cmd.upload_data = RefCell::new(Some(upload_data));

        backend_handle.add_cmd(AllCommands::Misc(Commands::RecreateBufferObject(cmd)));
    }

    pub fn recreate_buffer_object_slow(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        buffer_index: &BufferObjectIndex,
        upload_data_size: usize,
        upload_data: Vec<u8>,
        create_flags: i32,
    ) {
        let mut buffer_mem = backend_handle
            .backend
            .mem_alloc(GraphicsMemoryAllocationType::Buffer, upload_data.len());
        buffer_mem.copy_from_slice(&upload_data);
        self.recreate_buffer_object(
            backend_handle,
            buffer_index,
            upload_data_size,
            buffer_mem,
            create_flags,
        )
    }

    pub fn update_buffer_object_internal(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        buffer_index: &BufferObjectIndex,
        _upload_data_size: usize,
        upload_data: Vec<u8>,
        offset: usize,
    ) {
        let mut cmd = CommandUpdateBufferObject::default();
        cmd.buffer_index = buffer_index.get_index_unsafe();
        cmd.offset = offset;
        cmd.upload_data = upload_data;

        backend_handle.add_cmd(AllCommands::Misc(Commands::UpdateBufferObject(cmd)));
    }

    pub fn copy_buffer_object_internal(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        write_buffer_index: &BufferObjectIndex,
        read_buffer_index: &BufferObjectIndex,
        write_offset: usize,
        read_offset: usize,
        copy_data_size: usize,
    ) {
        let mut cmd = CommandCopyBufferObject::default();
        cmd.write_buffer_index = write_buffer_index.get_index_unsafe();
        cmd.read_buffer_index = read_buffer_index.get_index_unsafe();
        cmd.write_offset = write_offset;
        cmd.read_offset = read_offset;
        cmd.copy_size = copy_data_size;

        backend_handle.add_cmd(AllCommands::Misc(Commands::CopyBufferObject(cmd)));
    }

    pub fn delete_buffer_object(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        buffer_index: BufferObjectIndex,
    ) {
        let cmd = CommandDeleteBufferObject {
            buffer_index: buffer_index.get_index_unsafe(),
        };

        backend_handle.add_cmd(AllCommands::Misc(Commands::DleteBufferObject(cmd)));

        // also clear the buffer object index
        let index = buffer_index.get_index_unsafe();
        self.buffer_object_indices[index].next_free_index = self.first_free_buffer_object_index;
        self.first_free_buffer_object_index = Some(index);
    }
}

pub trait GraphicsBufferObjectHandleInterface
where
    Self: GraphicsHandlesTrait,
{
    fn create_buffer_object(
        &mut self,
        upload_data_size: usize,
        upload_data: GraphicsBackendMemory,
        create_flags: i32,
    ) -> BufferObjectIndex {
        let (_, backend_handle, _, buffer_object_handle) = self.get_handles();
        buffer_object_handle.create_buffer_object(
            backend_handle,
            upload_data_size,
            upload_data,
            create_flags,
        )
    }
}

pub struct GraphicsBufferContainerHandle {
    vertex_array_info: Vec<SVertexArrayInfo>,
    first_free_vertex_array_info: Option<usize>,
}

impl GraphicsBufferContainerHandle {
    pub fn new() -> Self {
        Self {
            vertex_array_info: Vec::new(),
            first_free_vertex_array_info: None,
        }
    }

    pub fn create_buffer_container(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        container_info: &SBufferContainerInfo,
    ) -> BufferContainerIndex {
        let index;
        match self.first_free_vertex_array_info {
            None => {
                index = self.vertex_array_info.len();
                self.vertex_array_info.push(SVertexArrayInfo {
                    next_free_index: None,
                    index: BufferContainerIndex::new(index),
                    associated_buffer_object_index: None,
                });
            }
            Some(new_index) => {
                index = new_index;
                self.first_free_vertex_array_info = self.vertex_array_info[index].next_free_index;
                self.vertex_array_info[index].next_free_index = None;
            }
        }

        let mut cmd = CommandCreateBufferContainer::default();
        cmd.buffer_container_index = index;
        cmd.attributes = container_info.attributes.clone();
        cmd.stride = container_info.stride;
        cmd.vert_buffer_binding_index = container_info.vert_buffer_binding_index.get_index_unsafe();

        backend_handle.add_cmd(AllCommands::Misc(Commands::CreateBufferContainer(cmd)));

        self.vertex_array_info[index].associated_buffer_object_index =
            Some(container_info.vert_buffer_binding_index.clone());

        return self.vertex_array_info[index].index.clone();
    }

    pub fn delete_buffer_container(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        buffer_object_handle: &mut GraphicsBufferObjectHandle,
        container_index: BufferContainerIndex,
        destroy_all_associated_buffer_objects: bool,
    ) {
        let mut cmd = CommandDeleteBufferContainer::default();
        cmd.buffer_container_index = container_index.get_index_unsafe();
        cmd.destroy_all_buffer_objects = destroy_all_associated_buffer_objects;

        backend_handle.add_cmd(AllCommands::Misc(Commands::DeleteBufferContainer(cmd)));

        if destroy_all_associated_buffer_objects {
            // delete all associated references
            let mut buffer_object_index = None;
            std::mem::swap(
                &mut buffer_object_index,
                &mut self.vertex_array_info[container_index.get_index_unsafe()]
                    .associated_buffer_object_index,
            );

            // clear the buffer object index
            let buffer_index = buffer_object_index.as_ref().unwrap().get_index_unsafe();
            if buffer_object_index.is_some() {
                buffer_object_handle.buffer_object_indices[buffer_index].next_free_index =
                    buffer_object_handle.first_free_buffer_object_index;
                buffer_object_handle.first_free_buffer_object_index = Some(buffer_index);
            }
            buffer_object_index
                .unwrap()
                .drop_index_without_logic_unsafe();
        }

        // also clear the buffer object index
        let index = container_index.get_index_unsafe();
        container_index.drop_index_without_logic_unsafe();
        self.vertex_array_info[index].next_free_index = self.first_free_vertex_array_info;
        self.first_free_vertex_array_info = Some(index);
    }

    pub fn update_buffer_container_internal(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        container_index: &BufferContainerIndex,
        container_info: &SBufferContainerInfo,
    ) {
        let mut cmd = CommandUpdateBufferContainer::default();
        cmd.buffer_container_index = container_index.get_index_unsafe();
        cmd.attributes = container_info.attributes.clone();
        cmd.stride = container_info.stride;
        cmd.vert_buffer_binding_index = container_info.vert_buffer_binding_index.get_index_unsafe();

        backend_handle.add_cmd(AllCommands::Misc(Commands::UpdateBufferContainer(cmd)));

        self.vertex_array_info[container_index.get_index_unsafe()].associated_buffer_object_index =
            Some(container_info.vert_buffer_binding_index.clone());
    }
}

pub trait GraphicsBufferContainerHandleInterface
where
    Self: GraphicsHandlesTrait,
{
    fn create_buffer_container(
        &mut self,
        container_info: &SBufferContainerInfo,
    ) -> BufferContainerIndex {
        let (_, backend_handle, buffer_container_handle, _buffer_object_handle) =
            self.get_handles();
        buffer_container_handle.create_buffer_container(backend_handle, container_info)
    }

    fn delete_buffer_container(
        &mut self,
        container_index: BufferContainerIndex,
        destroy_all_associated_buffer_objects: bool,
    ) {
        let (_, backend_handle, buffer_container_handle, buffer_object_handle) = self.get_handles();
        buffer_container_handle.delete_buffer_container(
            backend_handle,
            buffer_object_handle,
            container_index,
            destroy_all_associated_buffer_objects,
        )
    }
}

#[derive(Clone)]
struct TextureContainerIndex {
    index: TextureIndex,
    next_free_index: Option<usize>,
}

pub struct Graphics {
    pub backend_handle: GraphicsBackendHandle,

    window: WindowProps,

    texture_indices: Vec<TextureContainerIndex>,
    first_free_texture: Option<usize>,

    pub quad_container_handle: GraphicsQuadContainerHandle,

    pub buffer_object_handle: GraphicsBufferObjectHandle,

    pub buffer_container_handle: GraphicsBufferContainerHandle,

    pub quad_render_info_pool: Pool<Vec<SQuadRenderInfo>>,
    pub sprite_render_info_pool: Pool<Vec<SRenderSpriteInfo>>,
    pub index_offset_or_draw_count_pool: Pool<Vec<usize>>,
}

impl Graphics {
    pub fn new() -> Graphics {
        Graphics {
            window: Default::default(),

            backend_handle: GraphicsBackendHandle::new(),

            texture_indices: Vec::new(),
            first_free_texture: None,

            quad_container_handle: GraphicsQuadContainerHandle::new(),

            buffer_object_handle: GraphicsBufferObjectHandle::new(),

            buffer_container_handle: GraphicsBufferContainerHandle::new(),

            quad_render_info_pool: Pool::with_sized(8, || Vec::with_capacity(64)),
            sprite_render_info_pool: Pool::with_sized(8, || Vec::with_capacity(512)),
            index_offset_or_draw_count_pool: Pool::with_sized(64 * 2, || Vec::with_capacity(128)),
        }
    }

    pub fn load_io(&mut self, io_pipe: &mut GraphicsLoadIOPipe) {
        self.backend_handle.load_io(io_pipe);
    }

    pub fn init_while_io(&mut self, pipe: &mut GraphicsLoadWhileIOPipe) {
        self.texture_indices
            .reserve(StreamDataMax::MaxTextures as usize);
        self.first_free_texture = None;

        self.backend_handle.init_while_io(pipe);

        self.window = *self.backend_handle.backend.get_window_props();
    }

    pub fn init(&mut self, io_batcher: &Arc<Mutex<TokIOBatcher>>) -> anyhow::Result<()> {
        Ok(self.backend_handle.init(io_batcher)?)
    }

    pub fn destroy(mut self) {
        self.backend_handle.run_backend_buffer();
        self.backend_handle.backend.destroy();
    }

    pub fn get_graphics_mt(&self) -> GraphicsMultiThreaded {
        GraphicsMultiThreaded::new(self.backend_handle.backend.get_backend_mt())
    }

    fn image_format_to_pixel_size(format: i32) -> usize {
        let f = ImageFormat::from_i32(format);
        if let Some(f) = f {
            match f {
                ImageFormat::Rgb => {
                    return 3;
                }
                ImageFormat::SingleComponent => {
                    return 1;
                }
                _ => {
                    return 4;
                }
            }
        }
        return 4;
    }

    fn convert_to_rgba(
        &mut self,
        mut src_mem: GraphicsBackendMemory,
        src_width: usize,
        src_height: usize,
        src_format: i32,
    ) -> Result<GraphicsBackendMemory, ArrayString<4096>> {
        if src_format == TexFormat::RGBA as i32 {
            return Ok(src_mem);
        } else {
            let mut data_slice = self.mem_alloc(
                GraphicsMemoryAllocationType::Texture,
                src_width * src_height * 4,
            );
            let mut data_opt: Option<GraphicsBackendMemory> = None;
            std::mem::swap(&mut data_slice.memory, &mut data_opt);
            drop(data_slice);
            let mut res = data_opt.unwrap();

            let src_channel_count = Self::image_format_to_pixel_size(src_format);
            let dst_channel_count = 4;
            for y in 0..src_height {
                for x in 0..src_width {
                    let img_offset_src =
                        (y * src_width * src_channel_count) + (x * src_channel_count);
                    let img_offset_dest =
                        (y * src_width * dst_channel_count) + (x * dst_channel_count);
                    let copy_size = src_channel_count;
                    if src_channel_count == 3 {
                        res.as_mut_slice()[img_offset_dest..img_offset_dest + copy_size]
                            .copy_from_slice(
                                &src_mem.as_mut_slice()[img_offset_src..img_offset_src + copy_size],
                            );
                        res.as_mut_slice()[img_offset_dest + 3] = 255;
                    } else if src_channel_count == 1 {
                        res.as_mut_slice()[img_offset_dest + 0] = 255;
                        res.as_mut_slice()[img_offset_dest + 1] = 255;
                        res.as_mut_slice()[img_offset_dest + 2] = 255;
                        res.as_mut_slice()[img_offset_dest + 3] =
                            src_mem.as_mut_slice()[img_offset_src];
                    }
                }
            }

            self.backend_handle.backend.mem_free(src_mem);
            return Ok(res);
        }
    }

    fn load_texture_impl<'a>(
        &mut self,
        width: usize,
        height: usize,
        depth: usize,
        is_3d_tex: bool,
        format: i32,
        data: GraphicsBackendMemory,
        _store_format: i32,
        flags: TexFlags,
        _tex_name: &str,
    ) -> anyhow::Result<TextureIndex> {
        if width == 0 || height == 0 {
            return Err(anyhow!("width and/or height was 0"));
        }

        // grab texture
        let mut tex = self.first_free_texture;
        if tex.is_none() {
            let cur_size = self.texture_indices.len();
            self.texture_indices.reserve(cur_size * 2);
            self.texture_indices.push(TextureContainerIndex {
                index: TextureIndex::new(cur_size),
                next_free_index: None,
            });

            tex = Some(cur_size);
        }
        self.first_free_texture = self.texture_indices[tex.unwrap()].next_free_index;

        // flags
        let mut cmd_flags = TexFlags::empty();
        if !(flags & TexFlags::TEXFLAG_NOMIPMAPS).is_empty() {
            cmd_flags |= TexFlags::TEXFLAG_NOMIPMAPS;
        }

        let pixel_size = 4;

        // copy texture data
        let _mem_size = width * height * pixel_size;
        let tmp_buff_data = self.convert_to_rgba(data, width as usize, height as usize, format);
        //if(!)
        //{
        // TODO dbg_msg("graphics", "converted image %s to RGBA, consider making its file format RGBA", pTexName ? pTexName : "(no name)");
        //}
        if let Ok(data) = tmp_buff_data {
            let cmd = CommandTextureCreate {
                slot: tex.unwrap(),
                width,
                height,
                depth: depth,
                is_3d_tex: is_3d_tex,
                pixel_size,
                format: TexFormat::RGBA as i32,
                store_format: TexFormat::RGBA as i32,
                flags: cmd_flags,
                data: RefCell::new(Some(data)),
            };

            self.backend_handle
                .add_cmd(AllCommands::Misc(Commands::TextureCreate(cmd)));

            Ok(self.texture_indices[tex.unwrap()].index.clone())
        } else {
            // TODO: log error
            Err(anyhow!("image could not be converted to rgba"))
        }
    }

    pub fn load_texture(
        &mut self,
        width: usize,
        height: usize,
        format: i32,
        data: GraphicsBackendMemory,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<TextureIndex> {
        self.load_texture_impl(
            width,
            height,
            1,
            false,
            format,
            data,
            store_format,
            flags,
            tex_name,
        )
    }

    pub fn load_texture_3d(
        &mut self,
        width: usize,
        height: usize,
        depth: usize,
        format: i32,
        data: GraphicsBackendMemory,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<TextureIndex> {
        self.load_texture_impl(
            width,
            height,
            depth,
            true,
            format,
            data,
            store_format,
            flags,
            tex_name,
        )
    }

    pub fn unload_texture(&mut self, texture_id: TextureIndex) {
        let tex_index = texture_id.get_index_unsafe();
        texture_id.drop_index_without_logic_unsafe();
        let cmd = CommandTextureDestroy { slot: tex_index };
        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::TextureDestroy(cmd)));

        self.texture_indices[tex_index].next_free_index = self.first_free_texture;
        self.first_free_texture = Some(tex_index);
    }

    pub fn is_tile_buffering_enabled(&self) -> bool {
        false
    }

    pub fn resized(
        &mut self,
        window_handling: &mut dyn NativeImpl,
        new_width: u32,
        new_height: u32,
    ) {
        self.backend_handle
            .backend
            .resized(window_handling, new_width, new_height);
        self.window = *self.backend_handle.backend.get_window_props();
    }

    pub fn swap(&mut self) {
        let cmd = CommandSwap {};
        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::Swap(cmd)));
        self.backend_handle.run_backend_buffer();
    }

    pub fn switch_to_dual_pass(&mut self) {
        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::SwitchToDualPass));
        self.backend_handle.run_backend_buffer();
    }

    /**
     * Allocates memory to be used in the backend
     */
    pub fn mem_alloc(
        &mut self,
        alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> GraphicsRawMemoryBuffer {
        GraphicsRawMemoryBuffer {
            memory: Some(self.backend_handle.backend.mem_alloc(alloc_type, req_size)),
            graphics: Some(self),
        }
    }

    pub fn indices_num_required_notify(&mut self, required_indices_count: usize) {
        let mut cmd = CommandIndicesRequiredNumNotify::default();
        cmd.required_indices_num = required_indices_count;

        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::IndicesRequiredNumNotify(cmd)));
    }

    pub fn render_tile_layer(
        &mut self,
        state: &State,
        buffer_container_index: &BufferContainerIndex,
        color: &ColorRGBA,
        offsets: PoolVec<usize>,
        indiced_vertex_draw_num: PoolVec<usize>,
        num_indices_offset: usize,
    ) {
        if num_indices_offset == 0 {
            return;
        }

        // add the VertexArrays and draw
        let mut cmd = CommandRenderTileLayer {
            state: *state,
            indices_draw_num: num_indices_offset,
            buffer_container_index: buffer_container_index.get_index_unsafe(),
            color: *color,

            indices_offsets: offsets,
            draw_count: indiced_vertex_draw_num,
        };

        self.backend_handle
            .add_cmd(AllCommands::Render(CommandsRender::TileLayer(cmd)));

        // TODO m_pCommandBuffer->AddRenderCalls(NumIndicesOffset);
        // todo max indices group check!!
    }

    pub fn render_border_tiles(
        &mut self,
        state: &State,
        buffer_container_index: &BufferContainerIndex,
        color: &ColorRGBA,
        index_buffer_offset: usize,
        offset: &vec2,
        dir: &vec2,
        jump_index: i32,
        draw_num: usize,
    ) {
        if draw_num == 0 {
            return;
        }
        // Draw a border tile a lot of times
        let mut cmd = CommandRenderBorderTile::default();
        cmd.state = *state;
        cmd.draw_num = draw_num;
        cmd.buffer_container_index = buffer_container_index.get_index_unsafe();
        cmd.color = *color;

        cmd.indices_offset = index_buffer_offset;
        cmd.jump_index = jump_index;

        cmd.offset = *offset;
        cmd.dir = *dir;

        self.backend_handle
            .add_cmd(AllCommands::Render(CommandsRender::BorderTile(cmd)));

        // TODO: m_pCommandBuffer->AddRenderCalls(1);
    }

    pub fn render_border_tile_lines(
        &mut self,
        state: &State,
        buffer_container_index: &BufferContainerIndex,
        color: &ColorRGBA,
        index_buffer_offset: usize,
        offset: &vec2,
        dir: &vec2,
        index_draw_num: usize,
        redraw_num: usize,
    ) {
        if index_draw_num == 0 || redraw_num == 0 {
            return;
        }
        // Draw a border tile a lot of times
        let mut cmd = CommandRenderBorderTileLine::default();
        cmd.state = *state;
        cmd.index_draw_num = index_draw_num;
        cmd.draw_num = redraw_num;
        cmd.buffer_container_index = buffer_container_index.get_index_unsafe();
        cmd.color = *color;

        cmd.indices_offset = index_buffer_offset;

        cmd.offset = *offset;
        cmd.dir = *dir;

        self.backend_handle
            .add_cmd(AllCommands::Render(CommandsRender::BorderTileLine(cmd)));

        // TODO m_pCommandBuffer->AddRenderCalls(1);
    }

    pub fn render_quad_layer(
        &mut self,
        state: &State,
        buffer_container_index: &BufferContainerIndex,
        quad_render_infos: PoolVec<SQuadRenderInfo>,
        quad_num: usize,
        quad_offset: usize,
    ) {
        if quad_num == 0 {
            return;
        }

        // add the VertexArrays and draw
        let mut cmd = CommandRenderQuadLayer {
            state: *state,
            quad_num: quad_num,
            quad_offset: quad_offset,
            buffer_container_index: buffer_container_index.get_index_unsafe(),

            quad_info: quad_render_infos,
        };

        self.backend_handle
            .add_cmd(AllCommands::Render(CommandsRender::QuadLayer(cmd)));

        // TODO m_pCommandBuffer->AddRenderCalls(((QuadNum - 1) / gs_GraphicsMaxQuadsRenderCount) + 1);
    }

    pub fn last_render_call_as_second_pass_transition(&mut self) {
        if let Some(AllCommands::Render(CommandsRender::Render(cmd))) =
            self.backend_handle.backend_buffer.cmds.pop()
        {
            self.backend_handle
                .backend_buffer
                .cmds
                .push(AllCommands::Misc(Commands::NextSubpass));
            self.backend_handle
                .backend_buffer
                .cmds
                .push(AllCommands::Render(CommandsRender::RenderFirstPassBlurred(
                    cmd,
                )));
        }
    }

    pub fn do_screenshot(&mut self, fs: &Arc<FileSystem>, io_batcher: &Arc<Mutex<TokIOBatcher>>) {
        let mut width: u32 = 0;
        let mut height: u32 = 0;
        let mut dest_data_buffer: Vec<u8> = Default::default();
        let image_format = self.backend_handle.backend.do_screenshot(
            &mut width,
            &mut height,
            &mut dest_data_buffer,
        );
        if let Ok(_) = image_format {
            // TODO: save_png_image
            let fs = fs.clone();
            io_batcher.lock().unwrap().spawn_without_queue(async move {
                fs.write_file("", dest_data_buffer).await;
                Ok(())
            });
        }
    }
}

impl GraphicsSizeQuery for Graphics {
    fn canvas_aspect(&self) -> f32 {
        (self.window.canvas_width as f32) / self.window.canvas_height as f32
    }

    fn canvas_width(&self) -> u32 {
        self.window.canvas_width as u32
    }

    fn canvas_height(&self) -> u32 {
        self.window.canvas_height as u32
    }

    fn window_width(&self) -> u32 {
        self.window.window_width
    }

    fn window_height(&self) -> u32 {
        self.window.window_height
    }
}

impl GraphicsRenderGeometry for Graphics {
    fn lines_begin(&mut self) -> DrawLines {
        self.backend_handle.lines_begin()
    }

    fn triangles_begin(&mut self) -> DrawTriangles {
        self.backend_handle.triangles_begin()
    }

    fn quads_begin(&mut self) -> DrawQuads {
        self.backend_handle.quads_begin()
    }

    fn quads_tex_3d_begin(&mut self) -> DrawQuads {
        self.backend_handle.quads_tex_3d_begin()
    }

    fn quad_scope_begin(&mut self) -> DrawScope<4> {
        self.backend_handle.quad_scope_begin()
    }
}

impl GraphicsRenderHandles for Graphics {
    fn get_render_handles(
        &mut self,
    ) -> (
        &mut dyn GraphicsRenderGeometry,
        &mut dyn GraphicsRenderQuadContainer,
    ) {
        (&mut self.backend_handle, &mut self.quad_container_handle)
    }
}

impl GraphicsTextureAllocations for Graphics {
    fn load_texture_slow(
        &mut self,
        width: usize,
        height: usize,
        format: i32,
        data: Vec<u8>,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<TextureIndex> {
        let mut data_slice = self.mem_alloc(GraphicsMemoryAllocationType::Texture, data.len());
        let mut data_opt: Option<GraphicsBackendMemory> = None;
        std::mem::swap(&mut data_slice.memory, &mut data_opt);
        drop(data_slice);
        let mut mem = data_opt.unwrap();
        mem.copy_from_slice(data.as_slice());
        self.load_texture_impl(
            width,
            height,
            1,
            false,
            format,
            mem,
            store_format,
            flags,
            tex_name,
        )
    }

    fn load_texture_3d_slow(
        &mut self,
        width: usize,
        height: usize,
        depth: usize,
        format: i32,
        data: Vec<u8>,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<TextureIndex> {
        let mut data_slice = self.mem_alloc(GraphicsMemoryAllocationType::Texture, data.len());
        let mut data_opt: Option<GraphicsBackendMemory> = None;
        std::mem::swap(&mut data_slice.memory, &mut data_opt);
        drop(data_slice);
        let mut mem = data_opt.unwrap();
        mem.copy_from_slice(data.as_slice());
        self.load_texture_impl(
            width,
            height,
            depth,
            true,
            format,
            mem,
            store_format,
            flags,
            tex_name,
        )
    }
}

impl GraphicsHandlesTrait for Graphics {
    fn get_handles(
        &mut self,
    ) -> (
        &mut GraphicsQuadContainerHandle,
        &mut GraphicsBackendHandle,
        &mut GraphicsBufferContainerHandle,
        &mut GraphicsBufferObjectHandle,
    ) {
        (
            &mut self.quad_container_handle,
            &mut self.backend_handle,
            &mut self.buffer_container_handle,
            &mut self.buffer_object_handle,
        )
    }
}

impl GraphicsQuadContainerInterface for Graphics {
    fn get_quad_container_handle(&mut self) -> &mut GraphicsQuadContainerHandle {
        &mut self.quad_container_handle
    }
}

impl GraphicsBufferContainerHandleInterface for Graphics {}

impl GraphicsBufferObjectHandleInterface for Graphics {}

impl GraphicsInterface for Graphics {}
