use std::cell::RefCell;

use bitflags::bitflags;

use crate::rendering::{
    ColorRGBA, ETextureIndex, GL_SColor, GL_SColorf, GL_SPoint, GL_STexCoord, SPoint, State,
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

pub struct GL_STexCoord3D {
    u: f32,
    v: f32,
    w: f32,
}

impl GL_STexCoord3D {
    fn set(&mut self, tex_coord: &GL_STexCoord) -> &mut Self {
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

pub struct GL_SVertexTex3DStream {
    pub pos: GL_SPoint,
    pub color: GL_SColor,
    pub tex: GL_STexCoord3D,
}

pub type STexCoord = vec2;
type SColorf = GL_SColorf;
pub type SColor = GL_SColor;
/*
type SVertexTex3D = GL_SVertexTex3D;
type SVertexTex3DStream = GL_SVertexTex3DStream; */

pub struct SCommand_Clear {
    pub color: SColorf,
    pub force_clear: bool,
}

pub trait RenderCommand {
    fn set_state(&mut self, state: State);
    fn set_prim_type(&mut self, prim_type: PrimType);
    fn set_prim_count(&mut self, prim_count: usize);
}

#[repr(C)]
pub struct SCommand_Render {
    pub state: State,
    pub prim_type: PrimType,
    pub prim_count: usize,
    pub vertices_offset: usize,
}

impl SCommand_Render {
    pub fn new() -> SCommand_Render {
        SCommand_Render {
            state: State::new(),
            prim_type: PrimType::Invalid,
            prim_count: 0,
            vertices_offset: 0,
        }
    }
}

impl RenderCommand for SCommand_Render {
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
pub struct SCommand_RenderTex3D
{
    SCommand_RenderTex3D() :
        SCommand(CMD_RENDER_TEX3D) {}
    SState m_State;
    unsigned m_PrimType;
    unsigned m_PrimCount;
    SVertexTex3DStream *m_pVertices; // you should use the command buffer data to allocate vertices for this command
}
*/
#[derive(Default)]
pub struct SCommand_CreateBufferObject {
    pub buffer_index: usize,

    pub upload_data: RefCell<Option<&'static mut [u8]>>,

    pub flags: i32, // @see EBufferObjectCreateFlags
}

#[derive(Default)]
pub struct SCommand_RecreateBufferObject {
    pub buffer_index: usize,

    pub upload_data: RefCell<Option<&'static mut [u8]>>,

    pub flags: i32, // @see EBufferObjectCreateFlags
}

#[derive(Default)]
pub struct SCommand_UpdateBufferObject {
    pub buffer_index: usize,

    pub offset: usize,
    pub upload_data: Vec<u8>,
}

#[derive(Default)]
pub struct SCommand_CopyBufferObject {
    pub write_buffer_index: usize,
    pub read_buffer_index: usize,

    pub read_offset: usize,
    pub write_offset: usize,
    pub copy_size: usize,
}

pub type BufferObjectIndex = Option<usize>;
pub type BufferContainerIndex = Option<usize>;
#[derive(Copy, Clone, Default)]
pub enum GraphicsType {
    UnsignedByte,
    UnsignedShort,
    Int,
    #[default]
    UnsignedInt,
    Float,
}

pub struct SCommand_DeleteBufferObject {
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

#[derive(Default)]
pub struct SBufferContainerInfo {
    pub stride: usize,
    pub vert_buffer_binding_index: BufferObjectIndex,

    pub attributes: Vec<SAttribute>,
}

#[derive(Default)]
pub struct SCommand_CreateBufferContainer {
    pub buffer_container_index: usize,

    pub stride: usize,
    pub vert_buffer_binding_index: usize,

    pub attributes: Vec<SAttribute>,
}

#[derive(Default)]
pub struct SCommand_UpdateBufferContainer {
    pub buffer_container_index: usize,

    pub stride: usize,
    pub vert_buffer_binding_index: usize,

    pub attributes: Vec<SAttribute>,
}

#[derive(Default)]
pub struct SCommand_DeleteBufferContainer {
    pub buffer_container_index: usize,
    pub destroy_all_buffer_objects: bool,
}

#[derive(Default)]
pub struct SCommand_IndicesRequiredNumNotify {
    pub required_indices_num: usize,
}

#[derive(Default)]
pub struct SCommand_RenderTileLayer {
    pub state: State,
    pub color: SColorf, // the color of the whole tilelayer -- already enveloped

    // the char offset of all indices that should be rendered, and the amount of renders
    pub indices_offsets: Vec<usize>,
    pub draw_count: Vec<usize>,

    pub indices_draw_num: usize,
    pub buffer_container_index: usize,
}

#[derive(Default)]
pub struct SCommand_RenderBorderTile {
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
pub struct SCommand_RenderBorderTileLine {
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

#[derive(Default)]
pub struct SCommand_RenderQuadLayer {
    pub state: State,

    pub buffer_container_index: usize,
    pub quad_info: Vec<SQuadRenderInfo>,
    pub quad_num: usize,
    pub quad_offset: usize,
}

/*
pub struct SCommand_RenderText
{
    SCommand_RenderText() :
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
pub struct SCommand_RenderQuadContainer {
    pub state: State,

    pub buffer_container_index: usize,

    pub draw_num: usize,
    pub offset: usize,
}

#[derive(Default)]
pub struct SCommand_RenderQuadContainerEx {
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

#[derive(Default)]
pub struct SCommand_RenderQuadContainerAsSpriteMultiple {
    pub state: State,

    pub buffer_container_index: usize,

    pub render_info: Vec<SRenderSpriteInfo>,

    pub center: SPoint,
    pub vertex_color: SColorf,

    pub draw_num: usize,
    pub draw_count: usize,
    pub offset: usize,
}

/*
pub struct SCommand_TrySwapAndScreenshot
{
    CImageInfo *m_pImage; // processor will fill this out, the one who adds this command must free the data as well
    bool *m_pSwapped;
}*/

#[repr(C)]
pub struct SCommand_Swap {}

/*

pub struct SCommand_VSync
    {
        SCommand_VSync() :
            SCommand(CMD_VSYNC) {}

      pub m_VSync: i32,
        bool *m_pRetOk;
    }

pub struct SCommand_MultiSampling
    {
        SCommand_MultiSampling() :
            SCommand(CMD_MULTISAMPLING) {}

        uint32_t m_RequestedMultiSamplingCount;
        uint32_t *m_pRetMultiSamplingCount;
        bool *m_pRetOk;
    }
*/

pub struct SCommand_Update_Viewport {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub by_resize: bool, // resized by an resize event.. a hint to make clear that the viewport update can be deferred if wanted
}

pub struct SCommand_Texture_Create {
    // texture information
    pub slot: ETextureIndex,

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
    pub data: RefCell<Option<&'static mut [u8]>>,
}

pub struct SCommand_Texture_Update {
    // texture information
    pub slot: ETextureIndex,

    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub format: i32,
    pub data: Vec<u8>,
}

pub struct SCommand_Texture_Destroy {
    // texture information
    pub slot: ETextureIndex,
}
/*
pub struct SCommand_TextTextures_Create
    {
        SCommand_TextTextures_Create() :
            SCommand(CMD_TEXT_TEXTURES_CREATE) {}

        // texture information
      pub m_Slot: i32,
      pub m_SlotOutline: i32,

      pub m_Width: i32,
      pub m_Height: i32,

        void *m_pTextData;
        void *m_pTextOutlineData;
    }

pub struct SCommand_TextTextures_Destroy
    {
        SCommand_TextTextures_Destroy() :
            SCommand(CMD_TEXT_TEXTURES_DESTROY) {}

        // texture information
      pub m_Slot: i32,
      pub m_SlotOutline: i32,
    }

pub struct SCommand_TextTexture_Update
    {
        SCommand_TextTexture_Update() :
            SCommand(CMD_TEXT_TEXTURE_UPDATE) {}

        // texture information
      pub m_Slot: i32,

      pub m_X: i32,
      pub m_Y: i32,
      pub m_Width: i32,
      pub m_Height: i32,
        void *m_pData; // will be freed by the command processor
    }

pub struct SCommand_WindowCreateNtf : public CCommandBuffer::SCommand
    {
        SCommand_WindowCreateNtf() :
            SCommand(CMD_WINDOW_CREATE_NTF) {}

        uint32_t m_WindowID;
    }

pub struct SCommand_WindowDestroyNtf : public CCommandBuffer::SCommand
    {
        SCommand_WindowDestroyNtf() :
            SCommand(CMD_WINDOW_DESTROY_NTF) {}

        uint32_t m_WindowID;
    }
*/

pub enum ETWGraphicsGPUType {
    GRAPHICS_GPU_TYPE_DISCRETE = 0,
    GRAPHICS_GPU_TYPE_INTEGRATED,
    GRAPHICS_GPU_TYPE_VIRTUAL,
    GRAPHICS_GPU_TYPE_CPU,

    // should stay at last position in this enum
    GRAPHICS_GPU_TYPE_INVALID,
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
pub struct SCommand_PreInit {
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

pub struct SCommand_Shutdown {}

pub struct SCommand_PostShutdown {}

pub enum CommandsRender {
    // rendering
    CMD_CLEAR(SCommand_Clear),

    CMD_RENDER(SCommand_Render),
    CMD_RENDER_TEX3D,

    CMD_RENDER_TILE_LAYER(SCommand_RenderTileLayer), // render a tilelayer
    CMD_RENDER_BORDER_TILE(SCommand_RenderBorderTile), // render one tile multiple times
    CMD_RENDER_BORDER_TILE_LINE(SCommand_RenderBorderTileLine), // render an amount of tiles multiple times
    CMD_RENDER_QUAD_LAYER(SCommand_RenderQuadLayer),            // render a quad layer
    CMD_RENDER_TEXT,                                            // render text
    CMD_RENDER_QUAD_CONTAINER(SCommand_RenderQuadContainer),    // render a quad buffer container
    CMD_RENDER_QUAD_CONTAINER_EX(SCommand_RenderQuadContainerEx), // render a quad buffer container with extended parameters
    CMD_RENDER_QUAD_CONTAINER_SPRITE_MULTIPLE(SCommand_RenderQuadContainerAsSpriteMultiple), // render a quad buffer container as sprite multiple times
}

pub enum Commands {
    // texture commands
    CMD_TEXTURE_CREATE(SCommand_Texture_Create),
    CMD_TEXTURE_DESTROY(SCommand_Texture_Destroy),
    CMD_TEXTURE_UPDATE(SCommand_Texture_Update),
    CMD_TEXT_TEXTURES_CREATE,
    CMD_TEXT_TEXTURES_DESTROY,
    CMD_TEXT_TEXTURE_UPDATE,

    // opengl 2.0+ commands (some are just emulated and only exist in opengl 3.3+)
    CMD_CREATE_BUFFER_OBJECT(SCommand_CreateBufferObject), // create vbo
    CMD_RECREATE_BUFFER_OBJECT(SCommand_RecreateBufferObject), // recreate vbo
    CMD_UPDATE_BUFFER_OBJECT(SCommand_UpdateBufferObject), // update vbo
    CMD_COPY_BUFFER_OBJECT(SCommand_CopyBufferObject),     // copy vbo to another
    CMD_DELETE_BUFFER_OBJECT(SCommand_DeleteBufferObject), // delete vbo

    CMD_CREATE_BUFFER_CONTAINER(SCommand_CreateBufferContainer), // create vao
    CMD_DELETE_BUFFER_CONTAINER(SCommand_DeleteBufferContainer), // delete vao
    CMD_UPDATE_BUFFER_CONTAINER(SCommand_UpdateBufferContainer), // update vao

    CMD_INDICES_REQUIRED_NUM_NOTIFY(SCommand_IndicesRequiredNumNotify), // create indices that are required

    // swap
    CMD_SWAP(SCommand_Swap),

    // misc
    CMD_UPDATE_VIEWPORT(SCommand_Update_Viewport),
    CMD_MULTISAMPLING,
    CMD_VSYNC,
    CMD_TRY_SWAP_AND_SCREENSHOT,

    // in Android a window that minimizes gets destroyed
    CMD_WINDOW_CREATE_NTF,
    CMD_WINDOW_DESTROY_NTF,

    CMD_COUNT,

    // CMDGROUP_PLATFORM_GL
    CMD_SHUTDOWN,
    CMD_POST_SHUTDOWN,
}

pub enum AllCommands {
    Render(CommandsRender),
    Misc(Commands),
    None,
}

pub enum ERunCommandReturnTypes {
    RUN_COMMAND_COMMAND_HANDLED = 0,
    RUN_COMMAND_COMMAND_UNHANDLED,
    RUN_COMMAND_COMMAND_WARNING,
    RUN_COMMAND_COMMAND_ERROR,
}
