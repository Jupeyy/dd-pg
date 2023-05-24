use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    num::NonZeroUsize,
    os::raw::c_void,
    str::FromStr,
    sync::{
        atomic::{AtomicU64, AtomicU8},
        Arc, Mutex,
    },
    time::Duration,
};

use graphics_types::{
    command_buffer::{
        AllCommands, Commands, CommandsRender, ERunCommandReturnTypes, PrimType,
        SBackendCapabilites, SCommand_Clear, SCommand_CopyBufferObject,
        SCommand_CreateBufferContainer, SCommand_CreateBufferObject,
        SCommand_DeleteBufferContainer, SCommand_DeleteBufferObject,
        SCommand_IndicesRequiredNumNotify, SCommand_RecreateBufferObject, SCommand_Render,
        SCommand_RenderBorderTile, SCommand_RenderBorderTileLine, SCommand_RenderQuadContainer,
        SCommand_RenderQuadContainerAsSpriteMultiple, SCommand_RenderQuadContainerEx,
        SCommand_RenderQuadLayer, SCommand_RenderTileLayer, SCommand_Texture_Create,
        SCommand_Texture_Destroy, SCommand_Texture_Update, SCommand_UpdateBufferContainer,
        SCommand_UpdateBufferObject, SCommand_Update_Viewport, SQuadRenderInfo, SRenderSpriteInfo,
        StreamDataMax, TexFlags, GRAPHICS_MAX_PARTICLES_RENDER_COUNT,
        GRAPHICS_MAX_QUADS_RENDER_COUNT,
    },
    rendering::{BlendType, ColorRGBA, ETextureIndex, GL_SColorf, GL_SVertex, State, WrapType},
    types::GraphicsMemoryAllocationType,
};
use num_traits::FromPrimitive;

use arrayvec::ArrayString;
use ash::vk::{self, Handle};

use crate::{
    backend::BackendBuffer,
    backends::{GraphicsBackendInterface, GraphicsBackendMtInterface},
    image::Resize,
};

use base::config::EDebugGFXModes;
use base::system::{self, SystemLogInterface};
use math::math::vector::{vec2, vec4};

const gs_BackendVulkanMajor: usize = 1;
const gs_BackendVulkanMinor: usize = 1;
const gs_BackendVulkanPatch: usize = 1;

const shader_main_func_name: [u8; 5] = ['m' as u8, 'a' as u8, 'i' as u8, 'n' as u8, '\0' as u8];
const app_name: [u8; 6] = [
    'D' as u8, 'D' as u8, 'N' as u8, 'e' as u8, 't' as u8, '\0' as u8,
];
const app_vk_name: [u8; 13] = [
    'D' as u8, 'D' as u8, 'N' as u8, 'e' as u8, 't' as u8, '-' as u8, 'V' as u8, 'u' as u8,
    'l' as u8, 'k' as u8, 'a' as u8, 'n' as u8, '\0' as u8,
];
use super::{
    common::{
        image_mip_level_count, tex_format_to_image_color_channel_count,
        texture_format_to_vulkan_format, vulkan_format_to_image_color_channel_count, EGFXErrorType,
        ETWGraphicsGPUType, STWGraphicGPUItem, TTWGraphicsGPUList,
    },
    vulkan_allocator::{
        VulkanAllocator, THREADED_STAGING_BUFFER_CACHE_ID, THREADED_STAGING_BUFFER_IMAGE_CACHE_ID,
    },
    vulkan_dbg::{is_verbose, is_verbose_mode},
    vulkan_device::Device,
    vulkan_error::{CheckResult, Error},
    vulkan_limits::Limits,
    vulkan_mem::Memory,
    vulkan_types::{
        CTexture, ESupportedSamplerTypes, EVulkanBackendAddressModes, EVulkanBackendBlendModes,
        EVulkanBackendClipModes, EVulkanBackendTextureModes, SDelayedBufferCleanupItem,
        SDeviceDescriptorSet, SDeviceMemoryBlock, SFrameBuffers, SFrameUniformBuffers,
        SMemoryBlock, SMemoryImageBlock, SPipelineContainer, SRenderCommandExecuteBuffer,
        SRenderThread, SShaderFileCache, SShaderModule, SSwapChainMultiSampleImage,
        SSwapImgViewportExtent, StreamMemory, IMAGE_BUFFER_CACHE_ID, STAGING_BUFFER_CACHE_ID,
        STAGING_BUFFER_IMAGE_CACHE_ID,
    },
    vulkan_uniform::{
        SUniformGPos, SUniformGTextPos, SUniformPrimExGPos, SUniformPrimExGPosRotationless,
        SUniformPrimExGVertColor, SUniformPrimExGVertColorAlign, SUniformQuadGPos,
        SUniformQuadPushGBufferObject, SUniformQuadPushGPos, SUniformSpriteMultiGPos,
        SUniformSpriteMultiGVertColor, SUniformSpriteMultiGVertColorAlign,
        SUniformSpriteMultiPushGPos, SUniformSpriteMultiPushGPosBase,
        SUniformSpriteMultiPushGVertColor, SUniformTextGFragmentConstants,
        SUniformTextGFragmentOffset, SUniformTileGPos, SUniformTileGPosBorder,
        SUniformTileGPosBorderLine, SUniformTileGVertColor, SUniformTileGVertColorAlign,
    },
    Options,
};

type TCommandList = Vec<SRenderCommandExecuteBuffer>;
type TThreadCommandList = Vec<TCommandList>;

pub struct VulkanBackend {
    /************************
     * MEMBER VARIABLES
     ************************/
    dbg: Arc<AtomicU8>, // @see EDebugGFXModes
    gfx_vsync: bool,

    shader_files: HashMap<String, SShaderFileCache>,

    texture_memory_usage: Arc<AtomicU64>,
    buffer_memory_usage: Arc<AtomicU64>,
    stream_memory_usage: Arc<AtomicU64>,
    staging_memory_usage: Arc<AtomicU64>,

    // TODO: m_pGPUList: TTWGraphicsGPUList,
    next_multi_sampling_count: u32,

    recreate_swap_chain: bool,
    swap_chain_created: bool,
    rendering_paused: bool,
    has_dynamic_viewport: bool,
    dynamic_viewport_offset: vk::Offset2D,

    dynamic_viewport_size: vk::Extent2D,

    index_buffer: vk::Buffer,
    index_buffer_memory: SDeviceMemoryBlock,

    render_index_buffer: vk::Buffer,
    render_index_buffer_memory: SDeviceMemoryBlock,
    cur_render_index_primitive_count: usize,

    screenshot_helper: Vec<u8>,

    get_presented_img_data_helper_mem: SDeviceMemoryBlock,
    get_presented_img_data_helper_image: vk::Image,
    get_presented_img_data_helper_mapped_memory: *mut u8,
    get_presented_img_data_helper_mapped_layout_offset: vk::DeviceSize,
    get_presented_img_data_helper_mapped_layout_pitch: vk::DeviceSize,
    get_presented_img_data_helper_width: u32,
    get_presented_img_data_helper_height: u32,
    get_presented_img_data_helper_fence: vk::Fence,

    thread_count: usize,
    cur_command_in_pipe: usize,
    cur_render_call_count_in_pipe: usize,
    commands_in_pipe: usize,
    render_calls_in_pipe: usize,
    last_commands_in_pipe_thread_index: usize,

    render_threads: Vec<Arc<(Mutex<SRenderThread>, std::sync::Condvar)>>,

    swap_chain_image_view_list: Vec<vk::ImageView>,
    swap_chain_multi_sampling_images: Vec<SSwapChainMultiSampleImage>,
    framebuffer_list: Vec<vk::Framebuffer>,
    main_draw_command_buffers: Vec<vk::CommandBuffer>,

    thread_draw_command_buffers: Vec<Vec<vk::CommandBuffer>>,
    helper_thread_draw_command_buffers: Vec<vk::CommandBuffer>,
    used_thread_draw_command_buffer: Vec<Vec<bool>>,

    // swapped by use case
    wait_semaphores: Vec<vk::Semaphore>,
    sig_semaphores: Vec<vk::Semaphore>,

    memory_sempahores: Vec<vk::Semaphore>,

    frame_fences: Vec<vk::Fence>,
    image_fences: Vec<vk::Fence>,

    cur_frame: u64,
    image_last_frame_check: Vec<u64>,

    last_presented_swap_chain_image_index: u32,

    vk_instance: ash::Instance,
    vk_entry: ash::Entry,
    surface: ash::extensions::khr::Surface,
    vk_gpu: vk::PhysicalDevice,
    vk_graphics_queue_index: u32,
    device: Device,
    vk_device: ash::Device,
    vk_graphics_queue: vk::Queue,
    vk_present_queue: vk::Queue,
    vk_present_surface: vk::SurfaceKHR,
    vk_swap_img_and_viewport_extent: SSwapImgViewportExtent,

    debug_messenger: vk::DebugUtilsMessengerEXT,

    standard_pipeline: SPipelineContainer,
    standard_line_pipeline: SPipelineContainer,
    standard_3d_pipeline: SPipelineContainer,
    text_pipeline: SPipelineContainer,
    tile_pipeline: SPipelineContainer,
    tile_border_pipeline: SPipelineContainer,
    tile_border_line_pipeline: SPipelineContainer,
    prim_ex_pipeline: SPipelineContainer,
    prim_ex_rotationless_pipeline: SPipelineContainer,
    sprite_multi_pipeline: SPipelineContainer,
    sprite_multi_push_pipeline: SPipelineContainer,
    quad_pipeline: SPipelineContainer,
    quad_push_pipeline: SPipelineContainer,

    last_pipeline_per_thread: Vec<vk::Pipeline>,

    command_pools: Vec<vk::CommandPool>,

    vk_render_pass: vk::RenderPass,

    vk_surf_format: vk::SurfaceFormatKHR,

    vk_swap_chain_ash: ash::extensions::khr::Swapchain,
    vk_swap_chain_khr: vk::SwapchainKHR,
    vk_swap_chain_images: Vec<vk::Image>,

    cur_frames: u32,
    cur_image_index: u32,

    canvas_width: u32,
    canvas_height: u32,

    // TODO! m_pWindow: sdl2::video::Window,
    clear_color: [f32; 4],

    thread_command_lists: TThreadCommandList,
    thread_helper_had_commands: Vec<bool>,

    //m_aCommandCallbacks: [SCommandCallback; 1],

    /************************
     * ERROR MANAGEMENT
     ************************/
    error: Arc<std::sync::Mutex<Error>>,
    check_res: CheckResult,

    sys: system::System,

    runtime_threadpool: Arc<rayon::ThreadPool>,
}

impl VulkanBackend {
    // TODO fn ErroneousCleanup(&mut self )  { self.CleanupVulkanSDL(); }

    /************************
     * COMMAND CALLBACKS
     ************************/

    // TODO fn  CommandBufferCMDOff(CCommandBuffer::ECommandBufferCMD CommandBufferCMD) -> usize {
    // TODO return (usize)CommandBufferCMD - CCommandBuffer::ECommandBufferCMD::CMD_FIRST; }

    fn CommandCB(
        &mut self,
        cmd_param: &AllCommands,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        match &cmd_param {
            AllCommands::Render(render_cmd) => match render_cmd {
                CommandsRender::CMD_CLEAR(cmd) => {
                    return self.Cmd_Clear(exec_buffer, cmd);
                }
                CommandsRender::CMD_RENDER(cmd) => {
                    return self.Cmd_Render(cmd, exec_buffer);
                }
                CommandsRender::CMD_RENDER_TEX3D => {}
                CommandsRender::CMD_RENDER_TILE_LAYER(cmd) => {
                    return self.Cmd_RenderTileLayer(cmd, exec_buffer);
                }
                CommandsRender::CMD_RENDER_BORDER_TILE(cmd) => {
                    return self.Cmd_RenderBorderTile(cmd, exec_buffer);
                }
                CommandsRender::CMD_RENDER_BORDER_TILE_LINE(cmd) => {
                    return self.Cmd_RenderBorderTileLine(cmd, exec_buffer);
                }
                CommandsRender::CMD_RENDER_QUAD_LAYER(cmd) => {
                    return self.Cmd_RenderQuadLayer(cmd, exec_buffer);
                }
                CommandsRender::CMD_RENDER_TEXT => {}
                CommandsRender::CMD_RENDER_QUAD_CONTAINER(cmd) => {
                    return self.Cmd_RenderQuadContainer(cmd, exec_buffer);
                }
                CommandsRender::CMD_RENDER_QUAD_CONTAINER_EX(cmd) => {
                    return self.Cmd_RenderQuadContainerEx(cmd, exec_buffer);
                }
                CommandsRender::CMD_RENDER_QUAD_CONTAINER_SPRITE_MULTIPLE(cmd) => {
                    return self.Cmd_RenderQuadContainerAsSpriteMultiple(cmd, exec_buffer);
                }
            },
            AllCommands::Misc(misc_cmd) => match misc_cmd {
                Commands::CMD_TEXTURE_CREATE(cmd) => {
                    return self.Cmd_Texture_Create(cmd);
                }
                Commands::CMD_TEXTURE_DESTROY(cmd) => {
                    return self.Cmd_Texture_Destroy(cmd);
                }
                Commands::CMD_TEXTURE_UPDATE(cmd) => {
                    return self.Cmd_Texture_Update(cmd);
                }
                Commands::CMD_TEXT_TEXTURES_CREATE => {}
                Commands::CMD_TEXT_TEXTURES_DESTROY => {}
                Commands::CMD_TEXT_TEXTURE_UPDATE => {}
                Commands::CMD_CREATE_BUFFER_OBJECT(cmd) => {
                    return self.Cmd_CreateBufferObject(cmd);
                }
                Commands::CMD_RECREATE_BUFFER_OBJECT(cmd) => {
                    return self.Cmd_RecreateBufferObject(cmd);
                }
                Commands::CMD_UPDATE_BUFFER_OBJECT(cmd) => {
                    return self.Cmd_UpdateBufferObject(cmd);
                }
                Commands::CMD_COPY_BUFFER_OBJECT(cmd) => {
                    return self.Cmd_CopyBufferObject(cmd);
                }
                Commands::CMD_DELETE_BUFFER_OBJECT(cmd) => {
                    return self.Cmd_DeleteBufferObject(cmd);
                }
                Commands::CMD_CREATE_BUFFER_CONTAINER(cmd) => {
                    return self.Cmd_CreateBufferContainer(cmd);
                }
                Commands::CMD_DELETE_BUFFER_CONTAINER(cmd) => {
                    return self.Cmd_DeleteBufferContainer(cmd);
                }
                Commands::CMD_UPDATE_BUFFER_CONTAINER(cmd) => {
                    return self.Cmd_UpdateBufferContainer(cmd);
                }
                Commands::CMD_INDICES_REQUIRED_NUM_NOTIFY(cmd) => {
                    return self.Cmd_IndicesRequiredNumNotify(cmd);
                }
                Commands::CMD_SWAP(_) => return self.Cmd_Swap(),
                Commands::CMD_UPDATE_VIEWPORT(cmd) => return self.Cmd_Update_Viewport(cmd),
                Commands::CMD_MULTISAMPLING => {}
                Commands::CMD_VSYNC => {}
                Commands::CMD_TRY_SWAP_AND_SCREENSHOT => {}
                Commands::CMD_WINDOW_CREATE_NTF => {}
                Commands::CMD_WINDOW_DESTROY_NTF => {}
                _ => todo!(),
            },
            AllCommands::None => {}
        }

        return true;
    }

    fn FillExecuteBuffer(
        &mut self,
        cmd: &AllCommands,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
    ) {
        match &cmd {
            AllCommands::Render(render_cmd) => match render_cmd {
                CommandsRender::CMD_CLEAR(cmd) => {
                    self.Cmd_Clear_FillExecuteBuffer(exec_buffer, cmd)
                }
                CommandsRender::CMD_RENDER(SCommand_Render) => {
                    self.Cmd_Render_FillExecuteBuffer(exec_buffer, SCommand_Render)
                }
                CommandsRender::CMD_RENDER_TEX3D => {}
                CommandsRender::CMD_RENDER_TILE_LAYER(cmd) => {
                    self.Cmd_RenderTileLayer_FillExecuteBuffer(exec_buffer, cmd)
                }
                CommandsRender::CMD_RENDER_BORDER_TILE(cmd) => {
                    self.Cmd_RenderBorderTile_FillExecuteBuffer(exec_buffer, cmd)
                }
                CommandsRender::CMD_RENDER_BORDER_TILE_LINE(cmd) => {
                    self.Cmd_RenderBorderTileLine_FillExecuteBuffer(exec_buffer, cmd)
                }
                CommandsRender::CMD_RENDER_QUAD_LAYER(cmd) => {
                    self.Cmd_RenderQuadLayer_FillExecuteBuffer(exec_buffer, cmd)
                }
                CommandsRender::CMD_RENDER_TEXT => {}
                CommandsRender::CMD_RENDER_QUAD_CONTAINER(cmd) => {
                    self.Cmd_RenderQuadContainer_FillExecuteBuffer(exec_buffer, cmd)
                }
                CommandsRender::CMD_RENDER_QUAD_CONTAINER_EX(cmd) => {
                    self.Cmd_RenderQuadContainerEx_FillExecuteBuffer(exec_buffer, cmd)
                }
                CommandsRender::CMD_RENDER_QUAD_CONTAINER_SPRITE_MULTIPLE(cmd) => {
                    self.Cmd_RenderQuadContainerAsSpriteMultiple_FillExecuteBuffer(exec_buffer, cmd)
                }
            },
            AllCommands::Misc(misc_cmd) => match misc_cmd {
                Commands::CMD_TEXTURE_CREATE(_cmd) => {}
                Commands::CMD_TEXTURE_DESTROY(_) => {}
                Commands::CMD_TEXTURE_UPDATE(_) => {}
                Commands::CMD_TEXT_TEXTURES_CREATE => {}
                Commands::CMD_TEXT_TEXTURES_DESTROY => {}
                Commands::CMD_TEXT_TEXTURE_UPDATE => {}
                Commands::CMD_CREATE_BUFFER_OBJECT(_cmd) => {}
                Commands::CMD_RECREATE_BUFFER_OBJECT(_cmd) => {}
                Commands::CMD_UPDATE_BUFFER_OBJECT(_cmd) => {}
                Commands::CMD_COPY_BUFFER_OBJECT(_cmd) => {}
                Commands::CMD_DELETE_BUFFER_OBJECT(_cmd) => {}
                Commands::CMD_CREATE_BUFFER_CONTAINER(_cmd) => {}
                Commands::CMD_DELETE_BUFFER_CONTAINER(_cmd) => {}
                Commands::CMD_UPDATE_BUFFER_CONTAINER(_cmd) => {}
                Commands::CMD_INDICES_REQUIRED_NUM_NOTIFY(_cmd) => {}
                Commands::CMD_SWAP(_swap_cmd) => {}
                Commands::CMD_UPDATE_VIEWPORT(cmd) => {
                    self.Cmd_Update_Viewport_FillExecuteBuffer(exec_buffer, cmd);
                }
                Commands::CMD_MULTISAMPLING => {}
                Commands::CMD_VSYNC => {}
                Commands::CMD_TRY_SWAP_AND_SCREENSHOT => {}
                Commands::CMD_WINDOW_CREATE_NTF => {}
                Commands::CMD_WINDOW_DESTROY_NTF => {}
                _ => todo!(),
            },
            AllCommands::None => {}
        }

        /*  self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_TEXTURE_DESTROY)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_Texture_Destroy((pBaseCommand) as const CCommandBuffer::SCommand_Texture_Destroy *); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_TEXTURE_UPDATE)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_Texture_Update((pBaseCommand) as const CCommandBuffer::SCommand_Texture_Update *); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_TEXT_TEXTURES_CREATE)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_TextTextures_Create((pBaseCommand) as const CCommandBuffer::SCommand_TextTextures_Create *); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_TEXT_TEXTURES_DESTROY)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_TextTextures_Destroy((pBaseCommand) as const CCommandBuffer::SCommand_TextTextures_Destroy *); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_TEXT_TEXTURE_UPDATE)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_TextTexture_Update((pBaseCommand) as const CCommandBuffer::SCommand_TextTexture_Update *); }};

                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_CREATE_BUFFER_OBJECT)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_CreateBufferObject((pBaseCommand) as const CCommandBuffer::SCommand_CreateBufferObject *); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_RECREATE_BUFFER_OBJECT)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_RecreateBufferObject((pBaseCommand) as const CCommandBuffer::SCommand_RecreateBufferObject *); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_UPDATE_BUFFER_OBJECT)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_UpdateBufferObject((pBaseCommand) as const CCommandBuffer::SCommand_UpdateBufferObject *); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_COPY_BUFFER_OBJECT)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_CopyBufferObject((pBaseCommand) as const CCommandBuffer::SCommand_CopyBufferObject *); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_DELETE_BUFFER_OBJECT)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_DeleteBufferObject((pBaseCommand) as const CCommandBuffer::SCommand_DeleteBufferObject *); }};

                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_CREATE_BUFFER_CONTAINER)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_CreateBufferContainer((pBaseCommand) as const CCommandBuffer::SCommand_CreateBufferContainer *); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_DELETE_BUFFER_CONTAINER)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_DeleteBufferContainer((pBaseCommand) as const CCommandBuffer::SCommand_DeleteBufferContainer *); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_UPDATE_BUFFER_CONTAINER)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_UpdateBufferContainer((pBaseCommand) as const CCommandBuffer::SCommand_UpdateBufferContainer *); }};

                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_INDICES_REQUIRED_NUM_NOTIFY)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_IndicesRequiredNumNotify((pBaseCommand) as const CCommandBuffer::SCommand_IndicesRequiredNumNotify *); }};

                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_SWAP)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_Swap((pBaseCommand) as const CCommandBuffer::SCommand_Swap *); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_FINISH)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_Finish((pBaseCommand) as const CCommandBuffer::SCommand_Finish *); }};

                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_VSYNC)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_VSync((pBaseCommand) as const CCommandBuffer::SCommand_VSync *); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_MULTISAMPLING)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_MultiSampling((pBaseCommand) as const CCommandBuffer::SCommand_MultiSampling *); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_TRY_SWAP_AND_SCREENSHOT)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_Screenshot((pBaseCommand) as const CCommandBuffer::SCommand_TrySwapAndScreenshot *); }};

                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_WINDOW_CREATE_NTF)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_WindowCreateNtf((pBaseCommand) as const CCommandBuffer::SCommand_WindowCreateNtf *); }, false};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_WINDOW_DESTROY_NTF)] = {false, [](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) {}, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_WindowDestroyNtf((pBaseCommand) as const CCommandBuffer::SCommand_WindowDestroyNtf *); }, false};

                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_UPDATE_VIEWPORT)] = {false, [this](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) { Cmd_Update_Viewport_FillExecuteBuffer(exec_buffer, (pBaseCommand) as const CCommandBuffer::SCommand_Update_Viewport *>(pBaseCommand)); }, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_Update_Viewport(static_cast<const CCommandBuffer::SCommand_Update_Viewport *); }};

                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_CLEAR)] = {true, [this](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) { Cmd_Clear_FillExecuteBuffer(exec_buffer, (pBaseCommand) as const CCommandBuffer::SCommand_Clear *>(pBaseCommand)); }, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_Clear(exec_buffer, static_cast<const CCommandBuffer::SCommand_Clear *); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_RENDER)] = {true, [this](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) { Cmd_Render_FillExecuteBuffer(exec_buffer, (pBaseCommand) as const CCommandBuffer::SCommand_Render *>(pBaseCommand)); }, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_Render(static_cast<const CCommandBuffer::SCommand_Render *, exec_buffer); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_RENDER_TEX3D)] = {true, [this](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) { Cmd_RenderTex3D_FillExecuteBuffer(exec_buffer, (pBaseCommand) as const CCommandBuffer::SCommand_RenderTex3D *>(pBaseCommand)); }, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_RenderTex3D(static_cast<const CCommandBuffer::SCommand_RenderTex3D *, exec_buffer); }};

                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_RENDER_TILE_LAYER)] = {true, [this](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) { Cmd_RenderTileLayer_FillExecuteBuffer(exec_buffer, (pBaseCommand) as const CCommandBuffer::SCommand_RenderTileLayer *>(pBaseCommand)); }, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_RenderTileLayer(static_cast<const CCommandBuffer::SCommand_RenderTileLayer *, exec_buffer); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_RENDER_BORDER_TILE)] = {true, [this](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) { Cmd_RenderBorderTile_FillExecuteBuffer(exec_buffer, (pBaseCommand) as const CCommandBuffer::SCommand_RenderBorderTile *>(pBaseCommand)); }, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_RenderBorderTile(static_cast<const CCommandBuffer::SCommand_RenderBorderTile *, exec_buffer); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_RENDER_BORDER_TILE_LINE)] = {true, [this](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) { Cmd_RenderBorderTileLine_FillExecuteBuffer(exec_buffer, (pBaseCommand) as const CCommandBuffer::SCommand_RenderBorderTileLine *>(pBaseCommand)); }, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_RenderBorderTileLine(static_cast<const CCommandBuffer::SCommand_RenderBorderTileLine *, exec_buffer); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_RENDER_QUAD_LAYER)] = {true, [this](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) { Cmd_RenderQuadLayer_FillExecuteBuffer(exec_buffer, (pBaseCommand) as const CCommandBuffer::SCommand_RenderQuadLayer *>(pBaseCommand)); }, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_RenderQuadLayer(static_cast<const CCommandBuffer::SCommand_RenderQuadLayer *, exec_buffer); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_RENDER_TEXT)] = {true, [this](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) { Cmd_RenderText_FillExecuteBuffer(exec_buffer, (pBaseCommand) as const CCommandBuffer::SCommand_RenderText *>(pBaseCommand)); }, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_RenderText(static_cast<const CCommandBuffer::SCommand_RenderText *, exec_buffer); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_RENDER_QUAD_CONTAINER)] = {true, [this](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) { Cmd_RenderQuadContainer_FillExecuteBuffer(exec_buffer, (pBaseCommand) as const CCommandBuffer::SCommand_RenderQuadContainer *>(pBaseCommand)); }, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_RenderQuadContainer(static_cast<const CCommandBuffer::SCommand_RenderQuadContainer *, exec_buffer); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_RENDER_QUAD_CONTAINER_EX)] = {true, [this](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) { Cmd_RenderQuadContainerEx_FillExecuteBuffer(exec_buffer, (pBaseCommand) as const CCommandBuffer::SCommand_RenderQuadContainerEx *>(pBaseCommand)); }, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_RenderQuadContainerEx(static_cast<const CCommandBuffer::SCommand_RenderQuadContainerEx *, exec_buffer); }};
                    self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::CMD_RENDER_QUAD_CONTAINER_SPRITE_MULTIPLE)] = {true, [this](exec_buffer: &mut SRenderCommandExecuteBuffer, const void *pBaseCommand) { Cmd_RenderQuadContainerAsSpriteMultiple_FillExecuteBuffer(exec_buffer, (pBaseCommand) as const CCommandBuffer::SCommand_RenderQuadContainerAsSpriteMultiple *>(pBaseCommand)); }, [this](const void *pBaseCommand, exec_buffer: &SRenderCommandExecuteBuffer ) { return Cmd_RenderQuadContainerAsSpriteMultiple(static_cast<const CCommandBuffer::SCommand_RenderQuadContainerAsSpriteMultiple *, exec_buffer); }};
        */
    }

    /*
                      /*****************************
                       * VIDEO AND SCREENSHOT HELPER
                       ******************************/

                      #[must_use] fn PreparePresentedImageDataImage(&mut self,u8 *&pResImageData, Width: u32, u32 Height) -> bool
                      {
                          let NeedsNewImg: bool = Width != self.m_GetPresentedImgDataHelperWidth || Height != self.m_GetPresentedImgDataHelperHeight;
                          if(self.m_GetPresentedImgDataHelperImage == vk::NULL_HANDLE || NeedsNewImg)
                          {
                              if(self.m_GetPresentedImgDataHelperImage != vk::NULL_HANDLE)
                              {
                                  DeletePresentedImageDataImage();
                              }
                              self.m_GetPresentedImgDataHelperWidth = Width;
                              self.m_GetPresentedImgDataHelperHeight = Height;

                             let mut ImageInfo =  vk::ImageCreateInfo::default();
                              ImageInfo.imageType = vk::ImageType::TYPE_2D;
                              ImageInfo.extent.width = Width;
                              ImageInfo.extent.height = Height;
                              ImageInfo.extent.depth = 1;
                              ImageInfo.mipLevels = 1;
                              ImageInfo.arrayLayers = 1;
                              ImageInfo.format = vk::Format::R8G8B8A8_UNORM;
                              ImageInfo.tiling = vk::IMAGE_TILING_LINEAR;
                              ImageInfo.initialLayout = vk::ImageLayout::UNDEFINED;
                              ImageInfo.usage = vk::ImageUsageFlags::TRANSFER_DST;
                              ImageInfo.samples = vk::SampleCountFlags::TYPE_1;
                              ImageInfo.sharingMode = vk::SharingMode::EXCLUSIVE;

                              unsafe {self.m_VKDevice.CreateImage( &ImageInfo, std::ptr::null(), &m_GetPresentedImgDataHelperImage);}
                              // Create memory to back up the image
                              vk::MemoryRequirements MemRequirements;
                              unsafe {self.m_VKDevice.GetImageMemoryRequirements( self.m_GetPresentedImgDataHelperImage, &MemRequirements);}

                              let mut  MemAllocInfo = vk::MemoryAllocateInfo::default();
                              MemAllocInfo.allocationSize = MemRequirements.size;
                              MemAllocInfo.memoryTypeIndex = FindMemoryType(self.m_VKGPU, MemRequirements.memory_type_bits, vk::MEMORY_PROPERTY_HOST_VISIBLE_BIT | vk::MEMORY_PROPERTY_HOST_CACHED_BIT);

                              unsafe {self.m_VKDevice.AllocateMemory( &MemAllocInfo, std::ptr::null(), &m_GetPresentedImgDataHelperMem.m_Mem);}
                              unsafe {self.m_VKDevice.BindImageMemory( self.m_GetPresentedImgDataHelperImage, self.m_GetPresentedImgDataHelperMem.m_Mem, 0);}

                              if(!ImageBarrier(self.m_GetPresentedImgDataHelperImage, 0, 1, 0, 1, vk::Format::R8G8B8A8_UNORM, vk::ImageLayout::UNDEFINED, vk::ImageLayout::GENERAL))
                                  return false;

                              vk::ImageSubresource SubResource{vk::ImageAspectFlags::COLOR, 0, 0};
                              vk::SubresourceLayout SubResourceLayout;
                              unsafe {self.m_VKDevice.GetImageSubresourceLayout( self.m_GetPresentedImgDataHelperImage, &SubResource, &SubResourceLayout);}

                              self.m_VKDevice.map_memory( self.m_GetPresentedImgDataHelperMem.m_Mem, 0, vk::WHOLE_SIZE, 0, (void **)&m_pGetPresentedImgDataHelperMappedMemory);
                              self.m_GetPresentedImgDataHelperMappedLayoutOffset = SubResourceLayout.offset;
                              self.m_GetPresentedImgDataHelperMappedLayoutPitch = SubResourceLayout.rowPitch;
                              self.m_pGetPresentedImgDataHelperMappedMemory += self.m_GetPresentedImgDataHelperMappedLayoutOffset;

                              let mut FenceInfo = vk::FenceCreateInfo ::default();
                              FenceInfo.flags = vk::FENCE_CREATE_SIGNALED_BIT;
                              unsafe {self.m_VKDevice.CreateFence( &FenceInfo, std::ptr::null(), &m_GetPresentedImgDataHelperFence);}
                          }
                          pResImageData = self.m_pGetPresentedImgDataHelperMappedMemory;
                          return true;
                      }

                      fn DeletePresentedImageDataImage(&mut self )
                      {
                          if(self.m_GetPresentedImgDataHelperImage != vk::NULL_HANDLE)
                          {
                              unsafe {self.m_VKDevice.DestroyFence( self.m_GetPresentedImgDataHelperFence, std::ptr::null());}

                              self.m_GetPresentedImgDataHelperFence = vk::NULL_HANDLE;

                              unsafe {self.m_VKDevice.DestroyImage( self.m_GetPresentedImgDataHelperImage, std::ptr::null());}
                              unsafe {self.m_VKDevice.UnmapMemory( self.m_GetPresentedImgDataHelperMem.m_Mem);}
                              unsafe {self.m_VKDevice.FreeMemory( self.m_GetPresentedImgDataHelperMem.m_Mem, std::ptr::null());}

                              self.m_GetPresentedImgDataHelperImage = vk::NULL_HANDLE;
                              self.m_GetPresentedImgDataHelperMem = {};
                              self.m_pGetPresentedImgDataHelperMappedMemory = std::ptr::null();

                              self.m_GetPresentedImgDataHelperWidth = 0;
                              self.m_GetPresentedImgDataHelperHeight = 0;
                          }
                      }

