use bincode::{BorrowDecode, Decode, Encode};
use bitflags::bitflags;
use pool::mt_datatypes::PoolVec;
use serde::{Deserialize, Serialize};

use crate::{
    rendering::{ColorRGBA, GlColor, GlColorf, GlPoint, SPoint, State},
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
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
    pub struct TexFlags: i32 {
        const TEXFLAG_NOMIPMAPS = (1 << 0);
    }
}

impl Encode for TexFlags {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        let conf = *encoder.config();
        bincode::serde::encode_into_writer(self, encoder.writer(), conf)
    }
}

impl Decode for TexFlags {
    fn decode<D: bincode::de::Decoder>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let conf = *decoder.config();
        bincode::serde::decode_from_reader(decoder.reader(), conf)
    }
}

impl<'de> BorrowDecode<'de> for TexFlags {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Self::decode(decoder)
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Encode, Decode)]
pub enum PrimType {
    Lines,
    Quads,
    Triangles,
}

pub struct GlTexCoord3D {
    _u: f32,
    _v: f32,
    _w: f32,
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

#[derive(Debug, Encode, Decode)]
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
#[derive(Debug, Encode, Decode)]
pub struct CommandRender {
    pub state: State,
    pub prim_type: PrimType,
    pub prim_count: usize,
    pub vertices_offset: usize,
}

impl CommandRender {
    pub fn new(prim_type: PrimType) -> CommandRender {
        CommandRender {
            state: State::new(),
            prim_type,
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

#[derive(Debug, Encode, Decode)]
pub struct CommandRenderTex3D {
    pub state: State,
    pub prim_type: PrimType,
    pub prim_count: usize,
    pub vertices_offset: usize,
}

#[derive(Debug, Encode, Decode)]
pub struct CommandCreateBufferObject {
    pub buffer_index: u128,

    pub upload_data: GraphicsBackendMemory,
}

#[derive(Debug, Encode, Decode)]
pub struct CommandRecreateBufferObject {
    pub buffer_index: u128,

    pub upload_data: GraphicsBackendMemory,
}

#[derive(Debug, Copy, Clone, Default, Encode, Decode)]
pub enum GraphicsType {
    UnsignedByte,
    UnsignedShort,
    Int,
    #[default]
    UnsignedInt,
    Float,
}

#[derive(Debug, Encode, Decode)]
pub struct CommandDeleteBufferObject {
    pub buffer_index: u128,
}

#[derive(Debug, Default, Encode, Decode)]
pub struct CommandIndicesRequiredNumNotify {
    pub required_indices_num: usize,
}

#[derive(Debug, Encode, Decode)]
pub struct CommandRenderTileLayer {
    pub state: State,
    pub color: SColorf, // the color of the whole tilelayer -- already enveloped

    // the char offset of all indices that should be rendered, and the amount of renders
    pub indices_offsets: PoolVec<usize>,
    pub draw_count: PoolVec<usize>,

    pub indices_draw_num: usize,
    pub buffer_object_index: u128,
}

#[derive(Debug, Default, Encode, Decode)]
pub struct CommandRenderBorderTile {
    pub state: State,
    pub color: SColorf, // the color of the whole tilelayer -- already enveloped
    pub indices_offset: usize, // you should use the command buffer data to allocate vertices for this command
    pub draw_num: usize,
    pub buffer_object_index: u128,

    pub offset: vec2,
    pub dir: vec2,
    pub jump_index: i32,
}

#[derive(Debug, Default, Encode, Decode)]
pub struct CommandRenderBorderTileLine {
    pub state: State,
    pub color: SColorf, // the color of the whole tilelayer -- already enveloped
    pub indices_offset: usize, // you should use the command buffer data to allocate vertices for this command
    pub index_draw_num: usize,
    pub draw_num: usize,
    pub buffer_object_index: u128,

    pub offset: vec2,
    pub dir: vec2,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct SQuadRenderInfo {
    pub color: ColorRGBA,
    pub offsets: vec2,
    pub rotation: f32,
    // allows easier upload for uniform buffers because of the alignment requirements
    pub padding: f32,
}

impl SQuadRenderInfo {
    pub fn new(color: ColorRGBA, offsets: vec2, rotation: f32) -> Self {
        Self {
            color,
            offsets,
            rotation,
            padding: 0.0,
        }
    }
}

#[derive(Debug, Encode, Decode)]
pub struct CommandRenderQuadLayer {
    pub state: State,

    pub buffer_object_index: u128,
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

#[derive(Debug, Default, Encode, Decode)]
pub struct CommandRenderQuadContainer {
    pub state: State,

    pub buffer_object_index: u128,

    pub rotation: f32,
    pub center: SPoint,

    pub vertex_color: SColorf,

    pub draw_num: usize,
    pub offset: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Encode, Decode, Serialize, Deserialize)]
pub struct SRenderSpriteInfo {
    pub pos: vec2,
    pub scale: f32,
    pub rotation: f32,
}

impl SRenderSpriteInfo {
    pub fn new(pos: vec2, scale: f32, rotation: f32) -> Self {
        Self {
            pos,
            scale,
            rotation,
        }
    }
}

#[derive(Debug, Encode, Decode)]
pub struct CommandRenderQuadContainerAsSpriteMultiple {
    pub state: State,

    pub buffer_object_index: u128,

    pub render_info: PoolVec<SRenderSpriteInfo>,

    pub center: SPoint,
    pub vertex_color: SColorf,

    pub draw_num: usize,
    pub draw_count: usize,
    pub offset: usize,
}

#[derive(Debug)]
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

#[derive(Debug, Encode, Decode)]
pub struct CommandUpdateViewport {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub by_resize: bool, // resized by an resize event.. a hint to make clear that the viewport update can be deferred if wanted
}

#[derive(Debug, Encode, Decode)]
pub struct CommandTextureCreate {
    // texture information
    pub texture_index: u128,

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
    pub data: GraphicsBackendMemory,
}

#[derive(Debug, Encode, Decode)]
pub struct CommandTextureUpdate {
    // texture information
    pub texture_index: u128,

    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub format: i32,
    pub data: Vec<u8>,
}

#[derive(Debug, Encode, Decode)]
pub struct CommandTextureDestroy {
    // texture information
    pub texture_index: u128,
}
/*
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

#[derive(Debug, Copy, Clone)]
pub enum ETWGraphicsGPUType {
    Discrete = 0,
    Integrated,
    Virtual,
    CPU,

    // should stay at last position in this enum
    Invalid,
}

#[repr(C)]
#[derive(Debug)]
pub struct STWGraphicGPUItem {
    pub name: [std::os::raw::c_char; 256],
    pub gpu_type: u32, // @see ETWGraphicsGPUType
}

#[repr(C)]
#[derive(Debug)]
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

#[derive(Debug, Encode, Decode)]
pub enum CommandsRenderStream {
    Render(CommandRender),
    RenderTex3D(CommandRenderTex3D),
    RenderBlurred {
        cmd: CommandRender,
        blur_radius: f32,
        blur_horizontal: bool,
        blur_color: vec4,
    },
    RenderStencil {
        cmd: CommandRender,
    },
    RenderStencilNotPased {
        cmd: CommandRender,
        clear_stencil: bool,
    },
}

#[derive(Debug, Encode, Decode)]
pub enum CommandsRenderMap {
    TileLayer(CommandRenderTileLayer),           // render a tilelayer
    BorderTile(CommandRenderBorderTile),         // render one tile multiple times
    BorderTileLine(CommandRenderBorderTileLine), // render an amount of tiles multiple times
    QuadLayer(CommandRenderQuadLayer),           // render a quad layer
}

#[derive(Debug, Encode, Decode)]
pub enum CommandsRenderQuadContainer {
    Render(CommandRenderQuadContainer), // render a quad buffer container with extended parameters
    RenderAsSpriteMultiple(CommandRenderQuadContainerAsSpriteMultiple), // render a quad buffer container as sprite multiple times
}

#[derive(Debug, Encode, Decode)]
pub enum CommandsRender {
    // rendering
    Clear(CommandClear),

    Stream(CommandsRenderStream),

    Map(CommandsRenderMap),

    QuadContainer(CommandsRenderQuadContainer),
}

#[derive(Debug, Encode, Decode)]
pub enum Commands {
    // texture commands
    TextureCreate(CommandTextureCreate),
    TextureDestroy(CommandTextureDestroy),
    TextureUpdate(CommandTextureUpdate),

    // opengl 2.0+ commands (some are just emulated and only exist in opengl 3.3+)
    CreateBufferObject(CommandCreateBufferObject), // create vbo
    RecreateBufferObject(CommandRecreateBufferObject), // recreate vbo
    DeleteBufferObject(CommandDeleteBufferObject), // delete vbo

    IndicesRequiredNumNotify(CommandIndicesRequiredNumNotify), // create indices that are required

    // swap
    Swap,

    // passes
    NextSwitchPass,

    // misc
    UpdateViewport(CommandUpdateViewport),
    Multisampling,
    VSync,
    TrySwapAndScreenshot,

    // in Android a window that minimizes gets destroyed
    WindowCreateNtf,
    WindowDestroyNtf,
}

#[derive(Debug, Encode, Decode)]
pub enum AllCommands {
    Render(CommandsRender),
    Misc(Commands),
}
