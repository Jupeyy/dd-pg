use std::{cell::RefCell, collections::HashMap, rc::Rc};

use anyhow::anyhow;
use arrayvec::ArrayString;

use base::shared_index::{SharedIndexCleanup, SharedIndexGetIndexUnsafe};
use graphics_backend_traits::{traits::GraphicsBackendInterface, types::BackendCommands};
use graphics_base::{
    buffer_object_handle::{BufferObjectIndex, GraphicsBufferObjectHandleInterface},
    quad_container::{
        GraphicsQuadContainerHandleInterface, QuadContainerIndex, SQuad, SQuadContainer,
    },
    streaming::{DrawLines, DrawQuads, DrawScope, DrawTriangles, GraphicsStreamHandleInterface},
};
use graphics_base_traits::traits::{
    GraphicsBackendHandleInterface, GraphicsSizeQuery, GraphicsStreamDataInterface,
};
use graphics_render_traits::{GraphicsHandles, GraphicsHandlesInterface};
use graphics_traits::GraphicsInterface;
use num_traits::FromPrimitive;

use math::math::vector::vec2;
use pool::{mt_datatypes::PoolVec, mt_pool::Pool};
use thiserror::Error;

use crate::{
    graphics_mt::GraphicsMultiThreaded,
    types::{GraphicsBufferObject, TextureContainer},
};

use graphics_types::{
    command_buffer::{
        AllCommands, CommandCreateBufferObject, CommandDeleteBufferObject,
        CommandIndicesRequiredNumNotify, CommandRecreateBufferObject, CommandRender,
        CommandRenderBorderTile, CommandRenderBorderTileLine,
        CommandRenderQuadContainerAsSpriteMultiple, CommandRenderQuadContainerEx,
        CommandRenderQuadLayer, CommandRenderTileLayer, CommandSwap, CommandTextureCreate,
        CommandTextureDestroy, CommandUpdateViewport, Commands, CommandsRender, PrimType,
        RenderCommand, SQuadRenderInfo, SRenderSpriteInfo, StreamDataMax, TexFlags, TexFormat,
    },
    rendering::{ColorRGBA, State, WriteVertexAttributes},
    textures_handle::{GraphicsTextureHandleInterface, TextureIndex},
    types::{
        DrawModes, GraphicsBackendMemory, GraphicsMemoryAllocationType, ImageFormat, VideoMode,
        WindowProps,
    },
};

const _FAKE_MODES: [VideoMode; 68] = [
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

/**
 * This buffer is useful if you want to
 * allocate memory that is going to be used by the
 * graphics engine. It might optimize away expensive
 * heap allocations and copying.
 */
pub struct GraphicsRawMemoryBuffer<'a, B: GraphicsBackendInterface> {
    memory: Option<GraphicsBackendMemory>,
    graphics: Option<&'a mut GraphicsBase<B>>,
}

/**
 * implements minimal graphics traits that are useful for
 * memory management from the backend
 */
pub struct StagingGraphics<'a, B: GraphicsBackendInterface> {
    memory: Option<GraphicsBackendMemory>,
    graphics: &'a mut GraphicsBase<B>,
}