                      #[must_use] fn GetPresentedImageDataImpl(&mut self,u32 &Width, u32 &Height, u32 &Format, Vec<u8> &vDstData, FlipImgData: bool, bool ResetAlpha) -> bool
                      {
                          let IsB8G8R8A8: bool = self.m_VKSurfFormat.format == vk::Format::B8G8R8A8_UNORM;
                          let UsesRGBALikeFormat: bool = self.m_VKSurfFormat.format == vk::Format::R8G8B8A8_UNORM || IsB8G8R8A8;
                          if(UsesRGBALikeFormat && self.m_LastPresentedSwapChainImageIndex != std::numeric_limits<decltype(self.m_LastPresentedSwapChainImageIndex)>::max())
                          {
                              let Viewport = self.m_VKSwapImgAndViewportExtent.GetPresentedImageViewport();
                              Width = Viewport.width;
                              Height = Viewport.height;
                              Format = CImageInfo::FORMAT_RGBA;

                              let ImageTotalSize: usize = (usize)Width * Height * 4;

                              u8 *pResImageData;
                              if(!PreparePresentedImageDataImage(pResImageData, Width, Height))
                                  return false;

                              vk::CommandBuffer *command_buffer_ptr;
                              if(!GetMemoryCommandBuffer(command_buffer_ptr))
                                  return false;
                              vk::CommandBuffer &CommandBuffer = *command_buffer_ptr;

                              vk::BufferImageCopy Region{};
                              Region.buffer_offset = 0;
                              Region.buffer_row_length = 0;
                              Region.buffer_image_height = 0;
                              Region.image_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
                              Region.image_subresource.mip_level = 0;
                              Region.image_subresource.base_array_layer = 0;
                              Region.image_subresource.layer_count = 1;
                              Region.image_offset = {0, 0, 0};
                              Region.image_extent = {Viewport.width, Viewport.height, 1};

                              let SwapImg = &mut  self.m_vSwapChainImages[m_LastPresentedSwapChainImageIndex];

                              if(!ImageBarrier(self.m_GetPresentedImgDataHelperImage, 0, 1, 0, 1, vk::Format::R8G8B8A8_UNORM, vk::ImageLayout::GENERAL, vk::ImageLayout::TRANSFER_DST_OPTIMAL))
                                  return false;
                              if(!ImageBarrier(SwapImg, 0, 1, 0, 1, self.m_VKSurfFormat.format, vk::ImageLayout::PRESENT_SRC_KHR, vk::ImageLayout::TRANSFER_SRC_OPTIMAL))
                                  return false;

                              // If source and destination support blit we'll blit as this also does
                              // automatic format conversion (e.g. from BGR to RGB)
                              if(self.m_OptimalSwapChainImageBlitting && self.device.m_LinearRGBAImageBlitting)
                              {
                                  vk::Offset3D BlitSize;
                                  BlitSize.x = Width;
                                  BlitSize.y = Height;
                                  BlitSize.z = 1;
                                  vk::ImageBlit ImageBlitRegion{};
                                  ImageBlitRegion.src_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
                                  ImageBlitRegion.src_subresource.layer_count = 1;
                                  ImageBlitRegion.src_offsets[1] = BlitSize;
                                  ImageBlitRegion.dst_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
                                  ImageBlitRegion.dst_subresource.layer_count = 1;
                                  ImageBlitRegion.dst_offsets[1] = BlitSize;

                                  // Issue the blit command
                                  unsafe {self.m_VKDevice.CmdBlitImage(CommandBuffer, SwapImg, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, self.m_GetPresentedImgDataHelperImage, vk::ImageLayout::TRANSFER_DST_OPTIMAL, 1, &ImageBlitRegion, vk::Filter::NEAREST);}

                                  // transformed to RGBA
                                  IsB8G8R8A8 = false;
                              }
                              else
                              {
                                  // Otherwise use image copy (requires us to manually flip components)
                                  vk::ImageCopy ImageCopyRegion{};
                                  ImageCopyRegion.src_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
                                  ImageCopyRegion.src_subresource.layer_count = 1;
                                  ImageCopyRegion.dst_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
                                  ImageCopyRegion.dst_subresource.layer_count = 1;
                                  ImageCopyRegion.extent.width = Width;
                                  ImageCopyRegion.extent.height = Height;
                                  ImageCopyRegion.extent.depth = 1;

                                  // Issue the copy command
                                  unsafe {self.m_VKDevice.CmdCopyImage(CommandBuffer, SwapImg, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, self.m_GetPresentedImgDataHelperImage, vk::ImageLayout::TRANSFER_DST_OPTIMAL, 1, &ImageCopyRegion);}
                              }

                              if(!ImageBarrier(self.m_GetPresentedImgDataHelperImage, 0, 1, 0, 1, vk::Format::R8G8B8A8_UNORM, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::GENERAL))
                                  return false;
                              if(!ImageBarrier(SwapImg, 0, 1, 0, 1, self.m_VKSurfFormat.format, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::PRESENT_SRC_KHR))
                                  return false;

                              unsafe { self.m_VKDevice.end_command_buffer(CommandBuffer);}
                              self.device.m_vUsedMemoryCommandBuffer[self.m_CurImageIndex as usize] = false;

                              let mut  SubmitInfo = vk::SubmitInfo::default();

                              SubmitInfo.command_buffer_count = 1;
                              SubmitInfo.p_command_buffers = &CommandBuffer;

                              unsafe {self.m_VKDevice.ResetFences( 1, &m_GetPresentedImgDataHelperFence);}
                              vkQueueSubmit(self.m_VKGraphicsQueue, 1, &SubmitInfo, self.m_GetPresentedImgDataHelperFence);
                              unsafe {self.m_VKDevice.WaitForFences( 1, &m_GetPresentedImgDataHelperFence, vk::TRUE, u64::MAX);}

                            let mut  MemRange =   vk::MappedMemoryRange::default();
                              MemRange.memory = self.m_GetPresentedImgDataHelperMem.m_Mem;
                              MemRange.offset = self.m_GetPresentedImgDataHelperMappedLayoutOffset;
                              MemRange.size = vk::WHOLE_SIZE;
                              unsafe {self.m_VKDevice.InvalidateMappedMemoryRanges( 1, &MemRange);}

                              let RealFullImageSize: usize = std::max(ImageTotalSize, (usize)(Height * self.m_GetPresentedImgDataHelperMappedLayoutPitch));
                              if(vDstData.len() < RealFullImageSize + (Width * 4))
                                  vDstData.resize(RealFullImageSize + (Width * 4)); // extra space for flipping

                              mem_copy(vDstData.as_ptr(), pResImageData, RealFullImageSize);

                              // pack image data together without any offset that the driver might
                              // require
                              if(Width * 4 < self.m_GetPresentedImgDataHelperMappedLayoutPitch)
                              {
                                  for(u32 Y = 0; Y < Height; ++Y)
                                  {
                                      let OffsetImagePacked: usize = (Y * Width * 4);
                                      let OffsetImageUnpacked: usize = (Y * self.m_GetPresentedImgDataHelperMappedLayoutPitch);
                                      mem_copy(vDstData.as_ptr() + OffsetImagePacked, vDstData.as_ptr() + OffsetImageUnpacked, Width * 4);
                                  }
                              }

                              if(IsB8G8R8A8 || ResetAlpha)
                              {
                                  // swizzle
                                  for(u32 Y = 0; Y < Height; ++Y)
                                  {
                                      for(u32 X = 0; X < Width; ++X)
                                      {
                                          let ImgOff: usize = (Y * Width * 4) + (X * 4);
                                          if(IsB8G8R8A8)
                                          {
                                              std::mem::swap(&mut vDstData[ImgOff],&mut  vDstData[ImgOff + 2]);
                                          }
                                          vDstData[ImgOff + 3] = 255;
                                      }
                                  }
                              }

                              if(FlipImgData)
                              {
                                  u8 *pTempRow = vDstData.as_ptr() + Width * Height * 4;
                                  for(u32 Y = 0; Y < Height / 2; ++Y)
                                  {
                                      mem_copy(pTempRow, vDstData.as_ptr() + Y * Width * 4, Width * 4);
                                      mem_copy(vDstData.as_ptr() + Y * Width * 4, vDstData.as_ptr() + ((Height - Y) - 1) * Width * 4, Width * 4);
                                      mem_copy(vDstData.as_ptr() + ((Height - Y) - 1) * Width * 4, pTempRow, Width * 4);
                                  }
                              }

                              return true;
                          }
                          else
                          {
                              if(!UsesRGBALikeFormat)
                              {
                                  dbg_msg("vulkan", "swap chain image was not in a RGBA like format.");
                              }
                              else
                              {
                                  dbg_msg("vulkan", "swap chain image was not ready to be copied.");
                              }
                              return false;
                          }
                      }

                      #[must_use] fn GetPresentedImageData(u32 &Width, u32 &Height, u32 &Format, Vec<u8> &vDstData) override { return GetPresentedImageDataImpl(&mut self,Width, Height, Format, vDstData, false, false); } -> bool
    */

    /************************
     * SAMPLERS
     ************************/

    #[must_use]
    fn CreateTextureSamplers(&mut self) -> bool {
        let mut Ret: bool = true;
        Ret &= Device::CreateTextureSamplersImpl(
            &self.vk_device,
            self.device.limits.max_sampler_anisotropy,
            self.device.global_texture_lod_bias,
            &mut self.device.samplers[ESupportedSamplerTypes::Repeat as usize],
            vk::SamplerAddressMode::REPEAT,
            vk::SamplerAddressMode::REPEAT,
            vk::SamplerAddressMode::REPEAT,
        );
        Ret &= Device::CreateTextureSamplersImpl(
            &self.vk_device,
            self.device.limits.max_sampler_anisotropy,
            self.device.global_texture_lod_bias,
            &mut self.device.samplers[ESupportedSamplerTypes::ClampToEdge as usize],
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
        );
        Ret &= Device::CreateTextureSamplersImpl(
            &self.vk_device,
            self.device.limits.max_sampler_anisotropy,
            self.device.global_texture_lod_bias,
            &mut self.device.samplers[ESupportedSamplerTypes::Texture2DArray as usize],
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerAddressMode::MIRRORED_REPEAT,
        );
        return Ret;
    }

    fn DestroyTextureSamplers(&mut self) {
        unsafe {
            self.vk_device.destroy_sampler(
                self.device.samplers[ESupportedSamplerTypes::Repeat as usize],
                None,
            );
        }
        unsafe {
            self.vk_device.destroy_sampler(
                self.device.samplers[ESupportedSamplerTypes::ClampToEdge as usize],
                None,
            );
        }
        unsafe {
            self.vk_device.destroy_sampler(
                self.device.samplers[ESupportedSamplerTypes::Texture2DArray as usize],
                None,
            );
        }
    }

    fn GetTextureSampler(&self, SamplerType: ESupportedSamplerTypes) -> vk::Sampler {
        return self.device.samplers[SamplerType as usize];
    }

    #[must_use]
    fn CreateDescriptorPools(&mut self, thread_count: usize) -> bool {
        self.device.standard_texture_descr_pool.is_uniform_pool = false;
        self.device.standard_texture_descr_pool.default_alloc_size = 1024;
        self.device.text_texture_descr_pool.is_uniform_pool = false;
        self.device.text_texture_descr_pool.default_alloc_size = 8;

        self.device
            .uniform_buffer_descr_pools
            .resize(thread_count, Default::default());
        for UniformBufferDescrPool in &mut self.device.uniform_buffer_descr_pools {
            UniformBufferDescrPool.is_uniform_pool = true;
            UniformBufferDescrPool.default_alloc_size = 512;
        }

        let mut Ret = Device::AllocateDescriptorPool(
            &self.error,
            &self.vk_device,
            &mut self.device.standard_texture_descr_pool,
            StreamDataMax::MaxTextures as usize,
        );
        Ret |= Device::AllocateDescriptorPool(
            &self.error,
            &self.vk_device,
            &mut self.device.text_texture_descr_pool,
            8,
        );

        for UniformBufferDescrPool in &mut self.device.uniform_buffer_descr_pools {
            Ret |= Device::AllocateDescriptorPool(
                &self.error,
                &self.vk_device,
                UniformBufferDescrPool,
                64,
            );
        }

        return Ret;
    }

    fn DestroyDescriptorPools(&mut self) {
        for DescrPool in &mut self.device.standard_texture_descr_pool.pools {
            unsafe {
                self.vk_device.destroy_descriptor_pool(DescrPool.pool, None);
            }
        }
        for DescrPool in &mut self.device.text_texture_descr_pool.pools {
            unsafe {
                self.vk_device.destroy_descriptor_pool(DescrPool.pool, None);
            }
        }

        for UniformBufferDescrPool in &mut self.device.uniform_buffer_descr_pools {
            for DescrPool in &mut UniformBufferDescrPool.pools {
                unsafe {
                    self.vk_device.destroy_descriptor_pool(DescrPool.pool, None);
                }
            }
        }
        self.device.uniform_buffer_descr_pools.clear();
    }

    #[must_use]
    fn GetUniformBufferObject(
        &mut self,
        RenderThreadIndex: usize,
        RequiresSharedStagesDescriptor: bool,
        DescrSet: &mut SDeviceDescriptorSet,
        _ParticleCount: usize,
        pData: *const c_void,
        DataSize: usize,
        cur_image_index: u32,
    ) -> bool {
        return self
            .device
            .GetUniformBufferObjectImpl::<SRenderSpriteInfo, 512, 128>(
                RenderThreadIndex,
                RequiresSharedStagesDescriptor,
                DescrSet,
                pData,
                DataSize,
                cur_image_index,
            );
    }

    /************************
     * SWAPPING MECHANISM
     ************************/

    fn StartRenderThread(&mut self, ThreadIndex: usize) {
        let List = &mut self.thread_command_lists[ThreadIndex];
        if !List.is_empty() {
            self.thread_helper_had_commands[ThreadIndex] = true;
            let pThread = &mut self.render_threads[ThreadIndex];
            let mut Lock = pThread.0.lock().unwrap();
            Lock.is_rendering = true;
            pThread.1.notify_one();
        }
    }

    fn FinishRenderThreads(&mut self) {
        if self.thread_count > 1 {
            // execute threads

            for ThreadIndex in 0..self.thread_count - 1 {
                if !self.thread_helper_had_commands[ThreadIndex] {
                    self.StartRenderThread(ThreadIndex);
                }
            }

            for ThreadIndex in 0..self.thread_count - 1 {
                if self.thread_helper_had_commands[ThreadIndex] {
                    let pRenderThread = &mut self.render_threads[ThreadIndex];
                    self.thread_helper_had_commands[ThreadIndex] = false;
                    let mut Lock = pRenderThread.0.lock().unwrap();
                    Lock = pRenderThread
                        .1
                        .wait_while(Lock, |p| {
                            return p.is_rendering;
                        })
                        .unwrap();
                    self.last_pipeline_per_thread[ThreadIndex + 1] = vk::Pipeline::null();
                }
            }
        }
    }

    fn ExecuteMemoryCommandBuffer(&mut self) {
        if self.device.used_memory_command_buffer[self.cur_image_index as usize] {
            let MemoryCommandBuffer =
                &mut self.device.memory_command_buffers[self.cur_image_index as usize];
            unsafe {
                self.vk_device.end_command_buffer(*MemoryCommandBuffer);
            }

            let mut SubmitInfo = vk::SubmitInfo::default();

            SubmitInfo.command_buffer_count = 1;
            SubmitInfo.p_command_buffers = MemoryCommandBuffer;
            unsafe {
                self.vk_device.queue_submit(
                    self.vk_graphics_queue,
                    &[SubmitInfo],
                    vk::Fence::null(),
                );
            }
            unsafe {
                self.vk_device.queue_wait_idle(self.vk_graphics_queue);
            }

            self.device.used_memory_command_buffer[self.cur_image_index as usize] = false;
        }
    }

    fn UploadStagingBuffers(&mut self) {
        if !self.device.non_flushed_staging_buffer_ranges.is_empty() {
            unsafe {
                self.vk_device.flush_mapped_memory_ranges(
                    self.device.non_flushed_staging_buffer_ranges.as_slice(),
                );
            }

            self.device.non_flushed_staging_buffer_ranges.clear();
        }
    }

    fn UploadNonFlushedBuffers<const FlushForRendering: bool>(&mut self, cur_image_index: u32) {
        // streamed vertices
        Device::UploadStreamedBuffer::<{ FlushForRendering }, _>(
            &self.vk_device,
            self.device.limits.non_coherent_mem_alignment,
            &mut self.device.streamed_vertex_buffer,
            cur_image_index,
        );

        // now the buffer objects
        for StreamUniformBuffer in &mut self.device.streamed_uniform_buffers {
            Device::UploadStreamedBuffer::<{ FlushForRendering }, _>(
                &self.vk_device,
                self.device.limits.non_coherent_mem_alignment,
                StreamUniformBuffer,
                cur_image_index,
            );
        }

        self.UploadStagingBuffers();
    }

    fn ClearFrameData(&mut self, FrameImageIndex: usize) {
        self.UploadStagingBuffers();

        // clear pending buffers, that require deletion
        for BufferPair in &mut self.device.frame_delayed_buffer_cleanups[FrameImageIndex] {
            if !BufferPair.mapped_data.is_null() {
                unsafe {
                    self.vk_device.unmap_memory(BufferPair.mem.mem);
                }
            }
            self.device.mem.CleanBufferPair(
                FrameImageIndex,
                &mut BufferPair.buffer,
                &mut BufferPair.mem,
            );
        }
        self.device.frame_delayed_buffer_cleanups[FrameImageIndex].clear();

        // clear pending textures, that require deletion
        for Texture in &mut self.device.frame_delayed_texture_cleanups[FrameImageIndex] {
            Device::DestroyTexture(
                &mut self.device.frame_delayed_buffer_cleanups,
                &mut self.device.image_buffer_caches,
                &self.vk_device,
                Texture,
                FrameImageIndex as u32,
            ); // TODO FrameImageIndex is a behaviour change, self.m_CurImageIndex was used before implictly
        }
        self.device.frame_delayed_texture_cleanups[FrameImageIndex].clear();

        for TexturePair in &mut self.device.frame_delayed_text_textures_cleanups[FrameImageIndex] {
            Device::DestroyTextTexture(
                &mut self.device.frame_delayed_buffer_cleanups,
                &mut self.device.image_buffer_caches,
                &self.vk_device,
                &mut TexturePair.0,
                &mut TexturePair.1,
                FrameImageIndex as u32,
            ); // TODO FrameImageIndex is a behaviour change, self.m_CurImageIndex was used before implictly
        }
        self.device.frame_delayed_text_textures_cleanups[FrameImageIndex].clear();

        self.device.staging_buffer_cache.cleanup(FrameImageIndex);
        self.device
            .staging_buffer_cache_image
            .cleanup(FrameImageIndex);
        self.device.vertex_buffer_cache.cleanup(FrameImageIndex);
        for ImageBufferCache in &mut self.device.image_buffer_caches {
            ImageBufferCache.1.cleanup(FrameImageIndex);
        }
        self.device
            .mem_allocator
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .free_mems_of_frame(FrameImageIndex as u32);
    }

    fn ClearFrameMemoryUsage(&mut self) {
        self.ClearFrameData(self.cur_image_index as usize);
        self.device.ShrinkUnusedCaches();
    }

    #[must_use]
    fn WaitFrame(&mut self) -> bool {
        self.FinishRenderThreads();
        self.last_commands_in_pipe_thread_index = 0;

        self.UploadNonFlushedBuffers::<true>(self.cur_image_index);

        let CommandBuffer = &mut self.main_draw_command_buffers[self.cur_image_index as usize];

        // render threads
        if self.thread_count > 1 {
            let mut ThreadedCommandsUsedCount: usize = 0;
            let RenderThreadCount: usize = self.thread_count - 1;
            for i in 0..RenderThreadCount {
                if self.used_thread_draw_command_buffer[i + 1][self.cur_image_index as usize] {
                    let GraphicThreadCommandBuffer =
                        &self.thread_draw_command_buffers[i + 1][self.cur_image_index as usize];
                    self.helper_thread_draw_command_buffers[ThreadedCommandsUsedCount] =
                        *GraphicThreadCommandBuffer;
                    ThreadedCommandsUsedCount += 1;

                    self.used_thread_draw_command_buffer[i + 1][self.cur_image_index as usize] =
                        false;
                }
            }
            if ThreadedCommandsUsedCount > 0 {
                unsafe {
                    self.vk_device.cmd_execute_commands(
                        *CommandBuffer,
                        self.helper_thread_draw_command_buffers
                            .split_at(ThreadedCommandsUsedCount)
                            .0,
                    );
                }
            }

            // special case if swap chain was not completed in one runbuffer call

            if self.used_thread_draw_command_buffer[0][self.cur_image_index as usize] {
                let GraphicThreadCommandBuffer =
                    &mut self.thread_draw_command_buffers[0][self.cur_image_index as usize];
                unsafe {
                    self.vk_device
                        .end_command_buffer(*GraphicThreadCommandBuffer);
                }

                unsafe {
                    self.vk_device
                        .cmd_execute_commands(*CommandBuffer, &[*GraphicThreadCommandBuffer]);
                }

                self.used_thread_draw_command_buffer[0][self.cur_image_index as usize] = false;
            }
        }

        unsafe { self.vk_device.cmd_end_render_pass(*CommandBuffer) };

        let res = unsafe { self.vk_device.end_command_buffer(*CommandBuffer) };
        if res.is_err() {
            self.error.lock().unwrap().SetError(
                EGFXErrorType::RenderRecording,
                "Command buffer cannot be ended anymore.",
            );
            return false;
        }

        let WaitSemaphore = self.wait_semaphores[self.cur_frames as usize];

        let mut SubmitInfo = vk::SubmitInfo::default();

        SubmitInfo.command_buffer_count = 1;
        SubmitInfo.p_command_buffers = CommandBuffer;

        let mut aCommandBuffers: [vk::CommandBuffer; 2] = Default::default();

        if self.device.used_memory_command_buffer[self.cur_image_index as usize] {
            let MemoryCommandBuffer =
                &mut self.device.memory_command_buffers[self.cur_image_index as usize];
            unsafe {
                self.vk_device.end_command_buffer(*MemoryCommandBuffer);
            }

            aCommandBuffers[0] = *MemoryCommandBuffer;
            aCommandBuffers[1] = *CommandBuffer;
            SubmitInfo.command_buffer_count = 2;
            SubmitInfo.p_command_buffers = aCommandBuffers.as_ptr();

            self.device.used_memory_command_buffer[self.cur_image_index as usize] = false;
        }

        let aWaitSemaphores: [vk::Semaphore; 1] = [WaitSemaphore];
        let aWaitStages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        SubmitInfo.wait_semaphore_count = aWaitSemaphores.len() as u32;
        SubmitInfo.p_wait_semaphores = aWaitSemaphores.as_ptr();
        SubmitInfo.p_wait_dst_stage_mask = aWaitStages.as_ptr();

        let aSignalSemaphores = [self.sig_semaphores[self.cur_frames as usize]];
        SubmitInfo.signal_semaphore_count = aSignalSemaphores.len() as u32;
        SubmitInfo.p_signal_semaphores = aSignalSemaphores.as_ptr();

        unsafe {
            self.vk_device
                .reset_fences(&[self.frame_fences[self.cur_frames as usize]]);
        }

        let QueueSubmitRes = unsafe {
            self.vk_device.queue_submit(
                self.vk_graphics_queue,
                &[SubmitInfo],
                self.frame_fences[self.cur_frames as usize],
            )
        };
        if let Err(err) = QueueSubmitRes {
            let pCritErrorMsg = self.check_res.CheckVulkanCriticalError(
                err,
                &self.error,
                &mut self.recreate_swap_chain,
            );
            if let Some(crit_err) = pCritErrorMsg {
                self.error.lock().unwrap().SetErrorExtra(
                    EGFXErrorType::RenderSubmitFailed,
                    "Submitting to graphics queue failed.",
                    Some(crit_err),
                );
                return false;
            }
        }

        std::mem::swap(
            &mut self.wait_semaphores[self.cur_frames as usize],
            &mut self.sig_semaphores[self.cur_frames as usize],
        );

        let mut PresentInfo = vk::PresentInfoKHR::default();

        PresentInfo.wait_semaphore_count = aSignalSemaphores.len() as u32;
        PresentInfo.p_wait_semaphores = aSignalSemaphores.as_ptr();

        let aSwapChains = [self.vk_swap_chain_khr];
        PresentInfo.swapchain_count = aSwapChains.len() as u32;
        PresentInfo.p_swapchains = aSwapChains.as_ptr();

        PresentInfo.p_image_indices = &mut self.cur_image_index;

        self.last_presented_swap_chain_image_index = self.cur_image_index;

        let QueuePresentRes = unsafe {
            self.vk_swap_chain_ash
                .queue_present(self.vk_present_queue, &PresentInfo)
        };
        if QueuePresentRes.is_err() && QueuePresentRes.unwrap_err() != vk::Result::SUBOPTIMAL_KHR {
            let pCritErrorMsg = self.check_res.CheckVulkanCriticalError(
                QueuePresentRes.unwrap_err(),
                &self.error,
                &mut self.recreate_swap_chain,
            );
            if let Some(crit_err) = pCritErrorMsg {
                self.error.lock().unwrap().SetErrorExtra(
                    EGFXErrorType::SwapFailed,
                    "Presenting graphics queue failed.",
                    Some(crit_err),
                );
                return false;
            }
        }

        self.cur_frames = (self.cur_frames + 1) % self.device.swap_chain_image_count;
        return true;
    }

