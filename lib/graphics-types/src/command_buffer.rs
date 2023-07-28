use std::cell::RefCell;

use base::counted_index::CountedIndex;
use bitflags::bitflags;
use pool::mt_datatypes::PoolVec;

use crate::{
    rendering::{ColorRGBA, GlColor, GlColorf, GlPoint, GlTexCoord, SPoint, State, TextureIndex},
    types::{GraphicsBackendMemory, ImageFormat},
};
use math::math::vector::*;

pub const GRAPHICS_MAX_PARTICLES_RENDER_COUNT: usize = 512;
pub const GRAPHICS_MAX_QUADS_RENDER_COUNT: usize = 256;

pub enum StreamDataMax {
    MaxTextures = 1024 * 8,
    MaxVertices = 32 * 1024,
}

pub enum TexFormat {
    Invalid = 0,
    RGBA,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TexFlags: i32 {
        const TEXFLAG_NOMIPMAPS = (1 << 0);
    }
}

#[derive(PartialEq, Copy, Clone)]
pub enum PrimType {
    //
    Invalid = 0,
    Lines,
    Quads,
    Triangles,
}

pub struct GlTexCoord3D {
    u: f32,
    v: f32,
    w: f32,
}

impl GlTexCoord3D {
    fn set(&mut self, tex_coord: &GlTexCoord) -> &mut Self {
        self.u = tex_coord.x;
        self.v = tex_coord.y;
        self
    }

    fn set_v3(&mut self, tex_coord: &vec3) -> &mut Self {
        self.u = tex_coord.x;
        self.v = tex_coord.y;
        self.w = tex_coord.z;
        self
    }
}

/*/
pub struct GL_SVertexTex3D
{
    GL_SPoint m_Pos;
    GL_SColorf m_Color;
    GL_STexCoord3D m_Tex;
}
*/

pub struct GlVertexTex3DStream {
    pub pos: GlPoint,
    pub color: GlColor,
    pub tex: GlTexCoord3D,
}

pub type STexCoord = vec2;
type SColorf = GlColorf;
pub type SColor = GlColor;
/*
type SVertexTex3D = GL_SVertexTex3D;
type SVertexTex3DStream = GL_SVertexTex3DStream; */

pub struct CommandClear {
    pub color: SColorf,
    pub force_clear: bool,
}

pub trait RenderCommand {
    fn set_state(&mut self, state: State);
    fn set_prim_type(&mut self, prim_type: PrimType);
    fn set_prim_count(&mut self, prim_count: usize);
}

#[repr(C)]
pub struct CommandRender {
    pub state: State,
    pub prim_type: PrimType,
    pub prim_count: usize,
    pub vertices_offset: usize,
}

impl CommandRender {
    pub fn new() -> CommandRender {
        CommandRender {
            state: State::new(),
            prim_type: PrimType::Invalid,
            prim_count: 0,
            vertices_offset: 0,
        }
    }
}

impl RenderCommand for CommandRender {
    fn set_state(&mut self, state: State) {
        self.state = state;
    }
    fn set_prim_type(&mut self, prim_type: PrimType) {
        self.prim_type = prim_type;
    }
    fn set_prim_count(&mut self, prim_count: usize) {
        self.prim_count = prim_count;
    }
}
/*
pub struct CommandRenderTex3D
{
    CommandRenderTex3D() :
        SCommand(CMD_RENDER_TEX3D) {}
    SState m_State;
    unsigned m_PrimType;
    unsigned m_PrimCount;
    SVertexTex3DStream *m_pVertices; // you should use the command buffer data to allocate vertices for this command
}
*/
#[derive(Default)]
pub struct CommandCreateBufferObject {
    pub buffer_index: usize,

    pub upload_data: RefCell<Option<GraphicsBackendMemory>>,

    pub flags: i32, // @see EBufferObjectCreateFlags
}

#[derive(Default)]
pub struct CommandRecreateBufferObject {
    pub buffer_index: usize,

    pub upload_data: RefCell<Option<GraphicsBackendMemory>>,

    pub flags: i32, // @see EBufferObjectCreateFlags
}

#[derive(Default)]
pub struct CommandUpdateBufferObject {
    pub buffer_index: usize,

    pub offset: usize,
    pub upload_data: Vec<u8>,
}