impl<'a, B: GraphicsBackendInterface + 'static> StagingGraphics<'a, B> {
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
        self.graphics.texture_handle.load_texture_impl(
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

impl<'a, B: GraphicsBackendInterface> GraphicsRawMemoryBuffer<'a, B> {
    pub fn exec(mut self) -> StagingGraphics<'a, B> {
        let mut memory: Option<GraphicsBackendMemory> = None;
        let mut graphics: Option<&'a mut GraphicsBase<B>> = None;
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

pub enum QuadContainerRenderCount {
    Auto,
    Count(usize),
}

#[derive(Error, Debug)]
pub enum GraphicsBackendHandleError {
    #[error("TODO: Not yet implemented.")]
    BackendInitializationError,
}

#[derive(Debug)]
pub struct GraphicsBackendHandle<B: GraphicsBackendInterface> {
    pub backend_cmds: BackendCommands,
    backend: Rc<RefCell<B>>,
}

impl<B: GraphicsBackendInterface> Clone for GraphicsBackendHandle<B> {
    fn clone(&self) -> Self {
        Self {
            backend_cmds: self.backend_cmds.clone(),
            backend: self.backend.clone(),
        }
    }
}

impl<B: GraphicsBackendInterface> GraphicsBackendHandleInterface for GraphicsBackendHandle<B> {
    fn run_backend_buffer(&mut self, stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>) {
        self.backend
            .borrow_mut()
            .run_cmds(&mut self.backend_cmds, stream_data);
    }

    fn add_cmd(&self, cmd: AllCommands) {
        self.backend_cmds.cmds.borrow_mut().push(cmd);
    }

    fn mem_alloc(
        &mut self,
        alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> GraphicsBackendMemory {
        self.backend.borrow_mut().mem_alloc(alloc_type, req_size)
    }
}

impl<B: GraphicsBackendInterface> GraphicsBackendHandle<B> {
    fn new(backend: Rc<RefCell<B>>) -> Self {
        Self {
            backend_cmds: BackendCommands::default(),
            backend,
        }
    }
}

pub struct GraphicsStreamHandle<B: GraphicsBackendInterface> {
    pub stream_data: Rc<RefCell<dyn GraphicsStreamDataInterface>>,

    backend_handle: GraphicsBackendHandle<B>,
}

impl<B: GraphicsBackendInterface> Clone for GraphicsStreamHandle<B> {
    fn clone(&self) -> Self {
        Self {
            stream_data: self.stream_data.clone(),
            backend_handle: self.backend_handle.clone(),
        }
    }
}

impl<B: GraphicsBackendInterface> GraphicsStreamHandle<B> {
    fn new(
        stream_data: Rc<RefCell<dyn GraphicsStreamDataInterface>>,
        backend_handle: GraphicsBackendHandle<B>,
    ) -> Self {
        Self {
            stream_data,
            backend_handle,
        }
    }
}

fn flush_vertices_impl<T>(
    stream_data: &mut dyn GraphicsStreamDataInterface,
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
    num_verts = stream_data.vertices_count() - vertices_offset;

    if num_verts == 0 {
        return false;
    }

    match draw_mode {
        DrawModes::Quads => {
            prim_type = PrimType::Quads;
            prim_count = num_verts / 4;
        }
        DrawModes::Lines => {
            prim_type = PrimType::Lines;
            prim_count = num_verts / 2;
        }
        DrawModes::Triangles => {
            prim_type = PrimType::Triangles;
            prim_count = num_verts / 3;
        }
    }

    cmd.set_state(*state);

    cmd.set_prim_type(prim_type);
    cmd.set_prim_count(prim_count);

    //TODO: m_pCommandBuffer->AddRenderCalls(1);
    return true;
}

impl<B: GraphicsBackendInterface> GraphicsStreamHandleInterface for GraphicsStreamHandle<B> {
    fn lines_begin<'a>(&'a mut self) -> DrawLines {
        let vertices_offset = self.stream_data.borrow().vertices_count();
        DrawLines::new(self, vertices_offset)
    }

    fn triangles_begin<'a>(&'a mut self) -> DrawTriangles {
        let vertices_offset = self.stream_data.borrow().vertices_count();
        DrawTriangles::new(self, vertices_offset)
    }

    fn quads_begin<'a>(&'a mut self) -> DrawQuads {
        let vertices_offset = self.stream_data.borrow().vertices_count();
        DrawQuads::new(self, vertices_offset)
    }

    fn quads_tex_3d_begin<'a>(&'a mut self) -> DrawQuads {
        let vertices_offset = self.stream_data.borrow().vertices_count();
        DrawQuads::new(self, vertices_offset)
    }

    fn flush_vertices(&mut self, state: &State, vertices_offset: usize, draw_mode: DrawModes) {
        let mut cmd = CommandRender::new(PrimType::Lines);
        if flush_vertices_impl(
            &mut *self.stream_data.borrow_mut(),
            state,
            draw_mode,
            vertices_offset,
            &mut cmd,
        ) {
            cmd.vertices_offset = vertices_offset;
            self.backend_handle
                .add_cmd(AllCommands::Render(CommandsRender::Render(cmd)));
        }
    }

    fn flush_commands_and_reset_vertices(&mut self, vertices_offset: &mut usize) {
        self.backend_handle.run_backend_buffer(&self.stream_data);
        *vertices_offset = 0;
    }

    fn stream_data(&self) -> &Rc<RefCell<dyn GraphicsStreamDataInterface>> {
        &self.stream_data
    }
}