    #[must_use]
    fn PrepareFrame(&mut self) -> bool {
        if self.recreate_swap_chain {
            self.recreate_swap_chain = false;
            if is_verbose(&*self.dbg) {
                self.sys
                    .log("vulkan")
                    .msg("recreating swap chain requested by user (prepare frame).");
            }
            self.RecreateSwapChain();
        }

        let AcqResult = unsafe {
            self.vk_swap_chain_ash.acquire_next_image(
                self.vk_swap_chain_khr,
                u64::MAX,
                self.sig_semaphores[self.cur_frames as usize],
                vk::Fence::null(),
            )
        };
        if AcqResult.is_err() || AcqResult.unwrap().1 {
            if (AcqResult.is_err() && AcqResult.unwrap_err() == vk::Result::ERROR_OUT_OF_DATE_KHR)
                || self.recreate_swap_chain
            {
                self.recreate_swap_chain = false;
                if is_verbose(&*self.dbg) {
                    self.sys.log("vulkan").msg(
                        "recreating swap chain requested by acquire next image (prepare frame).",
                    );
                }
                self.RecreateSwapChain();
                return self.PrepareFrame();
            } else {
                if AcqResult.is_ok() && AcqResult.as_ref().unwrap().1 {
                    self.sys.log("vulkan").msg("acquire next image failed ");
                }
                let res = if AcqResult.is_err() {
                    AcqResult.unwrap_err()
                } else {
                    vk::Result::SUBOPTIMAL_KHR
                };

                let pCritErrorMsg = self.check_res.CheckVulkanCriticalError(
                    res,
                    &self.error,
                    &mut self.recreate_swap_chain,
                );
                if let Some(crit_err) = pCritErrorMsg {
                    self.error.lock().unwrap().SetErrorExtra(
                        EGFXErrorType::SwapFailed,
                        "Acquiring next image failed.",
                        Some(crit_err),
                    );
                    return false;
                } else if res == vk::Result::ERROR_SURFACE_LOST_KHR {
                    self.rendering_paused = true;
                    return true;
                }
            }
        }
        self.cur_image_index = AcqResult.unwrap().0;
        std::mem::swap(
            &mut self.wait_semaphores[self.cur_frames as usize],
            &mut self.sig_semaphores[self.cur_frames as usize],
        );

        if self.image_fences[self.cur_image_index as usize] != vk::Fence::null() {
            unsafe {
                self.vk_device.wait_for_fences(
                    &[self.image_fences[self.cur_image_index as usize]],
                    true,
                    u64::MAX,
                );
            }
        }
        self.image_fences[self.cur_image_index as usize] =
            self.frame_fences[self.cur_frames as usize];

        // next frame
        self.cur_frame += 1;
        self.image_last_frame_check[self.cur_image_index as usize] = self.cur_frame;

        // check if older frames weren't used in a long time
        for FrameImageIndex in 0..self.image_last_frame_check.len() {
            let LastFrame = self.image_last_frame_check[FrameImageIndex];
            if self.cur_frame - LastFrame > self.device.swap_chain_image_count as u64 {
                if self.image_fences[FrameImageIndex] != vk::Fence::null() {
                    unsafe {
                        self.vk_device.wait_for_fences(
                            &[self.image_fences[FrameImageIndex]],
                            true,
                            u64::MAX,
                        );
                    }
                    self.ClearFrameData(FrameImageIndex);
                    self.image_fences[FrameImageIndex] = vk::Fence::null();
                }
                self.image_last_frame_check[FrameImageIndex] = self.cur_frame;
            }
        }

        // clear frame's memory data
        self.ClearFrameMemoryUsage();

        // clear frame
        unsafe {
            self.vk_device.reset_command_buffer(
                *&mut self.main_draw_command_buffers[self.cur_image_index as usize],
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )
        };

        let CommandBuffer = &mut self.main_draw_command_buffers[self.cur_image_index as usize];
        let mut BeginInfo = vk::CommandBufferBeginInfo::default();
        BeginInfo.flags = vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT;

        unsafe {
            if self
                .vk_device
                .begin_command_buffer(*CommandBuffer, &BeginInfo)
                .is_err()
            {
                self.error.lock().unwrap().SetError(
                    EGFXErrorType::RenderRecording,
                    "Command buffer cannot be filled anymore.",
                );
                return false;
            }
        }

        let mut RenderPassInfo = vk::RenderPassBeginInfo::default();
        RenderPassInfo.render_pass = self.vk_render_pass;
        RenderPassInfo.framebuffer = self.framebuffer_list[self.cur_image_index as usize];
        RenderPassInfo.render_area.offset = vk::Offset2D::default();
        RenderPassInfo.render_area.extent =
            self.vk_swap_img_and_viewport_extent.swap_image_viewport;

        let ClearColorVal = unsafe {
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [
                        self.clear_color[0],
                        self.clear_color[1],
                        self.clear_color[2],
                        self.clear_color[3],
                    ],
                },
            }
        };
        RenderPassInfo.clear_value_count = 1;
        RenderPassInfo.p_clear_values = &ClearColorVal;

        unsafe {
            self.vk_device.cmd_begin_render_pass(
                *CommandBuffer,
                &RenderPassInfo,
                if self.thread_count > 1 {
                    vk::SubpassContents::SECONDARY_COMMAND_BUFFERS
                } else {
                    vk::SubpassContents::INLINE
                },
            );
        }

        for LastPipe in &mut self.last_pipeline_per_thread {
            *LastPipe = vk::Pipeline::null();
        }

        return true;
    }

    #[must_use]
    fn PureMemoryFrame(&mut self) -> bool {
        self.ExecuteMemoryCommandBuffer();

        // reset streamed data
        self.UploadNonFlushedBuffers::<false>(self.cur_image_index);

        self.ClearFrameMemoryUsage();

        return true;
    }

    #[must_use]
    pub fn NextFrame(&mut self) -> bool {
        if !self.rendering_paused {
            if !self.WaitFrame() {
                return false;
            }
            if !self.PrepareFrame() {
                return false;
            }
        }
        // else only execute the memory command buffer
        else {
            if !self.PureMemoryFrame() {
                return false;
            }
        }

        return true;
    }

    /************************
     * TEXTURES
     ************************/
    #[must_use]
    fn UpdateTexture(
        &mut self,
        TextureSlot: usize,
        Format: vk::Format,
        pData: &mut Vec<u8>,
        mut XOff: i64,
        mut YOff: i64,
        mut Width: usize,
        mut Height: usize,
        ColorChannelCount: usize,
    ) -> bool {
        let ImageSize: usize = Width * Height * ColorChannelCount;
        let mut StagingBuffer = SMemoryBlock::<STAGING_BUFFER_IMAGE_CACHE_ID>::default();
        if !Device::get_staging_buffer_image(
            &mut self.device.mem,
            &mut self.device.staging_buffer_cache_image,
            &self.device.limits,
            &mut StagingBuffer,
            pData,
            ImageSize as u64,
        ) {
            return false;
        }

        let Tex = &self.device.textures[TextureSlot];

        if Tex.rescale_count > 0 {
            for _i in 0..Tex.rescale_count {
                Width >>= 1;
                Height >>= 1;

                XOff /= 2;
                YOff /= 2;
            }

            let mut pTmpData = Resize(
                &self.runtime_threadpool,
                pData,
                Width,
                Height,
                Width,
                Height,
                vulkan_format_to_image_color_channel_count(Format),
            );
            std::mem::swap(pData, &mut pTmpData);
        }

        let tex_img = Tex.img;
        if !self.device.ImageBarrier(
            tex_img,
            0,
            Tex.mip_map_count as usize,
            0,
            1,
            Format,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            self.cur_image_index,
        ) {
            return false;
        }
        if !self.device.CopyBufferToImage(
            StagingBuffer.buffer,
            StagingBuffer.heap_data.offset_to_align as u64,
            tex_img,
            XOff as i32,
            YOff as i32,
            Width as u32,
            Height as u32,
            1,
            self.cur_image_index,
        ) {
            return false;
        }

        let Tex = &self.device.textures[TextureSlot];
        if Tex.mip_map_count > 1 {
            if !self.device.BuildMipmaps(
                Tex.img,
                Format,
                Width,
                Height,
                1,
                Tex.mip_map_count as usize,
                self.cur_image_index,
            ) {
                return false;
            }
        } else {
            if !self.device.ImageBarrier(
                Tex.img,
                0,
                1,
                0,
                1,
                Format,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                self.cur_image_index,
            ) {
                return false;
            }
        }

        self.device
            .UploadAndFreeStagingImageMemBlock(&mut StagingBuffer, self.cur_image_index);

        return true;
    }

    #[must_use]
    fn CreateTextureCMD(
        &mut self,
        slot: usize,
        mut width: usize,
        mut height: usize,
        mut depth: usize,
        is_3d_tex: bool,
        pixel_size: usize,
        tex_format: vk::Format,
        _store_format: vk::Format,
        tex_flags: TexFlags,
        upload_data: &'static mut [u8], // TODO!: it must be free'd
    ) -> bool {
        let image_index = slot as usize;
        let image_color_channels = vulkan_format_to_image_color_channel_count(tex_format);

        while image_index >= self.device.textures.len() {
            self.device
                .textures
                .resize((self.device.textures.len() * 2) + 1, CTexture::default());
        }

        // resample if needed
        let mut rescale_count: u32 = 0;
        if width as u32 > self.device.limits.max_texture_size
            || height as u32 > self.device.limits.max_texture_size
            || (width * height * depth)
                > (self.device.limits.max_texture_size as usize
                    * self.device.limits.max_texture_size as usize)
        {
            loop {
                width >>= 1;
                height >>= 1;
                rescale_count += 1;
                if width as u32 > self.device.limits.max_texture_size
                    || height as u32 > self.device.limits.max_texture_size
                    || (width * height * depth)
                        > (self.device.limits.max_texture_size as usize
                            * self.device.limits.max_texture_size as usize)
                {
                    break;
                }
            }
            let mut tmp_data: Vec<u8> = Vec::new();
            // TODO split resize for 3d textures
            tmp_data = Resize(
                &self.runtime_threadpool,
                upload_data,
                width,
                height,
                width,
                height,
                image_color_channels,
            );
            // should be safe since we only downscale
            upload_data.copy_from_slice(tmp_data.as_slice());
        }

        let requires_mip_maps = (tex_flags & TexFlags::TEXFLAG_NOMIPMAPS).is_empty();
        let mut mip_map_level_count: usize = 1;
        if requires_mip_maps {
            let img_size = vk::Extent3D {
                width: width as u32,
                height: height as u32,
                depth: 1,
            };
            mip_map_level_count = image_mip_level_count(img_size);
            if !self.device.optimal_rgba_image_blitting {
                mip_map_level_count = 1;
            }
        }

        let mut texture = self.device.textures[image_index].clone();

        texture.width = width;
        texture.height = height;
        texture.depth = depth;
        texture.rescale_count = rescale_count;
        texture.mip_map_count = mip_map_level_count as u32;

        if !is_3d_tex {
            if !self.device.CreateTextureImage(
                image_index,
                &mut texture.img,
                &mut texture.img_mem,
                upload_data,
                tex_format,
                width as usize,
                height as usize,
                depth,
                pixel_size as usize,
                mip_map_level_count,
                self.cur_image_index,
            ) {
                return false;
            }
            let img_format = tex_format;
            let img_view = self.device.CreateTextureImageView(
                texture.img,
                img_format,
                vk::ImageViewType::TYPE_2D,
                1,
                mip_map_level_count,
            );
            texture.img_view = img_view;
            let mut img_sampler = self.GetTextureSampler(ESupportedSamplerTypes::Repeat);
            texture.samplers[0] = img_sampler;
            img_sampler = self.GetTextureSampler(ESupportedSamplerTypes::ClampToEdge);
            texture.samplers[1] = img_sampler;

            if !self
                .device
                .CreateNewTexturedStandardDescriptorSets(image_index, 0, &mut texture)
            {
                return false;
            }
            if !self
                .device
                .CreateNewTexturedStandardDescriptorSets(image_index, 1, &mut texture)
            {
                return false;
            }
        } else {
            let mut image_3d_width = width as usize;
            let mut image_3d_height = height as usize;

            let img_size = vk::Extent3D {
                width: image_3d_width as u32,
                height: image_3d_height as u32,
                depth: 1 as u32,
            };
            if requires_mip_maps {
                mip_map_level_count = image_mip_level_count(img_size);
                if !self.device.optimal_rgba_image_blitting {
                    mip_map_level_count = 1;
                }
            }

            if !self.device.CreateTextureImage(
                image_index,
                &mut texture.img_3d,
                &mut texture.img_3d_mem,
                upload_data,
                tex_format,
                image_3d_width as usize,
                image_3d_height as usize,
                depth,
                pixel_size as usize,
                mip_map_level_count,
                self.cur_image_index,
            ) {
                return false;
            }
            let ImgFormat = tex_format;
            let ImgView = self.device.CreateTextureImageView(
                texture.img_3d,
                ImgFormat,
                vk::ImageViewType::TYPE_2D_ARRAY,
                depth,
                mip_map_level_count,
            );
            texture.img_3d_view = ImgView;
            let img_sampler = self.GetTextureSampler(ESupportedSamplerTypes::Texture2DArray);
            texture.sampler_3d = img_sampler;

            if !self
                .device
                .CreateNew3DTexturedStandardDescriptorSets(image_index, &mut texture)
            {
                return false;
            }
        }

        self.device.textures[image_index] = texture; // TODO better fix
        return true;
    }

    /************************
     * RENDER STATES
     ************************/

    fn GetStateMatrix(State: &State, Matrix: &mut [f32; 4 * 2]) {
        *Matrix = [
            // column 1
            2.0 / (State.canvas_br.x - State.canvas_tl.x),
            0.0,
            // column 2
            0.0,
            2.0 / (State.canvas_br.y - State.canvas_tl.y),
            // column 3
            0.0,
            0.0,
            // column 4
            -((State.canvas_tl.x + State.canvas_br.x) / (State.canvas_br.x - State.canvas_tl.x)),
            -((State.canvas_tl.y + State.canvas_br.y) / (State.canvas_br.y - State.canvas_tl.y)),
        ];
    }

    #[must_use]
    fn GetIsTextured(state: &State) -> bool {
        if let ETextureIndex::Index(_) = state.texture_index {
            return true;
        }
        return false;
    }

    fn GetAddressModeIndex(state: &State) -> usize {
        return if state.wrap_mode == WrapType::WRAP_REPEAT {
            EVulkanBackendAddressModes::Repeat as usize
        } else {
            EVulkanBackendAddressModes::ClampEdges as usize
        };
    }

    fn GetBlendModeIndex(state: &State) -> usize {
        return if state.blend_mode == BlendType::BLEND_ADDITIVE {
            EVulkanBackendBlendModes::Additative as usize
        } else {
            if state.blend_mode == BlendType::BLEND_NONE {
                EVulkanBackendBlendModes::None as usize
            } else {
                EVulkanBackendBlendModes::Alpha as usize
            }
        };
    }

    fn GetDynamicModeIndexFromState(&self, state: &State) -> usize {
        return if state.clip_enable
            || self.has_dynamic_viewport
            || self.vk_swap_img_and_viewport_extent.has_forced_viewport
        {
            EVulkanBackendClipModes::DynamicScissorAndViewport as usize
        } else {
            EVulkanBackendClipModes::None as usize
        };
    }

    fn GetDynamicModeIndexFromExecBuffer(exec_buffer: &SRenderCommandExecuteBuffer) -> usize {
        return if exec_buffer.has_dynamic_state {
            EVulkanBackendClipModes::DynamicScissorAndViewport as usize
        } else {
            EVulkanBackendClipModes::None as usize
        };
    }

    fn GetPipeline<'a>(
        Container: &'a mut SPipelineContainer,
        IsTextured: bool,
        BlendModeIndex: usize,
        DynamicIndex: usize,
    ) -> &'a mut vk::Pipeline {
        return &mut Container.pipelines[BlendModeIndex][DynamicIndex][IsTextured as usize];
    }

    fn GetPipeLayout<'a>(
        Container: &'a mut SPipelineContainer,
        IsTextured: bool,
        BlendModeIndex: usize,
        DynamicIndex: usize,
    ) -> &'a mut vk::PipelineLayout {
        return &mut Container.pipeline_layouts[BlendModeIndex][DynamicIndex][IsTextured as usize];
    }

    fn GetPipelineAndLayout<'a>(
        Container: &'a SPipelineContainer,
        IsTextured: bool,
        BlendModeIndex: usize,
        DynamicIndex: usize,
    ) -> (&'a vk::Pipeline, &'a vk::PipelineLayout) {
        return (
            &Container.pipelines[BlendModeIndex][DynamicIndex][IsTextured as usize],
            &Container.pipeline_layouts[BlendModeIndex][DynamicIndex][IsTextured as usize],
        );
    }

    fn GetPipelineAndLayout_mut<'a>(
        Container: &'a mut SPipelineContainer,
        IsTextured: bool,
        BlendModeIndex: usize,
        DynamicIndex: usize,
    ) -> (&'a mut vk::Pipeline, &'a mut vk::PipelineLayout) {
        return (
            &mut Container.pipelines[BlendModeIndex][DynamicIndex][IsTextured as usize],
            &mut Container.pipeline_layouts[BlendModeIndex][DynamicIndex][IsTextured as usize],
        );
    }

    fn GetStandardPipeAndLayout<'a>(
        standard_line_pipeline: &'a SPipelineContainer,
        standard_pipeline: &'a SPipelineContainer,
        IsLineGeometry: bool,
        IsTextured: bool,
        BlendModeIndex: usize,
        DynamicIndex: usize,
    ) -> (&'a vk::Pipeline, &'a vk::PipelineLayout) {
        if IsLineGeometry {
            return Self::GetPipelineAndLayout(
                standard_line_pipeline,
                IsTextured,
                BlendModeIndex,
                DynamicIndex,
            );
        } else {
            return Self::GetPipelineAndLayout(
                standard_pipeline,
                IsTextured,
                BlendModeIndex,
                DynamicIndex,
            );
        }
    }

    fn GetTileLayerPipeLayout(
        &mut self,
        Type: i32,
        IsTextured: bool,
        BlendModeIndex: usize,
        DynamicIndex: usize,
    ) -> &mut vk::PipelineLayout {
        if Type == 0 {
            return Self::GetPipeLayout(
                &mut self.tile_pipeline,
                IsTextured,
                BlendModeIndex,
                DynamicIndex,
            );
        } else if Type == 1 {
            return Self::GetPipeLayout(
                &mut self.tile_border_pipeline,
                IsTextured,
                BlendModeIndex,
                DynamicIndex,
            );
        } else {
            return Self::GetPipeLayout(
                &mut self.tile_border_line_pipeline,
                IsTextured,
                BlendModeIndex,
                DynamicIndex,
            );
        }
    }

    fn GetTileLayerPipe(
        &mut self,
        Type: i32,
        IsTextured: bool,
        BlendModeIndex: usize,
        DynamicIndex: usize,
    ) -> &mut vk::Pipeline {
        if Type == 0 {
            return Self::GetPipeline(
                &mut self.tile_pipeline,
                IsTextured,
                BlendModeIndex,
                DynamicIndex,
            );
        } else if Type == 1 {
            return Self::GetPipeline(
                &mut self.tile_border_pipeline,
                IsTextured,
                BlendModeIndex,
                DynamicIndex,
            );
        } else {
            return Self::GetPipeline(
                &mut self.tile_border_line_pipeline,
                IsTextured,
                BlendModeIndex,
                DynamicIndex,
            );
        }
    }

    fn GetStateIndices(
        exec_buffer: &SRenderCommandExecuteBuffer,
        State: &State,
        IsTextured: &mut bool,
        BlendModeIndex: &mut usize,
        DynamicIndex: &mut usize,
        AddressModeIndex: &mut usize,
    ) {
        *IsTextured = Self::GetIsTextured(State);
        *AddressModeIndex = Self::GetAddressModeIndex(State);
        *BlendModeIndex = Self::GetBlendModeIndex(State);
        *DynamicIndex = Self::GetDynamicModeIndexFromExecBuffer(exec_buffer);
    }

    fn ExecBufferFillDynamicStates(
        &self,
        State: &State,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
    ) {
        let DynamicStateIndex: usize = self.GetDynamicModeIndexFromState(State);
        if DynamicStateIndex == EVulkanBackendClipModes::DynamicScissorAndViewport as usize {
            let mut Viewport = vk::Viewport::default();
            if self.has_dynamic_viewport {
                Viewport.x = self.dynamic_viewport_offset.x as f32;
                Viewport.y = self.dynamic_viewport_offset.y as f32;
                Viewport.width = self.dynamic_viewport_size.width as f32;
                Viewport.height = self.dynamic_viewport_size.height as f32;
                Viewport.min_depth = 0.0;
                Viewport.max_depth = 1.0;
            }
            // else check if there is a forced viewport
            else if self.vk_swap_img_and_viewport_extent.has_forced_viewport {
                Viewport.x = 0.0;
                Viewport.y = 0.0;
                Viewport.width = self.vk_swap_img_and_viewport_extent.forced_viewport.width as f32;
                Viewport.height =
                    self.vk_swap_img_and_viewport_extent.forced_viewport.height as f32;
                Viewport.min_depth = 0.0;
                Viewport.max_depth = 1.0;
            } else {
                Viewport.x = 0.0;
                Viewport.y = 0.0;
                Viewport.width = self
                    .vk_swap_img_and_viewport_extent
                    .swap_image_viewport
                    .width as f32;
                Viewport.height = self
                    .vk_swap_img_and_viewport_extent
                    .swap_image_viewport
                    .height as f32;
                Viewport.min_depth = 0.0;
                Viewport.max_depth = 1.0;
            }

            let mut Scissor = vk::Rect2D::default();
            // convert from OGL to vulkan clip

            // the scissor always assumes the presented viewport, because the
            // front-end keeps the calculation for the forced viewport in sync
            let ScissorViewport = self
                .vk_swap_img_and_viewport_extent
                .get_presented_image_viewport();
            if State.clip_enable {
                let ScissorY: i32 =
                    ScissorViewport.height as i32 - (State.clip_y as i32 + State.clip_h as i32);
                let ScissorH = State.clip_h as i32;
                Scissor.offset = vk::Offset2D {
                    x: State.clip_x as i32,
                    y: ScissorY,
                };
                Scissor.extent = vk::Extent2D {
                    width: State.clip_w as u32,
                    height: ScissorH as u32,
                };
            } else {
                Scissor.offset = vk::Offset2D::default();
                Scissor.extent = vk::Extent2D {
                    width: ScissorViewport.width as u32,
                    height: ScissorViewport.height as u32,
                };
            }

            // if there is a dynamic viewport make sure the scissor data is scaled
            // down to that
            if self.has_dynamic_viewport {
                Scissor.offset.x = ((Scissor.offset.x as f32 / ScissorViewport.width as f32)
                    * self.dynamic_viewport_size.width as f32)
                    as i32
                    + self.dynamic_viewport_offset.x;
                Scissor.offset.y = ((Scissor.offset.y as f32 / ScissorViewport.height as f32)
                    * self.dynamic_viewport_size.height as f32)
                    as i32
                    + self.dynamic_viewport_offset.y;
                Scissor.extent.width = ((Scissor.extent.width / ScissorViewport.width) as f32
                    * self.dynamic_viewport_size.width as f32)
                    as u32;
                Scissor.extent.height = ((Scissor.extent.height / ScissorViewport.height) as f32
                    * self.dynamic_viewport_size.height as f32)
                    as u32;
            }

            Viewport.x = Viewport.x.clamp(0.0, f32::MAX);
            Viewport.y = Viewport.y.clamp(0.0, f32::MAX);

            Scissor.offset.x = Scissor.offset.x.clamp(0, i32::MAX);
            Scissor.offset.y = Scissor.offset.y.clamp(0, i32::MAX);

            exec_buffer.has_dynamic_state = true;
            exec_buffer.viewport = Viewport;
            exec_buffer.scissor = Scissor;
        } else {
            exec_buffer.has_dynamic_state = false;
        }
    }

    fn BindPipeline(
        device: &ash::Device,
        last_pipeline: &mut Vec<vk::Pipeline>,
        RenderThreadIndex: usize,
        CommandBuffer: vk::CommandBuffer,
        exec_buffer: &SRenderCommandExecuteBuffer,
        BindingPipe: vk::Pipeline,
        _state: &State,
    ) {
        if last_pipeline[RenderThreadIndex] != BindingPipe {
            unsafe {
                device.cmd_bind_pipeline(
                    CommandBuffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    BindingPipe,
                );
            }
            last_pipeline[RenderThreadIndex] = BindingPipe;
        }

        let DynamicStateIndex: usize = Self::GetDynamicModeIndexFromExecBuffer(exec_buffer);
        if DynamicStateIndex == EVulkanBackendClipModes::DynamicScissorAndViewport as usize {
            unsafe {
                device.cmd_set_viewport(CommandBuffer, 0, &[exec_buffer.viewport]);
            }
            unsafe {
                device.cmd_set_scissor(CommandBuffer, 0, &[exec_buffer.scissor]);
            }
        }
    }

    /**************************
     * RENDERING IMPLEMENTATION
     ***************************/

    fn RenderTileLayer_FillExecuteBuffer(
        &mut self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        DrawCalls: usize,
        State: &State,
        buffer_container_index: usize,
    ) {
        let buffer_object_index: usize =
            self.device.buffer_containers[buffer_container_index].buffer_object_index as usize;
        let buffer_object = &self.device.buffer_objects[buffer_object_index];

        exec_buffer.buffer = buffer_object.cur_buffer;
        exec_buffer.buffer_off = buffer_object.cur_buffer_offset;

        let IsTextured: bool = Self::GetIsTextured(State);
        if IsTextured {
            exec_buffer.descriptors[0] = self.device.textures[State.texture_index.unwrap()]
                .vk_standard_3d_textured_descr_set
                .clone();
        }

        exec_buffer.index_buffer = self.render_index_buffer;

        exec_buffer.estimated_render_call_count = DrawCalls;

        self.ExecBufferFillDynamicStates(State, exec_buffer);
    }

    #[must_use]
    fn RenderTileLayer(
        &mut self,
        exec_buffer: &SRenderCommandExecuteBuffer,
        state: &State,
        Type: i32,
        Color: &GL_SColorf,
        dir: &vec2,
        off: &vec2,
        JumpIndex: i32,
        IndicesDrawNum: usize,
        pIndicesOffsets: &[usize],
        pDrawCount: &[usize],
        InstanceCount: usize,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::GetStateMatrix(state, &mut m);

        let mut is_textured: bool = Default::default();
        let mut BlendModeIndex: usize = Default::default();
        let mut DynamicIndex: usize = Default::default();
        let mut AddressModeIndex: usize = Default::default();
        Self::GetStateIndices(
            exec_buffer,
            state,
            &mut is_textured,
            &mut BlendModeIndex,
            &mut DynamicIndex,
            &mut AddressModeIndex,
        );
        let PipeLayout =
            *self.GetTileLayerPipeLayout(Type, is_textured, BlendModeIndex, DynamicIndex);
        let PipeLine = *self.GetTileLayerPipe(Type, is_textured, BlendModeIndex, DynamicIndex);

        let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.GetGraphicCommandBuffer(&mut command_buffer_ptr, exec_buffer.thread_index as usize)
        {
            return false;
        }
        let CommandBuffer = unsafe { &mut *command_buffer_ptr };

        Self::BindPipeline(
            &self.vk_device,
            &mut self.last_pipeline_per_thread,
            exec_buffer.thread_index as usize,
            *CommandBuffer,
            exec_buffer,
            PipeLine,
            state,
        );

        let vertex_buffers = [exec_buffer.buffer];
        let offsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            self.vk_device
                .cmd_bind_vertex_buffers(*CommandBuffer, 0, &vertex_buffers, &offsets);
        }

        if is_textured {
            unsafe {
                self.vk_device.cmd_bind_descriptor_sets(
                    *CommandBuffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    PipeLayout,
                    0,
                    &[exec_buffer.descriptors[0].descriptor],
                    &[],
                );
            }
        }

        let mut VertexPushConstants = SUniformTileGPosBorder::default();
        let mut VertexPushConstantSize: usize = std::mem::size_of::<SUniformTileGPos>();
        let mut FragPushConstants = SUniformTileGVertColor::default();
        let FragPushConstantSize: usize = std::mem::size_of::<SUniformTileGVertColor>();

        unsafe {
            libc::memcpy(
                VertexPushConstants.base.base.pos.as_mut_ptr() as *mut c_void,
                m.as_ptr() as *const c_void,
                m.len() * std::mem::size_of::<f32>(),
            );
        }
        FragPushConstants = *Color;

        if Type == 1 {
            VertexPushConstants.base.dir = *dir;
            VertexPushConstants.base.offset = *off;
            VertexPushConstants.jump_index = JumpIndex;
            VertexPushConstantSize = std::mem::size_of::<SUniformTileGPosBorder>();
        } else if Type == 2 {
            VertexPushConstants.base.dir = *dir;
            VertexPushConstants.base.offset = *off;
            VertexPushConstantSize = std::mem::size_of::<SUniformTileGPosBorderLine>();
        }

        unsafe {
            self.vk_device.cmd_push_constants(
                *CommandBuffer,
                PipeLayout,
                vk::ShaderStageFlags::VERTEX,
                0,
                unsafe {
                    std::slice::from_raw_parts(
                        (&VertexPushConstants) as *const _ as *const u8,
                        VertexPushConstantSize,
                    )
                },
            );
        }
        unsafe {
            self.vk_device.cmd_push_constants(
                *CommandBuffer,
                PipeLayout,
                vk::ShaderStageFlags::FRAGMENT,
                (std::mem::size_of::<SUniformTileGPosBorder>()
                    + std::mem::size_of::<SUniformTileGVertColorAlign>()) as u32,
                unsafe {
                    std::slice::from_raw_parts(
                        &FragPushConstants as *const _ as *const u8,
                        FragPushConstantSize,
                    )
                },
            );
        }

        let DrawCount: usize = IndicesDrawNum as usize;
        unsafe {
            self.vk_device.cmd_bind_index_buffer(
                *CommandBuffer,
                exec_buffer.index_buffer,
                0,
                vk::IndexType::UINT32,
            );
        }
        for i in 0..DrawCount {
            let IndexOffset =
                (pIndicesOffsets[i] as usize / std::mem::size_of::<u32>()) as vk::DeviceSize;

            unsafe {
                self.vk_device.cmd_draw_indexed(
                    *CommandBuffer,
                    pDrawCount[i] as u32,
                    InstanceCount as u32,
                    IndexOffset as u32,
                    0,
                    0,
                );
            }
        }

        return true;
    }

    #[must_use]
    fn RenderStandard<TName, const Is3DTextured: bool>(
        &mut self,
        exec_buffer: &SRenderCommandExecuteBuffer,
        State: &State,
        prim_type: PrimType,
        PrimitiveCount: usize,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::GetStateMatrix(State, &mut m);

        let IsLineGeometry: bool = prim_type == PrimType::Lines;

        let mut IsTextured: bool = Default::default();
        let mut BlendModeIndex: usize = Default::default();
        let mut DynamicIndex: usize = Default::default();
        let mut AddressModeIndex: usize = Default::default();
        Self::GetStateIndices(
            exec_buffer,
            State,
            &mut IsTextured,
            &mut BlendModeIndex,
            &mut DynamicIndex,
            &mut AddressModeIndex,
        );
        let (PipelineRef, PipeLayoutRef) = if Is3DTextured {
            Self::GetPipelineAndLayout(
                &mut self.standard_3d_pipeline,
                IsTextured,
                BlendModeIndex,
                DynamicIndex,
            )
        } else {
            Self::GetStandardPipeAndLayout(
                &mut self.standard_line_pipeline,
                &mut self.standard_pipeline,
                IsLineGeometry,
                IsTextured,
                BlendModeIndex,
                DynamicIndex,
            )
        };
        let (Pipeline, PipeLayout) = (*PipelineRef, *PipeLayoutRef);

        let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.GetGraphicCommandBuffer(&mut command_buffer_ptr, exec_buffer.thread_index as usize)
        {
            return false;
        }
        let CommandBuffer = unsafe { &mut *command_buffer_ptr };

        Self::BindPipeline(
            &self.vk_device,
            &mut self.last_pipeline_per_thread,
            exec_buffer.thread_index as usize,
            *CommandBuffer,
            exec_buffer,
            Pipeline,
            State,
        );

        let mut VertPerPrim: usize = 2;
        let mut IsIndexed: bool = false;
        if prim_type == PrimType::Quads {
            VertPerPrim = 4;
            IsIndexed = true;
        } else if prim_type == PrimType::Triangles {
            VertPerPrim = 3;
        }

        let aVertexBuffers = [exec_buffer.buffer];
        let aOffsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            self.vk_device.cmd_bind_vertex_buffers(
                *CommandBuffer,
                0,
                aVertexBuffers.as_slice(),
                aOffsets.as_slice(),
            );
        }

        if IsIndexed {
            unsafe {
                self.vk_device.cmd_bind_index_buffer(
                    *CommandBuffer,
                    exec_buffer.index_buffer,
                    0,
                    vk::IndexType::UINT32,
                );
            }
        }
        if IsTextured {
            unsafe {
                self.vk_device.cmd_bind_descriptor_sets(
                    *CommandBuffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    PipeLayout.clone(),
                    0,
                    &[exec_buffer.descriptors[0].descriptor],
                    &[],
                );
            }
        }

        unsafe {
            self.vk_device.cmd_push_constants(
                *CommandBuffer,
                PipeLayout.clone(),
                vk::ShaderStageFlags::VERTEX,
                0,
                unsafe {
                    std::slice::from_raw_parts(
                        m.as_ptr() as *const _ as *const u8,
                        m.len() * std::mem::size_of::<f32>(),
                    )
                },
            );
        }

        if IsIndexed {
            unsafe {
                self.vk_device.cmd_draw_indexed(
                    *CommandBuffer,
                    (PrimitiveCount * 6) as u32,
                    1,
                    0,
                    0,
                    0,
                );
            }
        } else {
            unsafe {
                self.vk_device.cmd_draw(
                    *CommandBuffer,
                    (PrimitiveCount as usize * VertPerPrim as usize) as u32,
                    1,
                    0,
                    0,
                );
            }
        }

        return true;
    }

    /************************
     * VULKAN SETUP CODE
     ************************/

    #[must_use]
    fn GetVulkanExtensions(window: &sdl2::video::Window) -> Result<Vec<String>, ArrayString<4096>> {
        let mut vk_extensions = Vec::<String>::new();

        let ext_list_res = window.vulkan_instance_extensions();
        if let Err(err) = ext_list_res {
            let mut res =
                ArrayString::from_str("Could not get instance extensions from SDL: ").unwrap();
            res.push_str(err.as_str());
            return Err(res);
        }
        let ext_list = ext_list_res.unwrap();

        for ext in ext_list {
            vk_extensions.push(ext.to_string());
        }

        return Ok(vk_extensions);
    }

    fn OurVKLayers(dbg: EDebugGFXModes) -> std::collections::BTreeSet<String> {
        let mut OurLayers: std::collections::BTreeSet<String> = Default::default();

        if dbg == EDebugGFXModes::Minimum || dbg == EDebugGFXModes::All {
            OurLayers.insert("VK_LAYER_KHRONOS_validation".to_string());
            // deprecated, but VK_LAYER_KHRONOS_validation was released after
            // vulkan 1.1
            OurLayers.insert("VK_LAYER_LUNARG_standard_validation".to_string());
        }

        return OurLayers;
    }

    fn OurDeviceExtensions() -> std::collections::BTreeSet<String> {
        let mut OurExt: std::collections::BTreeSet<String> = Default::default();
        OurExt.insert(vk::KhrSwapchainFn::name().to_str().unwrap().to_string());
        return OurExt;
    }

    fn OurImageUsages() -> Vec<vk::ImageUsageFlags> {
        let mut vImgUsages: Vec<vk::ImageUsageFlags> = Default::default();

        vImgUsages.push(vk::ImageUsageFlags::COLOR_ATTACHMENT);
        vImgUsages.push(vk::ImageUsageFlags::TRANSFER_SRC);

        return vImgUsages;
    }

    #[must_use]
    fn GetVulkanLayers(
        dbg: EDebugGFXModes,
        entry: &ash::Entry,
    ) -> Result<Vec<String>, ArrayString<4096>> {
        let Res = entry.enumerate_instance_layer_properties();
        if Res.is_err() {
            return Err(ArrayString::from_str("Could not get vulkan layers.").unwrap());
        }
        let mut vk_instance_layers = Res.unwrap();

        let req_layer_names = Self::OurVKLayers(dbg);
        let mut vk_layers = Vec::<String>::new();
        for layer_name in &mut vk_instance_layers {
            let layer_name = unsafe {
                CStr::from_ptr(layer_name.layer_name.as_ptr())
                    .to_str()
                    .unwrap()
                    .to_string()
            };
            let it = req_layer_names.get(&layer_name);
            if let Some(_layer) = it {
                vk_layers.push(layer_name);
            }
        }

        return Ok(vk_layers);
    }

    #[must_use]
    fn CreateVulkanInstance(
        dbg: EDebugGFXModes,
        entry: &ash::Entry,
        error: &Arc<Mutex<Error>>,
        vVKLayers: &Vec<String>,
        vVKExtensions: &Vec<String>,
        TryDebugExtensions: bool,
    ) -> Result<ash::Instance, ArrayString<4096>> {
        let mut vLayersCStr: Vec<*const libc::c_char> = Default::default();
        let mut vLayersCStrHelper: Vec<CString> = Default::default();
        vLayersCStr.reserve(vVKLayers.len());
        for Layer in vVKLayers {
            vLayersCStrHelper
                .push(unsafe { CString::from_vec_unchecked(Layer.as_bytes().to_vec()) });
            vLayersCStr.push(vLayersCStrHelper.last().unwrap().as_ptr());
        }

        let mut vExtCStr: Vec<*const libc::c_char> = Default::default();
        let mut vExtCStrHelper: Vec<CString> = Default::default();
        vExtCStr.reserve(vVKExtensions.len() + 1);
        for Ext in vVKExtensions {
            vExtCStrHelper.push(unsafe { CString::from_vec_unchecked(Ext.as_bytes().to_vec()) });
            vExtCStr.push(vExtCStrHelper.last().unwrap().as_ptr());
        }

        if TryDebugExtensions && (dbg == EDebugGFXModes::Minimum || dbg == EDebugGFXModes::All) {
            // debug message support
            vExtCStr.push(vk::ExtDebugUtilsFn::name().as_ptr());
        }

        let mut VKAppInfo = vk::ApplicationInfo::default();
        VKAppInfo.p_application_name = app_name.as_ptr() as *const i8;
        VKAppInfo.application_version = 1;
        VKAppInfo.p_engine_name = app_vk_name.as_ptr() as *const i8;
        VKAppInfo.engine_version = 1;
        VKAppInfo.api_version = vk::API_VERSION_1_1;

        let mut pExt = std::ptr::null();
        let mut Features = vk::ValidationFeaturesEXT::default();
        let aEnables = [
            vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
            vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
        ];
        if TryDebugExtensions
            && (dbg == EDebugGFXModes::AffectsPerformance || dbg == EDebugGFXModes::All)
        {
            Features.enabled_validation_feature_count = aEnables.len() as u32;
            Features.p_enabled_validation_features = aEnables.as_ptr();

            pExt = &Features;
        }

        let mut VKInstanceInfo = vk::InstanceCreateInfo::default();
        VKInstanceInfo.p_next = pExt as *const c_void;
        VKInstanceInfo.flags = vk::InstanceCreateFlags::empty();
        VKInstanceInfo.p_application_info = &VKAppInfo;
        VKInstanceInfo.enabled_extension_count = vExtCStr.len() as u32;
        VKInstanceInfo.pp_enabled_extension_names = vExtCStr.as_ptr();
        VKInstanceInfo.enabled_layer_count = vLayersCStr.len() as u32;
        VKInstanceInfo.pp_enabled_layer_names = vLayersCStr.as_ptr();

        let mut TryAgain: bool = false;

        let Res = unsafe { entry.create_instance(&VKInstanceInfo, None) };
        if let Err(res_err) = Res {
            let mut check_res = CheckResult::default();
            let mut recreate_swap_chain_dummy = false;
            let pCritErrorMsg =
                check_res.CheckVulkanCriticalError(res_err, error, &mut recreate_swap_chain_dummy);
            if let Some(_err_crit) = pCritErrorMsg {
                return Err(ArrayString::from_str("Creating instance failed.").unwrap());
            } else if Res.is_err()
                && (res_err == vk::Result::ERROR_LAYER_NOT_PRESENT
                    || res_err == vk::Result::ERROR_EXTENSION_NOT_PRESENT)
            {
                TryAgain = true;
            }
        }

        if TryAgain && TryDebugExtensions {
            return Self::CreateVulkanInstance(dbg, entry, error, vVKLayers, vVKExtensions, false);
        }

        Ok(Res.unwrap())
    }

    fn VKGPUTypeToGraphicsGPUType(VKGPUType: vk::PhysicalDeviceType) -> ETWGraphicsGPUType {
        if VKGPUType == vk::PhysicalDeviceType::DISCRETE_GPU {
            return ETWGraphicsGPUType::Discrete;
        } else if VKGPUType == vk::PhysicalDeviceType::INTEGRATED_GPU {
            return ETWGraphicsGPUType::Integrated;
        } else if VKGPUType == vk::PhysicalDeviceType::VIRTUAL_GPU {
            return ETWGraphicsGPUType::Virtual;
        } else if VKGPUType == vk::PhysicalDeviceType::CPU {
            return ETWGraphicsGPUType::CPU;
        }

        return ETWGraphicsGPUType::CPU;
    }

    // from:
    // https://github.com/SaschaWillems/vulkan.gpuinfo.org/blob/5c3986798afc39d736b825bf8a5fbf92b8d9ed49/includes/functions.php#L364
    fn GetDriverVerson(DriverVersion: u32, VendorID: u32) -> String {
        // NVIDIA
        if VendorID == 4318 {
            format!(
                "{}.{}.{}.{}",
                (DriverVersion >> 22) & 0x3ff,
                (DriverVersion >> 14) & 0x0ff,
                (DriverVersion >> 6) & 0x0ff,
                (DriverVersion) & 0x003f
            )
        }
        // windows only
        else if VendorID == 0x8086 {
            format!("{}.{}", (DriverVersion >> 14), (DriverVersion) & 0x3fff)
        } else {
            // Use Vulkan version conventions if vendor mapping is not available
            format!(
                "{}.{}.{}",
                (DriverVersion >> 22),
                (DriverVersion >> 12) & 0x3ff,
                DriverVersion & 0xfff
            )
        }
    }

    #[must_use]
    fn SelectGPU(
        instance: &ash::Instance,
        dbg: EDebugGFXModes,
        sys: &mut system::System,
    ) -> Result<
        (
            TTWGraphicsGPUList,
            Limits,
            String,
            String,
            String,
            vk::PhysicalDevice,
            u32,
        ),
        ArrayString<4096>,
    > {
        let res = unsafe { instance.enumerate_physical_devices() };
        if res.is_err() && *res.as_ref().unwrap_err() != vk::Result::INCOMPLETE {
            return Err(ArrayString::from_str("No vulkan compatible devices found.").unwrap());
        }
        if res.is_err() && *res.as_ref().unwrap_err() == vk::Result::INCOMPLETE {
            // TODO! GFX_WARNING_TYPE_INIT_FAILED_MISSING_INTEGRATED_GPU_DRIVER
            return Err(ArrayString::from_str("No vulkan compatible devices found.").unwrap());
        }
        let mut vDeviceList = res.unwrap();

        let mut renderer_name = String::default();
        let mut vendor_name = String::default();
        let mut version_name = String::default();
        let mut gpu_list = TTWGraphicsGPUList::default();

        let mut Index: usize = 0;
        let mut vDevicePropList = Vec::<vk::PhysicalDeviceProperties>::new();
        vDevicePropList.resize(vDeviceList.len(), Default::default());
        gpu_list.gpus.reserve(vDeviceList.len());

        let mut FoundDeviceIndex: usize = 0;
        let mut FoundGPUType: usize = ETWGraphicsGPUType::Invalid as usize;

        let mut AutoGPUType = ETWGraphicsGPUType::Invalid;

        let IsAutoGPU: bool = true; // TODO str_comp("auto" /* TODO: g_Config.m_GfxGPUName */, "auto") == 0;

        for CurDevice in &mut vDeviceList {
            vDevicePropList[Index] = unsafe { instance.get_physical_device_properties(*CurDevice) };

            let DeviceProp = &mut vDevicePropList[Index];

            let GPUType = Self::VKGPUTypeToGraphicsGPUType(DeviceProp.device_type);

            let mut NewGPU = STWGraphicGPUItem::default();
            NewGPU.name = unsafe {
                CStr::from_ptr(DeviceProp.device_name.as_ptr())
                    .to_str()
                    .unwrap()
                    .to_string()
            };
            NewGPU.gpu_type = GPUType as u32;
            gpu_list.gpus.push(NewGPU);

            Index += 1;

            let DevAPIMajor: i32 = vk::api_version_major(DeviceProp.api_version) as i32;
            let DevAPIMinor: i32 = vk::api_version_minor(DeviceProp.api_version) as i32;

            if (GPUType as usize) < AutoGPUType as usize
                && (DevAPIMajor > gs_BackendVulkanMajor as i32
                    || (DevAPIMajor == gs_BackendVulkanMajor as i32
                        && DevAPIMinor >= gs_BackendVulkanMinor as i32))
            {
                gpu_list.auto_gpu.name = unsafe {
                    CStr::from_ptr(DeviceProp.device_name.as_ptr())
                        .to_str()
                        .unwrap()
                        .to_string()
                };
                gpu_list.auto_gpu.gpu_type = GPUType as u32;

                AutoGPUType = GPUType;
            }

            if ((IsAutoGPU && (GPUType as usize) < FoundGPUType)
                || unsafe {
                    CStr::from_ptr(DeviceProp.device_name.as_ptr())
                        .to_str()
                        .unwrap()
                        .to_string()
                        == "auto" /* TODO: g_Config.m_GfxGPUName */
                })
                && (DevAPIMajor > gs_BackendVulkanMajor as i32
                    || (DevAPIMajor == gs_BackendVulkanMajor as i32
                        && DevAPIMinor >= gs_BackendVulkanMinor as i32))
            {
                FoundDeviceIndex = Index;
                FoundGPUType = GPUType as usize;
            }
        }

        if FoundDeviceIndex == 0 {
            FoundDeviceIndex = 1;
        }

        let DeviceProp = &mut vDevicePropList[FoundDeviceIndex - 1];

        let DevAPIMajor: i32 = vk::api_version_major(DeviceProp.api_version) as i32;
        let DevAPIMinor: i32 = vk::api_version_minor(DeviceProp.api_version) as i32;
        let DevAPIPatch: i32 = vk::api_version_patch(DeviceProp.api_version) as i32;

        renderer_name = unsafe {
            CStr::from_ptr(DeviceProp.device_name.as_ptr())
                .to_str()
                .unwrap()
                .to_string()
        };
        let pVendorNameStr: &str;
        match DeviceProp.vendor_id {
            0x1002 => pVendorNameStr = "AMD",
            0x1010 => pVendorNameStr = "ImgTec",
            0x106B => pVendorNameStr = "Apple",
            0x10DE => pVendorNameStr = "NVIDIA",
            0x13B5 => pVendorNameStr = "ARM",
            0x5143 => pVendorNameStr = "Qualcomm",
            0x8086 => pVendorNameStr = "INTEL",
            0x10005 => pVendorNameStr = "Mesa",
            _ => {
                sys.log("vulkan")
                    .msg("unknown gpu vendor ")
                    .msg_var(&DeviceProp.vendor_id);
                pVendorNameStr = "unknown"
            }
        }

        let mut limits = Limits::default();
        vendor_name = pVendorNameStr.to_string();
        version_name = format!(
            "Vulkan {}.{}.{} (driver: {})",
            DevAPIMajor,
            DevAPIMinor,
            DevAPIPatch,
            Self::GetDriverVerson(DeviceProp.driver_version, DeviceProp.vendor_id)
        );

        // get important device limits
        limits.non_coherent_mem_alignment = DeviceProp.limits.non_coherent_atom_size;
        limits.optimal_image_copy_mem_alignment =
            DeviceProp.limits.optimal_buffer_copy_offset_alignment;
        limits.max_texture_size = DeviceProp.limits.max_image_dimension2_d;
        limits.max_sampler_anisotropy = DeviceProp.limits.max_sampler_anisotropy as u32;

        limits.min_uniform_align = DeviceProp.limits.min_uniform_buffer_offset_alignment as u32;
        limits.max_multi_sample = DeviceProp.limits.framebuffer_color_sample_counts;

        if is_verbose_mode(dbg) {
            sys.log("vulkan")
                .msg("device prop: non-coherent align: ")
                .msg_var(&limits.non_coherent_mem_alignment)
                .msg(", optimal image copy align: ")
                .msg_var(&limits.optimal_image_copy_mem_alignment)
                .msg(", max texture size: ")
                .msg_var(&limits.max_texture_size)
                .msg(", max sampler anisotropy: ")
                .msg_var(&limits.max_sampler_anisotropy);
            sys.log("vulkan")
                .msg("device prop: min uniform align: ")
                .msg_var(&limits.min_uniform_align)
                .msg(", multi sample: ")
                .msg_var(&(limits.max_multi_sample.as_raw()));
        }

        let CurDevice = vDeviceList[FoundDeviceIndex - 1];

        let vQueuePropList =
            unsafe { instance.get_physical_device_queue_family_properties(CurDevice) };
        if vQueuePropList.len() == 0 {
            return Err(ArrayString::from_str("No vulkan queue family properties found.").unwrap());
        }

        let mut QueueNodeIndex: u32 = u32::MAX;
        for i in 0..vQueuePropList.len() {
            if vQueuePropList[i].queue_count > 0
                && !(vQueuePropList[i].queue_flags & vk::QueueFlags::GRAPHICS).is_empty()
            {
                QueueNodeIndex = i as u32;
            }
            /*if(vQueuePropList[i].queue_count > 0 && (vQueuePropList[i].queue_flags &
            vk::QueueFlags::COMPUTE))
            {
                QueueNodeIndex = i;
            }*/
        }

        if QueueNodeIndex == u32::MAX {
            return Err(ArrayString::from_str(
                "No vulkan queue found that matches the requirements: graphics queue.",
            )
            .unwrap());
        }

        Ok((
            gpu_list,
            limits,
            renderer_name,
            vendor_name,
            version_name,
            CurDevice,
            QueueNodeIndex,
        ))
    }

    #[must_use]
    fn CreateLogicalDevice(
        phy_gpu: &vk::PhysicalDevice,
        graphics_queue_index: u32,
        instance: &ash::Instance,
        layers: &Vec<String>,
    ) -> Result<ash::Device, ArrayString<4096>> {
        let mut vLayerCNames = Vec::<*const libc::c_char>::new();
        let mut vLayerCNamesHelper = Vec::<CString>::new();
        vLayerCNames.reserve(layers.len());
        vLayerCNamesHelper.reserve(layers.len());
        for Layer in layers {
            let mut bytes = Layer.clone().into_bytes();
            bytes.push(0);
            vLayerCNamesHelper.push(CString::from_vec_with_nul(bytes).unwrap());
            vLayerCNames.push(vLayerCNamesHelper.last().unwrap().as_ptr());
        }

        let res = unsafe { instance.enumerate_device_extension_properties(*phy_gpu) };
        if res.is_err() {
            return Err(ArrayString::from_str(
                "Querying logical device extension properties failed.",
            )
            .unwrap());
        }
        let mut vDevPropList = res.unwrap();

        let mut vDevPropCNames = Vec::<*const libc::c_char>::new();
        let mut vDevPropCNamesHelper = Vec::<CString>::new();
        let OurDevExt = Self::OurDeviceExtensions();

        for CurExtProp in &mut vDevPropList {
            let ext_name = unsafe {
                CStr::from_ptr(CurExtProp.extension_name.as_ptr())
                    .to_str()
                    .unwrap()
                    .to_string()
            };
            let it = OurDevExt.get(&ext_name);
            if let Some(str) = it {
                vDevPropCNamesHelper
                    .push(unsafe { CString::from_vec_unchecked(str.as_bytes().to_vec()) });
                vDevPropCNames.push(vDevPropCNamesHelper.last().unwrap().as_ptr());
            }
        }

        let mut VKQueueCreateInfo = vk::DeviceQueueCreateInfo::default();
        VKQueueCreateInfo.queue_family_index = graphics_queue_index;
        VKQueueCreateInfo.queue_count = 1;
        let QueuePrio = 1.0;
        VKQueueCreateInfo.p_queue_priorities = &QueuePrio;
        VKQueueCreateInfo.flags = vk::DeviceQueueCreateFlags::default();

        let mut VKCreateInfo = vk::DeviceCreateInfo::default();
        VKCreateInfo.queue_create_info_count = 1;
        VKCreateInfo.p_queue_create_infos = &VKQueueCreateInfo;
        VKCreateInfo.pp_enabled_layer_names = vLayerCNames.as_ptr();
        VKCreateInfo.enabled_layer_count = vLayerCNames.len() as u32;
        VKCreateInfo.pp_enabled_extension_names = vDevPropCNames.as_ptr();
        VKCreateInfo.enabled_extension_count = vDevPropCNames.len() as u32;
        VKCreateInfo.p_enabled_features = std::ptr::null();
        VKCreateInfo.flags = vk::DeviceCreateFlags::empty();

        let res = unsafe { instance.create_device(*phy_gpu, &VKCreateInfo, None) };
        if res.is_err() {
            return Err(ArrayString::from_str("Logical device could not be created.").unwrap());
        }
        Ok(res.unwrap())
    }

    #[must_use]
    fn CreateSurface(
        pWindow: &sdl2::video::Window,
        surface: &ash::extensions::khr::Surface,
        vk_instance: &vk::Instance,
        phy_gpu: &vk::PhysicalDevice,
        queue_family_index: u32,
    ) -> Result<vk::SurfaceKHR, ArrayString<4096>> {
        let mut surface_khr = vk::SurfaceKHR::null();
        //(!SDL_Vulkan_CreateSurface(pWindow, self.m_VKInstance, &mut self.m_VKPresentSurface))
        let surf_res = pWindow.vulkan_create_surface(vk_instance.as_raw() as usize);
        if let Err(err) = surf_res {
            // TODO dbg_msg("vulkan", "error from sdl: %s", SDL_GetError());
            let mut res =
                ArrayString::from_str("Creating a vulkan surface for the SDL window failed: ")
                    .unwrap();
            res.push_str(err.as_str());
            return Err(res);
        }
        surface_khr = vk::SurfaceKHR::from_raw(surf_res.unwrap() as u64);

        let is_supported_res = unsafe {
            surface.get_physical_device_surface_support(*phy_gpu, queue_family_index, surface_khr)
        };
        if let Err(_err) = is_supported_res {
            return Err(ArrayString::from_str("No surface support on this device.").unwrap());
        }
        let is_supported = is_supported_res.unwrap();
        if !is_supported {
            return Err(ArrayString::from_str("The device surface does not support presenting the framebuffer to a screen. (maybe the wrong GPU was selected?)").unwrap());
        }

        Ok(surface_khr)
    }

    fn DestroySurface(&mut self) {
        unsafe { self.surface.destroy_surface(self.vk_present_surface, None) };
    }

    #[must_use]
    fn GetPresentationMode(&mut self, VKIOMode: &mut vk::PresentModeKHR) -> bool {
        let res = unsafe {
            self.surface
                .get_physical_device_surface_present_modes(self.vk_gpu, self.vk_present_surface)
        };
        if res.is_err() {
            self.error.lock().unwrap().SetError(
                EGFXErrorType::Init,
                "The device surface presentation modes could not be fetched.",
            );
            return false;
        }

        let vPresentModeList = res.unwrap();

        *VKIOMode = /*TODO!: g_Config.*/ if self.gfx_vsync { vk::PresentModeKHR::FIFO } else { vk::PresentModeKHR::IMMEDIATE };
        for Mode in &vPresentModeList {
            if Mode == VKIOMode {
                return true;
            }
        }

        // TODO dbg_msg("vulkan", "warning: requested presentation mode was not available. falling back to mailbox / fifo relaxed.");
        *VKIOMode = /*TODO!: g_Config.*/ if self.gfx_vsync { vk::PresentModeKHR::FIFO_RELAXED } else { vk::PresentModeKHR::MAILBOX };
        for Mode in &vPresentModeList {
            if Mode == VKIOMode {
                return true;
            }
        }

        // TODO dbg_msg("vulkan", "warning: requested presentation mode was not available. using first available.");
        if vPresentModeList.len() > 0 {
            *VKIOMode = vPresentModeList[0];
        }

        return true;
    }

    #[must_use]
    fn GetSurfaceProperties(
        &mut self,
        VKSurfCapabilities: &mut vk::SurfaceCapabilitiesKHR,
    ) -> bool {
        let capabilities_res = unsafe {
            self.surface
                .get_physical_device_surface_capabilities(self.vk_gpu, self.vk_present_surface)
        };
        if let Err(_) = capabilities_res {
            self.error.lock().unwrap().SetError(
                EGFXErrorType::Init,
                "The device surface capabilities could not be fetched.",
            );
            return false;
        }
        *VKSurfCapabilities = capabilities_res.unwrap();
        return true;
    }

    fn GetNumberOfSwapImages(&mut self, VKCapabilities: &vk::SurfaceCapabilitiesKHR) -> u32 {
        let ImgNumber = VKCapabilities.min_image_count + 1;
        if is_verbose(&*self.dbg) {
            self.sys
                .log("vulkan")
                .msg("minimal swap image count ")
                .msg_var(&VKCapabilities.min_image_count);
        }
        return if VKCapabilities.max_image_count > 0 && ImgNumber > VKCapabilities.max_image_count {
            VKCapabilities.max_image_count
        } else {
            ImgNumber
        };
    }

    fn GetSwapImageSize(
        &mut self,
        VKCapabilities: &vk::SurfaceCapabilitiesKHR,
    ) -> SSwapImgViewportExtent {
        let mut RetSize = vk::Extent2D {
            width: self.canvas_width,
            height: self.canvas_height,
        };

        if VKCapabilities.current_extent.width == u32::MAX {
            RetSize.width = RetSize.width.clamp(
                VKCapabilities.min_image_extent.width,
                VKCapabilities.max_image_extent.width,
            );
            RetSize.height = RetSize.height.clamp(
                VKCapabilities.min_image_extent.height,
                VKCapabilities.max_image_extent.height,
            );
        } else {
            RetSize = VKCapabilities.current_extent;
        }

        let mut AutoViewportExtent = RetSize;
        let mut UsesForcedViewport: bool = false;
        // keep this in sync with graphics_threaded AdjustViewport's check
        if AutoViewportExtent.height > 4 * AutoViewportExtent.width / 5 {
            AutoViewportExtent.height = 4 * AutoViewportExtent.width / 5;
            UsesForcedViewport = true;
        }

        let mut Ext = SSwapImgViewportExtent::default();
        Ext.swap_image_viewport = RetSize;
        Ext.forced_viewport = AutoViewportExtent;
        Ext.has_forced_viewport = UsesForcedViewport;

        return Ext;
    }

    #[must_use]
    fn GetImageUsage(
        &mut self,
        VKCapabilities: &vk::SurfaceCapabilitiesKHR,
        VKOutUsage: &mut vk::ImageUsageFlags,
    ) -> bool {
        let vOurImgUsages = Self::OurImageUsages();
        if vOurImgUsages.is_empty() {
            self.error.lock().unwrap().SetError(
                EGFXErrorType::Init,
                "Framebuffer image attachment types not supported.",
            );
            return false;
        }

        *VKOutUsage = vOurImgUsages[0];

        for ImgUsage in &vOurImgUsages {
            let ImgUsageFlags = *ImgUsage & VKCapabilities.supported_usage_flags;
            if ImgUsageFlags != *ImgUsage {
                self.error.lock().unwrap().SetError(
                    EGFXErrorType::Init,
                    "Framebuffer image attachment types not supported.",
                );
                return false;
            }

            *VKOutUsage = *VKOutUsage | *ImgUsage;
        }

        return true;
    }

    fn GetTransform(VKCapabilities: &vk::SurfaceCapabilitiesKHR) -> vk::SurfaceTransformFlagsKHR {
        if !(VKCapabilities.supported_transforms & vk::SurfaceTransformFlagsKHR::IDENTITY)
            .is_empty()
        {
            return vk::SurfaceTransformFlagsKHR::IDENTITY;
        }
        return VKCapabilities.current_transform;
    }

    #[must_use]
    fn GetFormat(&mut self) -> bool {
        let _SurfFormats: u32 = 0;
        let Res = unsafe {
            self.surface
                .get_physical_device_surface_formats(self.vk_gpu, self.vk_present_surface)
        };
        if Res.is_err() && *Res.as_ref().unwrap_err() != vk::Result::INCOMPLETE {
            self.error.lock().unwrap().SetError(
                EGFXErrorType::Init,
                "The device surface format fetching failed.",
            );
            return false;
        }

        if Res.is_err() && *Res.as_ref().unwrap_err() == vk::Result::INCOMPLETE {
            // TODO dbg_msg("vulkan", "warning: not all surface formats are requestable with your current settings.");
            // TODO!  SetError(EGFXErrorType::GFX_ERROR_TYPE_INIT, ("The device surface format fetching failed."));
            return false;
        }

        let vSurfFormatList = Res.unwrap();

        if vSurfFormatList.len() == 1 && vSurfFormatList[0].format == vk::Format::UNDEFINED {
            self.vk_surf_format.format = vk::Format::B8G8R8A8_UNORM;
            self.vk_surf_format.color_space = vk::ColorSpaceKHR::SRGB_NONLINEAR;
            // TODO dbg_msg("vulkan", "warning: surface format was undefined. This can potentially cause bugs.");
            return true;
        }

        for FindFormat in &vSurfFormatList {
            if FindFormat.format == vk::Format::B8G8R8A8_UNORM
                && FindFormat.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            {
                self.vk_surf_format = *FindFormat;
                return true;
            } else if FindFormat.format == vk::Format::R8G8B8A8_UNORM
                && FindFormat.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            {
                self.vk_surf_format = *FindFormat;
                return true;
            }
        }

        // TODO dbg_msg("vulkan", "warning: surface format was not RGBA(or variants of it). This can potentially cause weird looking images(too bright etc.).");
        self.vk_surf_format = vSurfFormatList[0];
        return true;
    }

    #[must_use]
    fn CreateSwapChain(&mut self, OldSwapChain: &mut vk::SwapchainKHR) -> bool {
        let mut VKSurfCap = vk::SurfaceCapabilitiesKHR::default();
        if !self.GetSurfaceProperties(&mut VKSurfCap) {
            return false;
        }

        let mut PresentMode = vk::PresentModeKHR::IMMEDIATE;
        if !self.GetPresentationMode(&mut PresentMode) {
            return false;
        }

        let SwapImgCount = self.GetNumberOfSwapImages(&VKSurfCap);

        self.vk_swap_img_and_viewport_extent = self.GetSwapImageSize(&VKSurfCap);

        let mut UsageFlags = vk::ImageUsageFlags::default();
        if !self.GetImageUsage(&VKSurfCap, &mut UsageFlags) {
            return false;
        }

        let TransformFlagBits = Self::GetTransform(&VKSurfCap);

        if !self.GetFormat() {
            return false;
        }

        *OldSwapChain = self.vk_swap_chain_khr;

        let mut SwapInfo = vk::SwapchainCreateInfoKHR::default();
        SwapInfo.flags = vk::SwapchainCreateFlagsKHR::empty();
        SwapInfo.surface = self.vk_present_surface;
        SwapInfo.min_image_count = SwapImgCount;
        SwapInfo.image_format = self.vk_surf_format.format;
        SwapInfo.image_color_space = self.vk_surf_format.color_space;
        SwapInfo.image_extent = self.vk_swap_img_and_viewport_extent.swap_image_viewport;
        SwapInfo.image_array_layers = 1;
        SwapInfo.image_usage = UsageFlags;
        SwapInfo.image_sharing_mode = vk::SharingMode::EXCLUSIVE;
        SwapInfo.queue_family_index_count = 0;
        SwapInfo.p_queue_family_indices = std::ptr::null();
        SwapInfo.pre_transform = TransformFlagBits;
        SwapInfo.composite_alpha = vk::CompositeAlphaFlagsKHR::OPAQUE;
        SwapInfo.present_mode = PresentMode;
        SwapInfo.clipped = vk::TRUE;
        SwapInfo.old_swapchain = *OldSwapChain;

        self.vk_swap_chain_khr = vk::SwapchainKHR::default();
        let res = unsafe { self.vk_swap_chain_ash.create_swapchain(&SwapInfo, None) };
        if res.is_err() {
            let pCritErrorMsg = self.check_res.CheckVulkanCriticalError(
                res.unwrap_err(),
                &self.error,
                &mut self.recreate_swap_chain,
            );
            if let Some(crit_err) = pCritErrorMsg {
                self.error.lock().unwrap().SetErrorExtra(
                    EGFXErrorType::Init,
                    "Creating the swap chain failed.",
                    Some(crit_err),
                );
                return false;
            } else if res.unwrap_err() == vk::Result::ERROR_NATIVE_WINDOW_IN_USE_KHR {
                return false;
            }
        }

        self.vk_swap_chain_khr = res.unwrap();

        return true;
    }

    fn DestroySwapChain(&mut self, ForceDestroy: bool) {
        if ForceDestroy {
            unsafe {
                self.vk_swap_chain_ash
                    .destroy_swapchain(self.vk_swap_chain_khr, None);
            }
            self.vk_swap_chain_khr = vk::SwapchainKHR::null();
        }
    }

    #[must_use]
    fn GetSwapChainImageHandles(&mut self) -> bool {
        let res = unsafe {
            self.vk_swap_chain_ash
                .get_swapchain_images(self.vk_swap_chain_khr)
        };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .SetError(EGFXErrorType::Init, "Could not get swap chain images.");
            return false;
        }

        self.vk_swap_chain_images = res.unwrap();
        self.device.swap_chain_image_count = self.vk_swap_chain_images.len() as u32;

        return true;
    }

    fn ClearSwapChainImageHandles(&mut self) {
        self.vk_swap_chain_images.clear();
    }

    fn GetDeviceQueue(
        device: &ash::Device,
        graphics_queue_index: u32,
    ) -> Result<(vk::Queue, vk::Queue), ArrayString<4096>> {
        Ok((
            unsafe { device.get_device_queue(graphics_queue_index, 0) },
            unsafe { device.get_device_queue(graphics_queue_index, 0) },
        ))
    }

    unsafe extern "system" fn VKDebugCallback(
        MessageSeverity: vk::DebugUtilsMessageSeverityFlagsEXT,
        _MessageType: vk::DebugUtilsMessageTypeFlagsEXT,
        pCallbackData: *const vk::DebugUtilsMessengerCallbackDataEXT,
        _pUserData: *mut c_void,
    ) -> vk::Bool32 {
        if !(MessageSeverity & vk::DebugUtilsMessageSeverityFlagsEXT::ERROR).is_empty() {
            println!("[vulkan debug] error: {}", unsafe {
                CStr::from_ptr((*pCallbackData).p_message).to_str().unwrap()
            });
        } else {
            println!("[vulkan debug] {}", unsafe {
                CStr::from_ptr((*pCallbackData).p_message).to_str().unwrap()
            });
        }

        return vk::FALSE;
    }

    fn CreateDebugUtilsMessengerEXT(
        entry: &ash::Entry,
        instance: &ash::Instance,
        pCreateInfo: &vk::DebugUtilsMessengerCreateInfoEXT,
        pAllocator: Option<&vk::AllocationCallbacks>,
    ) -> vk::DebugUtilsMessengerEXT {
        let dbg_utils = ash::extensions::ext::DebugUtils::new(entry, instance);
        let res = unsafe { dbg_utils.create_debug_utils_messenger(pCreateInfo, pAllocator) };
        if let Err(_res) = res {
            return vk::DebugUtilsMessengerEXT::null();
        }
        res.unwrap()
    }

    fn DestroyDebugUtilsMessengerEXT(_DebugMessenger: &mut vk::DebugUtilsMessengerEXT) {
        /* TODO! let func = unsafe { self.m_VKEntry.get_instance_proc_addr(self.m_VKInstance, "vkDestroyDebugUtilsMessengerEXT") as Option<vk::PFN_vkDestroyDebugUtilsMessengerEXT> };
        if let Some(f) = func
        {
            f(self.m_VKInstance, DebugMessenger, std::ptr::null());
        }*/
    }

    fn SetupDebugCallback(
        entry: &ash::Entry,
        instance: &ash::Instance,
        sys: &mut system::System,
    ) -> Result<vk::DebugUtilsMessengerEXT, ArrayString<4096>> {
        let mut CreateInfo = vk::DebugUtilsMessengerCreateInfoEXT::default();
        CreateInfo.message_severity = vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
            | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
            | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR;
        CreateInfo.message_type = vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE; // | vk::DEBUG_UTILS_MESSAGE_TYPE_GENERAL_BIT_EXT <- too annoying
        CreateInfo.pfn_user_callback = Some(Self::VKDebugCallback);

        let res_dbg = Self::CreateDebugUtilsMessengerEXT(entry, instance, &CreateInfo, None);
        if res_dbg == vk::DebugUtilsMessengerEXT::null() {
            sys.log("vulkan").msg("didn't find vulkan debug layer.");
            return Err(ArrayString::from_str("Debug extension could not be loaded.").unwrap());
        } else {
            sys.log("vulkan").msg("enabled vulkan debug context.");
        }
        return Ok(res_dbg);
    }

    fn UnregisterDebugCallback(&mut self) {
        if self.debug_messenger != vk::DebugUtilsMessengerEXT::null() {
            Self::DestroyDebugUtilsMessengerEXT(&mut self.debug_messenger);
        }
    }

    #[must_use]
    fn CreateImageViews(&mut self) -> bool {
        self.swap_chain_image_view_list.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );

        for i in 0..self.device.swap_chain_image_count {
            let mut CreateInfo = vk::ImageViewCreateInfo::default();
            CreateInfo.image = self.vk_swap_chain_images[i as usize];
            CreateInfo.view_type = vk::ImageViewType::TYPE_2D;
            CreateInfo.format = self.vk_surf_format.format;
            CreateInfo.components.r = vk::ComponentSwizzle::IDENTITY;
            CreateInfo.components.g = vk::ComponentSwizzle::IDENTITY;
            CreateInfo.components.b = vk::ComponentSwizzle::IDENTITY;
            CreateInfo.components.a = vk::ComponentSwizzle::IDENTITY;
            CreateInfo.subresource_range.aspect_mask = vk::ImageAspectFlags::COLOR;
            CreateInfo.subresource_range.base_mip_level = 0;
            CreateInfo.subresource_range.level_count = 1;
            CreateInfo.subresource_range.base_array_layer = 0;
            CreateInfo.subresource_range.layer_count = 1;

            let res = unsafe { self.vk_device.create_image_view(&CreateInfo, None) };
            if res.is_err() {
                self.error.lock().unwrap().SetError(
                    EGFXErrorType::Init,
                    "Could not create image views for the swap chain framebuffers.",
                );
                return false;
            }
            self.swap_chain_image_view_list[i as usize] = res.unwrap();
        }

        return true;
    }

    fn DestroyImageViews(&mut self) {
        for ImageView in &mut self.swap_chain_image_view_list {
            unsafe {
                self.vk_device.destroy_image_view(*ImageView, None);
            }
        }

        self.swap_chain_image_view_list.clear();
    }

    #[must_use]
    fn CreateMultiSamplerImageAttachments(&mut self) -> bool {
        self.swap_chain_multi_sampling_images.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );
        if self.HasMultiSampling() {
            for i in 0..self.device.swap_chain_image_count {
                let mut Img = vk::Image::default();
                let mut ImgMem = SMemoryImageBlock::<IMAGE_BUFFER_CACHE_ID>::default();
                if !self.device.CreateImageEx(
                    self.vk_swap_img_and_viewport_extent
                        .swap_image_viewport
                        .width,
                    self.vk_swap_img_and_viewport_extent
                        .swap_image_viewport
                        .height,
                    1,
                    1,
                    self.vk_surf_format.format,
                    vk::ImageTiling::OPTIMAL,
                    &mut Img,
                    &mut ImgMem,
                    vk::ImageUsageFlags::TRANSIENT_ATTACHMENT
                        | vk::ImageUsageFlags::COLOR_ATTACHMENT,
                ) {
                    return false;
                }
                self.swap_chain_multi_sampling_images[i as usize].image = Img;
                self.swap_chain_multi_sampling_images[i as usize].img_mem = ImgMem;
                self.swap_chain_multi_sampling_images[i as usize].img_view =
                    self.device.CreateImageView(
                        self.swap_chain_multi_sampling_images[i as usize].image,
                        self.vk_surf_format.format,
                        vk::ImageViewType::TYPE_2D,
                        1,
                        1,
                    );
            }
        }

        return true;
    }

    fn DestroyMultiSamplerImageAttachments(&mut self) {
        if self.HasMultiSampling() {
            self.swap_chain_multi_sampling_images.resize(
                self.device.swap_chain_image_count as usize,
                Default::default(),
            );
            for i in 0..self.device.swap_chain_image_count {
                unsafe {
                    self.vk_device.destroy_image(
                        self.swap_chain_multi_sampling_images[i as usize].image,
                        None,
                    );
                }
                unsafe {
                    self.vk_device.destroy_image_view(
                        self.swap_chain_multi_sampling_images[i as usize].img_view,
                        None,
                    );
                }
                Device::FreeImageMemBlock(
                    &mut self.device.frame_delayed_buffer_cleanups,
                    &mut self.device.image_buffer_caches,
                    &mut self.swap_chain_multi_sampling_images[i as usize].img_mem,
                    self.cur_image_index,
                );
            }
        }
        self.swap_chain_multi_sampling_images.clear();
    }

    #[must_use]
    fn CreateRenderPass(&mut self, ClearAttachs: bool) -> bool {
        let HasMultiSamplingTargets: bool = self.HasMultiSampling();
        let mut MultiSamplingColorAttachment = vk::AttachmentDescription::default();
        MultiSamplingColorAttachment.format = self.vk_surf_format.format;
        MultiSamplingColorAttachment.samples = Device::GetSampleCount(&self.device.limits);
        MultiSamplingColorAttachment.load_op = if ClearAttachs {
            vk::AttachmentLoadOp::CLEAR
        } else {
            vk::AttachmentLoadOp::DONT_CARE
        };
        MultiSamplingColorAttachment.store_op = vk::AttachmentStoreOp::DONT_CARE;
        MultiSamplingColorAttachment.stencil_load_op = vk::AttachmentLoadOp::DONT_CARE;
        MultiSamplingColorAttachment.stencil_store_op = vk::AttachmentStoreOp::DONT_CARE;
        MultiSamplingColorAttachment.initial_layout = vk::ImageLayout::UNDEFINED;
        MultiSamplingColorAttachment.final_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        let mut ColorAttachment = vk::AttachmentDescription::default();
        ColorAttachment.format = self.vk_surf_format.format;
        ColorAttachment.samples = vk::SampleCountFlags::TYPE_1;
        ColorAttachment.load_op = if ClearAttachs && !HasMultiSamplingTargets {
            vk::AttachmentLoadOp::CLEAR
        } else {
            vk::AttachmentLoadOp::DONT_CARE
        };
        ColorAttachment.store_op = vk::AttachmentStoreOp::STORE;
        ColorAttachment.stencil_load_op = vk::AttachmentLoadOp::DONT_CARE;
        ColorAttachment.stencil_store_op = vk::AttachmentStoreOp::DONT_CARE;
        ColorAttachment.initial_layout = vk::ImageLayout::UNDEFINED;
        ColorAttachment.final_layout = vk::ImageLayout::PRESENT_SRC_KHR;

        let mut MultiSamplingColorAttachmentRef = vk::AttachmentReference::default();
        MultiSamplingColorAttachmentRef.attachment = 0;
        MultiSamplingColorAttachmentRef.layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        let mut ColorAttachmentRef = vk::AttachmentReference::default();
        ColorAttachmentRef.attachment = if HasMultiSamplingTargets { 1 } else { 0 };
        ColorAttachmentRef.layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        let mut Subpass = vk::SubpassDescription::default();
        Subpass.pipeline_bind_point = vk::PipelineBindPoint::GRAPHICS;
        Subpass.color_attachment_count = 1;
        Subpass.p_color_attachments = if HasMultiSamplingTargets {
            &MultiSamplingColorAttachmentRef
        } else {
            &ColorAttachmentRef
        };
        Subpass.p_resolve_attachments = if HasMultiSamplingTargets {
            &ColorAttachmentRef
        } else {
            std::ptr::null()
        };

        let aAttachments = [MultiSamplingColorAttachment, ColorAttachment];

        let mut Dependency = vk::SubpassDependency::default();
        Dependency.src_subpass = vk::SUBPASS_EXTERNAL;
        Dependency.dst_subpass = 0;
        Dependency.src_stage_mask = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
        Dependency.src_access_mask = vk::AccessFlags::empty();
        Dependency.dst_stage_mask = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
        Dependency.dst_access_mask = vk::AccessFlags::COLOR_ATTACHMENT_WRITE;

        let mut CreateRenderPassInfo = vk::RenderPassCreateInfo::default();
        CreateRenderPassInfo.attachment_count = if HasMultiSamplingTargets { 2 } else { 1 };
        CreateRenderPassInfo.p_attachments = if HasMultiSamplingTargets {
            aAttachments.as_ptr()
        } else {
            unsafe { aAttachments.as_ptr().offset(1) }
        };
        CreateRenderPassInfo.subpass_count = 1;
        CreateRenderPassInfo.p_subpasses = &Subpass;
        CreateRenderPassInfo.dependency_count = 1;
        CreateRenderPassInfo.p_dependencies = &Dependency;

        let res = unsafe {
            self.vk_device
                .create_render_pass(&CreateRenderPassInfo, None)
        };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .SetError(EGFXErrorType::Init, "Creating the render pass failed.");
            return false;
        }
        self.vk_render_pass = res.unwrap();

        return true;
    }

    fn DestroyRenderPass(&mut self) {
        unsafe {
            self.vk_device
                .destroy_render_pass(self.vk_render_pass, None);
        }
    }

    #[must_use]
    fn CreateFramebuffers(&mut self) -> bool {
        self.framebuffer_list.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );

        for i in 0..self.device.swap_chain_image_count {
            let aAttachments = [
                self.swap_chain_multi_sampling_images[i as usize].img_view,
                self.swap_chain_image_view_list[i as usize],
            ];

            let HasMultiSamplingTargets: bool = self.HasMultiSampling();

            let mut FramebufferInfo = vk::FramebufferCreateInfo::default();
            FramebufferInfo.render_pass = self.vk_render_pass;
            FramebufferInfo.attachment_count = if HasMultiSamplingTargets {
                aAttachments.len()
            } else {
                aAttachments.len() - 1
            } as u32;
            FramebufferInfo.p_attachments = if HasMultiSamplingTargets {
                aAttachments.as_ptr()
            } else {
                unsafe { aAttachments.as_ptr().offset(1) }
            };
            FramebufferInfo.width = self
                .vk_swap_img_and_viewport_extent
                .swap_image_viewport
                .width;
            FramebufferInfo.height = self
                .vk_swap_img_and_viewport_extent
                .swap_image_viewport
                .height;
            FramebufferInfo.layers = 1;

            let res = unsafe { self.vk_device.create_framebuffer(&FramebufferInfo, None) };
            if res.is_err() {
                self.error
                    .lock()
                    .unwrap()
                    .SetError(EGFXErrorType::Init, "Creating the framebuffers failed.");
                return false;
            }
            self.framebuffer_list[i as usize] = res.unwrap();
        }

        return true;
    }

    fn DestroyFramebuffers(&mut self) {
        for FrameBuffer in &mut self.framebuffer_list {
            unsafe {
                self.vk_device.destroy_framebuffer(*FrameBuffer, None);
            }
        }

        self.framebuffer_list.clear();
    }

    #[must_use]
    fn CreateShaderModule(&mut self, vCode: &Vec<u8>, ShaderModule: &mut vk::ShaderModule) -> bool {
        let mut CreateInfo = vk::ShaderModuleCreateInfo::default();
        CreateInfo.code_size = vCode.len();
        CreateInfo.p_code = vCode.as_ptr() as _;

        let res = unsafe { self.vk_device.create_shader_module(&CreateInfo, None) };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .SetError(EGFXErrorType::Init, "Shader module was not created.");
            return false;
        }
        *ShaderModule = res.unwrap();

        return true;
    }

    #[must_use]
    fn CreateDescriptorSetLayouts(&mut self) -> bool {
        let mut SamplerLayoutBinding = vk::DescriptorSetLayoutBinding::default();
        SamplerLayoutBinding.binding = 0;
        SamplerLayoutBinding.descriptor_count = 1;
        SamplerLayoutBinding.descriptor_type = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        SamplerLayoutBinding.p_immutable_samplers = std::ptr::null();
        SamplerLayoutBinding.stage_flags = vk::ShaderStageFlags::FRAGMENT;

        let aBindings = [SamplerLayoutBinding];
        let mut LayoutInfo = vk::DescriptorSetLayoutCreateInfo::default();
        LayoutInfo.binding_count = aBindings.len() as u32;
        LayoutInfo.p_bindings = aBindings.as_ptr();

        let res = unsafe {
            self.vk_device
                .create_descriptor_set_layout(&LayoutInfo, None)
        };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .SetError(EGFXErrorType::Init, "Creating descriptor layout failed.");
            return false;
        }
        self.device.standard_textured_descriptor_set_layout = res.unwrap();

        let res = unsafe {
            self.vk_device
                .create_descriptor_set_layout(&LayoutInfo, None)
        };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .SetError(EGFXErrorType::Init, "Creating descriptor layout failed.");
            return false;
        }
        self.device.standard_3d_textured_descriptor_set_layout = res.unwrap();
        return true;
    }

    fn DestroyDescriptorSetLayouts(&mut self) {
        unsafe {
            self.vk_device.destroy_descriptor_set_layout(
                self.device.standard_textured_descriptor_set_layout,
                None,
            );
        }
        unsafe {
            self.vk_device.destroy_descriptor_set_layout(
                self.device.standard_3d_textured_descriptor_set_layout,
                None,
            );
        }
    }

    #[must_use]
    fn LoadShader(&mut self, pFileName: &str) -> Result<Vec<u8>, ArrayString<4096>> {
        let it = self.shader_files.get(pFileName);
        if let Some(f) = it {
            Ok(f.binary.clone())
        } else {
            let mut res = ArrayString::from_str("Shader file was not loaded: ").unwrap();
            res.push_str(pFileName);
            Err(res)
        }
    }

    #[must_use]
    fn CreateShaders(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        aShaderStages: &mut [vk::PipelineShaderStageCreateInfo; 2],
        ShaderModule: &mut SShaderModule,
    ) -> bool {
        let ShaderLoaded: bool = true;

        let vVertBuff = self.LoadShader(pVertName).unwrap();
        let vFragBuff = self.LoadShader(pFragName).unwrap();

        ShaderModule.vk_device = self.vk_device.clone();

        if !ShaderLoaded {
            self.error.lock().unwrap().SetError(
                EGFXErrorType::Init,
                "A shader file could not load correctly.",
            );
            return false;
        }

        if !self.CreateShaderModule(&vVertBuff, &mut ShaderModule.vert_shader_module) {
            return false;
        }

        if !self.CreateShaderModule(&vFragBuff, &mut ShaderModule.frag_shader_module) {
            return false;
        }

        let VertShaderStageInfo = &mut aShaderStages[0];
        *VertShaderStageInfo = vk::PipelineShaderStageCreateInfo::default();
        VertShaderStageInfo.stage = vk::ShaderStageFlags::VERTEX;
        VertShaderStageInfo.module = ShaderModule.vert_shader_module;
        VertShaderStageInfo.p_name = shader_main_func_name.as_ptr() as *const i8;

        let FragShaderStageInfo = &mut aShaderStages[1];
        *FragShaderStageInfo = vk::PipelineShaderStageCreateInfo::default();
        FragShaderStageInfo.stage = vk::ShaderStageFlags::FRAGMENT;
        FragShaderStageInfo.module = ShaderModule.frag_shader_module;
        FragShaderStageInfo.p_name = shader_main_func_name.as_ptr() as *const i8;
        return true;
    }

    fn GetStandardPipelineInfo(
        &mut self,
        InputAssembly: &mut vk::PipelineInputAssemblyStateCreateInfo,
        Viewport: &mut vk::Viewport,
        Scissor: &mut vk::Rect2D,
        ViewportState: &mut vk::PipelineViewportStateCreateInfo,
        Rasterizer: &mut vk::PipelineRasterizationStateCreateInfo,
        Multisampling: &mut vk::PipelineMultisampleStateCreateInfo,
        ColorBlendAttachment: &mut vk::PipelineColorBlendAttachmentState,
        ColorBlending: &mut vk::PipelineColorBlendStateCreateInfo,
        blend_mode: EVulkanBackendBlendModes,
    ) -> bool {
        InputAssembly.topology = vk::PrimitiveTopology::TRIANGLE_LIST;
        InputAssembly.primitive_restart_enable = vk::FALSE;

        Viewport.x = 0.0;
        Viewport.y = 0.0;
        Viewport.width = self
            .vk_swap_img_and_viewport_extent
            .swap_image_viewport
            .width as f32;
        Viewport.height = self
            .vk_swap_img_and_viewport_extent
            .swap_image_viewport
            .height as f32;
        Viewport.min_depth = 0.0;
        Viewport.max_depth = 1.0;

        Scissor.offset = vk::Offset2D { x: 0, y: 0 };
        Scissor.extent = self.vk_swap_img_and_viewport_extent.swap_image_viewport;

        ViewportState.viewport_count = 1;
        ViewportState.p_viewports = Viewport;
        ViewportState.scissor_count = 1;
        ViewportState.p_scissors = Scissor;

        Rasterizer.depth_clamp_enable = vk::FALSE;
        Rasterizer.rasterizer_discard_enable = vk::FALSE;
        Rasterizer.polygon_mode = vk::PolygonMode::FILL;
        Rasterizer.line_width = 1.0;
        Rasterizer.cull_mode = vk::CullModeFlags::NONE;
        Rasterizer.front_face = vk::FrontFace::CLOCKWISE;
        Rasterizer.depth_bias_enable = vk::FALSE;

        Multisampling.sample_shading_enable = vk::FALSE;
        Multisampling.rasterization_samples = Device::GetSampleCount(&self.device.limits);

        ColorBlendAttachment.color_write_mask = vk::ColorComponentFlags::R
            | vk::ColorComponentFlags::G
            | vk::ColorComponentFlags::B
            | vk::ColorComponentFlags::A;

        ColorBlendAttachment.blend_enable = if blend_mode == EVulkanBackendBlendModes::None {
            vk::FALSE
        } else {
            vk::TRUE
        };

        let src_blend_factor_color = match blend_mode {
            EVulkanBackendBlendModes::Additative => vk::BlendFactor::ONE,
            EVulkanBackendBlendModes::Alpha => vk::BlendFactor::SRC_ALPHA,
            EVulkanBackendBlendModes::None => vk::BlendFactor::SRC_COLOR,
            _ => panic!("not implemented."),
        };

        let dst_blend_factor_color = match blend_mode {
            EVulkanBackendBlendModes::Additative => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            EVulkanBackendBlendModes::Alpha => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            EVulkanBackendBlendModes::None => vk::BlendFactor::SRC_COLOR,
            _ => panic!("not implemented."),
        };

        let src_blend_factor_alpha = match blend_mode {
            EVulkanBackendBlendModes::Additative => vk::BlendFactor::ONE,
            EVulkanBackendBlendModes::Alpha => vk::BlendFactor::SRC_ALPHA,
            EVulkanBackendBlendModes::None => vk::BlendFactor::SRC_COLOR,
            _ => panic!("not implemented."),
        };

        let dst_blend_factor_alpha = match blend_mode {
            EVulkanBackendBlendModes::Additative => vk::BlendFactor::ZERO,
            EVulkanBackendBlendModes::Alpha => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            EVulkanBackendBlendModes::None => vk::BlendFactor::SRC_COLOR,
            _ => panic!("not implemented."),
        };

        ColorBlendAttachment.src_color_blend_factor = src_blend_factor_color;
        ColorBlendAttachment.dst_color_blend_factor = dst_blend_factor_color;
        ColorBlendAttachment.color_blend_op = vk::BlendOp::ADD;
        ColorBlendAttachment.src_alpha_blend_factor = src_blend_factor_alpha;
        ColorBlendAttachment.dst_alpha_blend_factor = dst_blend_factor_alpha;
        ColorBlendAttachment.alpha_blend_op = vk::BlendOp::ADD;

        ColorBlending.logic_op_enable = vk::FALSE;
        ColorBlending.logic_op = vk::LogicOp::COPY;
        ColorBlending.attachment_count = 1;
        ColorBlending.p_attachments = ColorBlendAttachment;
        ColorBlending.blend_constants[0] = 0.0;
        ColorBlending.blend_constants[1] = 0.0;
        ColorBlending.blend_constants[2] = 0.0;
        ColorBlending.blend_constants[3] = 0.0;

        return true;
    }

    #[must_use]
    fn CreateGraphicsPipelineEx<const ForceRequireDescriptors: bool>(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        PipeContainer: &mut SPipelineContainer,
        Stride: u32,
        aInputAttr: &mut [vk::VertexInputAttributeDescription],
        aSetLayouts: &mut [vk::DescriptorSetLayout],
        aPushConstants: &mut [vk::PushConstantRange],
        TexMode: EVulkanBackendTextureModes,
        BlendMode: EVulkanBackendBlendModes,
        DynamicMode: EVulkanBackendClipModes,
        IsLinePrim: bool,
    ) -> bool {
        let mut aShaderStages: [vk::PipelineShaderStageCreateInfo; 2] = Default::default();
        let mut Module = SShaderModule::new(&self.vk_device);
        if !self.CreateShaders(pVertName, pFragName, &mut aShaderStages, &mut Module) {
            return false;
        }

        let HasSampler: bool = TexMode == EVulkanBackendTextureModes::Textured;

        let mut VertexInputInfo = vk::PipelineVertexInputStateCreateInfo::default();
        let mut BindingDescription = vk::VertexInputBindingDescription::default();
        BindingDescription.binding = 0;
        BindingDescription.stride = Stride;
        BindingDescription.input_rate = vk::VertexInputRate::VERTEX;

        VertexInputInfo.vertex_binding_description_count = 1;
        VertexInputInfo.vertex_attribute_description_count = aInputAttr.len() as u32;
        VertexInputInfo.p_vertex_binding_descriptions = &BindingDescription;
        VertexInputInfo.p_vertex_attribute_descriptions = aInputAttr.as_ptr();

        let mut InputAssembly = vk::PipelineInputAssemblyStateCreateInfo::default();
        let mut Viewport = vk::Viewport::default();
        let mut Scissor = vk::Rect2D::default();
        let mut ViewportState = vk::PipelineViewportStateCreateInfo::default();
        let mut Rasterizer = vk::PipelineRasterizationStateCreateInfo::default();
        let mut Multisampling = vk::PipelineMultisampleStateCreateInfo::default();
        let mut ColorBlendAttachment = vk::PipelineColorBlendAttachmentState::default();
        let mut ColorBlending = vk::PipelineColorBlendStateCreateInfo::default();

        self.GetStandardPipelineInfo(
            &mut InputAssembly,
            &mut Viewport,
            &mut Scissor,
            &mut ViewportState,
            &mut Rasterizer,
            &mut Multisampling,
            &mut ColorBlendAttachment,
            &mut ColorBlending,
            BlendMode,
        );
        InputAssembly.topology = if IsLinePrim {
            vk::PrimitiveTopology::LINE_LIST
        } else {
            vk::PrimitiveTopology::TRIANGLE_LIST
        };

        let mut PipelineLayoutInfo = vk::PipelineLayoutCreateInfo::default();
        PipelineLayoutInfo.set_layout_count = if HasSampler || ForceRequireDescriptors {
            aSetLayouts.len() as u32
        } else {
            0
        };
        PipelineLayoutInfo.p_set_layouts =
            if (HasSampler || ForceRequireDescriptors) && !aSetLayouts.is_empty() {
                aSetLayouts.as_ptr()
            } else {
                std::ptr::null()
            };

        PipelineLayoutInfo.push_constant_range_count = aPushConstants.len() as u32;
        PipelineLayoutInfo.p_push_constant_ranges = if !aPushConstants.is_empty() {
            aPushConstants.as_ptr()
        } else {
            std::ptr::null()
        };

        let (Pipeline, PipeLayout) = Self::GetPipelineAndLayout_mut(
            PipeContainer,
            HasSampler,
            BlendMode as usize,
            (DynamicMode) as usize,
        );

        let res = unsafe {
            self.vk_device
                .create_pipeline_layout(&PipelineLayoutInfo, None)
        };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .SetError(EGFXErrorType::Init, "Creating pipeline layout failed.");
            return false;
        }
        *PipeLayout = res.unwrap();

        let mut PipelineInfo = vk::GraphicsPipelineCreateInfo::default();
        PipelineInfo.stage_count = aShaderStages.len() as u32;
        PipelineInfo.p_stages = aShaderStages.as_ptr();
        PipelineInfo.p_vertex_input_state = &VertexInputInfo;
        PipelineInfo.p_input_assembly_state = &InputAssembly;
        PipelineInfo.p_viewport_state = &ViewportState;
        PipelineInfo.p_rasterization_state = &Rasterizer;
        PipelineInfo.p_multisample_state = &Multisampling;
        PipelineInfo.p_color_blend_state = &ColorBlending;
        PipelineInfo.layout = *PipeLayout;
        PipelineInfo.render_pass = self.vk_render_pass;
        PipelineInfo.subpass = 0;
        PipelineInfo.base_pipeline_handle = vk::Pipeline::null();

        let aDynamicStates = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        let mut DynamicStateCreate = vk::PipelineDynamicStateCreateInfo::default();
        DynamicStateCreate.dynamic_state_count = aDynamicStates.len() as u32;
        DynamicStateCreate.p_dynamic_states = aDynamicStates.as_ptr();

        if DynamicMode == EVulkanBackendClipModes::DynamicScissorAndViewport {
            PipelineInfo.p_dynamic_state = &DynamicStateCreate;
        }

        let res = unsafe {
            self.vk_device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[PipelineInfo],
                None,
            )
        };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .SetError(EGFXErrorType::Init, "Creating the graphic pipeline failed.");
            return false;
        }
        *Pipeline = res.unwrap()[0]; // TODO correct?

        return true;
    }

    #[must_use]
    fn CreateGraphicsPipeline<const ForceRequireDescriptors: bool>(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        PipeContainer: &mut SPipelineContainer,
        Stride: u32,
        aInputAttr: &mut [vk::VertexInputAttributeDescription],
        aSetLayouts: &mut [vk::DescriptorSetLayout],
        aPushConstants: &mut [vk::PushConstantRange],
        TexMode: EVulkanBackendTextureModes,
        BlendMode: EVulkanBackendBlendModes,
        DynamicMode: EVulkanBackendClipModes,
    ) -> bool {
        return self.CreateGraphicsPipelineEx::<{ ForceRequireDescriptors }>(
            pVertName,
            pFragName,
            PipeContainer,
            Stride,
            aInputAttr,
            aSetLayouts,
            aPushConstants,
            TexMode,
            BlendMode,
            DynamicMode,
            false,
        );
    }

    #[must_use]
    fn CreateStandardGraphicsPipelineImpl(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        PipeContainer: &mut SPipelineContainer,
        TexMode: EVulkanBackendTextureModes,
        BlendMode: EVulkanBackendBlendModes,
        DynamicMode: EVulkanBackendClipModes,
        IsLinePrim: bool,
    ) -> bool {
        let mut aAttributeDescriptions: [vk::VertexInputAttributeDescription; 3] =
            Default::default();

        aAttributeDescriptions[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        aAttributeDescriptions[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: (std::mem::size_of::<f32>() * 2) as u32,
        };
        aAttributeDescriptions[2] = vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * (2 + 2)) as u32,
        };

        let mut aSetLayouts = [self.device.standard_textured_descriptor_set_layout];

        let mut aPushConstants = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: std::mem::size_of::<SUniformGPos>() as u32,
        }];

        return self.CreateGraphicsPipelineEx::<false>(
            pVertName,
            pFragName,
            PipeContainer,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &mut aAttributeDescriptions,
            &mut aSetLayouts,
            &mut aPushConstants,
            TexMode,
            BlendMode,
            DynamicMode,
            IsLinePrim,
        );
    }

    #[must_use]
    fn CreateStandardGraphicsPipeline(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        HasSampler: bool,
        IsLinePipe: bool,
    ) -> bool {
        let mut Ret: bool = true;

        let TexMode = if HasSampler {
            EVulkanBackendTextureModes::Textured
        } else {
            EVulkanBackendTextureModes::NotTextured
        };

        let mut pipe_container = if IsLinePipe {
            self.standard_line_pipeline.clone()
        } else {
            self.standard_pipeline.clone()
        };
        for i in 0..EVulkanBackendBlendModes::Count as usize {
            for j in 0..EVulkanBackendClipModes::Count as usize {
                Ret &= self.CreateStandardGraphicsPipelineImpl(
                    pVertName,
                    pFragName,
                    &mut pipe_container,
                    TexMode,
                    EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                    EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                    IsLinePipe,
                );
            }
        }

        let cont = if IsLinePipe {
            &mut self.standard_line_pipeline
        } else {
            &mut self.standard_pipeline
        };
        *cont = pipe_container;

        return Ret;
    }

    #[must_use]
    fn CreateStandard3DGraphicsPipelineImpl(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        PipeContainer: &mut SPipelineContainer,
        TexMode: EVulkanBackendTextureModes,
        BlendMode: EVulkanBackendBlendModes,
        DynamicMode: EVulkanBackendClipModes,
    ) -> bool {
        let mut aAttributeDescriptions: [vk::VertexInputAttributeDescription; 3] =
            Default::default();

        aAttributeDescriptions[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        aAttributeDescriptions[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * 2) as u32,
        };
        aAttributeDescriptions[2] = vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: (std::mem::size_of::<f32>() * 2 + std::mem::size_of::<u8>() * 4) as u32,
        };

        let mut aSetLayouts = [self.device.standard_3d_textured_descriptor_set_layout];

        let mut aPushConstants = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: std::mem::size_of::<SUniformGPos>() as u32,
        }];

        return self.CreateGraphicsPipeline::<false>(
            pVertName,
            pFragName,
            PipeContainer,
            (std::mem::size_of::<f32>() * 2
                + std::mem::size_of::<u8>() * 4
                + std::mem::size_of::<f32>() * 3) as u32,
            &mut aAttributeDescriptions,
            &mut aSetLayouts,
            &mut aPushConstants,
            TexMode,
            BlendMode,
            DynamicMode,
        );
    }

    #[must_use]
    fn CreateStandard3DGraphicsPipeline(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        HasSampler: bool,
    ) -> bool {
        let mut Ret: bool = true;

        let TexMode = if HasSampler {
            EVulkanBackendTextureModes::Textured
        } else {
            EVulkanBackendTextureModes::NotTextured
        };

        let mut pipe_container = self.standard_3d_pipeline.clone();
        for i in 0..EVulkanBackendBlendModes::Count as usize {
            for j in 0..EVulkanBackendClipModes::Count as usize {
                Ret &= self.CreateStandard3DGraphicsPipelineImpl(
                    pVertName,
                    pFragName,
                    &mut pipe_container,
                    TexMode,
                    EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                    EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                );
            }
        }
        self.standard_3d_pipeline = pipe_container;

        return Ret;
    }

    #[must_use]
    fn CreateTextDescriptorSetLayout(&mut self) -> bool {
        let mut SamplerLayoutBinding = vk::DescriptorSetLayoutBinding::default();
        SamplerLayoutBinding.binding = 0;
        SamplerLayoutBinding.descriptor_count = 1;
        SamplerLayoutBinding.descriptor_type = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        SamplerLayoutBinding.p_immutable_samplers = std::ptr::null();
        SamplerLayoutBinding.stage_flags = vk::ShaderStageFlags::FRAGMENT;

        let mut SamplerLayoutBinding2 = SamplerLayoutBinding.clone();
        SamplerLayoutBinding2.binding = 1;

        let aBindings = [SamplerLayoutBinding, SamplerLayoutBinding2];
        let mut LayoutInfo = vk::DescriptorSetLayoutCreateInfo::default();
        LayoutInfo.binding_count = aBindings.len() as u32;
        LayoutInfo.p_bindings = aBindings.as_ptr();

        let res = unsafe {
            self.vk_device
                .create_descriptor_set_layout(&LayoutInfo, None)
        };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .SetError(EGFXErrorType::Init, "Creating descriptor layout failed.");
            return false;
        }
        self.device.text_descriptor_set_layout = res.unwrap();

        return true;
    }

    fn DestroyTextDescriptorSetLayout(&mut self) {
        unsafe {
            self.vk_device
                .destroy_descriptor_set_layout(self.device.text_descriptor_set_layout, None);
        }
    }

    #[must_use]
    fn CreateTextGraphicsPipelineImpl(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        PipeContainer: &mut SPipelineContainer,
        TexMode: EVulkanBackendTextureModes,
        BlendMode: EVulkanBackendBlendModes,
        DynamicMode: EVulkanBackendClipModes,
    ) -> bool {
        let mut aAttributeDescriptions: [vk::VertexInputAttributeDescription; 3] =
            Default::default();
        aAttributeDescriptions[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        aAttributeDescriptions[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: (std::mem::size_of::<f32>() * 2) as u32,
        };
        aAttributeDescriptions[2] = vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * (2 + 2)) as u32,
        };

        let mut aSetLayouts = [self.device.text_descriptor_set_layout];

        let mut aPushConstants = [
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX,
                offset: 0,
                size: std::mem::size_of::<SUniformGTextPos>() as u32,
            },
            vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                offset: (std::mem::size_of::<SUniformGTextPos>()
                    + std::mem::size_of::<SUniformTextGFragmentOffset>())
                    as u32,
                size: std::mem::size_of::<SUniformTextGFragmentConstants>() as u32,
            },
        ];

        return self.CreateGraphicsPipeline::<false>(
            pVertName,
            pFragName,
            PipeContainer,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &mut aAttributeDescriptions,
            &mut aSetLayouts,
            &mut aPushConstants,
            TexMode,
            BlendMode,
            DynamicMode,
        );
    }

    #[must_use]
    fn CreateTextGraphicsPipeline(&mut self, pVertName: &str, pFragName: &str) -> bool {
        let mut Ret: bool = true;

        let TexMode = EVulkanBackendTextureModes::Textured;

        let mut pipe_container = self.text_pipeline.clone();
        for i in 0..EVulkanBackendBlendModes::Count as usize {
            for j in 0..EVulkanBackendClipModes::Count as usize {
                Ret &= self.CreateTextGraphicsPipelineImpl(
                    pVertName,
                    pFragName,
                    &mut pipe_container,
                    TexMode,
                    EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                    EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                );
            }
        }
        self.text_pipeline = pipe_container;

        return Ret;
    }

    const fn IfSamplerThen<const HasSampler: bool>(a: usize, b: usize) -> usize {
        if HasSampler {
            a
        } else {
            b
        }
    }

    #[must_use]
    fn CreateTileGraphicsPipelineImpl<const HasSampler: bool>(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        Type: i32,
        PipeContainer: &mut SPipelineContainer,
        TexMode: EVulkanBackendTextureModes,
        BlendMode: EVulkanBackendBlendModes,
        DynamicMode: EVulkanBackendClipModes,
    ) -> bool {
        let mut aAttributeDescriptions: [vk::VertexInputAttributeDescription; 2] =
            Default::default();
        aAttributeDescriptions[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        if HasSampler {
            aAttributeDescriptions[1] = vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: (std::mem::size_of::<f32>() * 2) as u32,
            };
        }

        let mut aSetLayouts = [self.device.standard_3d_textured_descriptor_set_layout];

        let mut VertPushConstantSize = std::mem::size_of::<SUniformTileGPos>();
        if Type == 1 {
            VertPushConstantSize = std::mem::size_of::<SUniformTileGPosBorder>();
        } else if Type == 2 {
            VertPushConstantSize = std::mem::size_of::<SUniformTileGPosBorderLine>();
        }

        let FragPushConstantSize = std::mem::size_of::<SUniformTileGVertColor>();

        let mut aPushConstants: [vk::PushConstantRange; 2] = Default::default();
        aPushConstants[0] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: VertPushConstantSize as u32,
        };
        aPushConstants[1] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            offset: (std::mem::size_of::<SUniformTileGPosBorder>()
                + std::mem::size_of::<SUniformTileGVertColorAlign>()) as u32,
            size: FragPushConstantSize as u32,
        };

        return self.CreateGraphicsPipeline::<false>(
            pVertName,
            pFragName,
            PipeContainer,
            if HasSampler {
                (std::mem::size_of::<f32>() * (2 + 3)) as u32
            } else {
                (std::mem::size_of::<f32>() * 2) as u32
            },
            &mut aAttributeDescriptions
                .split_at_mut(if HasSampler { 2 } else { 1 })
                .0,
            &mut aSetLayouts,
            &mut aPushConstants,
            TexMode,
            BlendMode,
            DynamicMode,
        );
    }

    #[must_use]
    fn CreateTileGraphicsPipeline<const HasSampler: bool>(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        Type: i32,
    ) -> bool {
        let mut Ret: bool = true;

        let TexMode = if HasSampler {
            EVulkanBackendTextureModes::Textured
        } else {
            EVulkanBackendTextureModes::NotTextured
        };

        let mut pipe_container = if Type == 0 {
            self.tile_pipeline.clone()
        } else {
            if Type == 1 {
                self.tile_border_pipeline.clone()
            } else {
                self.tile_border_line_pipeline.clone()
            }
        };
        for i in 0..EVulkanBackendBlendModes::Count as usize {
            for j in 0..EVulkanBackendClipModes::Count as usize {
                Ret &= self.CreateTileGraphicsPipelineImpl::<HasSampler>(
                    pVertName,
                    pFragName,
                    Type,
                    &mut pipe_container,
                    TexMode,
                    EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                    EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                );
            }
        }

        let cont = if Type == 0 {
            &mut self.tile_pipeline
        } else {
            if Type == 1 {
                &mut self.tile_border_pipeline
            } else {
                &mut self.tile_border_line_pipeline
            }
        };
        *cont = pipe_container;

        return Ret;
    }

    #[must_use]
    fn CreatePrimExGraphicsPipelineImpl(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        Rotationless: bool,
        PipeContainer: &mut SPipelineContainer,
        TexMode: EVulkanBackendTextureModes,
        BlendMode: EVulkanBackendBlendModes,
        DynamicMode: EVulkanBackendClipModes,
    ) -> bool {
        let mut aAttributeDescriptions: [vk::VertexInputAttributeDescription; 3] =
            Default::default();
        aAttributeDescriptions[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        aAttributeDescriptions[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: (std::mem::size_of::<f32>() * 2) as u32,
        };
        aAttributeDescriptions[2] = vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * (2 + 2)) as u32,
        };

        let mut aSetLayouts = [self.device.standard_textured_descriptor_set_layout];
        let mut VertPushConstantSize = std::mem::size_of::<SUniformPrimExGPos>();
        if Rotationless {
            VertPushConstantSize = std::mem::size_of::<SUniformPrimExGPosRotationless>();
        }

        let FragPushConstantSize = std::mem::size_of::<SUniformPrimExGVertColor>();

        let mut aPushConstants: [vk::PushConstantRange; 2] = Default::default();
        aPushConstants[0] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: VertPushConstantSize as u32,
        };
        aPushConstants[1] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            offset: (std::mem::size_of::<SUniformPrimExGPos>()
                + std::mem::size_of::<SUniformPrimExGVertColorAlign>()) as u32,
            size: FragPushConstantSize as u32,
        };

        return self.CreateGraphicsPipeline::<false>(
            pVertName,
            pFragName,
            PipeContainer,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &mut aAttributeDescriptions,
            &mut aSetLayouts,
            &mut aPushConstants,
            TexMode,
            BlendMode,
            DynamicMode,
        );
    }

    #[must_use]
    fn CreatePrimExGraphicsPipeline(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        HasSampler: bool,
        Rotationless: bool,
    ) -> bool {
        let mut Ret: bool = true;

        let TexMode = if HasSampler {
            EVulkanBackendTextureModes::Textured
        } else {
            EVulkanBackendTextureModes::NotTextured
        };

        let mut pipe_container = if Rotationless {
            self.prim_ex_rotationless_pipeline.clone()
        } else {
            self.prim_ex_pipeline.clone()
        };
        for i in 0..EVulkanBackendBlendModes::Count as usize {
            for j in 0..EVulkanBackendClipModes::Count as usize {
                Ret &= self.CreatePrimExGraphicsPipelineImpl(
                    pVertName,
                    pFragName,
                    Rotationless,
                    &mut pipe_container,
                    TexMode,
                    EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                    EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                );
            }
        }
        let cont = if Rotationless {
            &mut self.prim_ex_rotationless_pipeline
        } else {
            &mut self.prim_ex_pipeline
        };
        *cont = pipe_container;

        return Ret;
    }

    #[must_use]
    fn CreateSpriteMultiGraphicsPipelineImpl(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        PipeContainer: &mut SPipelineContainer,
        TexMode: EVulkanBackendTextureModes,
        BlendMode: EVulkanBackendBlendModes,
        DynamicMode: EVulkanBackendClipModes,
    ) -> bool {
        let mut aAttributeDescriptions: [vk::VertexInputAttributeDescription; 3] =
            Default::default();
        aAttributeDescriptions[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        aAttributeDescriptions[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: (std::mem::size_of::<f32>() * 2) as u32,
        };
        aAttributeDescriptions[2] = vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * (2 + 2)) as u32,
        };

        let mut aSetLayouts = [
            self.device.standard_textured_descriptor_set_layout,
            self.device.sprite_multi_uniform_descriptor_set_layout,
        ];

        let VertPushConstantSize = std::mem::size_of::<SUniformSpriteMultiGPos>() as u32;
        let FragPushConstantSize = std::mem::size_of::<SUniformSpriteMultiGVertColor>() as u32;

        let mut aPushConstants: [vk::PushConstantRange; 2] = Default::default();
        aPushConstants[0] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: VertPushConstantSize,
        };
        aPushConstants[1] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            offset: (std::mem::size_of::<SUniformSpriteMultiGPos>()
                + std::mem::size_of::<SUniformSpriteMultiGVertColorAlign>())
                as u32,
            size: FragPushConstantSize,
        };

        return self.CreateGraphicsPipeline::<false>(
            pVertName,
            pFragName,
            PipeContainer,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &mut aAttributeDescriptions,
            &mut aSetLayouts,
            &mut aPushConstants,
            TexMode,
            BlendMode,
            DynamicMode,
        );
    }

    #[must_use]
    fn CreateSpriteMultiGraphicsPipeline(&mut self, pVertName: &str, pFragName: &str) -> bool {
        let mut Ret: bool = true;

        let TexMode = EVulkanBackendTextureModes::Textured;

        let mut pipe_container = self.sprite_multi_pipeline.clone();
        for i in 0..EVulkanBackendBlendModes::Count as usize {
            for j in 0..EVulkanBackendClipModes::Count as usize {
                Ret &= self.CreateSpriteMultiGraphicsPipelineImpl(
                    pVertName,
                    pFragName,
                    &mut pipe_container,
                    TexMode,
                    EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                    EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                );
            }
        }
        self.sprite_multi_pipeline = pipe_container;

        return Ret;
    }

    #[must_use]
    fn CreateSpriteMultiPushGraphicsPipelineImpl(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        PipeContainer: &mut SPipelineContainer,
        TexMode: EVulkanBackendTextureModes,
        BlendMode: EVulkanBackendBlendModes,
        DynamicMode: EVulkanBackendClipModes,
    ) -> bool {
        let mut aAttributeDescriptions: [vk::VertexInputAttributeDescription; 3] =
            Default::default();
        aAttributeDescriptions[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        aAttributeDescriptions[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: (std::mem::size_of::<f32>() * 2) as u32,
        };
        aAttributeDescriptions[2] = vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * (2 + 2)) as u32,
        };

        let mut aSetLayouts = [self.device.standard_textured_descriptor_set_layout];

        let VertPushConstantSize = std::mem::size_of::<SUniformSpriteMultiPushGPos>();
        let FragPushConstantSize = std::mem::size_of::<SUniformSpriteMultiPushGVertColor>();

        let mut aPushConstants: [vk::PushConstantRange; 2] = Default::default();
        aPushConstants[0] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: VertPushConstantSize as u32,
        };
        aPushConstants[1] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            offset: (std::mem::size_of::<SUniformSpriteMultiPushGPos>()) as u32,
            size: FragPushConstantSize as u32,
        };

        return self.CreateGraphicsPipeline::<false>(
            pVertName,
            pFragName,
            PipeContainer,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &mut aAttributeDescriptions,
            &mut aSetLayouts,
            &mut aPushConstants,
            TexMode,
            BlendMode,
            DynamicMode,
        );
    }

    #[must_use]
    fn CreateSpriteMultiPushGraphicsPipeline(&mut self, pVertName: &str, pFragName: &str) -> bool {
        let mut Ret: bool = true;

        let TexMode = EVulkanBackendTextureModes::Textured;

        let mut pipe_container = self.sprite_multi_push_pipeline.clone();
        for i in 0..EVulkanBackendBlendModes::Count as usize {
            for j in 0..EVulkanBackendClipModes::Count as usize {
                Ret &= self.CreateSpriteMultiPushGraphicsPipelineImpl(
                    pVertName,
                    pFragName,
                    &mut pipe_container,
                    TexMode,
                    EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                    EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                );
            }
        }
        self.sprite_multi_push_pipeline = pipe_container;

        return Ret;
    }

    #[must_use]
    fn CreateQuadGraphicsPipelineImpl<const IsTextured: bool>(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        PipeContainer: &mut SPipelineContainer,
        TexMode: EVulkanBackendTextureModes,
        BlendMode: EVulkanBackendBlendModes,
        DynamicMode: EVulkanBackendClipModes,
    ) -> bool {
        let mut aAttributeDescriptions: [vk::VertexInputAttributeDescription; 3] =
            Default::default();
        aAttributeDescriptions[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32B32A32_SFLOAT,
            offset: 0,
        };
        aAttributeDescriptions[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * 4) as u32,
        };
        if IsTextured {
            aAttributeDescriptions[2] = vk::VertexInputAttributeDescription {
                location: 2,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: (std::mem::size_of::<f32>() * 4 + std::mem::size_of::<u8>() * 4) as u32,
            };
        }

        let mut aSetLayouts: [vk::DescriptorSetLayout; 2] = Default::default();
        if IsTextured {
            aSetLayouts[0] = self.device.standard_textured_descriptor_set_layout;
            aSetLayouts[1] = self.device.quad_uniform_descriptor_set_layout;
        } else {
            aSetLayouts[0] = self.device.quad_uniform_descriptor_set_layout;
        }

        let PushConstantSize = std::mem::size_of::<SUniformQuadGPos>();

        let mut aPushConstants = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: PushConstantSize as u32,
        }];

        return self.CreateGraphicsPipeline::<true>(
            pVertName,
            pFragName,
            PipeContainer,
            (std::mem::size_of::<f32>() * 4
                + std::mem::size_of::<u8>() * 4
                + (if IsTextured {
                    std::mem::size_of::<f32>() * 2
                } else {
                    0
                })) as u32,
            &mut aAttributeDescriptions
                .split_at_mut(if IsTextured { 3 } else { 2 })
                .0,
            &mut aSetLayouts.split_at_mut(if IsTextured { 2 } else { 1 }).0,
            &mut aPushConstants,
            TexMode,
            BlendMode,
            DynamicMode,
        );
    }

    #[must_use]
    fn CreateQuadGraphicsPipeline<const HasSampler: bool>(
        &mut self,
        pVertName: &str,
        pFragName: &str,
    ) -> bool {
        let mut Ret: bool = true;

        let TexMode = if HasSampler {
            EVulkanBackendTextureModes::Textured
        } else {
            EVulkanBackendTextureModes::NotTextured
        };

        let mut pipe_container = self.quad_pipeline.clone();
        for i in 0..EVulkanBackendBlendModes::Count as usize {
            for j in 0..EVulkanBackendClipModes::Count as usize {
                Ret &= self.CreateQuadGraphicsPipelineImpl::<HasSampler>(
                    pVertName,
                    pFragName,
                    &mut pipe_container,
                    TexMode,
                    EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                    EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                );
            }
        }
        self.quad_pipeline = pipe_container;

        return Ret;
    }

    #[must_use]
    fn CreateQuadPushGraphicsPipelineImpl<const IsTextured: bool>(
        &mut self,
        pVertName: &str,
        pFragName: &str,
        PipeContainer: &mut SPipelineContainer,
        TexMode: EVulkanBackendTextureModes,
        BlendMode: EVulkanBackendBlendModes,
        DynamicMode: EVulkanBackendClipModes,
    ) -> bool {
        let mut aAttributeDescriptions: [vk::VertexInputAttributeDescription; 3] =
            Default::default();
        aAttributeDescriptions[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32B32A32_SFLOAT,
            offset: 0,
        };
        aAttributeDescriptions[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * 4) as u32,
        };
        if IsTextured {
            aAttributeDescriptions[2] = vk::VertexInputAttributeDescription {
                location: 2,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: (std::mem::size_of::<f32>() * 4 + std::mem::size_of::<u8>() * 4) as u32,
            };
        }

        let mut aSetLayouts = [self.device.standard_textured_descriptor_set_layout];

        let PushConstantSize = std::mem::size_of::<SUniformQuadPushGPos>();

        let mut aPushConstants = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: PushConstantSize as u32,
        }];

        return self.CreateGraphicsPipeline::<false>(
            pVertName,
            pFragName,
            PipeContainer,
            (std::mem::size_of::<f32>() * 4
                + std::mem::size_of::<u8>() * 4
                + (if IsTextured {
                    std::mem::size_of::<f32>() * 2
                } else {
                    0
                })) as u32,
            &mut aAttributeDescriptions
                .split_at_mut(if IsTextured { 3 } else { 2 })
                .0,
            &mut aSetLayouts,
            &mut aPushConstants,
            TexMode,
            BlendMode,
            DynamicMode,
        );
    }

    #[must_use]
    fn CreateQuadPushGraphicsPipeline<const HasSampler: bool>(
        &mut self,
        pVertName: &str,
        pFragName: &str,
    ) -> bool {
        let mut Ret: bool = true;

        let TexMode = if HasSampler {
            EVulkanBackendTextureModes::Textured
        } else {
            EVulkanBackendTextureModes::NotTextured
        };

        let mut pipe_container = self.quad_push_pipeline.clone();
        for i in 0..EVulkanBackendBlendModes::Count as usize {
            for j in 0..EVulkanBackendClipModes::Count as usize {
                Ret &= self.CreateQuadPushGraphicsPipelineImpl::<HasSampler>(
                    pVertName,
                    pFragName,
                    &mut pipe_container,
                    TexMode,
                    EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                    EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                );
            }
        }
        self.quad_push_pipeline = pipe_container;

        return Ret;
    }

    #[must_use]
    fn CreateCommandPool(&mut self) -> bool {
        let mut CreatePoolInfo = vk::CommandPoolCreateInfo::default();
        CreatePoolInfo.queue_family_index = self.vk_graphics_queue_index;
        CreatePoolInfo.flags = vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER;

        self.command_pools
            .resize(self.thread_count, Default::default());
        for i in 0..self.thread_count {
            let res = unsafe { self.vk_device.create_command_pool(&CreatePoolInfo, None) };
            if res.is_err() {
                self.error
                    .lock()
                    .unwrap()
                    .SetError(EGFXErrorType::Init, "Creating the command pool failed.");
                return false;
            }
            self.command_pools[i] = res.unwrap();
        }
        return true;
    }

    fn DestroyCommandPool(&mut self) {
        for i in 0..self.thread_count {
            unsafe {
                self.vk_device
                    .destroy_command_pool(self.command_pools[i], None);
            }
        }
    }

    #[must_use]
    fn CreateCommandBuffers(&mut self) -> bool {
        self.main_draw_command_buffers.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );
        if self.thread_count > 1 {
            self.thread_draw_command_buffers
                .resize(self.thread_count, Default::default());
            self.used_thread_draw_command_buffer
                .resize(self.thread_count, Default::default());
            self.helper_thread_draw_command_buffers
                .resize(self.thread_count, Default::default());
            for ThreadDrawCommandBuffers in &mut self.thread_draw_command_buffers {
                ThreadDrawCommandBuffers.resize(
                    self.device.swap_chain_image_count as usize,
                    Default::default(),
                );
            }
            for UsedThreadDrawCommandBuffer in &mut self.used_thread_draw_command_buffer {
                UsedThreadDrawCommandBuffer
                    .resize(self.device.swap_chain_image_count as usize, false);
            }
        }
        self.device.memory_command_buffers.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );
        self.device
            .used_memory_command_buffer
            .resize(self.device.swap_chain_image_count as usize, false);

        let mut AllocInfo = vk::CommandBufferAllocateInfo::default();
        AllocInfo.command_pool = self.command_pools[0];
        AllocInfo.level = vk::CommandBufferLevel::PRIMARY;
        AllocInfo.command_buffer_count = self.main_draw_command_buffers.len() as u32;

        let res = unsafe { self.vk_device.allocate_command_buffers(&AllocInfo) };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .SetError(EGFXErrorType::Init, "Allocating command buffers failed.");
            return false;
        }
        self.main_draw_command_buffers = res.unwrap();

        AllocInfo.command_buffer_count = self.device.memory_command_buffers.len() as u32;

        let res = unsafe { self.vk_device.allocate_command_buffers(&AllocInfo) };
        if res.is_err() {
            self.error.lock().unwrap().SetError(
                EGFXErrorType::Init,
                "Allocating memory command buffers failed.",
            );
            return false;
        }
        self.device.memory_command_buffers = res.unwrap();

        if self.thread_count > 1 {
            let mut Count: usize = 0;
            for ThreadDrawCommandBuffers in &mut self.thread_draw_command_buffers {
                AllocInfo.command_pool = self.command_pools[Count];
                Count += 1;
                AllocInfo.command_buffer_count = ThreadDrawCommandBuffers.len() as u32;
                AllocInfo.level = vk::CommandBufferLevel::SECONDARY;
                let res = unsafe { self.vk_device.allocate_command_buffers(&AllocInfo) };
                if res.is_err() {
                    self.error.lock().unwrap().SetError(
                        EGFXErrorType::Init,
                        "Allocating thread command buffers failed.",
                    );
                    return false;
                }
                *ThreadDrawCommandBuffers = res.unwrap();
            }
        }

        return true;
    }

    fn DestroyCommandBuffer(&mut self) {
        if self.thread_count > 1 {
            let mut Count: usize = 0;
            for ThreadDrawCommandBuffers in &self.thread_draw_command_buffers {
                unsafe {
                    self.vk_device.free_command_buffers(
                        self.command_pools[Count],
                        ThreadDrawCommandBuffers.as_slice(),
                    );
                }
                Count += 1;
            }
        }

        unsafe {
            self.vk_device.free_command_buffers(
                self.command_pools[0],
                self.device.memory_command_buffers.as_slice(),
            );
        }
        unsafe {
            self.vk_device.free_command_buffers(
                self.command_pools[0],
                self.main_draw_command_buffers.as_slice(),
            );
        }

        self.thread_draw_command_buffers.clear();
        self.used_thread_draw_command_buffer.clear();
        self.helper_thread_draw_command_buffers.clear();

        self.main_draw_command_buffers.clear();
        self.device.memory_command_buffers.clear();
        self.device.used_memory_command_buffer.clear();
    }

    #[must_use]
    fn CreateSyncObjects(&mut self) -> bool {
        self.wait_semaphores.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );
        self.sig_semaphores.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );

        self.memory_sempahores.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );

        self.frame_fences.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );
        self.image_fences.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );

        let CreateSemaphoreInfo = vk::SemaphoreCreateInfo::default();

        let mut FenceInfo = vk::FenceCreateInfo::default();
        FenceInfo.flags = vk::FenceCreateFlags::SIGNALED;

        for i in 0..self.device.swap_chain_image_count {
            let res = unsafe { self.vk_device.create_semaphore(&CreateSemaphoreInfo, None) };
            let res2 = unsafe { self.vk_device.create_semaphore(&CreateSemaphoreInfo, None) };
            let res3 = unsafe { self.vk_device.create_semaphore(&CreateSemaphoreInfo, None) };
            let res4 = unsafe { self.vk_device.create_fence(&FenceInfo, None) };
            if res.is_err() || res2.is_err() || res3.is_err() || res4.is_err() {
                self.error.lock().unwrap().SetError(
                    EGFXErrorType::Init,
                    "Creating swap chain sync objects(fences, semaphores) failed.",
                );
                return false;
            }
            self.wait_semaphores[i as usize] = res.unwrap();
            self.sig_semaphores[i as usize] = res2.unwrap();
            self.memory_sempahores[i as usize] = res3.unwrap();
            self.frame_fences[i as usize] = res4.unwrap();
        }

        return true;
    }

    fn DestroySyncObjects(&mut self) {
        for i in 0..self.device.swap_chain_image_count {
            unsafe {
                self.vk_device
                    .destroy_semaphore(self.wait_semaphores[i as usize], None);
            }
            unsafe {
                self.vk_device
                    .destroy_semaphore(self.sig_semaphores[i as usize], None);
            }
            unsafe {
                self.vk_device
                    .destroy_semaphore(self.memory_sempahores[i as usize], None);
            }
            unsafe {
                self.vk_device
                    .destroy_fence(self.frame_fences[i as usize], None);
            }
        }

        self.wait_semaphores.clear();
        self.sig_semaphores.clear();

        self.memory_sempahores.clear();

        self.frame_fences.clear();
        self.image_fences.clear();
    }

    fn DestroyBufferOfFrame(mem: &mut Memory, ImageIndex: usize, Buffer: &mut SFrameBuffers) {
        mem.CleanBufferPair(ImageIndex, &mut Buffer.buffer, &mut Buffer.buffer_mem);
    }

    fn DestroyUniBufferOfFrame(
        mem: &mut Memory,
        device: &ash::Device,
        ImageIndex: usize,
        Buffer: &mut SFrameUniformBuffers,
    ) {
        mem.CleanBufferPair(
            ImageIndex,
            &mut Buffer.base.buffer,
            &mut Buffer.base.buffer_mem,
        );
        for DescrSet in &mut Buffer.uniform_sets {
            if DescrSet.descriptor != vk::DescriptorSet::null() {
                Device::DestroyUniformDescriptorSets(device, DescrSet, 1);
            }
        }
    }

    /*************
     * SWAP CHAIN
     **************/

    fn CleanupVulkanSwapChain(&mut self, ForceSwapChainDestruct: bool) {
        self.standard_pipeline.destroy(&self.vk_device);
        self.standard_line_pipeline.destroy(&self.vk_device);
        self.standard_3d_pipeline.destroy(&self.vk_device);
        self.text_pipeline.destroy(&self.vk_device);
        self.tile_pipeline.destroy(&self.vk_device);
        self.tile_border_pipeline.destroy(&self.vk_device);
        self.tile_border_line_pipeline.destroy(&self.vk_device);
        self.prim_ex_pipeline.destroy(&self.vk_device);
        self.prim_ex_rotationless_pipeline.destroy(&self.vk_device);
        self.sprite_multi_pipeline.destroy(&self.vk_device);
        self.sprite_multi_push_pipeline.destroy(&self.vk_device);
        self.quad_pipeline.destroy(&self.vk_device);
        self.quad_push_pipeline.destroy(&self.vk_device);

        self.DestroyFramebuffers();

        self.DestroyRenderPass();

        self.DestroyMultiSamplerImageAttachments();

        self.DestroyImageViews();
        self.ClearSwapChainImageHandles();

        self.DestroySwapChain(ForceSwapChainDestruct);

        self.swap_chain_created = false;
    }

    fn CleanupVulkan<const IsLastCleanup: bool>(&mut self) {
        if IsLastCleanup {
            if self.swap_chain_created {
                self.CleanupVulkanSwapChain(true);
            }

            // clean all images, buffers, buffer containers
            for Texture in &mut self.device.textures {
                if Texture.vk_text_descr_set.descriptor != vk::DescriptorSet::null()
                    && is_verbose(&*self.dbg)
                {
                    // TODO  dbg_msg("vulkan", "text textures not cleared over cmd.");
                }
                Device::DestroyTexture(
                    &mut self.device.frame_delayed_buffer_cleanups,
                    &mut self.device.image_buffer_caches,
                    &self.vk_device,
                    Texture,
                    self.cur_image_index,
                );
            }

            for buffer_object in &mut self.device.buffer_objects {
                Device::FreeVertexMemBlock(
                    &mut self.device.frame_delayed_buffer_cleanups,
                    &mut self.device.vertex_buffer_cache,
                    &mut buffer_object.buffer_object.mem,
                    self.cur_image_index,
                );
            }

            self.device.buffer_containers.clear();
        }

        self.image_last_frame_check.clear();

        self.last_pipeline_per_thread.clear();

        self.device
            .streamed_vertex_buffer
            .destroy(&mut |ImageIndex, Buffer| {
                Self::DestroyBufferOfFrame(&mut self.device.mem, ImageIndex, Buffer);
            });
        for i in 0..self.thread_count {
            self.device.streamed_uniform_buffers[i].destroy(&mut |ImageIndex, Buffer| {
                Self::DestroyUniBufferOfFrame(
                    &mut self.device.mem,
                    &self.device.device,
                    ImageIndex,
                    Buffer,
                );
            });
        }
        self.device.streamed_vertex_buffer = Default::default();
        self.device.streamed_uniform_buffers.clear();

        for i in 0..self.device.swap_chain_image_count {
            self.ClearFrameData(i as usize);
        }

        self.device.frame_delayed_buffer_cleanups.clear();
        self.device.frame_delayed_texture_cleanups.clear();
        self.device.frame_delayed_text_textures_cleanups.clear();

        self.device
            .staging_buffer_cache
            .destroy_frame_data(self.device.swap_chain_image_count as usize);
        self.device
            .staging_buffer_cache_image
            .destroy_frame_data(self.device.swap_chain_image_count as usize);
        self.device
            .vertex_buffer_cache
            .destroy_frame_data(self.device.swap_chain_image_count as usize);
        for ImageBufferCache in &mut self.device.image_buffer_caches {
            ImageBufferCache
                .1
                .destroy_frame_data(self.device.swap_chain_image_count as usize);
        }

        if IsLastCleanup {
            self.device.staging_buffer_cache.destroy(&self.vk_device);
            self.device
                .staging_buffer_cache_image
                .destroy(&self.vk_device);
            self.device.vertex_buffer_cache.destroy(&self.vk_device);
            for ImageBufferCache in &mut self.device.image_buffer_caches {
                ImageBufferCache.1.destroy(&self.vk_device);
            }

            self.device.image_buffer_caches.clear();

            self.DestroyTextureSamplers();
            self.DestroyDescriptorPools();

            // TODO! self.DeletePresentedImageDataImage();
        }

        self.DestroySyncObjects();
        self.DestroyCommandBuffer();

        if IsLastCleanup {
            self.DestroyCommandPool();
        }

        if IsLastCleanup {
            self.device.DestroyUniformDescriptorSetLayouts();
            self.DestroyTextDescriptorSetLayout();
            self.DestroyDescriptorSetLayouts();
        }
    }

    fn CleanupVulkanSDL(&mut self) {
        if self.vk_instance.handle() != vk::Instance::null() {
            self.DestroySurface();
            unsafe {
                self.vk_device.destroy_device(None);
            }

            let dbg_val = self.dbg.load(std::sync::atomic::Ordering::Relaxed);
            if dbg_val == EDebugGFXModes::Minimum as u8 || dbg_val == EDebugGFXModes::All as u8 {
                self.UnregisterDebugCallback();
            }
            // TODO!: vkDestroyInstance(self.m_VKInstance, std::ptr::null());
            // self.m_VKInstance = vk::Instance::null();
        }
    }

    fn RecreateSwapChain(&mut self) -> i32 {
        let mut Ret: i32 = 0;
        unsafe { self.vk_device.device_wait_idle() };

        if is_verbose(&*self.dbg) {
            // TODO dbg_msg("vulkan", "recreating swap chain.");
        }

        let mut OldSwapChain = vk::SwapchainKHR::null();
        let OldSwapChainImageCount: u32 = self.device.swap_chain_image_count;

        if self.swap_chain_created {
            self.CleanupVulkanSwapChain(false);
        }

        // set new multi sampling if it was requested
        if self.next_multi_sampling_count != u32::MAX {
            self.device.limits.multi_sampling_count = self.next_multi_sampling_count;
            self.next_multi_sampling_count = u32::MAX;
        }

        if !self.swap_chain_created {
            Ret = self.InitVulkanSwapChain(&mut OldSwapChain);
        }

        if OldSwapChainImageCount != self.device.swap_chain_image_count {
            self.CleanupVulkan::<false>();
            self.InitVulkan::<false>();
        }

        if OldSwapChain != vk::SwapchainKHR::null() {
            // TODO! unsafe {self.m_VKDevice.DestroySwapchainKHR( OldSwapChain, std::ptr::null());}
        }

        if Ret != 0 && is_verbose(&*self.dbg) {
            // TODO  dbg_msg("vulkan", "recreating swap chain failed.");
        }

        return Ret;
    }

    fn InitVulkanSDL(
        window: &sdl2::video::Window,
        _CanvasWidth: u32,
        _CanvasHeight: u32,
        dbg: EDebugGFXModes,
        error: &Arc<Mutex<Error>>,
        sys: &mut system::System,
    ) -> Result<
        (
            ash::Entry,
            ash::Instance,
            ash::Device,
            TTWGraphicsGPUList,
            Limits,
            String,
            String,
            String,
            vk::PhysicalDevice,
            u32,
            vk::Queue,
            vk::Queue,
            ash::extensions::khr::Surface,
            vk::SurfaceKHR,
        ),
        ArrayString<4096>,
    > {
        let entry_res = unsafe { ash::Entry::load() };
        if let Err(err) = entry_res {
            return Err(ArrayString::from_str(err.to_string().as_str()).unwrap());
        }
        let entry = entry_res.unwrap();

        let extensions_res = Self::GetVulkanExtensions(window);
        if let Err(err) = extensions_res {
            return Err(err);
        }
        let mut extensions = extensions_res.unwrap();

        let layers_res = Self::GetVulkanLayers(dbg, &entry);
        if let Err(err) = layers_res {
            return Err(err);
        }
        let mut layers = layers_res.unwrap();

        let instance_res =
            Self::CreateVulkanInstance(dbg, &entry, error, &mut layers, &mut extensions, true);
        if let Err(err) = instance_res {
            return Err(err);
        }
        let instance = instance_res.unwrap();

        let mut dbg_callback = vk::DebugUtilsMessengerEXT::null();
        if dbg == EDebugGFXModes::Minimum || dbg == EDebugGFXModes::All {
            let dbg_res = Self::SetupDebugCallback(&entry, &instance, sys);
            if let Ok(dbg) = dbg_res {
                dbg_callback = dbg;
            }

            for VKLayer in &mut layers {
                sys.log("vulkan")
                    .msg("Validation layer: ")
                    .msg(VKLayer.as_str());
            }
        }

        let gpu_res = Self::SelectGPU(&instance, dbg, sys);
        if let Err(err) = gpu_res {
            return Err(err);
        }
        let (
            gpu_list,
            limits,
            renderer_name,
            vendor_name,
            version_name,
            physical_gpu,
            graphics_queue_index,
        ) = gpu_res.unwrap();

        let device_res =
            Self::CreateLogicalDevice(&physical_gpu, graphics_queue_index, &instance, &layers);
        if let Err(err) = device_res {
            return Err(err);
        }
        let device = device_res.unwrap();

        let dev_queue_res = Self::GetDeviceQueue(&device, graphics_queue_index);
        if let Err(err) = dev_queue_res {
            return Err(err);
        }
        let (graphics_queue, presentation_queue) = dev_queue_res.unwrap();

        let surface = ash::extensions::khr::Surface::new(&entry, &instance);

        let surf_res = Self::CreateSurface(
            window,
            &surface,
            &instance.handle(),
            &physical_gpu,
            graphics_queue_index,
        );
        if let Err(err) = surf_res {
            return Err(err);
        }
        let surf = surf_res.unwrap();

        Ok((
            entry,
            instance,
            device,
            gpu_list,
            limits,
            renderer_name,
            vendor_name,
            version_name,
            physical_gpu,
            graphics_queue_index,
            graphics_queue,
            presentation_queue,
            surface,
            surf,
        ))
    }

    #[must_use]
    fn HasMultiSampling(&mut self) -> bool {
        return Device::GetSampleCount(&self.device.limits) != vk::SampleCountFlags::TYPE_1;
    }

    fn InitVulkanSwapChain(&mut self, OldSwapChain: &mut vk::SwapchainKHR) -> i32 {
        *OldSwapChain = vk::SwapchainKHR::null();
        if !self.CreateSwapChain(OldSwapChain) {
            return -1;
        }

        if !self.GetSwapChainImageHandles() {
            return -1;
        }

        if !self.CreateImageViews() {
            return -1;
        }

        if !self.CreateMultiSamplerImageAttachments() {
            return -1;
        }

        self.last_presented_swap_chain_image_index = u32::MAX;

        if !self.CreateRenderPass(true) {
            return -1;
        }

        if !self.CreateFramebuffers() {
            return -1;
        }

        if !self.CreateStandardGraphicsPipeline(
            "shader/vulkan/prim.vert.spv",
            "shader/vulkan/prim.frag.spv",
            false,
            false,
        ) {
            return -1;
        }

        if !self.CreateStandardGraphicsPipeline(
            "shader/vulkan/prim_textured.vert.spv",
            "shader/vulkan/prim_textured.frag.spv",
            true,
            false,
        ) {
            return -1;
        }

        if !self.CreateStandardGraphicsPipeline(
            "shader/vulkan/prim.vert.spv",
            "shader/vulkan/prim.frag.spv",
            false,
            true,
        ) {
            return -1;
        }

        if !self.CreateStandard3DGraphicsPipeline(
            "shader/vulkan/prim3d.vert.spv",
            "shader/vulkan/prim3d.frag.spv",
            false,
        ) {
            return -1;
        }

        if !self.CreateStandard3DGraphicsPipeline(
            "shader/vulkan/prim3d_textured.vert.spv",
            "shader/vulkan/prim3d_textured.frag.spv",
            true,
        ) {
            return -1;
        }

        if !self.CreateTextGraphicsPipeline(
            "shader/vulkan/text.vert.spv",
            "shader/vulkan/text.frag.spv",
        ) {
            return -1;
        }

        if !self.CreateTileGraphicsPipeline::<false>(
            "shader/vulkan/tile.vert.spv",
            "shader/vulkan/tile.frag.spv",
            0,
        ) {
            return -1;
        }

        if !self.CreateTileGraphicsPipeline::<true>(
            "shader/vulkan/tile_textured.vert.spv",
            "shader/vulkan/tile_textured.frag.spv",
            0,
        ) {
            return -1;
        }

        if !self.CreateTileGraphicsPipeline::<false>(
            "shader/vulkan/tile_border.vert.spv",
            "shader/vulkan/tile_border.frag.spv",
            1,
        ) {
            return -1;
        }

        if !self.CreateTileGraphicsPipeline::<true>(
            "shader/vulkan/tile_border_textured.vert.spv",
            "shader/vulkan/tile_border_textured.frag.spv",
            1,
        ) {
            return -1;
        }

        if !self.CreateTileGraphicsPipeline::<false>(
            "shader/vulkan/tile_border_line.vert.spv",
            "shader/vulkan/tile_border_line.frag.spv",
            2,
        ) {
            return -1;
        }

        if !self.CreateTileGraphicsPipeline::<true>(
            "shader/vulkan/tile_border_line_textured.vert.spv",
            "shader/vulkan/tile_border_line_textured.frag.spv",
            2,
        ) {
            return -1;
        }

        if !self.CreatePrimExGraphicsPipeline(
            "shader/vulkan/primex_rotationless.vert.spv",
            "shader/vulkan/primex_rotationless.frag.spv",
            false,
            true,
        ) {
            return -1;
        }

        if !self.CreatePrimExGraphicsPipeline(
            "shader/vulkan/primex_tex_rotationless.vert.spv",
            "shader/vulkan/primex_tex_rotationless.frag.spv",
            true,
            true,
        ) {
            return -1;
        }

        if !self.CreatePrimExGraphicsPipeline(
            "shader/vulkan/primex.vert.spv",
            "shader/vulkan/primex.frag.spv",
            false,
            false,
        ) {
            return -1;
        }

        if !self.CreatePrimExGraphicsPipeline(
            "shader/vulkan/primex_tex.vert.spv",
            "shader/vulkan/primex_tex.frag.spv",
            true,
            false,
        ) {
            return -1;
        }

        if !self.CreateSpriteMultiGraphicsPipeline(
            "shader/vulkan/spritemulti.vert.spv",
            "shader/vulkan/spritemulti.frag.spv",
        ) {
            return -1;
        }

        if !self.CreateSpriteMultiPushGraphicsPipeline(
            "shader/vulkan/spritemulti_push.vert.spv",
            "shader/vulkan/spritemulti_push.frag.spv",
        ) {
            return -1;
        }

        if !self.CreateQuadGraphicsPipeline::<false>(
            "shader/vulkan/quad.vert.spv",
            "shader/vulkan/quad.frag.spv",
        ) {
            return -1;
        }

        if !self.CreateQuadGraphicsPipeline::<true>(
            "shader/vulkan/quad_textured.vert.spv",
            "shader/vulkan/quad_textured.frag.spv",
        ) {
            return -1;
        }

        if !self.CreateQuadPushGraphicsPipeline::<false>(
            "shader/vulkan/quad_push.vert.spv",
            "shader/vulkan/quad_push.frag.spv",
        ) {
            return -1;
        }

        if !self.CreateQuadPushGraphicsPipeline::<true>(
            "shader/vulkan/quad_push_textured.vert.spv",
            "shader/vulkan/quad_push_textured.frag.spv",
        ) {
            return -1;
        }

        self.swap_chain_created = true;
        return 0;
    }

    fn init_vulkan_without_io(&mut self) -> i32 {
        if !self.CreateDescriptorSetLayouts() {
            return -1;
        }

        if !self.CreateTextDescriptorSetLayout() {
            return -1;
        }

        if !self.device.CreateSpriteMultiUniformDescriptorSetLayout() {
            return -1;
        }

        if !self.device.CreateQuadUniformDescriptorSetLayout() {
            return -1;
        }

        return 0;
    }

    fn init_vulkan_with_io<const IsFirstInitialization: bool>(&mut self) -> i32 {
        if IsFirstInitialization {
            let mut OldSwapChain = vk::SwapchainKHR::null();
            if self.InitVulkanSwapChain(&mut OldSwapChain) != 0 {
                return -1;
            }
        }

        if IsFirstInitialization {
            if !self.CreateCommandPool() {
                return -1;
            }
        }

        if !self.CreateCommandBuffers() {
            return -1;
        }

        if !self.CreateSyncObjects() {
            return -1;
        }

        if IsFirstInitialization {
            if !self.CreateDescriptorPools(self.thread_count) {
                return -1;
            }

            if !self.CreateTextureSamplers() {
                return -1;
            }
        }

        self.device.streamed_vertex_buffer = Default::default();
        self.device
            .streamed_vertex_buffer
            .init(self.device.swap_chain_image_count as usize);
        self.device
            .streamed_uniform_buffers
            .resize(self.thread_count as usize, Default::default());
        for i in 0..self.thread_count {
            self.device.streamed_uniform_buffers[i]
                .init(self.device.swap_chain_image_count as usize);
        }

        self.last_pipeline_per_thread
            .resize(self.thread_count, Default::default());

        self.device.frame_delayed_buffer_cleanups.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );
        self.device.frame_delayed_texture_cleanups.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );
        self.device.frame_delayed_text_textures_cleanups.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );
        self.device
            .staging_buffer_cache
            .init(self.device.swap_chain_image_count as usize);
        self.device
            .staging_buffer_cache_image
            .init(self.device.swap_chain_image_count as usize);
        self.device
            .vertex_buffer_cache
            .init(self.device.swap_chain_image_count as usize);
        for ImageBufferCache in &mut self.device.image_buffer_caches {
            ImageBufferCache
                .1
                .init(self.device.swap_chain_image_count as usize);
        }

        self.image_last_frame_check
            .resize(self.device.swap_chain_image_count as usize, 0);

        if IsFirstInitialization {
            // check if image format supports linear blitting
            let mut FormatProperties = vk::FormatProperties::default();
            FormatProperties = unsafe {
                self.vk_instance
                    .get_physical_device_format_properties(self.vk_gpu, vk::Format::R8G8B8A8_UNORM)
            };
            if !(FormatProperties.optimal_tiling_features
                & vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR)
                .is_empty()
            {
                self.device.allows_linear_blitting = true;
            }
            if !(FormatProperties.optimal_tiling_features & vk::FormatFeatureFlags::BLIT_SRC)
                .is_empty()
                && !(FormatProperties.optimal_tiling_features & vk::FormatFeatureFlags::BLIT_DST)
                    .is_empty()
            {
                self.device.optimal_rgba_image_blitting = true;
            }
            // check if image format supports blitting to linear tiled images
            if !(FormatProperties.linear_tiling_features & vk::FormatFeatureFlags::BLIT_DST)
                .is_empty()
            {
                self.device.linear_rgba_image_blitting = true;
            }

            FormatProperties = unsafe {
                self.vk_instance
                    .get_physical_device_format_properties(self.vk_gpu, self.vk_surf_format.format)
            };
            if !(FormatProperties.optimal_tiling_features & vk::FormatFeatureFlags::BLIT_SRC)
                .is_empty()
            {
                self.device.optimal_swap_chain_image_blitting = true;
            }
        }

        return 0;
    }

    fn InitVulkan<const IsFirstInitialization: bool>(&mut self) -> i32 {
        let res = self.init_vulkan_without_io();
        if res != 0 {
            return res;
        }

        let res = self.init_vulkan_with_io::<{ IsFirstInitialization }>();
        if res != 0 {
            return res;
        }

        return 0;
    }

    #[must_use]
    fn GetGraphicCommandBuffer(
        &mut self,
        pDrawCommandBuffer: &mut *mut vk::CommandBuffer,
        RenderThreadIndex: usize,
    ) -> bool {
        if self.thread_count < 2 {
            *pDrawCommandBuffer =
                &mut self.main_draw_command_buffers[self.cur_image_index as usize];
            return true;
        } else {
            let DrawCommandBuffer = &mut self.thread_draw_command_buffers[RenderThreadIndex]
                [self.cur_image_index as usize];
            if !self.used_thread_draw_command_buffer[RenderThreadIndex]
                [self.cur_image_index as usize]
            {
                self.used_thread_draw_command_buffer[RenderThreadIndex]
                    [self.cur_image_index as usize] = true;

                unsafe {
                    self.vk_device.reset_command_buffer(
                        *DrawCommandBuffer,
                        vk::CommandBufferResetFlags::RELEASE_RESOURCES,
                    )
                };

                let mut BeginInfo = vk::CommandBufferBeginInfo::default();
                BeginInfo.flags = vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT
                    | vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE;

                let mut InheretInfo = vk::CommandBufferInheritanceInfo::default();
                InheretInfo.framebuffer = self.framebuffer_list[self.cur_image_index as usize];
                InheretInfo.occlusion_query_enable = vk::FALSE;
                InheretInfo.render_pass = self.vk_render_pass;
                InheretInfo.subpass = 0;

                BeginInfo.p_inheritance_info = &InheretInfo;

                let begin_res = unsafe {
                    self.vk_device
                        .begin_command_buffer(*DrawCommandBuffer, &BeginInfo)
                };
                if let Err(_) = begin_res {
                    self.error.lock().unwrap().SetError(
                        EGFXErrorType::RenderRecording,
                        "Thread draw command buffer cannot be filled anymore.",
                    );
                    return false;
                }
            }
            *pDrawCommandBuffer = DrawCommandBuffer;
            return true;
        }
    }

    /************************
     * COMMAND IMPLEMENTATION
     ************************/
    /*#[must_use] fn IsInCommandRange<TName>(TName CMD, TName Min, TName Max) -> bool
        {
            return CMD >= Min && CMD < Max;
        }
    */

    /*
    #[must_use] fn Cmd_Shutdown(&mut self,const SCommand_Shutdown *pCommand) -> bool
    {
        vkDeviceWaitIdle(self.m_VKDevice);

        DestroyIndexBuffer(self.m_IndexBuffer, self.m_IndexBufferMemory);
        DestroyIndexBuffer(self.m_RenderIndexBuffer, self.m_RenderIndexBufferMemory);

        CleanupVulkan<true>();

        return true;
    }*/

    #[must_use]
    fn Cmd_Texture_Update(&mut self, cmd: &SCommand_Texture_Update) -> bool {
        let IndexTex: usize = cmd.slot.unwrap();

        // TODO: useless copy?
        let mut pData = cmd.data.clone();

        if !self.UpdateTexture(
            IndexTex,
            vk::Format::B8G8R8A8_UNORM,
            &mut pData,
            cmd.x as i64,
            cmd.y as i64,
            cmd.width as usize,
            cmd.height as usize,
            tex_format_to_image_color_channel_count(cmd.format),
        ) {
            return false;
        }

        return true;
    }

    #[must_use]
    fn Cmd_Texture_Destroy(&mut self, cmd: &SCommand_Texture_Destroy) -> bool {
        let ImageIndex: usize = cmd.slot.unwrap();
        let Texture = &mut self.device.textures[ImageIndex];

        self.device.frame_delayed_texture_cleanups[self.cur_image_index as usize]
            .push(Texture.clone());

        *Texture = CTexture::default();

        return true;
    }

    #[must_use]
    fn Cmd_Texture_Create(&mut self, cmd: &SCommand_Texture_Create) -> bool {
        let Slot = cmd.slot;
        let Width = cmd.width;
        let Height = cmd.height;
        let depth = cmd.depth;
        let PixelSize = cmd.pixel_size;
        let Format = cmd.format;
        let StoreFormat = cmd.store_format;
        let Flags = cmd.flags;

        let mut data_opt = cmd.data.borrow_mut();
        let mut data: Option<&'static mut [u8]> = None;
        std::mem::swap(&mut data, &mut data_opt);

        let data_mem = data.unwrap();

        if !self.CreateTextureCMD(
            Slot.unwrap(),
            Width as usize,
            Height as usize,
            depth,
            cmd.is_3d_tex,
            PixelSize as usize,
            texture_format_to_vulkan_format(Format),
            texture_format_to_vulkan_format(StoreFormat),
            Flags,
            data_mem,
        ) {
            return false;
        }

        return true;
    }
    /*
                #[must_use] fn Cmd_TextTextures_Create(&mut self,cmd: &SCommand_TextTextures_Create) -> bool
                {
                    let Slot: i32 = cmd.m_Slot;
                    let SlotOutline: i32 = cmd.m_SlotOutline;
                    let Width: i32 = cmd.m_Width;
                    let Height: i32 = cmd.m_Height;

                    void *pTmpData = cmd.m_pTextData;
                    void *pTmpData2 = cmd.m_pTextOutlineData;

                    if(!CreateTextureCMD(Slot, Width, Height, 1, vk::Format::R8_UNORM, vk::Format::R8_UNORM, CCommandBuffer::TEXFLAG_NOMIPMAPS, pTmpData))
                        return false;
                    if(!CreateTextureCMD(SlotOutline, Width, Height, 1, vk::Format::R8_UNORM, vk::Format::R8_UNORM, CCommandBuffer::TEXFLAG_NOMIPMAPS, pTmpData2))
                        return false;

                    if(!CreateNewTextDescriptorSets(Slot, SlotOutline))
                        return false;

                    free(pTmpData);
                    free(pTmpData2);

                    return true;
                }

                #[must_use] fn Cmd_TextTextures_Destroy(&mut self,cmd: &SCommand_TextTextures_Destroy) -> bool
                {
                    let ImageIndex: usize = (usize)cmd.m_Slot;
                    let ImageIndexOutline: usize = (usize)cmd.m_SlotOutline;
                    let Texture = &mut  self.device.m_vTextures[ImageIndex];
                    let TextureOutline = &mut  self.device.m_vTextures[ImageIndexOutline];

                    self.device.m_vvFrameDelayedTextTexturesCleanup[self.m_CurImageIndex as usize].push(Texture, TextureOutline);

                    Texture = {};
                    TextureOutline = {};

                    return true;
                }

                #[must_use] fn Cmd_TextTexture_Update(&mut self,cmd: &SCommand_TextTexture_Update) -> bool
                {
                    let IndexTex: usize = cmd.m_Slot;

                    void *pData = cmd.m_pData;

                    if(!UpdateTexture(IndexTex, vk::Format::R8_UNORM, pData, cmd.m_X, cmd.m_Y, cmd.m_Width, cmd.m_Height, 1))
                        return false;

                    free(pData);

                    return true;
                }
    */

    fn Cmd_Clear_FillExecuteBuffer(
        &mut self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &SCommand_Clear,
    ) {
        if !cmd.force_clear {
            let ColorChanged: bool = self.clear_color[0] != cmd.color.r
                || self.clear_color[1] != cmd.color.g
                || self.clear_color[2] != cmd.color.b
                || self.clear_color[3] != cmd.color.a;
            self.clear_color[0] = cmd.color.r;
            self.clear_color[1] = cmd.color.g;
            self.clear_color[2] = cmd.color.b;
            self.clear_color[3] = cmd.color.a;
            if ColorChanged {
                exec_buffer.clear_color_in_render_thread = true;
            }
        } else {
            exec_buffer.clear_color_in_render_thread = true;
        }
        exec_buffer.estimated_render_call_count = 0;
    }

    #[must_use]
    fn Cmd_Clear(
        &mut self,
        exec_buffer: &SRenderCommandExecuteBuffer,
        cmd: &SCommand_Clear,
    ) -> bool {
        if exec_buffer.clear_color_in_render_thread {
            let aAttachments = [vk::ClearAttachment {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                color_attachment: 0,
                clear_value: vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [cmd.color.r, cmd.color.g, cmd.color.b, cmd.color.a],
                    },
                },
            }];
            let aClearRects = [vk::ClearRect {
                rect: vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.vk_swap_img_and_viewport_extent.swap_image_viewport,
                },
                base_array_layer: 0,
                layer_count: 1,
            }];

            let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
            if !self
                .GetGraphicCommandBuffer(&mut command_buffer_ptr, exec_buffer.thread_index as usize)
            {
                return false;
            }
            let CommandBuffer = unsafe { &mut *command_buffer_ptr };
            unsafe {
                self.vk_device
                    .cmd_clear_attachments(*CommandBuffer, &aAttachments, &aClearRects);
            }
        }

        return true;
    }

    fn Cmd_Render_FillExecuteBuffer(
        &mut self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &SCommand_Render,
    ) {
        let IsTextured: bool = Self::GetIsTextured(&cmd.state);
        if IsTextured {
            let AddressModeIndex: usize = Self::GetAddressModeIndex(&cmd.state);
            exec_buffer.descriptors[0] = self.device.textures[cmd.state.texture_index.unwrap()]
                .vk_standard_textured_descr_sets[AddressModeIndex]
                .clone();
        }

        exec_buffer.index_buffer = self.index_buffer;

        exec_buffer.estimated_render_call_count = 1;

        self.ExecBufferFillDynamicStates(&cmd.state, exec_buffer);

        let _VertPerPrim: usize = match cmd.prim_type {
            PrimType::Invalid => todo!(),
            PrimType::Lines => 2,
            PrimType::Quads => 4,
            PrimType::Triangles => 3,
        };

        let cur_stream_buffer = self
            .device
            .streamed_vertex_buffer
            .get_current_buffer(self.cur_image_index as usize);
        exec_buffer.buffer = cur_stream_buffer.buffer;
        exec_buffer.buffer_off = cur_stream_buffer.offset_in_buffer
            + cmd.vertices_offset * std::mem::size_of::<GL_SVertex>();
    }

    #[must_use]
    fn Cmd_Render(
        &mut self,
        cmd: &SCommand_Render,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        return self.RenderStandard::<GL_SVertex, false>(
            exec_buffer,
            &cmd.state,
            cmd.prim_type,
            cmd.prim_count,
        );
    }
    /*
                    #[must_use] fn Cmd_Screenshot(&mut self,cmd: &SCommand_TrySwapAndScreenshot) -> bool
                    {
                        if(!NextFrame())
                            return false;
                        *cmd.m_pSwapped = true;

                        u32 Width;
                        u32 Height;
                        u32 Format;
                        if(GetPresentedImageDataImpl(Width, Height, Format, self.m_vScreenshotHelper, false, true))
                        {
                            let ImgSize: usize = (usize)Width * (usize)Height * (usize)4;
                            cmd.m_pImage.m_pData = malloc(ImgSize);
                            mem_copy(cmd.m_pImage.m_pData, self.m_vScreenshotHelper.as_ptr(), ImgSize);
                        }
                        else
                        {
                            cmd.m_pImage.m_pData = std::ptr::null();
                        }
                        cmd.m_pImage.m_Width = (i32)Width;
                        cmd.m_pImage.m_Height = (i32)Height;
                        cmd.m_pImage.m_Format = (i32)Format;

                        return true;
                    }

                    void Cmd_RenderTex3D_FillExecuteBuffer(exec_buffer: &mut SRenderCommandExecuteBuffer, cmd: &SCommand_RenderTex3D)
                    {
                        let IsTextured: bool = Self::GetIsTextured(cmd.state);
                        if(IsTextured)
                        {
                            exec_buffer.m_aDescriptors[0] = self.device.m_vTextures[cmd.state.texture_index.unwrap()].m_VKStandard3DTexturedDescrSet;
                        }

                        exec_buffer.m_IndexBuffer = self.m_IndexBuffer;

                        exec_buffer.m_EstimatedRenderCallCount = 1;

                        ExecBufferFillDynamicStates(cmd.state, exec_buffer);
                    }

                    #[must_use] fn Cmd_RenderTex3D(cmd: &SCommand_RenderTex3D, exec_buffer: &SRenderCommandExecuteBuffer ) { return RenderStandard<CCommandBuffer::SVertexTex3DStream, true>(&mut self,exec_buffer, cmd.state, cmd.m_PrimType, cmd.m_pVertices, cmd.m_PrimCount); } -> bool
    */
    fn Cmd_Update_Viewport_FillExecuteBuffer(
        &self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        _pCommand: &SCommand_Update_Viewport,
    ) {
        exec_buffer.estimated_render_call_count = 0;
    }

    #[must_use]
    fn Cmd_Update_Viewport(&mut self, cmd: &SCommand_Update_Viewport) -> bool {
        if cmd.by_resize {
            if is_verbose(&*self.dbg) {
                self.sys
                    .log("vulkan")
                    .msg("queueing swap chain recreation because the viewport changed");
            }

            // TODO: rethink if this is a good idea (checking if width changed. maybe some weird edge cases)
            if self
                .vk_swap_img_and_viewport_extent
                .swap_image_viewport
                .width
                != cmd.width
                || self
                    .vk_swap_img_and_viewport_extent
                    .swap_image_viewport
                    .height
                    != cmd.height
            {
                self.canvas_width = cmd.width;
                self.canvas_height = cmd.height;
                self.recreate_swap_chain = true;
            }
        } else {
            let Viewport = self
                .vk_swap_img_and_viewport_extent
                .get_presented_image_viewport();
            if cmd.x != 0
                || cmd.y != 0
                || cmd.width != Viewport.width
                || cmd.height != Viewport.height
            {
                self.has_dynamic_viewport = true;

                // convert viewport from OGL to vulkan
                let ViewportY: i32 = Viewport.height as i32 - (cmd.y + cmd.height as i32);
                let ViewportH = cmd.height;
                self.dynamic_viewport_offset = vk::Offset2D {
                    x: cmd.x,
                    y: ViewportY,
                };
                self.dynamic_viewport_size = vk::Extent2D {
                    width: cmd.width,
                    height: ViewportH,
                };
            } else {
                self.has_dynamic_viewport = false;
            }
        }

        return true;
    }
    /*
                #[must_use] fn Cmd_VSync(&mut self,cmd: &SCommand_VSync) -> bool
                {
                    if(IsVerbose(&*self.dbg))
                    {
                        dbg_msg("vulkan", "queueing swap chain recreation because vsync was changed");
                    }
                    self.m_RecreateSwapChain = true;
                    *cmd.m_pRetOk = true;

                    return true;
                }

                #[must_use] fn Cmd_MultiSampling(&mut self,cmd: &SCommand_MultiSampling) -> bool
                {
                    if(IsVerbose(&*self.dbg))
                    {
                        dbg_msg("vulkan", "queueing swap chain recreation because multi sampling was changed");
                    }
                    self.m_RecreateSwapChain = true;

                    u32 MSCount = (std::min(cmd.m_RequestedMultiSamplingCount,
                                    (u32)GetMaxSampleCount()) &
                                0xFFFFFFFE); // ignore the uneven bits
                    self.m_NextMultiSamplingCount = MSCount;

                    *cmd.m_pRetMultiSamplingCount = MSCount;
                    *cmd.m_pRetOk = true;

                    return true;
                }

                #[must_use] fn Cmd_Finish(&mut self,cmd: &SCommand_Finish) -> bool
                {
                    // just ignore it with vulkan
                    return true;
                }
    */
    #[must_use]
    fn Cmd_Swap(&mut self) -> bool {
        return self.NextFrame();
    }

    #[must_use]
    fn Cmd_CreateBufferObject(&mut self, cmd: &SCommand_CreateBufferObject) -> bool {
        let mut upload_data = None;
        let mut cmd_data = cmd.upload_data.borrow_mut();
        std::mem::swap(&mut upload_data, &mut *cmd_data);

        let upload_data_size = upload_data.as_ref().unwrap().len() as vk::DeviceSize;

        if !self.device.CreateBufferObject(
            cmd.buffer_index,
            upload_data.unwrap(),
            upload_data_size,
            self.cur_image_index,
        ) {
            return false;
        }

        return true;
    }

    #[must_use]
    fn Cmd_UpdateBufferObject(&mut self, cmd: &SCommand_UpdateBufferObject) -> bool {
        let BufferIndex: usize = cmd.buffer_index;
        let Offset = cmd.offset as vk::DeviceSize;
        let pUploadData = cmd.upload_data.as_ptr() as *const c_void;
        let DataSize = cmd.upload_data.len() as vk::DeviceSize;

        let mut StagingBuffer = SMemoryBlock::<STAGING_BUFFER_CACHE_ID>::default();
        if !self
            .device
            .get_staging_buffer(&mut StagingBuffer, pUploadData, DataSize)
        {
            return false;
        }

        let MemBlock = self.device.buffer_objects[BufferIndex]
            .buffer_object
            .mem
            .clone();
        let VertexBuffer = MemBlock.buffer;
        if !self.device.MemoryBarrier(
            VertexBuffer,
            Offset + MemBlock.heap_data.offset_to_align as vk::DeviceSize,
            DataSize,
            vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            true,
            self.cur_image_index,
        ) {
            return false;
        }
        if !self.device.CopyBuffer(
            StagingBuffer.buffer,
            VertexBuffer,
            StagingBuffer.heap_data.offset_to_align as vk::DeviceSize,
            Offset + MemBlock.heap_data.offset_to_align as vk::DeviceSize,
            DataSize,
            self.cur_image_index,
        ) {
            return false;
        }
        if !self.device.MemoryBarrier(
            VertexBuffer,
            Offset + MemBlock.heap_data.offset_to_align as vk::DeviceSize,
            DataSize,
            vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            false,
            self.cur_image_index,
        ) {
            return false;
        }

        self.device
            .UploadAndFreeStagingMemBlock(&mut StagingBuffer, self.cur_image_index);

        return true;
    }

    #[must_use]
    fn Cmd_RecreateBufferObject(&mut self, cmd: &SCommand_RecreateBufferObject) -> bool {
        self.device
            .DeleteBufferObject(cmd.buffer_index, self.cur_image_index);

        let mut upload_data = None;
        let mut cmd_data = cmd.upload_data.borrow_mut();
        std::mem::swap(&mut upload_data, &mut *cmd_data);

        let upload_data_size = upload_data.as_ref().unwrap().len() as vk::DeviceSize;

        return self.device.CreateBufferObject(
            cmd.buffer_index,
            upload_data.unwrap(),
            upload_data_size,
            self.cur_image_index,
        );
    }

    #[must_use]
    fn Cmd_CopyBufferObject(&mut self, cmd: &SCommand_CopyBufferObject) -> bool {
        let ReadBufferIndex: usize = cmd.read_buffer_index;
        let WriteBufferIndex: usize = cmd.write_buffer_index;
        let ReadMemBlock = &self.device.buffer_objects[ReadBufferIndex]
            .buffer_object
            .mem;
        let WriteMemBlock = &self.device.buffer_objects[WriteBufferIndex]
            .buffer_object
            .mem;
        let ReadBuffer = ReadMemBlock.buffer;
        let WriteBuffer = WriteMemBlock.buffer;

        let DataSize = cmd.copy_size as vk::DeviceSize;
        let ReadOffset = cmd.read_offset as vk::DeviceSize
            + ReadMemBlock.heap_data.offset_to_align as vk::DeviceSize;
        let WriteOffset = cmd.write_offset as vk::DeviceSize
            + WriteMemBlock.heap_data.offset_to_align as vk::DeviceSize;

        if !self.device.MemoryBarrier(
            ReadBuffer,
            ReadOffset,
            DataSize,
            vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            true,
            self.cur_image_index,
        ) {
            return false;
        }
        if !self.device.MemoryBarrier(
            WriteBuffer,
            WriteOffset,
            DataSize,
            vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            true,
            self.cur_image_index,
        ) {
            return false;
        }
        if !self.device.CopyBuffer(
            ReadBuffer,
            WriteBuffer,
            ReadOffset,
            WriteOffset,
            DataSize,
            self.cur_image_index,
        ) {
            return false;
        }
        if !self.device.MemoryBarrier(
            WriteBuffer,
            WriteOffset,
            DataSize,
            vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            false,
            self.cur_image_index,
        ) {
            return false;
        }
        if !self.device.MemoryBarrier(
            ReadBuffer,
            ReadOffset,
            DataSize,
            vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            false,
            self.cur_image_index,
        ) {
            return false;
        }

        return true;
    }

    #[must_use]
    fn Cmd_DeleteBufferObject(&mut self, cmd: &SCommand_DeleteBufferObject) -> bool {
        let BufferIndex: usize = cmd.buffer_index;
        self.device
            .DeleteBufferObject(BufferIndex, self.cur_image_index);

        return true;
    }

    #[must_use]
    fn Cmd_CreateBufferContainer(&mut self, cmd: &SCommand_CreateBufferContainer) -> bool {
        let ContainerIndex: usize = cmd.buffer_container_index;
        while ContainerIndex >= self.device.buffer_containers.len() {
            self.device.buffer_containers.resize(
                (self.device.buffer_containers.len() * 2) + 1,
                Default::default(),
            );
        }

        self.device.buffer_containers[ContainerIndex].buffer_object_index =
            cmd.vert_buffer_binding_index;

        return true;
    }

    #[must_use]
    fn Cmd_UpdateBufferContainer(&mut self, cmd: &SCommand_UpdateBufferContainer) -> bool {
        let ContainerIndex: usize = cmd.buffer_container_index;
        self.device.buffer_containers[ContainerIndex].buffer_object_index =
            cmd.vert_buffer_binding_index;

        return true;
    }

    #[must_use]
    fn Cmd_DeleteBufferContainer(&mut self, cmd: &SCommand_DeleteBufferContainer) -> bool {
        let ContainerIndex: usize = cmd.buffer_container_index;
        let DeleteAllBO: bool = cmd.destroy_all_buffer_objects;
        if DeleteAllBO {
            let BufferIndex: usize =
                self.device.buffer_containers[ContainerIndex].buffer_object_index;
            self.device
                .DeleteBufferObject(BufferIndex, self.cur_image_index);
        }

        return true;
    }

    #[must_use]
    fn Cmd_IndicesRequiredNumNotify(&mut self, cmd: &SCommand_IndicesRequiredNumNotify) -> bool {
        let IndicesCount: usize = cmd.required_indices_num;
        if self.cur_render_index_primitive_count < IndicesCount / 6 {
            self.device.frame_delayed_buffer_cleanups[self.cur_image_index as usize].push(
                SDelayedBufferCleanupItem {
                    buffer: self.render_index_buffer,
                    mem: self.render_index_buffer_memory.clone(),
                    ..Default::default()
                },
            );
            let mut vIndices = Vec::<u32>::new();
            vIndices.resize(IndicesCount, Default::default());
            let mut Primq: u32 = 0;
            for i in (0..IndicesCount).step_by(6) {
                vIndices[i] = Primq;
                vIndices[i + 1] = Primq + 1;
                vIndices[i + 2] = Primq + 2;
                vIndices[i + 3] = Primq;
                vIndices[i + 4] = Primq + 2;
                vIndices[i + 5] = Primq + 3;
                Primq += 4;
            }
            if !self.device.CreateIndexBuffer(
                vIndices.as_ptr() as *const c_void,
                vIndices.len() * std::mem::size_of::<u32>(),
                &mut self.render_index_buffer,
                &mut self.render_index_buffer_memory,
                self.cur_image_index,
            ) {
                return false;
            }
            self.cur_render_index_primitive_count = IndicesCount / 6;
        }

        return true;
    }

    fn Cmd_RenderTileLayer_FillExecuteBuffer(
        &mut self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &SCommand_RenderTileLayer,
    ) {
        self.RenderTileLayer_FillExecuteBuffer(
            exec_buffer,
            cmd.indices_draw_num,
            &cmd.state,
            cmd.buffer_container_index,
        );
    }

    #[must_use]
    fn Cmd_RenderTileLayer(
        &mut self,
        cmd: &SCommand_RenderTileLayer,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        let Type: i32 = 0;
        let dir = vec2::default();
        let off = vec2::default();
        let JumpIndex: i32 = 0;
        return self.RenderTileLayer(
            exec_buffer,
            &cmd.state,
            Type,
            &cmd.color,
            &dir,
            &off,
            JumpIndex,
            cmd.indices_draw_num,
            &cmd.indices_offsets,
            &cmd.draw_count,
            1,
        );
    }

    fn Cmd_RenderBorderTile_FillExecuteBuffer(
        &mut self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &SCommand_RenderBorderTile,
    ) {
        self.RenderTileLayer_FillExecuteBuffer(
            exec_buffer,
            1,
            &cmd.state,
            cmd.buffer_container_index,
        );
    }

    #[must_use]
    fn Cmd_RenderBorderTile(
        &mut self,
        cmd: &SCommand_RenderBorderTile,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        let Type: i32 = 1;
        let dir = cmd.dir;
        let off = cmd.offset;
        let DrawNum = 6;
        return self.RenderTileLayer(
            exec_buffer,
            &cmd.state,
            Type,
            &cmd.color,
            &dir,
            &off,
            cmd.jump_index,
            1,
            &[cmd.indices_offset],
            &[DrawNum],
            cmd.draw_num,
        );
    }

    fn Cmd_RenderBorderTileLine_FillExecuteBuffer(
        &mut self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &SCommand_RenderBorderTileLine,
    ) {
        self.RenderTileLayer_FillExecuteBuffer(
            exec_buffer,
            1,
            &cmd.state,
            cmd.buffer_container_index,
        );
    }

    #[must_use]
    fn Cmd_RenderBorderTileLine(
        &mut self,
        cmd: &SCommand_RenderBorderTileLine,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        let Type: i32 = 2;
        let dir = cmd.dir;
        let off = cmd.offset;
        return self.RenderTileLayer(
            exec_buffer,
            &cmd.state,
            Type,
            &cmd.color,
            &dir,
            &off,
            0,
            1,
            &[cmd.indices_offset],
            &[cmd.index_draw_num],
            cmd.draw_num,
        );
    }

    fn Cmd_RenderQuadLayer_FillExecuteBuffer(
        &self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &SCommand_RenderQuadLayer,
    ) {
        let buffer_container_index: usize = cmd.buffer_container_index;
        let buffer_object_index: usize =
            self.device.buffer_containers[buffer_container_index].buffer_object_index;
        let buffer_object = &self.device.buffer_objects[buffer_object_index];

        exec_buffer.buffer = buffer_object.cur_buffer;
        exec_buffer.buffer_off = buffer_object.cur_buffer_offset;

        let is_textured: bool = Self::GetIsTextured(&cmd.state);
        if is_textured {
            let AddressModeIndex: usize = Self::GetAddressModeIndex(&cmd.state);
            exec_buffer.descriptors[0] = self.device.textures[cmd.state.texture_index.unwrap()]
                .vk_standard_textured_descr_sets[AddressModeIndex]
                .clone();
        }

        exec_buffer.index_buffer = self.render_index_buffer;

        exec_buffer.estimated_render_call_count =
            ((cmd.quad_num - 1) / GRAPHICS_MAX_QUADS_RENDER_COUNT) + 1;

        self.ExecBufferFillDynamicStates(&cmd.state, exec_buffer);
    }

    #[must_use]
    fn Cmd_RenderQuadLayer(
        &mut self,
        cmd: &SCommand_RenderQuadLayer,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::GetStateMatrix(&cmd.state, &mut m);

        let CanBePushed: bool = cmd.quad_num == 1;

        let mut IsTextured: bool = Default::default();
        let mut BlendModeIndex: usize = Default::default();
        let mut DynamicIndex: usize = Default::default();
        let mut AddressModeIndex: usize = Default::default();
        Self::GetStateIndices(
            exec_buffer,
            &cmd.state,
            &mut IsTextured,
            &mut BlendModeIndex,
            &mut DynamicIndex,
            &mut AddressModeIndex,
        );
        let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.GetGraphicCommandBuffer(&mut command_buffer_ptr, exec_buffer.thread_index as usize)
        {
            return false;
        }
        let CommandBuffer = unsafe { &*command_buffer_ptr };
        let (Pipeline, PipeLayout) = Self::GetPipelineAndLayout(
            if CanBePushed {
                &self.quad_push_pipeline
            } else {
                &self.quad_pipeline
            },
            IsTextured,
            BlendModeIndex as usize,
            DynamicIndex as usize,
        );
        let (Pipeline, PipeLayout) = (*Pipeline, *PipeLayout);

        Self::BindPipeline(
            &self.vk_device,
            &mut self.last_pipeline_per_thread,
            exec_buffer.thread_index,
            *CommandBuffer,
            exec_buffer,
            Pipeline,
            &cmd.state,
        );

        let aVertexBuffers = [exec_buffer.buffer];
        let aOffsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            self.vk_device
                .cmd_bind_vertex_buffers(*CommandBuffer, 0, &aVertexBuffers, &aOffsets);
        }

        unsafe {
            self.vk_device.cmd_bind_index_buffer(
                *CommandBuffer,
                exec_buffer.index_buffer,
                0,
                vk::IndexType::UINT32,
            );
        }

        if IsTextured {
            unsafe {
                self.vk_device.cmd_bind_descriptor_sets(
                    *CommandBuffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    PipeLayout,
                    0,
                    &[exec_buffer.descriptors[0].descriptor],
                    &[],
                );
            }
        }

        if CanBePushed {
            let mut PushConstantVertex = SUniformQuadPushGPos::default();

            unsafe {
                libc::memcpy(
                    &mut PushConstantVertex.bo_push as *mut SUniformQuadPushGBufferObject
                        as *mut c_void,
                    &cmd.quad_info[0] as *const SQuadRenderInfo as *const c_void,
                    std::mem::size_of::<SUniformQuadPushGBufferObject>(),
                )
            };

            PushConstantVertex.pos = m;
            PushConstantVertex.quad_offset = cmd.quad_offset as i32;

            unsafe {
                self.vk_device.cmd_push_constants(
                    *CommandBuffer,
                    PipeLayout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    unsafe {
                        std::slice::from_raw_parts(
                            &PushConstantVertex as *const SUniformQuadPushGPos as *const u8,
                            std::mem::size_of::<SUniformQuadPushGPos>(),
                        )
                    },
                );
            }
        } else {
            let mut PushConstantVertex = SUniformQuadGPos::default();
            PushConstantVertex.pos = m;
            PushConstantVertex.quad_offset = cmd.quad_offset as i32;

            unsafe {
                self.vk_device.cmd_push_constants(
                    *CommandBuffer,
                    PipeLayout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    unsafe {
                        std::slice::from_raw_parts(
                            &PushConstantVertex as *const SUniformQuadGPos as *const u8,
                            std::mem::size_of::<SUniformQuadGPos>(),
                        )
                    },
                );
            }
        }

        let mut DrawCount = cmd.quad_num;
        let mut render_offset: usize = 0;

        while DrawCount > 0 {
            let RealDrawCount = if DrawCount > GRAPHICS_MAX_QUADS_RENDER_COUNT {
                GRAPHICS_MAX_QUADS_RENDER_COUNT
            } else {
                DrawCount
            };

            let IndexOffset = (cmd.quad_offset + render_offset) * 6;
            if !CanBePushed {
                // create uniform buffer
                let mut UniDescrSet = SDeviceDescriptorSet::default();
                if !self.GetUniformBufferObject(
                    exec_buffer.thread_index,
                    true,
                    &mut UniDescrSet,
                    RealDrawCount,
                    &cmd.quad_info[render_offset] as *const SQuadRenderInfo as *const c_void,
                    RealDrawCount * std::mem::size_of::<SQuadRenderInfo>(),
                    self.cur_image_index,
                ) {
                    return false;
                }

                unsafe {
                    self.vk_device.cmd_bind_descriptor_sets(
                        *CommandBuffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        PipeLayout,
                        if IsTextured { 1 } else { 0 },
                        &[UniDescrSet.descriptor],
                        &[],
                    );
                }
                if render_offset > 0 {
                    let QuadOffset: i32 = (cmd.quad_offset + render_offset) as i32;
                    unsafe {
                        self.vk_device.cmd_push_constants(
                            *CommandBuffer,
                            PipeLayout,
                            vk::ShaderStageFlags::VERTEX,
                            (std::mem::size_of::<SUniformQuadGPos>() - std::mem::size_of::<i32>())
                                as u32,
                            unsafe {
                                std::slice::from_raw_parts(
                                    &QuadOffset as *const i32 as *const u8,
                                    std::mem::size_of::<i32>(),
                                )
                            },
                        );
                    }
                }
            }

            unsafe {
                self.vk_device.cmd_draw_indexed(
                    *CommandBuffer,
                    (RealDrawCount * 6) as u32,
                    1,
                    IndexOffset as u32,
                    0,
                    0,
                );
            }

            render_offset += RealDrawCount;
            DrawCount -= RealDrawCount;
        }

        return true;
    }

    /*
                fn Cmd_RenderText_FillExecuteBuffer(exec_buffer: &mut SRenderCommandExecuteBuffer, cmd: &SCommand_RenderText)
                {
                    let buffer_container_index: usize = cmd.m_BufferContainerIndex;
                    let buffer_object_index: usize = (usize)m_vBufferContainers[buffer_container_index].m_BufferObjectIndex;
                    const let buffer_object = &mut  self.device.m_vBufferObjects[buffer_object_index];

                    exec_buffer.m_Buffer = buffer_object.m_CurBuffer;
                    exec_buffer.m_BufferOff = buffer_object.m_CurBufferOffset;

                    exec_buffer.m_aDescriptors[0] = self.device.m_vTextures[cmd.m_TextTextureIndex].m_VKTextDescrSet;

                    exec_buffer.m_IndexBuffer = self.m_RenderIndexBuffer;

                    exec_buffer.m_EstimatedRenderCallCount = 1;

                    ExecBufferFillDynamicStates(cmd.state, exec_buffer);
                }

                #[must_use] fn Cmd_RenderText(&mut self,cmd: &SCommand_RenderText, exec_buffer: &SRenderCommandExecuteBuffer ) -> bool
                {
                    std::array<f32, (usize)4 * 2> m;
                    Self::GetStateMatrix(cmd.state, m);

                    bool IsTextured;
                    usize BlendModeIndex;
                    usize DynamicIndex;
                    usize AddressModeIndex;
                    GetStateIndices(exec_buffer, cmd.state, IsTextured, BlendModeIndex, DynamicIndex, AddressModeIndex);
                    IsTextured = true; // text is always textured
                    let PipeLayout = &mut  GetPipeLayout(self.m_TextPipeline, IsTextured, BlendModeIndex, DynamicIndex);
                    let PipeLine = &mut  GetPipeline(self.m_TextPipeline, IsTextured, BlendModeIndex, DynamicIndex);

            let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
            if (!self.GetGraphicCommandBuffer(&mut command_buffer_ptr, exec_buffer.m_ThreadIndex as usize)) {
                return false;
            }
                    let CommandBuffer = &mut  *command_buffer_ptr;

                    BindPipeline(exec_buffer.m_ThreadIndex, CommandBuffer, exec_buffer, PipeLine, cmd.state);

                    let aVertexBuffers = [exec_buffer.m_Buffer];
                    let aOffsets = [exec_buffer.m_BufferOff as vk::DeviceSize];
                    unsafe { self.m_VKDevice.cmd_bind_vertex_buffers(CommandBuffer, 0, 1, aVertexBuffers.as_ptr(), aOffsets.as_ptr()); }

                    unsafe { self.m_VKDevice.cmd_bind_index_buffer(CommandBuffer, exec_buffer.m_IndexBuffer, 0, vk::IndexType::UINT32); }

                    unsafe { self.m_VKDevice.cmd_bind_descriptor_sets(CommandBuffer, vk::PipelineBindPoint::GRAPHICS , PipeLayout, 0, 1, &exec_buffer.m_aDescriptors[0].m_Descriptor, 0, std::ptr::null()); }

                    SUniformGTextPos PosTexSizeConstant;
                    mem_copy(PosTexSizeConstant.m_aPos, m.as_ptr(), m.len() * std::mem::size_of::<f32>());
                    PosTexSizeConstant.m_TextureSize = cmd.m_TextureSize;

                    unsafe { self.m_VKDevice.cmd_push_constants(CommandBuffer, PipeLayout, vk::ShaderStageFlags::VERTEX, 0, std::mem::size_of::<SUniformGTextPos>(), &PosTexSizeConstant); }

                    SUniformTextFragment FragmentConstants;

                    FragmentConstants.m_Constants.m_TextColor = cmd.m_TextColor;
                    FragmentConstants.m_Constants.m_TextOutlineColor = cmd.m_TextOutlineColor;
                    unsafe { self.m_VKDevice.cmd_push_constants(CommandBuffer, PipeLayout, vk::ShaderStageFlags::FRAGMENT, std::mem::size_of::<SUniformGTextPos>() + std::mem::size_of::<SUniformTextGFragmentOffset>(), std::mem::size_of::<SUniformTextFragment>(), &FragmentConstants); }

                    unsafe { self.m_VKDevice.cmd_draw_indexed(CommandBuffer, (cmd.m_DrawNum) as u32, 1, 0, 0, 0); }

                    return true;
                }
    */

    fn BufferContainer_FillExecuteBuffer(
        &self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        State: &State,
        buffer_container_index: usize,
        DrawCalls: usize,
    ) {
        let buffer_object_index: usize =
            self.device.buffer_containers[buffer_container_index].buffer_object_index;
        let buffer_object = &self.device.buffer_objects[buffer_object_index];

        exec_buffer.buffer = buffer_object.cur_buffer;
        exec_buffer.buffer_off = buffer_object.cur_buffer_offset;

        let IsTextured: bool = Self::GetIsTextured(State);
        if IsTextured {
            let AddressModeIndex: usize = Self::GetAddressModeIndex(State);
            exec_buffer.descriptors[0] = self.device.textures[State.texture_index.unwrap()]
                .vk_standard_textured_descr_sets[AddressModeIndex]
                .clone();
        }

        exec_buffer.index_buffer = self.render_index_buffer;

        exec_buffer.estimated_render_call_count = DrawCalls;

        self.ExecBufferFillDynamicStates(&State, exec_buffer);
    }

    fn Cmd_RenderQuadContainer_FillExecuteBuffer(
        &self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &SCommand_RenderQuadContainer,
    ) {
        self.BufferContainer_FillExecuteBuffer(
            exec_buffer,
            &cmd.state,
            cmd.buffer_container_index,
            1,
        );
    }

    #[must_use]
    fn Cmd_RenderQuadContainer(
        &mut self,
        cmd: &SCommand_RenderQuadContainer,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::GetStateMatrix(&cmd.state, &mut m);

        let mut IsTextured: bool = Default::default();
        let mut BlendModeIndex: usize = Default::default();
        let mut DynamicIndex: usize = Default::default();
        let mut AddressModeIndex: usize = Default::default();
        Self::GetStateIndices(
            exec_buffer,
            &cmd.state,
            &mut IsTextured,
            &mut BlendModeIndex,
            &mut DynamicIndex,
            &mut AddressModeIndex,
        );
        let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.GetGraphicCommandBuffer(&mut command_buffer_ptr, exec_buffer.thread_index as usize)
        {
            return false;
        }
        let CommandBuffer = unsafe { &mut *command_buffer_ptr };
        let (PipeLine, PipeLayout) = Self::GetStandardPipeAndLayout(
            &mut self.standard_line_pipeline,
            &mut self.standard_pipeline,
            false,
            IsTextured,
            BlendModeIndex,
            DynamicIndex,
        );

        Self::BindPipeline(
            &self.vk_device,
            &mut self.last_pipeline_per_thread,
            exec_buffer.thread_index,
            *CommandBuffer,
            exec_buffer,
            *PipeLine,
            &cmd.state,
        );

        let aVertexBuffers = [exec_buffer.buffer];
        let aOffsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            self.vk_device
                .cmd_bind_vertex_buffers(*CommandBuffer, 0, &aVertexBuffers, &aOffsets);
        }

        let IndexOffset = cmd.offset as vk::DeviceSize;

        unsafe {
            self.vk_device.cmd_bind_index_buffer(
                *CommandBuffer,
                exec_buffer.index_buffer,
                IndexOffset,
                vk::IndexType::UINT32,
            );
        }

        if IsTextured {
            unsafe {
                self.vk_device.cmd_bind_descriptor_sets(
                    *CommandBuffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    *PipeLayout,
                    0,
                    &[exec_buffer.descriptors[0].descriptor],
                    &[],
                );
            }
        }

        unsafe {
            self.vk_device.cmd_push_constants(
                *CommandBuffer,
                *PipeLayout,
                vk::ShaderStageFlags::VERTEX,
                0,
                unsafe {
                    std::slice::from_raw_parts(
                        m.as_ptr() as *const u8,
                        std::mem::size_of::<SUniformGPos>(),
                    )
                },
            );
        }

        unsafe {
            self.vk_device
                .cmd_draw_indexed(*CommandBuffer, (cmd.draw_num) as u32, 1, 0, 0, 0);
        }

        return true;
    }

    fn Cmd_RenderQuadContainerEx_FillExecuteBuffer(
        &self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &SCommand_RenderQuadContainerEx,
    ) {
        self.BufferContainer_FillExecuteBuffer(
            exec_buffer,
            &cmd.state,
            cmd.buffer_container_index,
            1,
        );
    }

    #[must_use]
    fn Cmd_RenderQuadContainerEx(
        &mut self,
        cmd: &SCommand_RenderQuadContainerEx,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::GetStateMatrix(&cmd.state, &mut m);

        let is_rotationless: bool = !(cmd.rotation != 0.0);
        let mut is_textured: bool = Default::default();
        let mut blend_mode_index: usize = Default::default();
        let mut dynamic_index: usize = Default::default();
        let mut address_mode_index: usize = Default::default();
        Self::GetStateIndices(
            exec_buffer,
            &cmd.state,
            &mut is_textured,
            &mut blend_mode_index,
            &mut dynamic_index,
            &mut address_mode_index,
        );
        let mut command_buffer: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.GetGraphicCommandBuffer(&mut command_buffer, exec_buffer.thread_index as usize) {
            return false;
        }
        let command_buffer = unsafe { &mut *command_buffer };
        let (pipeline, pipe_layout) = Self::GetPipelineAndLayout(
            if is_rotationless {
                &self.prim_ex_rotationless_pipeline
            } else {
                &self.prim_ex_pipeline
            },
            is_textured,
            blend_mode_index as usize,
            dynamic_index as usize,
        );

        Self::BindPipeline(
            &self.vk_device,
            &mut self.last_pipeline_per_thread,
            exec_buffer.thread_index,
            *command_buffer,
            exec_buffer,
            *pipeline,
            &cmd.state,
        );

        let aVertexBuffers = [exec_buffer.buffer];
        let aOffsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            self.vk_device
                .cmd_bind_vertex_buffers(*command_buffer, 0, &aVertexBuffers, &aOffsets);
        }

        let index_offset = cmd.offset as vk::DeviceSize;

        unsafe {
            self.vk_device.cmd_bind_index_buffer(
                *command_buffer,
                exec_buffer.index_buffer,
                index_offset,
                vk::IndexType::UINT32,
            );
        }

        if is_textured {
            unsafe {
                self.vk_device.cmd_bind_descriptor_sets(
                    *command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    *pipe_layout,
                    0,
                    &[exec_buffer.descriptors[0].descriptor],
                    &[],
                );
            }
        }

        let mut push_constant_color = SUniformPrimExGVertColor::default();
        let mut push_constant_vertex = SUniformPrimExGPos::default();
        let mut vertex_push_constant_size: usize = std::mem::size_of::<SUniformPrimExGPos>();

        push_constant_color = cmd.vertex_color;
        push_constant_vertex.base.pos = m;

        if !is_rotationless {
            push_constant_vertex.rotation = cmd.rotation;
            push_constant_vertex.center = cmd.center;
        } else {
            vertex_push_constant_size = std::mem::size_of::<SUniformPrimExGPosRotationless>();
        }

        unsafe {
            self.vk_device.cmd_push_constants(
                *command_buffer,
                *pipe_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                unsafe {
                    std::slice::from_raw_parts(
                        &push_constant_vertex as *const SUniformPrimExGPos as *const u8,
                        vertex_push_constant_size,
                    )
                },
            );
        }
        unsafe {
            self.vk_device.cmd_push_constants(
                *command_buffer,
                *pipe_layout,
                vk::ShaderStageFlags::FRAGMENT,
                (std::mem::size_of::<SUniformTileGVertColorAlign>()
                    + std::mem::size_of::<SUniformPrimExGVertColorAlign>()) as u32,
                unsafe {
                    std::slice::from_raw_parts(
                        &push_constant_color as *const ColorRGBA as *const u8,
                        std::mem::size_of::<SUniformPrimExGVertColor>(),
                    )
                },
            );
        }

        unsafe {
            self.vk_device
                .cmd_draw_indexed(*command_buffer, (cmd.draw_num) as u32, 1, 0, 0, 0);
        }

        return true;
    }

    fn Cmd_RenderQuadContainerAsSpriteMultiple_FillExecuteBuffer(
        &self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &SCommand_RenderQuadContainerAsSpriteMultiple,
    ) {
        self.BufferContainer_FillExecuteBuffer(
            exec_buffer,
            &cmd.state,
            cmd.buffer_container_index,
            ((cmd.draw_count - 1) / GRAPHICS_MAX_PARTICLES_RENDER_COUNT) + 1,
        );
    }

    #[must_use]
    fn Cmd_RenderQuadContainerAsSpriteMultiple(
        &mut self,
        cmd: &SCommand_RenderQuadContainerAsSpriteMultiple,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::GetStateMatrix(&cmd.state, &mut m);

        let CanBePushed: bool = cmd.draw_count <= 1;

        let mut IsTextured: bool = Default::default();
        let mut BlendModeIndex: usize = Default::default();
        let mut DynamicIndex: usize = Default::default();
        let mut AddressModeIndex: usize = Default::default();
        Self::GetStateIndices(
            exec_buffer,
            &cmd.state,
            &mut IsTextured,
            &mut BlendModeIndex,
            &mut DynamicIndex,
            &mut AddressModeIndex,
        );
        let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.GetGraphicCommandBuffer(&mut command_buffer_ptr, exec_buffer.thread_index as usize)
        {
            return false;
        }
        let CommandBuffer = unsafe { &mut *command_buffer_ptr };
        let (Pipeline, PipeLayout) = Self::GetPipelineAndLayout(
            if CanBePushed {
                &self.sprite_multi_push_pipeline
            } else {
                &self.sprite_multi_pipeline
            },
            IsTextured,
            BlendModeIndex as usize,
            DynamicIndex as usize,
        );
        let (Pipeline, PipeLayout) = (*Pipeline, *PipeLayout);

        Self::BindPipeline(
            &self.vk_device,
            &mut self.last_pipeline_per_thread,
            exec_buffer.thread_index,
            *CommandBuffer,
            exec_buffer,
            Pipeline,
            &cmd.state,
        );

        let aVertexBuffers = [exec_buffer.buffer];
        let aOffsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            self.vk_device
                .cmd_bind_vertex_buffers(*CommandBuffer, 0, &aVertexBuffers, &aOffsets);
        }

        let IndexOffset = cmd.offset as vk::DeviceSize;
        unsafe {
            self.vk_device.cmd_bind_index_buffer(
                *CommandBuffer,
                exec_buffer.index_buffer,
                IndexOffset,
                vk::IndexType::UINT32,
            );
        }

        unsafe {
            self.vk_device.cmd_bind_descriptor_sets(
                *CommandBuffer,
                vk::PipelineBindPoint::GRAPHICS,
                PipeLayout,
                0,
                &[exec_buffer.descriptors[0].descriptor],
                &[],
            );
        }

        if CanBePushed {
            let mut PushConstantColor = SUniformSpriteMultiPushGVertColor::default();
            let mut PushConstantVertex = SUniformSpriteMultiPushGPos::default();

            PushConstantColor = cmd.vertex_color;

            PushConstantVertex.base.pos = m;
            PushConstantVertex.base.center = cmd.center;

            for i in 0..cmd.draw_count {
                PushConstantVertex.psr[i] = vec4 {
                    x: cmd.render_info[i].pos.x,
                    y: cmd.render_info[i].pos.y,
                    z: cmd.render_info[i].scale,
                    w: cmd.render_info[i].rotation,
                };
            }
            unsafe {
                self.vk_device.cmd_push_constants(
                    *CommandBuffer,
                    PipeLayout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    unsafe {
                        std::slice::from_raw_parts(
                            &PushConstantVertex as *const SUniformSpriteMultiPushGPos as *const u8,
                            std::mem::size_of::<SUniformSpriteMultiPushGPosBase>()
                                + std::mem::size_of::<vec4>() * cmd.draw_count,
                        )
                    },
                );
            }
            unsafe {
                self.vk_device.cmd_push_constants(
                    *CommandBuffer,
                    PipeLayout,
                    vk::ShaderStageFlags::FRAGMENT,
                    std::mem::size_of::<SUniformSpriteMultiPushGPos>() as u32,
                    unsafe {
                        std::slice::from_raw_parts(
                            &PushConstantColor as *const ColorRGBA as *const u8,
                            std::mem::size_of::<SUniformSpriteMultiPushGVertColor>(),
                        )
                    },
                );
            }
        } else {
            let mut PushConstantColor = SUniformSpriteMultiGVertColor::default();
            let mut PushConstantVertex = SUniformSpriteMultiGPos::default();

            PushConstantColor = cmd.vertex_color;

            PushConstantVertex.pos = m;
            PushConstantVertex.center = cmd.center;

            unsafe {
                self.vk_device.cmd_push_constants(
                    *CommandBuffer,
                    PipeLayout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    unsafe {
                        std::slice::from_raw_parts(
                            &PushConstantVertex as *const SUniformSpriteMultiGPos as *const u8,
                            std::mem::size_of::<SUniformSpriteMultiGPos>(),
                        )
                    },
                );
            }
            unsafe {
                self.vk_device.cmd_push_constants(
                    *CommandBuffer,
                    PipeLayout,
                    vk::ShaderStageFlags::FRAGMENT,
                    (std::mem::size_of::<SUniformSpriteMultiGPos>()
                        + std::mem::size_of::<SUniformSpriteMultiGVertColorAlign>())
                        as u32,
                    unsafe {
                        std::slice::from_raw_parts(
                            &PushConstantColor as *const SUniformSpriteMultiGVertColor as *const u8,
                            std::mem::size_of::<SUniformSpriteMultiGVertColor>(),
                        )
                    },
                );
            }
        }

        let RSPCount: usize = 512;
        let mut DrawCount = cmd.draw_count;
        let mut RenderOffset: usize = 0;

        while DrawCount > 0 {
            let UniformCount = if DrawCount > RSPCount {
                RSPCount
            } else {
                DrawCount
            };

            if !CanBePushed {
                // create uniform buffer
                let mut UniDescrSet = SDeviceDescriptorSet::default();
                if !self.GetUniformBufferObject(
                    exec_buffer.thread_index,
                    false,
                    &mut UniDescrSet,
                    UniformCount,
                    &cmd.render_info[RenderOffset] as *const SRenderSpriteInfo as *const c_void,
                    UniformCount * std::mem::size_of::<SRenderSpriteInfo>(),
                    self.cur_image_index,
                ) {
                    return false;
                }

                unsafe {
                    self.vk_device.cmd_bind_descriptor_sets(
                        *CommandBuffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        PipeLayout,
                        1,
                        &[UniDescrSet.descriptor],
                        &[],
                    );
                }
            }

            unsafe {
                self.vk_device.cmd_draw_indexed(
                    *CommandBuffer,
                    (cmd.draw_num) as u32,
                    UniformCount as u32,
                    0,
                    0,
                    0,
                );
            }

            RenderOffset += RSPCount;
            DrawCount -= RSPCount;
        }

        return true;
    }
    /*
            #[must_use] fn Cmd_WindowCreateNtf(&mut self,cmd: &SCommand_WindowCreateNtf) -> bool
            {
                dbg_msg("vulkan", "creating new surface.");
                self.m_pWindow = SDL_GetWindowFromID(cmd.m_WindowID);
                if(self.m_RenderingPaused)
                {
        #ifdef CONF_PLATFORM_ANDROID
                    if(!CreateSurface(self.m_pWindow))
                        return false;
                    self.m_RecreateSwapChain = true;
        #endif
                    self.m_RenderingPaused = false;
                    if(!PureMemoryFrame())
                        return false;
                    if(!PrepareFrame())
                        return false;
                }

                return true;
            }

            #[must_use] fn Cmd_WindowDestroyNtf(&mut self,cmd: &SCommand_WindowDestroyNtf) -> bool
            {
                dbg_msg("vulkan", "surface got destroyed.");
                if(!m_RenderingPaused)
                {
                    if(!WaitFrame())
                        return false;
                    self.m_RenderingPaused = true;
                    vkDeviceWaitIdle(self.m_VKDevice);
        #ifdef CONF_PLATFORM_ANDROID
                    CleanupVulkanSwapChain(true);
        #endif
                }

                return true;
            }
    */
    #[must_use]
    pub fn init_instance_while_io(
        window: &sdl2::video::Window,
        _gpu_list: &mut TTWGraphicsGPUList,
        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,

        canvas_width: u32,
        canvas_height: u32,

        runtime_threadpool: &Arc<rayon::ThreadPool>,

        options: &Options,
    ) -> Result<Self, ArrayString<4096>> {
        let dbg_mode = options.dbg_gfx; // TODO config / options
        let dbg = Arc::new(AtomicU8::new(dbg_mode as u8));
        let error = Arc::new(Mutex::new(Error::default()));
        let mut sys = system::System::new();

        let vk_res = Self::InitVulkanSDL(
            window,
            canvas_width,
            canvas_height,
            dbg_mode,
            &error,
            &mut sys,
        );
        if let Err(err) = vk_res {
            return Err(err);
        }
        let (
            entry,
            instance,
            device,
            _gpu_list,
            limits,
            _renderer_name,
            _vendor_name,
            _version_name,
            phy_gpu,
            graphics_queue_index,
            graphics_queue,
            presentation_queue,
            ash_surface,
            surface,
        ) = vk_res.unwrap();

        // TODO!  RegisterCommands();

        let mut thread_count = options.thread_count;
        if thread_count <= 1 {
            thread_count = 1;
        } else {
            thread_count = thread_count.clamp(
                3,
                3.max(
                    std::thread::available_parallelism()
                        .unwrap_or(NonZeroUsize::new(3).unwrap())
                        .get(),
                ),
            );
        }

        /*
        // start threads
        dbg_assert(self.m_ThreadCount != 2, "Either use 1 main thread or at least 2 extra rendering threads.");
        if(self.m_ThreadCount > 1)
        {
            self.m_vvThreadCommandLists.resize(self.m_ThreadCount - 1);
            self.m_vThreadHelperHadCommands.resize(self.m_ThreadCount - 1, false);
            for(auto &ThreadCommandList : self.m_vvThreadCommandLists)
            {
                ThreadCommandList.reserve(256);
            }

            for(let i: usize = 0; i < self.m_ThreadCount - 1; ++i)
            {
                auto *pRenderThread = new SRenderThread();
                std::unique_lock<std::mutex> Lock(pRenderThread.m_Mutex);
                self.m_vpRenderThreads.push(pRenderThread);
                pRenderThread.m_Thread = std::thread([this, i]() { RunThread(i); });
                // wait until thread started
                pRenderThread.m_Cond.wait(Lock, [pRenderThread]() -> bool { return pRenderThread.m_Started; });
            }
        }*/

        let swap_chain = ash::extensions::khr::Swapchain::new(&instance, &device);

        Ok(Self {
            dbg: dbg.clone(),
            gfx_vsync: Default::default(),
            shader_files: Default::default(),
            texture_memory_usage: texture_memory_usage.clone(),
            buffer_memory_usage: buffer_memory_usage.clone(),
            stream_memory_usage: stream_memory_usage.clone(),
            staging_memory_usage: staging_memory_usage.clone(),
            // m_pGPUList: gpu_list,
            next_multi_sampling_count: Default::default(),
            recreate_swap_chain: Default::default(),
            swap_chain_created: Default::default(),
            rendering_paused: Default::default(),
            has_dynamic_viewport: Default::default(),
            dynamic_viewport_offset: Default::default(),
            dynamic_viewport_size: Default::default(),
            index_buffer: Default::default(),
            index_buffer_memory: Default::default(),
            render_index_buffer: Default::default(),
            render_index_buffer_memory: Default::default(),
            cur_render_index_primitive_count: Default::default(),
            screenshot_helper: Default::default(),
            get_presented_img_data_helper_mem: Default::default(),
            get_presented_img_data_helper_image: Default::default(),
            get_presented_img_data_helper_mapped_memory: std::ptr::null_mut(),
            get_presented_img_data_helper_mapped_layout_offset: Default::default(),
            get_presented_img_data_helper_mapped_layout_pitch: Default::default(),
            get_presented_img_data_helper_width: Default::default(),
            get_presented_img_data_helper_height: Default::default(),
            get_presented_img_data_helper_fence: Default::default(),
            thread_count,
            cur_command_in_pipe: Default::default(),
            cur_render_call_count_in_pipe: Default::default(),
            commands_in_pipe: Default::default(),
            render_calls_in_pipe: Default::default(),
            last_commands_in_pipe_thread_index: Default::default(),
            render_threads: Default::default(),
            swap_chain_image_view_list: Default::default(),
            swap_chain_multi_sampling_images: Default::default(),
            framebuffer_list: Default::default(),
            main_draw_command_buffers: Default::default(),
            thread_draw_command_buffers: Default::default(),
            helper_thread_draw_command_buffers: Default::default(),
            used_thread_draw_command_buffer: Default::default(),
            wait_semaphores: Default::default(),
            sig_semaphores: Default::default(),
            memory_sempahores: Default::default(),
            frame_fences: Default::default(),
            image_fences: Default::default(),
            cur_frame: Default::default(),
            image_last_frame_check: Default::default(),
            last_presented_swap_chain_image_index: Default::default(),
            vk_instance: instance.clone(),
            vk_entry: entry.clone(),
            vk_gpu: phy_gpu,
            vk_graphics_queue_index: graphics_queue_index,
            surface: ash_surface,
            device: Device::new(
                dbg,
                &instance,
                &device,
                error.clone(),
                phy_gpu,
                texture_memory_usage,
                buffer_memory_usage,
                stream_memory_usage,
                staging_memory_usage,
                limits,
            ),
            vk_device: device,
            vk_graphics_queue: graphics_queue,
            vk_present_queue: presentation_queue,
            vk_present_surface: surface,
            vk_swap_img_and_viewport_extent: Default::default(),
            debug_messenger: Default::default(),
            standard_pipeline: Default::default(),
            standard_line_pipeline: Default::default(),
            standard_3d_pipeline: Default::default(),
            text_pipeline: Default::default(),
            tile_pipeline: Default::default(),
            tile_border_pipeline: Default::default(),
            tile_border_line_pipeline: Default::default(),
            prim_ex_pipeline: Default::default(),
            prim_ex_rotationless_pipeline: Default::default(),
            sprite_multi_pipeline: Default::default(),
            sprite_multi_push_pipeline: Default::default(),
            quad_pipeline: Default::default(),
            quad_push_pipeline: Default::default(),
            last_pipeline_per_thread: Default::default(),
            command_pools: Default::default(),
            vk_render_pass: Default::default(),
            vk_surf_format: Default::default(),
            vk_swap_chain_ash: swap_chain,
            vk_swap_chain_khr: Default::default(),
            vk_swap_chain_images: Default::default(),
            cur_frames: Default::default(),
            cur_image_index: Default::default(),
            canvas_width,
            canvas_height,
            //m_pWindow: window.clone(),
            clear_color: Default::default(),
            thread_command_lists: Default::default(),
            thread_helper_had_commands: Default::default(),
            //m_aCommandCallbacks: Default::default(),
            error: error,
            check_res: Default::default(),

            sys: sys,

            runtime_threadpool: runtime_threadpool.clone(),
        })
    }
    /*
            #[must_use] fn Cmd_PostShutdown(&mut self,const CCommandProcessorFragment_GLBase::SCommand_PostShutdown *pCommand) -> bool
            {
                for(let i: usize = 0; i < self.m_ThreadCount - 1; ++i)
                {
                    auto *pThread = self.m_vpRenderThreads[i].get();
                    {
                        std::unique_lock<std::mutex> Lock(pThread.m_Mutex);
                        pThread.m_Finished = true;
                        pThread.m_Cond.notify_one();
                    }
                    pThread.m_Thread.join();
                }
                self.m_vpRenderThreads.clear();
                self.m_vvThreadCommandLists.clear();
                self.m_vThreadHelperHadCommands.clear();

                self.m_ThreadCount = 1;

                CleanupVulkanSDL();

                return true;
            }
    */

    /****************
     * RENDER THREADS
     *****************/

    fn RunThread(
        &mut self,
        ThreadIndex: usize,
        thread: Arc<(Mutex<SRenderThread>, std::sync::Condvar)>,
    ) {
        //auto *pThread = self.m_vpRenderThreads[ThreadIndex].get();
        let mut Lock = thread.0.lock().unwrap();
        Lock.started = true;
        thread.1.notify_one();

        while !Lock.finished {
            Lock = thread
                .1
                .wait_while(Lock, |pThread| -> bool {
                    return !pThread.is_rendering && !pThread.finished;
                })
                .unwrap();
            thread.1.notify_one();

            // set this to true, if you want to benchmark the render thread times
            let s_BenchmarkRenderThreads = false;
            let _ThreadRenderTime = Duration::from_nanos(0);
            /*TODO! if(IsVerbose(&*self.dbg) && s_BenchmarkRenderThreads)
            {
                ThreadRenderTime = time_get_nanoseconds();
            }*/

            if !Lock.finished {
                let mut HasErrorFromCmd: bool = false;
                for _NextCmd in &self.thread_command_lists[ThreadIndex] {
                    // TODO! if (!self.CommandCB(&NextCmd.0, &NextCmd.1))
                    {
                        // an error occured, the thread will not continue execution
                        HasErrorFromCmd = true;
                        break;
                    }
                }
                self.thread_command_lists[ThreadIndex].clear();

                if !HasErrorFromCmd
                    && self.used_thread_draw_command_buffer[ThreadIndex + 1]
                        [self.cur_image_index as usize]
                {
                    let GraphicThreadCommandBuffer = &mut self.thread_draw_command_buffers
                        [ThreadIndex + 1][self.cur_image_index as usize];
                    unsafe {
                        self.vk_device
                            .end_command_buffer(*GraphicThreadCommandBuffer);
                    }
                }
            }

            if is_verbose(&*self.dbg) && s_BenchmarkRenderThreads {
                //self.sys.log ("vulkan").msg("render thread ").msg(ThreadIndex).msg(" took ").msg(time_get_nanoseconds() - ThreadRenderTime).msg(" ns to finish");
            }

            Lock.is_rendering = false;
        }
    }
    /*
    CCommandProcessorFragment_GLBase *CreateVulkanCommandProcessorFragment() { return new CCommandProcessorFragment_Vulkan(); }

    void *CreateBackend() { return CreateVulkanCommandProcessorFragment(); }

    fn DestroyBackend(&mut self, void *pBackend) { delete(CCommandProcessorFragment_GLBase *)pBackend; }

    fn StartCommands(&mut self, void *pBackendRaw, CommandCount: usize, EstimatedRenderCallCount: usize)
    {
        auto *pBackend = (CCommandProcessorFragment_GLBase *)pBackendRaw;
        pBackend->StartCommands(CommandCount, EstimatedRenderCallCount);
    }

    fn EndCommands(&mut self, void *pBackendRaw)
    {
        auto *pBackend = (CCommandProcessorFragment_GLBase *)pBackendRaw;
        pBackend->EndCommands();
    }
     */

    pub fn get_mt_backend(&self) -> VulkanBackendMt {
        VulkanBackendMt {
            mem_allocator: self.device.mem_allocator.clone(),
        }
    }
}