#[derive(Default)]
pub struct CommandCopyBufferObject {
    pub write_buffer_index: usize,
    pub read_buffer_index: usize,

    pub read_offset: usize,
    pub write_offset: usize,
    pub copy_size: usize,
}

pub type BufferObjectIndex = CountedIndex<true>;
pub type BufferContainerIndex = CountedIndex<true>;
#[derive(Copy, Clone, Default)]
pub enum GraphicsType {
    UnsignedByte,
    UnsignedShort,
    Int,
    #[default]
    UnsignedInt,
    Float,
}

pub struct CommandDeleteBufferObject {
    pub buffer_index: usize,
}

// the attributes of the container
#[derive(Copy, Clone, Default)]
pub struct SAttribute {
    pub data_type_count: i32,
    pub graphics_type: GraphicsType,
    pub normalized: bool,
    pub offset: usize,

    //0: float, 1:integer
    pub func_type: u32,
}

pub struct SBufferContainerInfo {
    pub stride: usize,
    pub vert_buffer_binding_index: BufferObjectIndex,

    pub attributes: Vec<SAttribute>,
}

#[derive(Default)]
pub struct CommandCreateBufferContainer {
    pub buffer_container_index: usize,

    pub stride: usize,
    pub vert_buffer_binding_index: usize,

    pub attributes: Vec<SAttribute>,
}

#[derive(Default)]
pub struct CommandUpdateBufferContainer {
    pub buffer_container_index: usize,

    pub stride: usize,
    pub vert_buffer_binding_index: usize,

    pub attributes: Vec<SAttribute>,
}

#[derive(Default)]
pub struct CommandDeleteBufferContainer {
    pub buffer_container_index: usize,
    pub destroy_all_buffer_objects: bool,
}

#[derive(Default)]
pub struct CommandIndicesRequiredNumNotify {
    pub required_indices_num: usize,
}

pub struct CommandRenderTileLayer {
    pub state: State,
    pub color: SColorf, // the color of the whole tilelayer -- already enveloped

    // the char offset of all indices that should be rendered, and the amount of renders
    pub indices_offsets: PoolVec<usize>,
    pub draw_count: PoolVec<usize>,

    pub indices_draw_num: usize,
    pub buffer_container_index: usize,
}

#[derive(Default)]
pub struct CommandRenderBorderTile {
    pub state: State,
    pub color: SColorf, // the color of the whole tilelayer -- already enveloped
    pub indices_offset: usize, // you should use the command buffer data to allocate vertices for this command
    pub draw_num: usize,
    pub buffer_container_index: usize,

    pub offset: vec2,
    pub dir: vec2,
    pub jump_index: i32,
}

#[derive(Default)]
pub struct CommandRenderBorderTileLine {
    pub state: State,
    pub color: SColorf, // the color of the whole tilelayer -- already enveloped
    pub indices_offset: usize, // you should use the command buffer data to allocate vertices for this command
    pub index_draw_num: usize,
    pub draw_num: usize,
    pub buffer_container_index: usize,

    pub offset: vec2,
    pub dir: vec2,
}

#[repr(C)]
#[derive(Default, Clone)]
pub struct SQuadRenderInfo {
    pub color: ColorRGBA,
    pub offsets: vec2,
    pub rotation: f32,
    // allows easier upload for uniform buffers because of the alignment requirements
    pub padding: f32,
}

pub struct CommandRenderQuadLayer {
    pub state: State,

    pub buffer_container_index: usize,
    pub quad_info: PoolVec<SQuadRenderInfo>,
    pub quad_num: usize,
    pub quad_offset: usize,
}

/*
pub struct CommandRenderText
{
    CommandRenderText() :
        SCommand(CMD_RENDER_TEXT) {}
    SState m_State;

  pub m_BufferContainerIndex: i32,
  pub m_TextureSize: i32,

  pub m_TextTextureIndex: i32,
  pub m_TextOutlineTextureIndex: i32,

  pub m_DrawNum: i32,
    ColorRGBA m_TextColor;
    ColorRGBA m_TextOutlineColor;
}
*/
#[derive(Default)]
pub struct CommandRenderQuadContainer {
    pub state: State,

    pub buffer_container_index: usize,

    pub draw_num: usize,
    pub offset: usize,
}

#[derive(Default)]
pub struct CommandRenderQuadContainerEx {
    pub state: State,

