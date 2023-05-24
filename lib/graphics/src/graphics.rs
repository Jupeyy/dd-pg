use std::{alloc::System, cell::RefCell, str::FromStr, sync::Arc};

use arrayvec::ArrayString;

use graphics_base::streaming::{
    rotate, DrawLines, DrawQuads, DrawScope, DrawScopeImpl, DrawTriangles,
};
use graphics_traits::{GraphicsBachendBufferInterface, GraphicsStreamHandler};
use native::native::Native;
use num_traits::FromPrimitive;

use base::{config::Config, filesys::FileSystem, io_batcher::IOBatcher};

use math::math::vector::{ubvec4, vec2, vec4};

use crate::{
    backend::{BackendBuffer, GraphicsBackend},
    graphics_mt::GraphicsMultiThreaded,
    traits::{GraphicsLoadIOPipe, GraphicsLoadWhileIOPipe},
};

use graphics_types::{
    command_buffer::{
        AllCommands, BufferContainerIndex, BufferObjectIndex, Commands, CommandsRender,
        GraphicsType, PrimType, RenderCommand, SAttribute, SBufferContainerInfo, SColor,
        SCommand_CopyBufferObject, SCommand_CreateBufferContainer, SCommand_CreateBufferObject,
        SCommand_DeleteBufferContainer, SCommand_DeleteBufferObject,
        SCommand_IndicesRequiredNumNotify, SCommand_RecreateBufferObject, SCommand_Render,
        SCommand_RenderBorderTile, SCommand_RenderBorderTileLine, SCommand_RenderQuadContainer,
        SCommand_RenderQuadContainerAsSpriteMultiple, SCommand_RenderQuadContainerEx,
        SCommand_RenderQuadLayer, SCommand_RenderTileLayer, SCommand_Swap, SCommand_Texture_Create,
        SCommand_Texture_Destroy, SCommand_UpdateBufferContainer, SCommand_UpdateBufferObject,
        SQuadRenderInfo, SRenderSpriteInfo, STexCoord, StreamDataMax, TexFlags, TexFormat,
    },
    rendering::{
        ColorRGBA, ETextureIndex, GL_SPoint, GL_SVertex, SVertex, State, WriteVertexAttributes,
    },
    types::{
        CQuadItem, DrawModes, GraphicsMemoryAllocationType, ImageFormat, Line, Triangle, VideoMode,
        WindowProps,
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
    NUM_CMDBUFFERS = 2,
}

pub type QuadContainerIndex = Option<usize>;

/**
 * This buffer is useful if you want to
 * allocate memory that is going to be used by the
 * graphics engine. It might optimize away expensive
 * heap allocations and copying.
 */
pub struct GraphicsRawMemoryBuffer<'a> {
    memory: Option<&'static mut [u8]>,
    graphics: Option<&'a mut Graphics>,
}

impl<'a> Drop for GraphicsRawMemoryBuffer<'a> {
    fn drop(&mut self) {
        let mut memory: Option<&'static mut [u8]> = None;
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
    memory: Option<&'static mut [u8]>,
    graphics: &'a mut Graphics,
}

impl<'a> StagingGraphics<'a> {
    pub fn load_texture(
        &mut self,
        texture_id: &mut ETextureIndex,
        width: usize,
        height: usize,
        format: i32,
        store_format: i32,
        Flags: TexFlags,
        pTexName: &str,
    ) {
        let mut memory: Option<&'static mut [u8]> = None;
        std::mem::swap(&mut memory, &mut self.memory);
        self.graphics.load_texture_impl(
            texture_id,
            width,
            height,
            1,
            false,
            format,
            memory.unwrap(),
            store_format,
            Flags,
            pTexName,
        )
    }
}

impl<'a> GraphicsRawMemoryBuffer<'a> {
    pub fn exec(mut self) -> StagingGraphics<'a> {
        let mut memory: Option<&'static mut [u8]> = None;
        let mut graphics: Option<&'a mut Graphics> = None;
        std::mem::swap(&mut memory, &mut self.memory);
        std::mem::swap(&mut graphics, &mut self.graphics);
        StagingGraphics {
            graphics: graphics.unwrap(),
            memory: memory,
        }
    }

    pub fn mem<'b>(&'b mut self) -> &'b mut [u8] {
        self.memory.as_mut().unwrap()
    }
}

pub trait GraphicsTextureAllocations {
    fn load_texture_slow(
        &mut self,
        texture_id: &mut ETextureIndex,
        width: usize,
        height: usize,
        format: i32,
        data: Vec<u8>,
        store_format: i32,
        Flags: TexFlags,
        pTexName: &str,
    );
    fn load_texture_3d_slow(
        &mut self,
        texture_id: &mut ETextureIndex,
        width: usize,
        height: usize,
        depth: usize,
        format: i32,
        data: Vec<u8>,
        store_format: i32,
        Flags: TexFlags,
        pTexName: &str,
    );
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

    quad_buffer_object_index: BufferObjectIndex,
    quad_buffer_container_index: BufferContainerIndex,

    free_index: QuadContainerIndex,

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

#[derive(Default)]
struct SVertexArrayInfo {
    // keep a reference to it, so we can free the ID
    associated_buffer_object_index: BufferObjectIndex,

    free_index: BufferContainerIndex,
}

pub struct QuadContainerBuilder {
    AutomaticUpload: bool,
}

impl QuadContainerBuilder {
    pub fn new(atomatic_upload: bool) -> Self {
        Self {
            AutomaticUpload: atomatic_upload,
        }
    }

    pub fn build(&self) -> SQuadContainer {
        SQuadContainer {
            quads: Vec::new(),
            quad_buffer_object_index: None,
            quad_buffer_container_index: None,
            free_index: None,
            automatic_upload: self.AutomaticUpload,
        }
    }
}

pub struct GraphicsBackendHandle {
    backend_buffer: BackendBuffer,
    backend: GraphicsBackend,
}

impl GraphicsStreamHandler for GraphicsBackendHandle {
    fn backend_buffer_mut(&mut self) -> &mut dyn GraphicsBachendBufferInterface {
        &mut self.backend_buffer
    }