impl GraphicsBackendInterface for VulkanBackend {
    fn set_files(&mut self, files: Vec<(String, Vec<u8>)>) {
        for (file_name, binary) in files {
            self.shader_files.insert(
                "shader/vulkan/".to_string() + &file_name,
                SShaderFileCache { binary },
            );
        }
    }

    #[must_use]
    fn init_while_io(
        &mut self,
        capabilities: &mut SBackendCapabilites,
    ) -> Result<(), ArrayString<4096>> {
        capabilities.tile_buffering = true;
        capabilities.quad_buffering = true;
        capabilities.text_buffering = true;
        capabilities.quad_container_buffering = true;
        capabilities.shader_support = true;

        capabilities.mip_mapping = true;
        capabilities.has_3d_textures = false;
        capabilities.has_2d_array_textures = true;
        capabilities.npot_textures = true;

        capabilities.triangles_as_quads = true;

        self.device.global_texture_lod_bias = 500; // TODO! g_Config.m_GfxGLTextureLODBIAS;

        self.device.limits.multi_sampling_count =
            0 /* TODO! g_Config.m_GfxFsaaSamples */ & 0xFFFFFFFE; // ignore the uneven bit, only even multi sampling works

        /* TODO: TGLBackendReadPresentedImageData &ReadPresentedImgDataFunc =
        *cmd.m_pReadPresentedImageDataFunc;
        ReadPresentedImgDataFunc = [this](u32 &Width, u32 &Height,
                          u32 &Format,
                          Vec<u8> &vDstData) {
          return GetPresentedImageData(Width, Height, Format, vDstData);
        };*/

        if self.init_vulkan_without_io() != 0 {
            return Err(ArrayString::from_str("Failed to initialize vulkan.").unwrap());
        }

        Ok(())
    }