    pub buffer_container_index: usize,

    pub rotation: f32,
    pub center: SPoint,

    pub vertex_color: SColorf,

    pub draw_num: usize,
    pub offset: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SRenderSpriteInfo {
    pub pos: vec2,
    pub scale: f32,
    pub rotation: f32,
}

pub struct CommandRenderQuadContainerAsSpriteMultiple {
    pub state: State,

    pub buffer_container_index: usize,

    pub render_info: PoolVec<SRenderSpriteInfo>,

    pub center: SPoint,
    pub vertex_color: SColorf,

    pub draw_num: usize,
    pub draw_count: usize,
    pub offset: usize,
}

pub struct ScreenshotBuffData {
    pub img_data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
}
/*
pub struct CommandTrySwapAndScreenshot
{
    CImageInfo *m_pImage; // processor will fill this out, the one who adds this command must free the data as well
    bool *m_pSwapped;
}*/

#[repr(C)]
pub struct CommandSwap {}

/*

pub struct CommandVSync
    {
        CommandVSync() :
            SCommand(CMD_VSYNC) {}

      pub m_VSync: i32,
        bool *m_pRetOk;
    }

pub struct CommandMultiSampling
    {
        CommandMultiSampling() :
            SCommand(CMD_MULTISAMPLING) {}

        uint32_t m_RequestedMultiSamplingCount;
        uint32_t *m_pRetMultiSamplingCount;
        bool *m_pRetOk;
    }
*/

pub struct CommandUpdateViewport {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub by_resize: bool, // resized by an resize event.. a hint to make clear that the viewport update can be deferred if wanted
}

pub struct CommandTextureCreate {
    // texture information
    pub slot: usize,

    pub width: usize,
    pub height: usize,
    pub depth: usize,
    pub is_3d_tex: bool,
    pub pixel_size: usize,
    pub format: i32,
    pub store_format: i32,
    pub flags: TexFlags,
    /// note that this data must be memory allocated by mem_alloc of the graphics implementation
    /// it will be automatically free'd by the backend!
    pub data: RefCell<Option<GraphicsBackendMemory>>,
}

pub struct CommandTextureUpdate {
    // texture information
    pub slot: usize,

    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub format: i32,
    pub data: Vec<u8>,
}

pub struct CommandTextureDestroy {
    // texture information
    pub slot: usize,
}
/*
pub struct CommandTextTextures_Create
    {
        CommandTextTextures_Create() :
            SCommand(CMD_TEXT_TEXTURES_CREATE) {}

        // texture information
      pub m_Slot: i32,
      pub m_SlotOutline: i32,

      pub m_Width: i32,
      pub m_Height: i32,

        void *m_pTextData;
        void *m_pTextOutlineData;
    }

pub struct CommandTextTextures_Destroy
    {
        CommandTextTextures_Destroy() :
            SCommand(CMD_TEXT_TEXTURES_DESTROY) {}

        // texture information
      pub m_Slot: i32,
      pub m_SlotOutline: i32,
    }

pub struct CommandTextTexture_Update
    {
        CommandTextTexture_Update() :
            SCommand(CMD_TEXT_TEXTURE_UPDATE) {}

        // texture information
      pub m_Slot: i32,

      pub m_X: i32,
      pub m_Y: i32,
      pub m_Width: i32,
      pub m_Height: i32,
        void *m_pData; // will be freed by the command processor
    }

pub struct CommandWindowCreateNtf : public CCommandBuffer::SCommand
    {
        CommandWindowCreateNtf() :
            SCommand(CMD_WINDOW_CREATE_NTF) {}

        uint32_t m_WindowID;
    }

pub struct CommandWindowDestroyNtf : public CCommandBuffer::SCommand
    {
        CommandWindowDestroyNtf() :
            SCommand(CMD_WINDOW_DESTROY_NTF) {}

        uint32_t m_WindowID;
    }
*/

pub enum ETWGraphicsGPUType {
    Discrete = 0,
    Integrated,
    Virtual,
    CPU,