#[derive(Debug)]
pub struct GraphicsQuadContainerHandle<B: GraphicsBackendInterface> {
    quad_containers: Rc<RefCell<HashMap<u128, SQuadContainer>>>,
    id_gen: Rc<RefCell<u128>>,

    backend_handle: GraphicsBackendHandle<B>,
    buffer_object_handle: GraphicsBufferObjectHandle<B>,
}

impl<B: GraphicsBackendInterface> Clone for GraphicsQuadContainerHandle<B> {
    fn clone(&self) -> Self {
        Self {
            quad_containers: self.quad_containers.clone(),
            id_gen: self.id_gen.clone(),
            backend_handle: self.backend_handle.clone(),
            buffer_object_handle: self.buffer_object_handle.clone(),
        }
    }
}

impl<B: GraphicsBackendInterface> SharedIndexCleanup for GraphicsQuadContainerHandle<B> {
    fn destroy_from_index(&self, index: u128) {
        // also clear the container index
        self.quad_containers.borrow_mut().remove(&index).unwrap();
    }
}

impl<B: GraphicsBackendInterface> GraphicsQuadContainerHandle<B> {
    pub fn new(
        backend_handle: GraphicsBackendHandle<B>,
        buffer_object_handle: GraphicsBufferObjectHandle<B>,
    ) -> Self {
        Self {
            quad_containers: Default::default(),
            id_gen: Default::default(),

            backend_handle,
            buffer_object_handle,
        }
    }

    pub fn quad_container_change_automatic_upload(
        &mut self,
        container_index: &QuadContainerIndex,
        automatic_upload: bool,
    ) {
        let mut containers = self.quad_containers.borrow_mut();
        let container = containers
            .get_mut(&container_index.get_index_unsafe())
            .unwrap();
        container.automatic_upload = automatic_upload;
    }