    #[must_use]
    fn init(&mut self) -> Result<(), ArrayString<4096>> {
        if self.init_vulkan_with_io::<true>() != 0 {
            return Err(ArrayString::from_str("Failed to initialize vulkan.").unwrap());
        }

        let mut aIndices: [u32; StreamDataMax::MaxVertices as usize / 4 * 6] =
            unsafe { std::mem::MaybeUninit::uninit().assume_init() };
        let mut Primq: u32 = 0;
        for i in (0..(StreamDataMax::MaxVertices as usize / 4 * 6) as usize).step_by(6) {
            aIndices[i] = Primq;
            aIndices[i + 1] = Primq + 1;
            aIndices[i + 2] = Primq + 2;
            aIndices[i + 3] = Primq;
            aIndices[i + 4] = Primq + 2;
            aIndices[i + 5] = Primq + 3;
            Primq += 4;
        }

        if !self.PrepareFrame() {
            return Err(ArrayString::from_str("Failed to prepare frame.").unwrap());
        }

        // TODO: ??? looks completely stupid.. better handle all errors instead
        if self.error.lock().unwrap().has_error {
            return Err(ArrayString::from_str("This is a stupid call.").unwrap());
        }

        if !self.device.CreateIndexBuffer(
            aIndices.as_mut_ptr() as *mut c_void,
            std::mem::size_of::<u32>() * aIndices.len(),
            &mut self.index_buffer,
            &mut self.index_buffer_memory,
            0,
        ) {
            return Err(ArrayString::from_str("Failed to create index buffer.").unwrap());
        }
        if !self.device.CreateIndexBuffer(
            aIndices.as_mut_ptr() as *mut c_void,
            std::mem::size_of::<u32>() * aIndices.len(),
            &mut self.render_index_buffer,
            &mut self.render_index_buffer_memory,
            0,
        ) {
            return Err(ArrayString::from_str("Failed to create index buffer.").unwrap());
        }
        self.cur_render_index_primitive_count = StreamDataMax::MaxVertices as usize / 4;

        self.error.lock().unwrap().can_assert = true;

        Ok(())
    }