    fn flush_vertices(&mut self, state: &State, vertices_offset: usize, draw_mode: DrawModes) {
        let mut cmd = SCommand_Render::new();
        if self.flush_vertices_impl(state, draw_mode, vertices_offset, &mut cmd) {
            cmd.vertices_offset = vertices_offset;
            self.add_cmd(AllCommands::Render(CommandsRender::CMD_RENDER(cmd)));
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
    fn new(native: Native) -> Self {
        Self {
            backend_buffer: BackendBuffer::default(),
            backend: GraphicsBackend::new(native),
        }
    }

    fn load_io(&mut self, io_pipe: &mut GraphicsLoadIOPipe) {
        self.backend.load_io(io_pipe);
    }

    fn init_while_io(&mut self, pipe: &mut GraphicsLoadWhileIOPipe) {
        self.backend.init_while_io(pipe);
    }

    pub fn init_graphics(&mut self) -> Result<(), ArrayString<4096>> {
        let res = self.backend.init();
        match res {
            Ok(backend_buffer) => {
                self.backend_buffer = backend_buffer;
                Ok(())
            }
            Err(_) => Err(ArrayString::from_str(
                "Backend initialization failed for an unknown error",
            )
            .unwrap()),
        }
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

    pub fn lines_begin(&mut self) -> DrawLines {
        let vertices_offset = self.backend_buffer.num_vertices;
        DrawLines::new(self, vertices_offset)
    }

    pub fn triangles_begin(&mut self) -> DrawTriangles {
        let vertices_offset = self.backend_buffer.num_vertices;
        DrawTriangles::new(self, vertices_offset)
    }

    pub fn quads_begin(&mut self) -> DrawQuads {
        let vertices_offset = self.backend_buffer.num_vertices;
        DrawQuads::new(self, vertices_offset)
    }

    pub fn quads_begin_from_draw_scope(draw_scope: DrawScope<4>) -> DrawQuads {
        DrawQuads::from_draw_scope(draw_scope)
    }

    pub fn quads_tex_3d_begin(&mut self) -> DrawQuads {
        let vertices_offset = self.backend_buffer.num_vertices;
        DrawQuads::new(self, vertices_offset)
    }

    pub fn quad_scope_begin(&mut self) -> DrawScope<4> {
        let vertices_offset = self.backend_buffer.num_vertices;
        DrawScope::<4>::new(self, vertices_offset)
    }
}

pub struct GraphicsQuadContainerHandle {
    quad_containers: Vec<SQuadContainer>,
    first_free_quad_container: QuadContainerIndex,
}

impl GraphicsQuadContainerHandle {
    pub fn new() -> Self {
        Self {
            quad_containers: Vec::new(),
            first_free_quad_container: None,
        }
    }

    fn IsQuadContainerBufferingEnabled() -> bool {
        false
    }

    pub fn create_quad_container(&mut self, builder: &QuadContainerBuilder) -> QuadContainerIndex {
        let mut Index: QuadContainerIndex = None;
        if self.first_free_quad_container.is_none() {
            Index = Some(self.quad_containers.len());
            self.quad_containers.push(builder.build());
        } else {
            Index = self.first_free_quad_container;
            self.first_free_quad_container = self.quad_containers[Index.unwrap()].free_index;
            self.quad_containers[Index.unwrap()].free_index = Index;
        }

        return Index;
    }

    pub fn QuadContainerChangeAutomaticUpload(
        &mut self,
        ContainerIndex: &QuadContainerIndex,
        AutomaticUpload: bool,
    ) {
        let mut Container = &mut self.quad_containers[ContainerIndex.unwrap()];
        Container.automatic_upload = AutomaticUpload;
    }

    pub fn QuadContainerUpload(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        buffer_container_handle: &mut GraphicsBufferContainerHandle,
        buffer_object_handle: &mut GraphicsBufferObjectHandle,
        ContainerIndex: &QuadContainerIndex,
    ) {
        if Self::IsQuadContainerBufferingEnabled() {
            let Container = &mut self.quad_containers[ContainerIndex.unwrap()];
            if !Container.quads.is_empty() {
                if Container.quad_buffer_object_index.is_none() {
                    let UploadDataSize = Container.quads.len() * std::mem::size_of::<SQuad>();
                    Container.quad_buffer_object_index = buffer_object_handle
                        .create_buffer_object_slow(
                            backend_handle,
                            UploadDataSize,
                            Container.quads_to_bytes(),
                            0,
                        );
                } else {
                    let UploadDataSize = Container.quads.len() * std::mem::size_of::<SQuad>();
                    buffer_object_handle.recreate_buffer_object_slow(
                        backend_handle,
                        &Container.quad_buffer_object_index,
                        UploadDataSize,
                        Container.quads_to_bytes(),
                        0,
                    );
                }

                if Container.quad_buffer_container_index.is_none() {
                    let mut Info = SBufferContainerInfo::default();
                    Info.stride = std::mem::size_of::<SVertex>();
                    Info.vert_buffer_binding_index = Container.quad_buffer_object_index;

                    Info.attributes.push(SAttribute {
                        data_type_count: 2,
                        graphics_type: GraphicsType::Float,
                        normalized: false,
                        offset: 0,
                        func_type: 0,
                    });
                    Info.attributes.push(SAttribute {
                        data_type_count: 2,
                        graphics_type: GraphicsType::Float,
                        normalized: false,
                        offset: (std::mem::size_of::<f32>() * 2),
                        func_type: 0,
                    });
                    Info.attributes.push(SAttribute {
                        data_type_count: 4,
                        graphics_type: GraphicsType::UnsignedByte,
                        normalized: true,
                        offset: (std::mem::size_of::<f32>() * 2 + std::mem::size_of::<f32>() * 2),
                        func_type: 0,
                    });

                    Container.quad_buffer_container_index =
                        buffer_container_handle.CreateBufferContainer(backend_handle, &Info);
                }
            }
        }
    }

    /**
     * Returns the index of the first added quad
     */
    pub fn QuadContainerAddQuads(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        buffer_container_handle: &mut GraphicsBufferContainerHandle,
        buffer_object_handle: &mut GraphicsBufferObjectHandle,
        ContainerIndex: &QuadContainerIndex,
        pArray: &[SQuad],
    ) -> usize {
        let Container = &mut self.quad_containers[ContainerIndex.unwrap()];

        // TODO!: *4 -> *4 or *6 (if triangles) and also check for other add quads calls
        if Container.quads.len() > pArray.len() * 4 + StreamDataMax::MaxVertices as usize {
            panic!("quad count exceeded the maximum allowed number")
        }

        let RetOff = Container.quads.len();

        Container.quads.append(&mut pArray.to_vec());

        if Container.automatic_upload {
            self.QuadContainerUpload(
                backend_handle,
                buffer_container_handle,
                buffer_object_handle,
                ContainerIndex,
            );
        }

        return RetOff;
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

    pub fn QuadContainerReset(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        buffer_container_handle: &mut GraphicsBufferContainerHandle,
        buffer_object_handle: &mut GraphicsBufferObjectHandle,
        ContainerIndex: &QuadContainerIndex,
    ) {
        let mut Container = &mut self.quad_containers[ContainerIndex.unwrap()];
        if Self::IsQuadContainerBufferingEnabled() {
            if Container.quad_buffer_container_index.is_some() {
                buffer_container_handle.DeleteBufferContainer(
                    backend_handle,
                    buffer_object_handle,
                    &mut Container.quad_buffer_container_index,
                    true,
                );
            }
        }
        Container.quads.clear();
        Container.quad_buffer_container_index = None;
        Container.quad_buffer_object_index = None;
    }

    pub fn DeleteQuadContainer(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        buffer_container_handle: &mut GraphicsBufferContainerHandle,
        buffer_object_handle: &mut GraphicsBufferObjectHandle,
        ContainerIndex: &QuadContainerIndex,
    ) {
        self.QuadContainerReset(
            backend_handle,
            buffer_container_handle,
            buffer_object_handle,
            ContainerIndex,
        );

        // also clear the container index
        self.quad_containers[ContainerIndex.unwrap()].free_index = self.first_free_quad_container;
        self.first_free_quad_container = *ContainerIndex;
    }

    pub fn RenderQuadContainer(
        &mut self,
        ContainerIndex: &QuadContainerIndex,
        QuadDrawNum: &QuadContainerRenderCount,
        quad_scope: DrawScope<4>,
    ) {
        self.RenderQuadContainerEx(ContainerIndex, 0, QuadDrawNum, quad_scope);
    }

    pub fn RenderQuadContainerEx(
        &mut self,
        ContainerIndex: &QuadContainerIndex,
        QuadOffset: usize,
        QuadDrawCount: &QuadContainerRenderCount,
        quad_scope: DrawScope<4>,
    ) {
        let Container = &mut self.quad_containers[ContainerIndex.unwrap()];

        let mut QuadDrawNum = 0;
        match QuadDrawCount {
            QuadContainerRenderCount::Auto => QuadDrawNum = Container.quads.len() - QuadOffset,
            QuadContainerRenderCount::Count(count) => QuadDrawNum = *count,
        }

        if Container.quads.len() < QuadOffset + QuadDrawNum || QuadDrawNum == 0 {
            return;
        }

        if Self::IsQuadContainerBufferingEnabled() {
            if Container.quad_buffer_container_index.is_none() {
                return;
            }

            let mut Cmd = SCommand_RenderQuadContainer::default();
            Cmd.state = quad_scope.state;
            Cmd.draw_num = QuadDrawNum * 6;
            Cmd.offset = QuadOffset * 6 * std::mem::size_of::<u32>();
            Cmd.buffer_container_index = Container.quad_buffer_container_index.unwrap();

            quad_scope.backend_handle.add_cmd(AllCommands::Render(
                CommandsRender::CMD_RENDER_QUAD_CONTAINER(Cmd),
            ));

            // TODO: m_pCommandBuffer->AddRenderCalls(1);
        } else {
            let mut draw_quads = GraphicsBackendHandle::quads_begin_from_draw_scope(quad_scope);
            let prims = draw_quads.get_raw_handle(QuadDrawNum);
            prims.iter_mut().enumerate().for_each(|(index, prim)| {
                *prim = Container.quads[QuadOffset + (index / 4)].vertices[index % 4]
            });
            drop(prims);
            drop(draw_quads);
        }
    }

    pub fn RenderQuadContainerEx2(
        &mut self,
        ContainerIndex: &QuadContainerIndex,
        QuadOffset: usize,
        QuadDrawCount: &QuadContainerRenderCount,
        X: f32,
        Y: f32,
        ScaleX: f32,
        ScaleY: f32,
        mut quad_scope: DrawScope<4>,
    ) {
        let Container = &mut self.quad_containers[ContainerIndex.unwrap()];

        if Container.quads.len() < QuadOffset + 1 {
            return;
        }

        let mut QuadDrawNum = 0;
        match QuadDrawCount {
            QuadContainerRenderCount::Auto => QuadDrawNum = Container.quads.len() - QuadOffset,
            QuadContainerRenderCount::Count(count) => QuadDrawNum = *count,
        }

        if Self::IsQuadContainerBufferingEnabled() {
            if Container.quad_buffer_container_index.is_none() {
                return;
            }

            let Quad = &Container.quads[QuadOffset];
            let mut Cmd = SCommand_RenderQuadContainerEx::default();

            quad_scope.wrap_clamp();

            let (mut CanvasX0, mut CanvasY0, mut CanvasX1, mut CanvasY1) = (0.0, 0.0, 0.0, 0.0);
            quad_scope.get_canvas_mapping(
                &mut CanvasX0,
                &mut CanvasY0,
                &mut CanvasX1,
                &mut CanvasY1,
            );
            quad_scope.map_canvas(
                (CanvasX0 - X) / ScaleX,
                (CanvasY0 - Y) / ScaleY,
                (CanvasX1 - X) / ScaleX,
                (CanvasY1 - Y) / ScaleY,
            );
            Cmd.state = quad_scope.state;
            quad_scope.map_canvas(CanvasX0, CanvasY0, CanvasX1, CanvasY1);

            Cmd.draw_num = QuadDrawNum * 6;
            Cmd.offset = QuadOffset * 6 * std::mem::size_of::<u32>();
            Cmd.buffer_container_index = Container.quad_buffer_container_index.unwrap();

            Cmd.vertex_color.r = quad_scope.colors[0].r() as f32 / 255.0;
            Cmd.vertex_color.g = quad_scope.colors[0].g() as f32 / 255.0;
            Cmd.vertex_color.b = quad_scope.colors[0].b() as f32 / 255.0;
            Cmd.vertex_color.a = quad_scope.colors[0].a() as f32 / 255.0;

            Cmd.rotation = quad_scope.rotation;

            // rotate before positioning
            Cmd.center.x = Quad.vertices[0].get_pos().x
                + (Quad.vertices[1].get_pos().x - Quad.vertices[0].get_pos().x) / 2.0;
            Cmd.center.y = Quad.vertices[0].get_pos().y
                + (Quad.vertices[2].get_pos().y - Quad.vertices[0].get_pos().y) / 2.0;

            quad_scope.backend_handle.add_cmd(AllCommands::Render(
                CommandsRender::CMD_RENDER_QUAD_CONTAINER_EX(Cmd),
            ));

            // TODO! m_pCommandBuffer->AddRenderCalls(1);
        } else {
            let rotation = quad_scope.rotation;
            let color = quad_scope.colors[0];
            let mut draw_quads = GraphicsBackendHandle::quads_begin_from_draw_scope(quad_scope);
            let mut verts = draw_quads.get_raw_handle(QuadDrawNum);
            verts.iter_mut().enumerate().for_each(|(index, v)| {
                *v = Container.quads[QuadOffset + (index / 4)].vertices[index % 4];
            });
            for i in 0..QuadDrawNum {
                for n in 0..4 {
                    verts[i * 4 + n].pos.x *= ScaleX;
                    verts[i * 4 + n].pos.y *= ScaleY;
                    verts[i * 4 + n].set_color(&color);
                }

                if rotation != 0.0 {
                    let Center = vec2 {
                        x: verts[i * 4 + 0].pos.x
                            + (verts[i * 4 + 1].pos.x - verts[i * 4 + 0].pos.x) / 2.0,
                        y: verts[i * 4 + 0].pos.y
                            + (verts[i * 4 + 2].pos.y - verts[i * 4 + 0].pos.y) / 2.0,
                    };
                    rotate(&Center, rotation, &mut verts[i * 4..i * 4 + 4]);
                }

                for n in 0..4 {
                    verts[i * 4 + n].pos.x += X;
                    verts[i * 4 + n].pos.y += Y;
                }
            }
            draw_quads.wrap_clamp();
        }
    }

    pub fn RenderQuadContainerAsSprite(
        &mut self,
        ContainerIndex: &QuadContainerIndex,
        QuadOffset: usize,
        X: f32,
        Y: f32,
        ScaleX: f32,
        ScaleY: f32,
        quad_scope: DrawScope<4>,
    ) {
        self.RenderQuadContainerEx2(
            ContainerIndex,
            QuadOffset,
            &QuadContainerRenderCount::Count(1),
            X,
            Y,
            ScaleX,
            ScaleY,
            quad_scope,
        );
    }

    pub fn RenderQuadContainerAsSpriteMultiple(
        &mut self,
        ContainerIndex: &QuadContainerIndex,
        QuadOffset: usize,
        QuadDrawCount: &QuadContainerRenderCount,
        pRenderInfo: Vec<SRenderSpriteInfo>,
        mut quad_scope: DrawScope<4>,
    ) {
        let Container = &mut self.quad_containers[ContainerIndex.unwrap()];

        let mut DrawCount = 0;
        match QuadDrawCount {
            QuadContainerRenderCount::Auto => DrawCount = Container.quads.len() - QuadOffset,
            QuadContainerRenderCount::Count(count) => DrawCount = *count,
        }
        if DrawCount == 0 {
            return;
        }

        if Self::IsQuadContainerBufferingEnabled() {
            if Container.quad_buffer_container_index.is_none() {
                return;
            }

            quad_scope.wrap_clamp();
            let Quad = &Container.quads[0];
            let mut Cmd = SCommand_RenderQuadContainerAsSpriteMultiple::default();

            Cmd.state = quad_scope.state;

            Cmd.draw_num = 1 * 6;
            Cmd.draw_count = DrawCount;
            Cmd.offset = QuadOffset * 6 * std::mem::size_of::<u32>();
            Cmd.buffer_container_index = Container.quad_buffer_container_index.unwrap();

            Cmd.vertex_color.r = quad_scope.colors[0].r() as f32 / 255.0;
            Cmd.vertex_color.g = quad_scope.colors[0].g() as f32 / 255.0;
            Cmd.vertex_color.b = quad_scope.colors[0].b() as f32 / 255.0;
            Cmd.vertex_color.a = quad_scope.colors[0].a() as f32 / 255.0;

            // rotate before positioning
            Cmd.center.x =
                Quad.vertices[0].pos.x + (Quad.vertices[1].pos.x - Quad.vertices[0].pos.x) / 2.0;
            Cmd.center.y =
                Quad.vertices[0].pos.y + (Quad.vertices[2].pos.y - Quad.vertices[0].pos.y) / 2.0;

            Cmd.render_info = pRenderInfo;

            quad_scope.backend_handle.add_cmd(AllCommands::Render(
                CommandsRender::CMD_RENDER_QUAD_CONTAINER_SPRITE_MULTIPLE(Cmd),
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
        ContainerIndex: &QuadContainerIndex,
        pArray: &[SQuad],
    ) -> usize {
        let (quad_cont_handle, backend_handle, buffer_cont_handle, buffer_obj_handle) =
            self.get_handles();
        quad_cont_handle.QuadContainerAddQuads(
            backend_handle,
            buffer_cont_handle,
            buffer_obj_handle,
            ContainerIndex,
            pArray,
        )
    }

    fn quad_container_upload(&mut self, ContainerIndex: &QuadContainerIndex) {
        let (quad_cont_handle, backend_handle, buffer_container_handle, buffer_object_handle) =
            self.get_handles();
        quad_cont_handle.QuadContainerUpload(
            backend_handle,
            buffer_container_handle,
            buffer_object_handle,
            ContainerIndex,
        )
    }
}
pub struct GraphicsBufferObjectHandle {
    buffer_object_indices: Vec<BufferObjectIndex>,
    first_free_buffer_object_index: BufferObjectIndex,
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
        upload_data: &'static mut [u8],
        create_flags: i32,
    ) -> BufferObjectIndex {
        let mut Index = None;
        if self.first_free_buffer_object_index.is_none() {
            Index = Some(self.buffer_object_indices.len());
            self.buffer_object_indices.push(Index);
        } else {
            Index = self.first_free_buffer_object_index;
            self.first_free_buffer_object_index = self.buffer_object_indices[Index.unwrap()];
            self.buffer_object_indices[Index.unwrap()] = Index;
        }

        let mut Cmd = SCommand_CreateBufferObject::default();
        Cmd.buffer_index = Index.unwrap();
        Cmd.flags = create_flags;
        Cmd.upload_data = RefCell::new(Some(upload_data));

        backend_handle.add_cmd(AllCommands::Misc(Commands::CMD_CREATE_BUFFER_OBJECT(Cmd)));

        return Index;
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
        BufferIndex: &BufferObjectIndex,
        _UploadDataSize: usize,
        pUploadData: &'static mut [u8],
        CreateFlags: i32,
    ) {
        let mut Cmd = SCommand_RecreateBufferObject::default();
        Cmd.buffer_index = BufferIndex.unwrap();
        Cmd.flags = CreateFlags;
        Cmd.upload_data = RefCell::new(Some(pUploadData));

        backend_handle.add_cmd(AllCommands::Misc(Commands::CMD_RECREATE_BUFFER_OBJECT(Cmd)));
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

    pub fn UpdateBufferObjectInternal(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        BufferIndex: &BufferObjectIndex,
        _UploadDataSize: usize,
        pUploadData: Vec<u8>,
        pOffset: usize,
    ) {
        let mut Cmd = SCommand_UpdateBufferObject::default();
        Cmd.buffer_index = BufferIndex.unwrap();
        Cmd.offset = pOffset;
        Cmd.upload_data = pUploadData;

        backend_handle.add_cmd(AllCommands::Misc(Commands::CMD_UPDATE_BUFFER_OBJECT(Cmd)));
    }

    pub fn CopyBufferObjectInternal(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        WriteBufferIndex: &BufferObjectIndex,
        ReadBufferIndex: &BufferObjectIndex,
        WriteOffset: usize,
        ReadOffset: usize,
        CopyDataSize: usize,
    ) {
        let mut Cmd = SCommand_CopyBufferObject::default();
        Cmd.write_buffer_index = WriteBufferIndex.unwrap();
        Cmd.read_buffer_index = ReadBufferIndex.unwrap();
        Cmd.write_offset = WriteOffset;
        Cmd.read_offset = ReadOffset;
        Cmd.copy_size = CopyDataSize;

        backend_handle.add_cmd(AllCommands::Misc(Commands::CMD_COPY_BUFFER_OBJECT(Cmd)));
    }

    pub fn DeleteBufferObject(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        BufferIndex: &mut BufferObjectIndex,
    ) {
        if BufferIndex.is_none() {
            return;
        }

        let Cmd = SCommand_DeleteBufferObject {
            buffer_index: BufferIndex.unwrap(),
        };

        backend_handle.add_cmd(AllCommands::Misc(Commands::CMD_DELETE_BUFFER_OBJECT(Cmd)));

        // also clear the buffer object index
        self.buffer_object_indices[BufferIndex.unwrap()] = self.first_free_buffer_object_index;
        self.first_free_buffer_object_index = *BufferIndex;

        *BufferIndex = None;
    }
}

pub trait GraphicsBufferObjectHandleInterface
where
    Self: GraphicsHandlesTrait,
{
    fn create_buffer_object(
        &mut self,
        upload_data_size: usize,
        upload_data: &'static mut [u8],
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
    first_free_vertex_array_info: BufferContainerIndex,
}

impl GraphicsBufferContainerHandle {
    pub fn new() -> Self {
        Self {
            vertex_array_info: Vec::new(),
            first_free_vertex_array_info: None,
        }
    }

    pub fn CreateBufferContainer(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        pContainerInfo: &SBufferContainerInfo,
    ) -> BufferContainerIndex {
        let mut Index: BufferContainerIndex = None;
        if self.first_free_vertex_array_info.is_none() {
            Index = Some(self.vertex_array_info.len());
            self.vertex_array_info.push(SVertexArrayInfo::default());
        } else {
            Index = self.first_free_vertex_array_info;
            self.first_free_vertex_array_info = self.vertex_array_info[Index.unwrap()].free_index;
            self.vertex_array_info[Index.unwrap()].free_index = Index;
        }

        let mut Cmd = SCommand_CreateBufferContainer::default();
        Cmd.buffer_container_index = Index.unwrap();
        Cmd.attributes = pContainerInfo.attributes.clone();
        Cmd.stride = pContainerInfo.stride;
        Cmd.vert_buffer_binding_index = pContainerInfo.vert_buffer_binding_index.unwrap();

        backend_handle.add_cmd(AllCommands::Misc(Commands::CMD_CREATE_BUFFER_CONTAINER(
            Cmd,
        )));

        self.vertex_array_info[Index.unwrap()].associated_buffer_object_index =
            pContainerInfo.vert_buffer_binding_index;

        return Index;
    }

    pub fn DeleteBufferContainer(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        buffer_object_handle: &mut GraphicsBufferObjectHandle,
        ContainerIndex: &mut BufferContainerIndex,
        DestroyAllBO: bool,
    ) {
        if ContainerIndex.is_none() {
            return;
        }
        let mut Cmd = SCommand_DeleteBufferContainer::default();
        Cmd.buffer_container_index = ContainerIndex.unwrap();
        Cmd.destroy_all_buffer_objects = DestroyAllBO;

        backend_handle.add_cmd(AllCommands::Misc(Commands::CMD_DELETE_BUFFER_CONTAINER(
            Cmd,
        )));

        if DestroyAllBO {
            // delete all associated references
            let buffer_object_index =
                self.vertex_array_info[ContainerIndex.unwrap()].associated_buffer_object_index;
            if buffer_object_index.is_some() {
                // clear the buffer object index
                buffer_object_handle.buffer_object_indices[buffer_object_index.unwrap()] =
                    buffer_object_handle.first_free_buffer_object_index;
                buffer_object_handle.first_free_buffer_object_index = buffer_object_index;
            }
        }
        self.vertex_array_info[ContainerIndex.unwrap()].associated_buffer_object_index = None;

        // also clear the buffer object index
        self.vertex_array_info[ContainerIndex.unwrap()].free_index =
            self.first_free_vertex_array_info;
        self.first_free_vertex_array_info = *ContainerIndex;

        *ContainerIndex = None;
    }

    pub fn UpdateBufferContainerInternal(
        &mut self,
        backend_handle: &mut GraphicsBackendHandle,
        ContainerIndex: &BufferContainerIndex,
        pContainerInfo: &SBufferContainerInfo,
    ) {
        let mut Cmd = SCommand_UpdateBufferContainer::default();
        Cmd.buffer_container_index = ContainerIndex.unwrap();
        Cmd.attributes = pContainerInfo.attributes.clone();
        Cmd.stride = pContainerInfo.stride;
        Cmd.vert_buffer_binding_index = pContainerInfo.vert_buffer_binding_index.unwrap();

        backend_handle.add_cmd(AllCommands::Misc(Commands::CMD_UPDATE_BUFFER_CONTAINER(
            Cmd,
        )));

        self.vertex_array_info[ContainerIndex.unwrap()].associated_buffer_object_index =
            pContainerInfo.vert_buffer_binding_index;
    }
}

pub trait GraphicsBufferContainerHandleInterface
where
    Self: GraphicsHandlesTrait,
{
    fn CreateBufferContainer(
        &mut self,
        pContainerInfo: &SBufferContainerInfo,
    ) -> BufferContainerIndex {
        let (_, backend_handle, buffer_container_handle, _buffer_object_handle) =
            self.get_handles();
        buffer_container_handle.CreateBufferContainer(backend_handle, pContainerInfo)
    }

    fn DeleteBufferContainer(
        &mut self,
        ContainerIndex: &mut BufferContainerIndex,
        DestroyAllBO: bool,
    ) {
        let (_, backend_handle, buffer_container_handle, buffer_object_handle) = self.get_handles();
        buffer_container_handle.DeleteBufferContainer(
            backend_handle,
            buffer_object_handle,
            ContainerIndex,
            DestroyAllBO,
        )
    }
}

#[derive(Copy, Clone, Default)]
struct TextureContainerIndex {
    index: ETextureIndex,
}

pub struct Graphics {
    pub backend_handle: GraphicsBackendHandle,

    window: WindowProps,

    texture_indices: Vec<TextureContainerIndex>,
    first_free_texture: ETextureIndex,

    pub quad_container_handle: GraphicsQuadContainerHandle,

    pub buffer_object_handle: GraphicsBufferObjectHandle,

    pub buffer_container_handle: GraphicsBufferContainerHandle,
}

impl Graphics {
    pub fn new(native: Native) -> Graphics {
        Graphics {
            window: Default::default(),

            backend_handle: GraphicsBackendHandle::new(native),

            texture_indices: Vec::new(),
            first_free_texture: ETextureIndex::Invalid,

            quad_container_handle: GraphicsQuadContainerHandle::new(),

            buffer_object_handle: GraphicsBufferObjectHandle::new(),

            buffer_container_handle: GraphicsBufferContainerHandle::new(),
        }
    }

    pub fn load_io(&mut self, io_pipe: &mut GraphicsLoadIOPipe) {
        self.backend_handle.load_io(io_pipe);
    }

    pub fn init_while_io(&mut self, pipe: &mut GraphicsLoadWhileIOPipe) {
        self.texture_indices
            .resize(StreamDataMax::MaxTextures as usize, Default::default());
        for i in 0..self.texture_indices.len() - 1 {
            self.texture_indices[i] = TextureContainerIndex {
                index: ETextureIndex::Index(i + 1),
            };
        }
        *self.texture_indices.last_mut().unwrap() = TextureContainerIndex {
            index: ETextureIndex::Invalid,
        };
        self.first_free_texture = ETextureIndex::Index(0);

        self.backend_handle.init_while_io(pipe);

        self.window = *self.backend_handle.backend.get_window_props();
    }

    pub fn init_graphics(&mut self) -> Result<(), ArrayString<4096>> {
        self.backend_handle.init_graphics()
    }

    pub fn get_graphics_mt(&self) -> GraphicsMultiThreaded {
        GraphicsMultiThreaded::new(self.backend_handle.backend.get_backend_mt())
    }

    fn ImageFormatToPixelSize(Format: i32) -> usize {
        let f = ImageFormat::from_i32(Format);
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

    fn ConvertToRGBA(
        &mut self,
        pSrc: &'static mut [u8],
        SrcWidth: usize,
        SrcHeight: usize,
        SrcFormat: i32,
    ) -> Result<&'static mut [u8], ArrayString<4096>> {
        if SrcFormat == TexFormat::RGBA as i32 {
            return Ok(pSrc);
        } else {
            let mut data_slice = self.mem_alloc(
                GraphicsMemoryAllocationType::Texture,
                SrcWidth * SrcHeight * 4,
            );
            let mut data_opt: Option<&'static mut [u8]> = None;
            std::mem::swap(&mut data_slice.memory, &mut data_opt);
            drop(data_slice);
            let res = data_opt.unwrap();

            let SrcChannelCount = Self::ImageFormatToPixelSize(SrcFormat);
            let DstChannelCount = 4;
            for Y in 0..SrcHeight {
                for X in 0..SrcWidth {
                    let ImgOffsetSrc = (Y * SrcWidth * SrcChannelCount) + (X * SrcChannelCount);
                    let ImgOffsetDest = (Y * SrcWidth * DstChannelCount) + (X * DstChannelCount);
                    let CopySize = SrcChannelCount;
                    if SrcChannelCount == 3 {
                        res[ImgOffsetDest..ImgOffsetDest + CopySize]
                            .copy_from_slice(&pSrc[ImgOffsetSrc..ImgOffsetSrc + CopySize]);
                        res[ImgOffsetDest + 3] = 255;
                    } else if SrcChannelCount == 1 {
                        res[ImgOffsetDest + 0] = 255;
                        res[ImgOffsetDest + 1] = 255;
                        res[ImgOffsetDest + 2] = 255;
                        res[ImgOffsetDest + 3] = pSrc[ImgOffsetSrc];
                    }
                }
            }

            self.backend_handle.backend.mem_free(pSrc);
            return Ok(res);
        }
    }

    fn load_texture_impl<'a>(
        &mut self,
        texture_id: &mut ETextureIndex,
        width: usize,
        height: usize,
        depth: usize,
        is_3d_tex: bool,
        format: i32,
        data: &'static mut [u8],
        _store_format: i32,
        flags: TexFlags,
        _pTexName: &str,
    ) {
        if width == 0 || height == 0 {
            *texture_id = ETextureIndex::Invalid;
            return;
        }

        // grab texture
        let mut tex = self.first_free_texture;
        if tex.is_invalid() {
            let cur_size = self.texture_indices.len();
            self.texture_indices
                .resize(cur_size * 2, Default::default());
            for i in 0..cur_size - 1 {
                self.texture_indices[cur_size + i].index = ETextureIndex::Index(cur_size + i + 1);
            }
            self.texture_indices.last_mut().unwrap().index = ETextureIndex::Invalid;

            tex = ETextureIndex::Index(cur_size);
        }
        self.first_free_texture = self.texture_indices[tex.unwrap()].index;
        self.texture_indices[tex.unwrap()].index = ETextureIndex::Invalid;

        // flags
        let mut cmd_flags = TexFlags::empty();
        if !(flags & TexFlags::TEXFLAG_NOMIPMAPS).is_empty() {
            cmd_flags |= TexFlags::TEXFLAG_NOMIPMAPS;
        }

        let PixelSize = 4;

        // copy texture data
        let _MemSize = width * height * PixelSize;
        let pTmpData = self.ConvertToRGBA(data, width as usize, height as usize, format);
        //if(!)
        //{
        // TODO dbg_msg("graphics", "converted image %s to RGBA, consider making its file format RGBA", pTexName ? pTexName : "(no name)");
        //}
        if let Ok(data) = pTmpData {
            let Cmd = SCommand_Texture_Create {
                slot: tex,
                width,
                height,
                depth: depth,
                is_3d_tex: is_3d_tex,
                pixel_size: PixelSize,
                format: TexFormat::RGBA as i32,
                store_format: TexFormat::RGBA as i32,
                flags: cmd_flags,
                data: RefCell::new(Some(data)),
            };

            self.backend_handle
                .add_cmd(AllCommands::Misc(Commands::CMD_TEXTURE_CREATE(Cmd)));

            *texture_id = tex;
        } else {
            // TODO: log error
            *texture_id = ETextureIndex::Invalid;
        }
    }

    pub fn load_texture(
        &mut self,
        texture_id: &mut ETextureIndex,
        width: usize,
        height: usize,
        format: i32,
        data: &'static mut [u8],
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) {
        self.load_texture_impl(
            texture_id,
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
        texture_id: &mut ETextureIndex,
        width: usize,
        height: usize,
        depth: usize,
        format: i32,
        data: &'static mut [u8],
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) {
        self.load_texture_impl(
            texture_id,
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

    pub fn unload_texture(&mut self, texture_id: &mut ETextureIndex) {
        if texture_id.is_invalid() {
            return;
        }

        let Cmd = SCommand_Texture_Destroy { slot: *texture_id };
        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::CMD_TEXTURE_DESTROY(Cmd)));

        self.texture_indices[texture_id.unwrap()].index = self.first_free_texture;
        self.first_free_texture = *texture_id;

        *texture_id = ETextureIndex::Invalid;
        return;
    }

    pub fn is_tile_buffering_enabled(&self) -> bool {
        false
    }

    pub fn canvas_aspect(&self) -> f32 {
        (self.window.canvas_width as f32) / self.window.canvas_height as f32
    }

    pub fn canvas_width(&self) -> u32 {
        self.window.canvas_width
    }

    pub fn canvas_height(&self) -> u32 {
        self.window.canvas_height
    }

    pub fn window_width(&self) -> u32 {
        self.window.window_width
    }

    pub fn window_height(&self) -> u32 {
        self.window.window_height
    }

    pub fn resized(&mut self, new_width: u32, new_height: u32) {
        self.backend_handle.backend.resized(new_width, new_height);
        self.window = *self.backend_handle.backend.get_window_props();
    }

    pub fn swap(&mut self) {
        let cmd = SCommand_Swap {};
        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::CMD_SWAP(cmd)));
        self.backend_handle.run_backend_buffer();
    }

    pub fn borrow_window(&self) -> &sdl2::video::Window {
        self.backend_handle.backend.borrow_window()
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

    pub fn IndicesNumRequiredNotify(&mut self, RequiredIndicesCount: usize) {
        let mut Cmd = SCommand_IndicesRequiredNumNotify::default();
        Cmd.required_indices_num = RequiredIndicesCount;

        self.backend_handle.add_cmd(AllCommands::Misc(
            Commands::CMD_INDICES_REQUIRED_NUM_NOTIFY(Cmd),
        ));
    }

    pub fn RenderTileLayer(
        &mut self,
        state: &State,
        buffer_container_index: &BufferContainerIndex,
        Color: &ColorRGBA,
        pOffsets: Vec<usize>,
        pIndicedVertexDrawNum: Vec<usize>,
        NumIndicesOffset: usize,
    ) {
        if NumIndicesOffset == 0 {
            return;
        }

        // add the VertexArrays and draw
        let mut Cmd = SCommand_RenderTileLayer::default();
        Cmd.state = *state;
        Cmd.indices_draw_num = NumIndicesOffset;
        Cmd.buffer_container_index = buffer_container_index.unwrap();
        Cmd.color = *Color;

        Cmd.indices_offsets = pOffsets;
        Cmd.draw_count = pIndicedVertexDrawNum;

        self.backend_handle
            .add_cmd(AllCommands::Render(CommandsRender::CMD_RENDER_TILE_LAYER(
                Cmd,
            )));

        // TODO m_pCommandBuffer->AddRenderCalls(NumIndicesOffset);
        // todo max indices group check!!
    }

    pub fn RenderBorderTiles(
        &mut self,
        state: &State,
        buffer_container_index: &BufferContainerIndex,
        Color: &ColorRGBA,
        pIndexBufferOffset: usize,
        Offset: &vec2,
        dir: &vec2,
        JumpIndex: i32,
        DrawNum: usize,
    ) {
        if DrawNum == 0 {
            return;
        }
        // Draw a border tile a lot of times
        let mut Cmd = SCommand_RenderBorderTile::default();
        Cmd.state = *state;
        Cmd.draw_num = DrawNum;
        Cmd.buffer_container_index = buffer_container_index.unwrap();
        Cmd.color = *Color;

        Cmd.indices_offset = pIndexBufferOffset;
        Cmd.jump_index = JumpIndex;

        Cmd.offset = *Offset;
        Cmd.dir = *dir;

        self.backend_handle
            .add_cmd(AllCommands::Render(CommandsRender::CMD_RENDER_BORDER_TILE(
                Cmd,
            )));

        // TODO: m_pCommandBuffer->AddRenderCalls(1);
    }

    pub fn RenderBorderTileLines(
        &mut self,
        state: &State,
        buffer_container_index: &BufferContainerIndex,
        Color: &ColorRGBA,
        pIndexBufferOffset: usize,
        Offset: &vec2,
        dir: &vec2,
        IndexDrawNum: usize,
        RedrawNum: usize,
    ) {
        if IndexDrawNum == 0 || RedrawNum == 0 {
            return;
        }
        // Draw a border tile a lot of times
        let mut Cmd = SCommand_RenderBorderTileLine::default();
        Cmd.state = *state;
        Cmd.index_draw_num = IndexDrawNum;
        Cmd.draw_num = RedrawNum;
        Cmd.buffer_container_index = buffer_container_index.unwrap();
        Cmd.color = *Color;

        Cmd.indices_offset = pIndexBufferOffset;

        Cmd.offset = *Offset;
        Cmd.dir = *dir;

        self.backend_handle.add_cmd(AllCommands::Render(
            CommandsRender::CMD_RENDER_BORDER_TILE_LINE(Cmd),
        ));

        // TODO m_pCommandBuffer->AddRenderCalls(1);
    }

    pub fn RenderQuadLayer(
        &mut self,
        state: &State,
        buffer_container_index: &BufferContainerIndex,
        pQuadInfo: Vec<SQuadRenderInfo>,
        QuadNum: usize,
        QuadOffset: usize,
    ) {
        if QuadNum == 0 {
            return;
        }

        // add the VertexArrays and draw
        let mut Cmd = SCommand_RenderQuadLayer::default();
        Cmd.state = *state;
        Cmd.quad_num = QuadNum;
        Cmd.quad_offset = QuadOffset;
        Cmd.buffer_container_index = buffer_container_index.unwrap();

        Cmd.quad_info = pQuadInfo;

        self.backend_handle
            .add_cmd(AllCommands::Render(CommandsRender::CMD_RENDER_QUAD_LAYER(
                Cmd,
            )));

        // TODO m_pCommandBuffer->AddRenderCalls(((QuadNum - 1) / gs_GraphicsMaxQuadsRenderCount) + 1);
    }
}

impl GraphicsTextureAllocations for Graphics {
    fn load_texture_slow(
        &mut self,
        texture_id: &mut ETextureIndex,
        width: usize,
        height: usize,
        format: i32,
        data: Vec<u8>,
        store_format: i32,
        Flags: TexFlags,
        pTexName: &str,
    ) {
        let mut data_slice = self.mem_alloc(GraphicsMemoryAllocationType::Texture, data.len());
        let mut data_opt: Option<&'static mut [u8]> = None;
        std::mem::swap(&mut data_slice.memory, &mut data_opt);
        drop(data_slice);
        let mem = data_opt.unwrap();
        mem.copy_from_slice(data.as_slice());
        self.load_texture_impl(
            texture_id,
            width,
            height,
            1,
            false,
            format,
            mem,
            store_format,
            Flags,
            pTexName,
        )
    }

    fn load_texture_3d_slow(
        &mut self,
        texture_id: &mut ETextureIndex,
        width: usize,
        height: usize,
        depth: usize,
        format: i32,
        data: Vec<u8>,
        store_format: i32,
        Flags: TexFlags,
        pTexName: &str,
    ) {
        let mut data_slice = self.mem_alloc(GraphicsMemoryAllocationType::Texture, data.len());
        let mut data_opt: Option<&'static mut [u8]> = None;
        std::mem::swap(&mut data_slice.memory, &mut data_opt);
        drop(data_slice);
        let mem = data_opt.unwrap();
        mem.copy_from_slice(data.as_slice());
        self.load_texture_impl(
            texture_id,
            width,
            height,
            depth,
            true,
            format,
            mem,
            store_format,
            Flags,
            pTexName,
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