    pub fn render_quad_container(
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
        let mut containers = self.quad_containers.borrow_mut();
        let container = containers
            .get_mut(&container_index.get_index_unsafe())
            .unwrap();

        if container.quads.len() < quad_offset + 1 {
            return;
        }

        let quad_draw_num = match quad_draw_count {
            QuadContainerRenderCount::Auto => container.quads.len() - quad_offset,
            QuadContainerRenderCount::Count(count) => *count,
        };

        if container.quad_buffer_object_index.is_none() {
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
        cmd.buffer_object_index = container
            .quad_buffer_object_index
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

        self.backend_handle
            .add_cmd(AllCommands::Render(CommandsRender::QuadContainerEx(cmd)));

        // TODO! m_pCommandBuffer->AddRenderCalls(1);
    }

    pub fn render_quad_container_as_sprite_multiple(
        &mut self,
        container_index: &QuadContainerIndex,
        quad_offset: usize,
        quad_draw_count: &QuadContainerRenderCount,
        render_infos: PoolVec<SRenderSpriteInfo>,
        mut quad_scope: DrawScope<4>,
    ) {
        let mut containers = self.quad_containers.borrow_mut();
        let container = containers
            .get_mut(&container_index.get_index_unsafe())
            .unwrap();

        let draw_count;
        match quad_draw_count {
            QuadContainerRenderCount::Auto => draw_count = container.quads.len() - quad_offset,
            QuadContainerRenderCount::Count(count) => draw_count = *count,
        }
        if draw_count == 0 {
            return;
        }

        if container.quad_buffer_object_index.is_none() {
            return;
        }

        quad_scope.wrap_clamp();
        let quad = &container.quads[0];
        let cmd = CommandRenderQuadContainerAsSpriteMultiple {
            state: quad_scope.state,

            draw_num: 1 * 6,
            draw_count: draw_count,
            offset: quad_offset * 6 * std::mem::size_of::<u32>(),
            buffer_object_index: container
                .quad_buffer_object_index
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
                quad.vertices[0].pos.x + (quad.vertices[1].pos.x - quad.vertices[0].pos.x) / 2.0,
                quad.vertices[0].pos.y + (quad.vertices[2].pos.y - quad.vertices[0].pos.y) / 2.0,
            ),

            render_info: render_infos,
        };

        self.backend_handle.add_cmd(AllCommands::Render(
            CommandsRender::QuadContainerSpriteMultiple(cmd),
        ));

        // TODO! m_pCommandBuffer->AddRenderCalls(((DrawCount - 1) / gs_GraphicsMaxParticlesRenderCount) + 1);

        quad_scope.wrap_normal();
    }
}

impl<B: GraphicsBackendInterface + 'static> GraphicsQuadContainerHandleInterface
    for GraphicsQuadContainerHandle<B>
{
    fn create_quad_container(&mut self, automatic_upload: bool) -> QuadContainerIndex {
        *self.id_gen.borrow_mut() += 1;
        let index = *self.id_gen.borrow();

        self.quad_containers.borrow_mut().insert(
            index,
            SQuadContainer {
                quads: Default::default(),
                quad_buffer_object_index: None,
                automatic_upload,
            },
        );

        return QuadContainerIndex::new(index, Rc::new(self.clone()));
    }

    fn quad_container_add_quads(
        &mut self,
        container_index: &QuadContainerIndex,
        quad_array: &[SQuad],
    ) -> usize {
        let mut containers = self.quad_containers.borrow_mut();
        let container = containers
            .get_mut(&container_index.get_index_unsafe())
            .unwrap();

        // TODO!: *4 -> *4 or *6 (if triangles) and also check for other add quads calls
        if container.quads.len() > quad_array.len() * 4 + StreamDataMax::MaxVertices as usize {
            panic!("quad count exceeded the maximum allowed number")
        }

        let ret_off = container.quads.len();

        container.quads.append(&mut quad_array.to_vec());

        if container.automatic_upload {
            drop(containers);
            self.quad_container_upload(container_index);
        }

        return ret_off;
    }

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
        self.render_quad_container(
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

    fn quad_container_upload(&mut self, container_index: &QuadContainerIndex) {
        let mut containers = self.quad_containers.borrow_mut();
        let container = containers
            .get_mut(&container_index.get_index_unsafe())
            .unwrap();
        if !container.quads.is_empty() {
            match container.quad_buffer_object_index.as_ref() {
                None => {
                    container.quad_buffer_object_index = Some(
                        self.buffer_object_handle
                            .create_buffer_object_slow(container.quads_to_bytes()),
                    );
                }
                Some(quad_buffer_object_index) => {
                    self.buffer_object_handle.recreate_buffer_object_slow(
                        quad_buffer_object_index,
                        container.quads_to_bytes(),
                    );
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct GraphicsBufferObjectHandle<B: GraphicsBackendInterface> {
    buffer_objects: Rc<RefCell<HashMap<u128, GraphicsBufferObject>>>,
    id_gen: Rc<RefCell<u128>>,

    backend_handle: GraphicsBackendHandle<B>,
}

impl<B: GraphicsBackendInterface> Clone for GraphicsBufferObjectHandle<B> {
    fn clone(&self) -> Self {
        Self {
            buffer_objects: self.buffer_objects.clone(),
            id_gen: self.id_gen.clone(),
            backend_handle: self.backend_handle.clone(),
        }
    }
}

impl<B: GraphicsBackendInterface + 'static> GraphicsBufferObjectHandle<B> {
    pub fn new(backend_handle: GraphicsBackendHandle<B>) -> Self {
        Self {
            buffer_objects: Rc::new(RefCell::new(HashMap::new())),
            id_gen: Default::default(),

            backend_handle,
        }
    }

    pub fn create_buffer_object(
        &mut self,
        upload_data: GraphicsBackendMemory,
    ) -> BufferObjectIndex {
        *self.id_gen.borrow_mut() += 1;
        let index = *self.id_gen.borrow();

        let upload_len = upload_data.len();

        let mut cmd = CommandCreateBufferObject::default();
        cmd.buffer_index = index;
        cmd.upload_data = RefCell::new(Some(upload_data));

        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::CreateBufferObject(cmd)));

        self.buffer_objects.borrow_mut().insert(
            index,
            GraphicsBufferObject {
                alloc_size: upload_len,
            },
        );
        BufferObjectIndex::new(index, Rc::new(self.clone()))
    }

    pub fn recreate_buffer_object(
        &mut self,
        buffer_index: &BufferObjectIndex,
        upload_data: GraphicsBackendMemory,
    ) {
        let upload_len = upload_data.len();

        let mut cmd = CommandRecreateBufferObject::default();
        cmd.buffer_index = buffer_index.get_index_unsafe();
        cmd.upload_data = RefCell::new(Some(upload_data));

        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::RecreateBufferObject(cmd)));

        self.buffer_objects
            .borrow_mut()
            .get_mut(&buffer_index.get_index_unsafe())
            .unwrap()
            .alloc_size = upload_len;
    }
}

impl<B: GraphicsBackendInterface> SharedIndexCleanup for GraphicsBufferObjectHandle<B> {
    fn destroy_from_index(&self, index: u128) {
        self.buffer_objects.borrow_mut().remove(&index).unwrap();
        let cmd = CommandDeleteBufferObject {
            buffer_index: index,
        };

        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::DeleteBufferObject(cmd)));
    }
}