    #[must_use]
    fn run_command(&mut self, cmd: &AllCommands) -> ERunCommandReturnTypes {
        /* TODO! no locking pls if(self.m_HasError)
        {
            // ignore all further commands
            return ERunCommandReturnTypes::RUN_COMMAND_COMMAND_ERROR;
        }*/

        //let CallbackObj = &mut  self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::ECommandBufferCMD(Cmd))];
        let mut Buffer = SRenderCommandExecuteBuffer::default();
        Buffer.raw_command = cmd;
        Buffer.thread_index = 0;

        if self.cur_command_in_pipe + 1 == self.commands_in_pipe {
            self.last_commands_in_pipe_thread_index = usize::MAX;
        }

        let mut CanStartThread: bool = false;
        if let AllCommands::Render(_) = cmd {
            let ForceSingleThread: bool = self.last_commands_in_pipe_thread_index == usize::MAX;

            let PotentiallyNextThread: usize =
                ((self.cur_command_in_pipe * (self.thread_count - 1)) / self.commands_in_pipe) + 1;
            if PotentiallyNextThread - 1 > self.last_commands_in_pipe_thread_index {
                CanStartThread = true;
                self.last_commands_in_pipe_thread_index = PotentiallyNextThread - 1;
            }
            Buffer.thread_index = if self.thread_count > 1 && !ForceSingleThread {
                self.last_commands_in_pipe_thread_index + 1
            } else {
                0
            };
            self.FillExecuteBuffer(&cmd, &mut Buffer);
            self.cur_render_call_count_in_pipe += Buffer.estimated_render_call_count;
        }
        let mut is_misc_cmd = false;
        if let AllCommands::Misc(_) = cmd {
            is_misc_cmd = true;
        }
        if is_misc_cmd || (Buffer.thread_index == 0 && !self.rendering_paused) {
            if !self.CommandCB(&cmd, &Buffer) {
                // an error occured, stop this command and ignore all further commands
                return ERunCommandReturnTypes::RUN_COMMAND_COMMAND_ERROR;
            }
        } else if !self.rendering_paused {
            if CanStartThread {
                self.StartRenderThread(self.last_commands_in_pipe_thread_index - 1);
            }
            self.thread_command_lists[Buffer.thread_index as usize - 1].push(Buffer);
        }

        self.cur_command_in_pipe += 1;
        return ERunCommandReturnTypes::RUN_COMMAND_COMMAND_HANDLED;
    }

