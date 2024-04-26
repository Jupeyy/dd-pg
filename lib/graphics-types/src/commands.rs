use bitflags::bitflags;
use hiarc::Hiarc;
use pool::mt_datatypes::{PoolString, PoolVec};
use serde::{Deserialize, Serialize};

use crate::{
    rendering::{GlColor, GlColorf, GlPoint, SPoint, State, StateTexture},
    types::{GraphicsBackendMemory, ImageFormat},
};
use math::math::vector::*;

// max uniform entries if the size of the uniform entry is default size
// if they are bigger => fewer count, if they are smaller => more
pub const GRAPHICS_MAX_UNIFORM_RENDER_COUNT: usize = 512;
pub const GRAPHICS_DEFAULT_UNIFORM_SIZE: usize = std::mem::size_of::<vec4>();
pub const GRAPHICS_UNIFORM_INSTANCE_COUNT: usize = 128;

pub enum StreamDataMax {
    MaxTextures = 1024 * 8,
    MaxVertices = 32 * 1024,
}

#[derive(Debug, Hiarc, PartialEq, Eq)]
pub enum TexFormat {
    Invalid = 0,
    RGBA,
}

#[derive(
    Debug, Hiarc, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct TexFlags(i32);
bitflags! {
    impl TexFlags: i32 {
        const TEXFLAG_NOMIPMAPS = (1 << 0);
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize, Hiarc)]
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
pub type SColorf = GlColorf;
pub type SColor = GlColor;
/*
type SVertexTex3D = GL_SVertexTex3D;
type SVertexTex3DStream = GL_SVertexTex3DStream; */

#[derive(Debug, Serialize, Deserialize, Hiarc)]
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
#[derive(Debug, Serialize, Deserialize, Hiarc)]
pub struct CommandRender {
    pub state: State,
    pub texture_index: StateTexture,
    pub prim_type: PrimType,
    pub prim_count: usize,
    pub vertices_offset: usize,
}

impl CommandRender {
    pub fn new(prim_type: PrimType, texture_index: StateTexture) -> CommandRender {
        CommandRender {
            state: State::new(),
            texture_index,
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

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct CommandRenderTex3D {
    pub state: State,
    pub texture_index: StateTexture,
    pub prim_type: PrimType,
    pub prim_count: usize,
    pub vertices_offset: usize,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct CommandCreateBufferObject {
    pub buffer_index: u128,

    pub upload_data: GraphicsBackendMemory,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct CommandRecreateBufferObject {
    pub buffer_index: u128,

    pub upload_data: GraphicsBackendMemory,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct CommandUpdateBufferObjectRegion {
    pub src_offset: usize,
    pub dst_offset: usize,
    pub size: usize,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct CommandUpdateBufferObject {
    pub buffer_index: u128,

    pub update_data: Vec<u8>,
    pub update_regions: Vec<CommandUpdateBufferObjectRegion>,
}

#[derive(Debug, Hiarc, Copy, Clone, Default, Serialize, Deserialize)]
pub enum GraphicsType {
    UnsignedByte,
    UnsignedShort,
    Int,
    #[default]
    UnsignedInt,
    Float,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct CommandDeleteBufferObject {
    pub buffer_index: u128,
}

#[derive(Debug, Hiarc, Default, Serialize, Deserialize)]
pub struct CommandIndicesRequiredNumNotify {
    pub required_indices_num: u64,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum CommandSwitchCanvasModeType {
    Onscreen,
    Offscreen {
        id: u64,
        width: u32,
        height: u32,
        has_multi_sampling: Option<u32>,
    },
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct CommandSwitchCanvasMode {
    pub mode: CommandSwitchCanvasModeType,
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

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct CommandRenderQuadContainer {
    pub state: State,
    pub texture_index: StateTexture,

    pub buffer_object_index: u128,

    pub rotation: f32,
    pub center: SPoint,

    pub vertex_color: SColorf,

    pub draw_num: usize,
    pub offset: usize,
}

#[repr(C)]
#[derive(Debug, Hiarc, Clone, Copy, Serialize, Deserialize)]
pub struct RenderSpriteInfo {
    pub pos: vec2,
    pub scale: f32,
    pub rotation: f32,
}

impl RenderSpriteInfo {
    pub fn new(pos: vec2, scale: f32, rotation: f32) -> Self {
        Self {
            pos,
            scale,
            rotation,
        }
    }
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct CommandRenderQuadContainerAsSpriteMultiple {
    pub state: State,
    pub texture_index: StateTexture,

    pub buffer_object_index: u128,

    pub render_info_uniform_instance: usize,

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

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct CommandUpdateViewport {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub by_resize: bool, // resized by an resize event.. a hint to make clear that the viewport update can be deferred if wanted
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
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

#[derive(Debug, Hiarc, Serialize, Deserialize)]
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

#[derive(Debug, Hiarc, Serialize, Deserialize)]
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

pub struct CommandShutdown {}

pub struct CommandPostShutdown {}

#[derive(Debug, Serialize, Deserialize, Hiarc)]
pub enum CommandsRenderStream {
    Render(CommandRender),
    RenderTex3D(CommandRenderTex3D),
    RenderBlurred {
        cmd: CommandRender,
        blur_radius: f32,
        scale: vec2,
        blur_color: vec4,
    },
}

#[derive(Debug, Serialize, Deserialize, Hiarc)]
pub enum CommandsRenderQuadContainer {
    Render(CommandRenderQuadContainer), // render a quad buffer container with extended parameters
    RenderAsSpriteMultiple(CommandRenderQuadContainerAsSpriteMultiple), // render a quad buffer container as sprite multiple times
}

#[derive(Debug, Serialize, Deserialize, Hiarc)]
pub struct CommandsRenderMod {
    pub mod_name: PoolString,
    pub cmd: PoolVec<u8>,
}

#[derive(Debug, Serialize, Deserialize, Hiarc)]
pub enum CommandsRender {
    // rendering
    Clear(CommandClear),

    Stream(CommandsRenderStream),

    QuadContainer(CommandsRenderQuadContainer),

    /// a mod can use this variant
    Mod(CommandsRenderMod),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum Commands {
    // texture commands
    TextureCreate(CommandTextureCreate),
    TextureDestroy(CommandTextureDestroy),
    TextureUpdate(CommandTextureUpdate),

    CreateBufferObject(CommandCreateBufferObject),
    RecreateBufferObject(CommandRecreateBufferObject),
    UpdateBufferObject(CommandUpdateBufferObject),
    DeleteBufferObject(CommandDeleteBufferObject),

    IndicesRequiredNumNotify(CommandIndicesRequiredNumNotify), // create indices that are required

    // swap
    Swap,

    // passes
    NextSwitchPass,
    ConsumeMultiSamplingTargets,

    // canvas
    SwitchCanvas(CommandSwitchCanvasMode),

    // misc
    UpdateViewport(CommandUpdateViewport),
    Multisampling,
    VSync,

    // in Android a window that minimizes gets destroyed
    WindowCreateNtf,
    WindowDestroyNtf,
}

#[derive(Debug, Serialize, Deserialize, Hiarc)]
pub enum AllCommands {
    Render(CommandsRender),
    Misc(Commands),
}