impl<B: GraphicsBackendInterface + 'static> GraphicsBufferObjectHandleInterface
    for GraphicsBufferObjectHandle<B>
{
    fn create_buffer_object_slow(&mut self, upload_data: Vec<u8>) -> BufferObjectIndex {
        let mut buffer_mem = self
            .backend_handle
            .mem_alloc(GraphicsMemoryAllocationType::Buffer, upload_data.len());
        buffer_mem.copy_from_slice(&upload_data);
        self.create_buffer_object(buffer_mem)
    }

    fn recreate_buffer_object_slow(
        &mut self,
        buffer_index: &BufferObjectIndex,
        upload_data: Vec<u8>,
    ) {
        let mut buffer_mem = self
            .backend_handle
            .mem_alloc(GraphicsMemoryAllocationType::Buffer, upload_data.len());
        buffer_mem.copy_from_slice(&upload_data);
        self.recreate_buffer_object(buffer_index, buffer_mem)
    }
}

#[derive(Debug)]
pub struct GraphicsTextureHandle<B: GraphicsBackendInterface> {
    texture_indices: Rc<RefCell<HashMap<u128, TextureContainer>>>,
    id_gen: Rc<RefCell<u128>>,

    backend_handle: GraphicsBackendHandle<B>,
}

impl<B: GraphicsBackendInterface> Clone for GraphicsTextureHandle<B> {
    fn clone(&self) -> Self {
        Self {
            texture_indices: self.texture_indices.clone(),
            id_gen: self.id_gen.clone(),
            backend_handle: self.backend_handle.clone(),
        }
    }
}