    fn start_commands(
        &mut self,
        backend_buffer: &BackendBuffer,
        CommandCount: usize,
        EstimatedRenderCallCount: usize,
    ) {
        self.commands_in_pipe = CommandCount;
        self.render_calls_in_pipe = EstimatedRenderCallCount;
        self.cur_command_in_pipe = 0;
        self.cur_render_call_count_in_pipe = 0;

        self.device.update_stream_vertex_buffer(
            backend_buffer.num_vertices * std::mem::size_of::<GL_SVertex>(),
            self.cur_image_index,
        );
    }

    fn end_commands(&mut self) -> Result<&'static mut [GL_SVertex], ()> {
        self.FinishRenderThreads();
        self.commands_in_pipe = 0;
        self.render_calls_in_pipe = 0;

        let mut VKBuffer: vk::Buffer = Default::default();
        let mut VKBufferMem: SDeviceMemoryBlock = Default::default();
        let mut BufferOff: usize = 0;
        let mut memory_ptr: *mut u8 = std::ptr::null_mut();
        let memory_size = StreamDataMax::MaxVertices as usize * std::mem::size_of::<GL_SVertex>();
        if !self.device.CreateStreamVertexBuffer(
            &mut VKBuffer,
            &mut VKBufferMem,
            &mut BufferOff,
            &mut memory_ptr,
            memory_size,
            self.cur_image_index,
        ) {
            return Err(());
        }