    // should stay at last position in this enum
    Invalid,
}

#[repr(C)]
pub struct STWGraphicGPUItem {
    pub name: [std::os::raw::c_char; 256],
    pub gpu_type: u32, // @see ETWGraphicsGPUType
}

#[repr(C)]
pub struct STWGraphicGPU {
    pub gpus: *mut STWGraphicGPUItem,
    pub gpu_count: u32,
    pub auto_gpu: STWGraphicGPUItem,
}
/*/
#[repr(C)]
pub struct CommandPreInit {
    pub window: *const std::ffi::c_void,
    pub width: u32,
    pub height: u32,

    pub vendor_str: *mut std::os::raw::c_char,
    pub version_string: *mut std::os::raw::c_char,
    pub renderer_string: *mut std::os::raw::c_char,
    pub m_pGPUList: *mut STWGraphicGPU,
}*/

#[derive(Debug)]
#[repr(C)]
pub struct SBackendCapabilites {
    pub tile_buffering: bool,
    pub quad_buffering: bool,
    pub text_buffering: bool,
    pub quad_container_buffering: bool,

    pub mip_mapping: bool,
    pub npot_textures: bool,
    pub has_3d_textures: bool,
    pub has_2d_array_textures: bool,
    pub has_2d_array_textures_as_extension: bool,
    pub shader_support: bool,

    // use quads as much as possible, even if the user config says otherwise
    pub triangles_as_quads: bool,
}

impl Default for SBackendCapabilites {
    fn default() -> Self {
        SBackendCapabilites {
            tile_buffering: false,
            quad_buffering: false,
            text_buffering: false,
            quad_container_buffering: false,
            mip_mapping: false,
            npot_textures: false,
            has_3d_textures: false,
            has_2d_array_textures: false,
            has_2d_array_textures_as_extension: false,
            shader_support: false,
            triangles_as_quads: false,
        }
    }
}

pub struct CommandShutdown {}

pub struct CommandPostShutdown {}

pub enum CommandsRender {
    // rendering
    Clear(CommandClear),

    Render(CommandRender),
    RenderTex3D,
    RenderFirstPassBlurred(CommandRender),

    TileLayer(CommandRenderTileLayer),   // render a tilelayer
    BorderTile(CommandRenderBorderTile), // render one tile multiple times
    BorderTileLine(CommandRenderBorderTileLine), // render an amount of tiles multiple times
    QuadLayer(CommandRenderQuadLayer),   // render a quad layer
    Text,                                // render text
    QuadContainer(CommandRenderQuadContainer), // render a quad buffer container
    QuadContainerEx(CommandRenderQuadContainerEx), // render a quad buffer container with extended parameters
    QuadContainerSpriteMultiple(CommandRenderQuadContainerAsSpriteMultiple), // render a quad buffer container as sprite multiple times
}

pub enum Commands {
    // texture commands
    TextureCreate(CommandTextureCreate),
    TextureDestroy(CommandTextureDestroy),
    TextureUpdate(CommandTextureUpdate),
    TextTexturesCreate,
    TextTexturesDestroy,
    TextTextureUpdate,

    // opengl 2.0+ commands (some are just emulated and only exist in opengl 3.3+)
    CreateBufferObject(CommandCreateBufferObject), // create vbo
    RecreateBufferObject(CommandRecreateBufferObject), // recreate vbo
    UpdateBufferObject(CommandUpdateBufferObject), // update vbo
    CopyBufferObject(CommandCopyBufferObject),     // copy vbo to another
    DleteBufferObject(CommandDeleteBufferObject),  // delete vbo

    CreateBufferContainer(CommandCreateBufferContainer), // create vao
    DeleteBufferContainer(CommandDeleteBufferContainer), // delete vao
    UpdateBufferContainer(CommandUpdateBufferContainer), // update vao

    IndicesRequiredNumNotify(CommandIndicesRequiredNumNotify), // create indices that are required

    // swap
    Swap(CommandSwap),
    SwitchToDualPass,
    NextSubpass,

    // misc
    UpdateViewport(CommandUpdateViewport),
    Multisampling,
    VSync,
    TrySwapAndScreenshot,

    // in Android a window that minimizes gets destroyed
    WindowCreateNtf,
    WindowDestroyNtf,

    // CMDGROUP_PLATFORM_GL
    Shutdown,
    PostShutdown,
}

pub enum AllCommands {
    Render(CommandsRender),
    Misc(Commands),
}

pub enum ERunCommandReturnTypes {
    CmdHandled = 0,
    CmdUnhandled,
    CmdWarning,
    CmdError,
}