impl<B: GraphicsBackendInterface> SharedIndexCleanup for GraphicsTextureHandle<B> {
    fn destroy_from_index(&self, index: u128) {
        // unwrap is important to prevent corruptions
        let _ = self.texture_indices.borrow_mut().remove(&index).unwrap();

        let cmd = CommandTextureDestroy {
            texture_index: index,
        };
        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::TextureDestroy(cmd)));
    }
}

impl<B: GraphicsBackendInterface + 'static> GraphicsTextureHandle<B> {
    fn new(backend_handle: GraphicsBackendHandle<B>) -> Self {
        Self {
            texture_indices: Rc::new(RefCell::new(HashMap::with_capacity(
                StreamDataMax::MaxTextures as usize,
            ))),
            id_gen: Default::default(),

            backend_handle,
        }
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
            let mut data_slice = self.backend_handle.mem_alloc(
                GraphicsMemoryAllocationType::Texture,
                src_width * src_height * 4,
            );

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
                        data_slice.as_mut_slice()[img_offset_dest..img_offset_dest + copy_size]
                            .copy_from_slice(
                                &src_mem.as_mut_slice()[img_offset_src..img_offset_src + copy_size],
                            );
                        data_slice.as_mut_slice()[img_offset_dest + 3] = 255;
                    } else if src_channel_count == 1 {
                        data_slice.as_mut_slice()[img_offset_dest + 0] = 255;
                        data_slice.as_mut_slice()[img_offset_dest + 1] = 255;
                        data_slice.as_mut_slice()[img_offset_dest + 2] = 255;
                        data_slice.as_mut_slice()[img_offset_dest + 3] =
                            src_mem.as_mut_slice()[img_offset_src];
                    }
                }
            }

            return Ok(data_slice);
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
        *self.id_gen.borrow_mut() += 1;
        let tex_index = *self.id_gen.borrow();

        // flags
        let mut cmd_flags = TexFlags::empty();
        if !(flags & TexFlags::TEXFLAG_NOMIPMAPS).is_empty() {
            cmd_flags |= TexFlags::TEXFLAG_NOMIPMAPS;
        }

        let pixel_size = 4;

        // copy texture data
        let _mem_size = width * height * pixel_size;
        let tmp_buff_data = self.convert_to_rgba(data, width as usize, height as usize, format);

        if let Ok(data) = tmp_buff_data {
            let cmd = CommandTextureCreate {
                texture_index: tex_index,
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

            let index = TextureIndex::new(tex_index, Rc::new(self.clone()));
            self.texture_indices.borrow_mut().insert(
                tex_index,
                TextureContainer {
                    width: width,
                    height: height,
                    depth: depth,
                },
            );

            Ok(index)
        } else {
            // TODO: add logging dbg_msg("graphics", "converted image %s to RGBA, consider making its file format RGBA", pTexName ? pTexName : "(no name)");
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
}

impl<B: GraphicsBackendInterface + 'static> GraphicsTextureHandleInterface
    for GraphicsTextureHandle<B>
{
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
        let mut mem = self
            .backend_handle
            .mem_alloc(GraphicsMemoryAllocationType::Texture, data.len());
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
        let mut mem = self
            .backend_handle
            .mem_alloc(GraphicsMemoryAllocationType::Texture, data.len());
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

pub struct GraphicsBase<B: GraphicsBackendInterface> {
    pub backend_handle: GraphicsBackendHandle<B>,

    window_props: WindowProps,

    pub quad_container_handle: GraphicsQuadContainerHandle<B>,

    pub buffer_object_handle: GraphicsBufferObjectHandle<B>,

    pub stream_handle: GraphicsStreamHandle<B>,

    pub texture_handle: GraphicsTextureHandle<B>,

    pub quad_render_info_pool: Pool<Vec<SQuadRenderInfo>>,
    pub sprite_render_info_pool: Pool<Vec<SRenderSpriteInfo>>,
    pub index_offset_or_draw_count_pool: Pool<Vec<usize>>,
}

impl<B: GraphicsBackendInterface + 'static> GraphicsBase<B> {
    pub fn new(
        backend: Rc<RefCell<B>>,
        stream_data: Rc<RefCell<dyn GraphicsStreamDataInterface>>,
        window: WindowProps,
    ) -> GraphicsBase<B> {
        let backend_handle = GraphicsBackendHandle::new(backend);
        let buffer_object_handle = GraphicsBufferObjectHandle::new(backend_handle.clone());
        GraphicsBase {
            window_props: window,

            // handles
            quad_container_handle: GraphicsQuadContainerHandle::new(
                backend_handle.clone(),
                buffer_object_handle.clone(),
            ),
            buffer_object_handle,
            stream_handle: GraphicsStreamHandle::new(stream_data, backend_handle.clone()),
            texture_handle: GraphicsTextureHandle::new(backend_handle.clone()),
            backend_handle,

            // pools
            quad_render_info_pool: Pool::with_sized(8, || Vec::with_capacity(64)),
            sprite_render_info_pool: Pool::with_sized(8, || Vec::with_capacity(512)),
            index_offset_or_draw_count_pool: Pool::with_sized(64 * 2, || Vec::with_capacity(128)),
        }
    }

    pub fn get_graphics_mt(&self) -> GraphicsMultiThreaded {
        GraphicsMultiThreaded::new(self.backend_handle.backend.borrow_mut().get_backend_mt())
    }

    pub fn resized(&mut self, window_props: WindowProps) {
        self.window_props = window_props;
    }

    pub fn swap(&mut self) {
        let cmd = CommandSwap {};
        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::Swap(cmd)));
        self.backend_handle
            .run_backend_buffer(&mut self.stream_handle.stream_data);
    }

    pub fn switch_to_dual_pass(&mut self) {
        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::SwitchToDualPass));
        self.backend_handle
            .run_backend_buffer(&mut self.stream_handle.stream_data);
    }

    /**
     * Allocates memory to be used in the backend
     */
    pub fn mem_alloc(
        &mut self,
        alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> GraphicsRawMemoryBuffer<B> {
        GraphicsRawMemoryBuffer {
            memory: Some(self.backend_handle.mem_alloc(alloc_type, req_size)),
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
        buffer_object_index: &BufferObjectIndex,
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
            indices_draw_num: num_indices_offset,
            buffer_object_index: buffer_object_index.get_index_unsafe(),
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
        buffer_object_index: &BufferObjectIndex,
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
        cmd.buffer_object_index = buffer_object_index.get_index_unsafe();
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
        buffer_object_index: &BufferObjectIndex,
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
        cmd.buffer_object_index = buffer_object_index.get_index_unsafe();
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
        buffer_object_index: &BufferObjectIndex,
        quad_render_infos: PoolVec<SQuadRenderInfo>,
        quad_num: usize,
        quad_offset: usize,
    ) {
        if quad_num == 0 {
            return;
        }

        // add the VertexArrays and draw
        let cmd = CommandRenderQuadLayer {
            state: *state,
            quad_num: quad_num,
            quad_offset: quad_offset,
            buffer_object_index: buffer_object_index.get_index_unsafe(),

            quad_info: quad_render_infos,
        };

        self.backend_handle
            .add_cmd(AllCommands::Render(CommandsRender::QuadLayer(cmd)));

        // TODO m_pCommandBuffer->AddRenderCalls(((QuadNum - 1) / gs_GraphicsMaxQuadsRenderCount) + 1);
    }

    pub fn last_render_call_as_second_pass_transition(&mut self) {
        let cmd = self.backend_handle.backend_cmds.cmds.borrow_mut().pop();
        if let Some(AllCommands::Render(CommandsRender::Render(cmd))) = cmd {
            self.backend_handle
                .add_cmd(AllCommands::Misc(Commands::NextSubpass));
            self.backend_handle.add_cmd(AllCommands::Render(
                CommandsRender::RenderFirstPassBlurred(cmd),
            ));
        }
    }

    /*pub fn do_screenshot(&mut self, fs: &Arc<FileSystem>, io_batcher: &TokIOBatcher) {
        let mut width: u32 = 0;
        let mut height: u32 = 0;
        let mut dest_data_buffer: Vec<u8> = Default::default();
        let image_format = self.backend_handle.backend.borrow_mut().do_screenshot(
            &mut width,
            &mut height,
            &mut dest_data_buffer,
        );
        if let Ok(_) = image_format {
            let res = save_png_image(&dest_data_buffer, width, height).unwrap();
            let fs = fs.clone();
            io_batcher.spawn_without_queue(async move {
                fs.write_file("", res).await?;
                Ok(())
            });
        }
    }*/

    pub fn update_viewport(&mut self, x: i32, y: i32, width: u32, height: u32) {
        let cmd = CommandUpdateViewport {
            x,
            y,
            width,
            height,
            by_resize: false,
        };
        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::UpdateViewport(cmd)));
    }

    pub fn reset_viewport(&mut self) {
        self.update_viewport(0, 0, self.window_width(), self.window_height())
    }
}