        Ok(unsafe {
            std::slice::from_raw_parts_mut(
                memory_ptr as *mut GL_SVertex,
                StreamDataMax::MaxVertices as usize,
            )
        })
    }
}

pub struct VulkanBackendMt {
    pub mem_allocator: Arc<std::sync::Mutex<Option<VulkanAllocator>>>,
}

impl GraphicsBackendMtInterface for VulkanBackendMt {
    fn mem_alloc(
        &self,
        alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> &'static mut [u8] {
        let buffer_data: *const c_void = std::ptr::null();
        let mut mem_allocator = self.mem_allocator.lock().unwrap();
        let mut allocator = mem_allocator.as_mut().unwrap();
        match alloc_type {
            GraphicsMemoryAllocationType::Buffer => {
                let mut res_block: SMemoryBlock<THREADED_STAGING_BUFFER_CACHE_ID> =
                    Default::default();
                allocator.get_staging_buffer(
                    &mut res_block,
                    buffer_data,
                    req_size as vk::DeviceSize,
                );
                unsafe {
                    std::slice::from_raw_parts_mut(res_block.mapped_buffer as *mut u8, req_size)
                }
            }
            GraphicsMemoryAllocationType::Texture => {
                let mut res_block: SMemoryBlock<THREADED_STAGING_BUFFER_IMAGE_CACHE_ID> =
                    Default::default();
                allocator.get_staging_buffer_image(
                    &mut res_block,
                    buffer_data,
                    req_size as vk::DeviceSize,
                );
                unsafe {
                    std::slice::from_raw_parts_mut(res_block.mapped_buffer as *mut u8, req_size)
                }
            }
        }
    }

    fn mem_free(&self, mem: &'static mut [u8]) {
        let mut mem_allocator = self.mem_allocator.lock().unwrap();
        mem_allocator.as_mut().unwrap().free_mem(mem);
    }
}