impl<B: GraphicsBackendInterface> Drop for GraphicsBase<B> {
    fn drop(&mut self) {
        self.backend_handle
            .run_backend_buffer(&mut self.stream_handle.stream_data);
    }
}

impl<B: GraphicsBackendInterface> GraphicsSizeQuery for GraphicsBase<B> {
    fn canvas_aspect(&self) -> f32 {
        (self.window_props.canvas_width / self.window_props.canvas_height) as f32
    }

    fn canvas_width(&self) -> u32 {
        self.window_props.canvas_width as u32
    }

    fn canvas_height(&self) -> u32 {
        self.window_props.canvas_height as u32
    }

    fn window_width(&self) -> u32 {
        self.window_props.window_width
    }

    fn window_height(&self) -> u32 {
        self.window_props.window_height
    }

    fn window_props(&self) -> WindowProps {
        self.window_props
    }
}

impl<B: GraphicsBackendInterface> GraphicsStreamHandleInterface for GraphicsBase<B> {
    fn lines_begin(&mut self) -> DrawLines {
        self.stream_handle.lines_begin()
    }

    fn triangles_begin(&mut self) -> DrawTriangles {
        self.stream_handle.triangles_begin()
    }

    fn quads_begin(&mut self) -> DrawQuads {
        self.stream_handle.quads_begin()
    }

    fn quads_tex_3d_begin(&mut self) -> DrawQuads {
        self.stream_handle.quads_tex_3d_begin()
    }

    fn flush_vertices(&mut self, state: &State, vertices_offset: usize, draw_mode: DrawModes) {
        self.stream_handle
            .flush_vertices(state, vertices_offset, draw_mode)
    }

    fn flush_commands_and_reset_vertices(&mut self, vertices_offset: &mut usize) {
        self.stream_handle
            .flush_commands_and_reset_vertices(vertices_offset)
    }

    fn stream_data(&self) -> &Rc<RefCell<dyn GraphicsStreamDataInterface>> {
        self.stream_handle.stream_data()
    }
}

impl<B: GraphicsBackendInterface + 'static> GraphicsHandlesInterface for GraphicsBase<B> {
    fn get_handles(&mut self) -> GraphicsHandles {
        GraphicsHandles {
            backend_handle: &mut self.backend_handle,
            stream_handle: &mut self.stream_handle,
            quad_container_handle: &mut self.quad_container_handle,
            buffer_object_handle: &mut self.buffer_object_handle,
        }
    }
}

impl<B: GraphicsBackendInterface + 'static> GraphicsInterface for GraphicsBase<B> {}
