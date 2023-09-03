use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::{CStr, CString},
    num::NonZeroUsize,
    os::raw::c_void,
    pin::Pin,
    rc::Rc,
    str::FromStr,
    sync::{
        atomic::{AtomicU64, AtomicU8},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};

use base_log::log::{LogLevel, SystemLogGroup, SystemLogInterface};
use graphics::image::resize;
use graphics_backend_traits::{
    traits::{DriverBackendInterface, GraphicsBackendMtInterface},
    types::BackendCommands,
};
use graphics_base_traits::traits::GraphicsStreamDataInterface;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use anyhow::anyhow;
use graphics_types::{
    command_buffer::{
        AllCommands, CommandClear, CommandCreateBufferObject, CommandDeleteBufferObject,
        CommandIndicesRequiredNumNotify, CommandRecreateBufferObject, CommandRender,
        CommandRenderBorderTile, CommandRenderBorderTileLine,
        CommandRenderQuadContainerAsSpriteMultiple, CommandRenderQuadContainerEx,
        CommandRenderQuadLayer, CommandRenderTileLayer, CommandTextureCreate,
        CommandTextureDestroy, CommandTextureUpdate, CommandUpdateViewport, Commands,
        CommandsRender, ERunCommandReturnTypes, PrimType, SBackendCapabilites, SQuadRenderInfo,
        SRenderSpriteInfo, StreamDataMax, TexFlags, GRAPHICS_MAX_PARTICLES_RENDER_COUNT,
        GRAPHICS_MAX_QUADS_RENDER_COUNT,
    },
    rendering::{BlendType, ColorRGBA, GlColorf, GlVertex, State, WrapType},
    types::{
        GraphicsBackendMemory, GraphicsBackendMemoryStatic, GraphicsBackendMemoryStaticCleaner,
        GraphicsMemoryAllocationType, ImageFormat,
    },
};
use num_traits::FromPrimitive;

use arrayvec::ArrayString;
use ash::vk::{self};

use crate::backends::vulkan::vulkan_types::{
    SRenderThreadInner, MAX_SUB_PASS_COUNT, RENDER_PASS_TYPE_COUNT,
};

use base::{shared_index::SharedIndexGetIndexUnsafe, system::System};
use config::config::EDebugGFXModes;
use math::math::vector::{vec2, vec4};

const VK_BACKEND_MAJOR: usize = 1;
const VK_BACKEND_MINOR: usize = 1;
const _VK_BACKEND_PATCH: usize = 1; // TODO

const SHADER_MAIN_FUNC_NAME: [u8; 5] = ['m' as u8, 'a' as u8, 'i' as u8, 'n' as u8, '\0' as u8];
const APP_NAME: [u8; 6] = [
    'D' as u8, 'D' as u8, 'N' as u8, 'e' as u8, 't' as u8, '\0' as u8,
];
const APP_VK_NAME: [u8; 13] = [
    'D' as u8, 'D' as u8, 'N' as u8, 'e' as u8, 't' as u8, '-' as u8, 'V' as u8, 'u' as u8,
    'l' as u8, 'k' as u8, 'a' as u8, 'n' as u8, '\0' as u8,
];
use super::{
    common::{
        image_mip_level_count, tex_format_to_image_color_channel_count,
        texture_format_to_vulkan_format, vulkan_format_to_image_color_channel_count, EGFXErrorType,
        ETWGraphicsGPUType, STWGraphicGPUItem, TTWGraphicsGPUList,
    },
    vulkan_allocator::{VulkanAllocator, VulkanDeviceInternalMemory},
    vulkan_config::Config,
    vulkan_dbg::{is_verbose, is_verbose_mode},
    vulkan_device::Device,
    vulkan_error::{CheckResult, Error},
    vulkan_limits::Limits,
    vulkan_mem::Memory,
    vulkan_types::{
        CTexture, ESupportedSamplerTypes, EVulkanBackendAddressModes, EVulkanBackendBlendModes,
        EVulkanBackendClipModes, EVulkanBackendTextureModes, RenderPassType, SDeviceDescriptorSet,
        SDeviceMemoryBlock, SFrameBuffers, SFrameUniformBuffers, SMemoryImageBlock,
        SPipelineContainer, SRenderCommandExecuteBuffer, SRenderThread, SShaderFileCache,
        SShaderModule, SSwapImgViewportExtent, SwapChainImage, VKDelayedBufferCleanupItem,
        IMAGE_BUFFER_CACHE_ID,
    },
    vulkan_uniform::{
        SUniformGBlur, SUniformGPos, SUniformGTextPos, SUniformPrimExGPos,
        SUniformPrimExGPosRotationless, SUniformPrimExGVertColor, SUniformPrimExGVertColorAlign,
        SUniformQuadGPos, SUniformQuadPushGBufferObject, SUniformQuadPushGPos,
        SUniformSpriteMultiGPos, SUniformSpriteMultiGVertColor, SUniformSpriteMultiGVertColorAlign,
        SUniformSpriteMultiPushGPos, SUniformSpriteMultiPushGPosBase,
        SUniformSpriteMultiPushGVertColor, SUniformTextGFragmentConstants,
        SUniformTextGFragmentOffset, SUniformTileGPos, SUniformTileGPosBorder,
        SUniformTileGPosBorderLine, SUniformTileGVertColor, SUniformTileGVertColorAlign,
    },
    Options,
};

#[derive(Copy, Clone)]
enum StencilOpType {
    AlwaysPass,
    OnlyWhenPassed,
    OnlyWhenNotPassed,
    None,
}

type TCommandList = Vec<SRenderCommandExecuteBuffer>;
type TThreadCommandList = Vec<TCommandList>;

pub struct VulkanBackendAsh {
    vk_swap_chain_ash: ash::extensions::khr::Swapchain,
    vk_instance: ash::Instance,
    _vk_entry: ash::Entry,
    surface: ash::extensions::khr::Surface,
    vk_device: ash::Device,
}

impl std::fmt::Debug for VulkanBackendAsh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanBackendAsh").finish()
    }
}

#[derive(Debug)]
pub struct VulkanBackend {
    /************************
     * MEMBER VARIABLES
     ************************/
    dbg: Arc<AtomicU8>, // @see EDebugGFXModes
    gfx_vsync: bool,

    shader_files: HashMap<String, SShaderFileCache>,

    // TODO: m_pGPUList: TTWGraphicsGPUList,
    next_multi_sampling_count: u32,
    next_multi_sampling_second_pass_count: u32,

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
    cur_stream_vertex_byte_offset: usize,
    commands_in_pipe: usize,
    render_calls_in_pipe: usize,
    last_commands_in_pipe_thread_index: usize,

    render_threads: Vec<Arc<SRenderThread>>,

    vk_surf_format: vk::SurfaceFormatKHR,

    vk_swap_chain_khr: vk::SwapchainKHR,

    // swap chain images, that are created by the surface
    vk_swap_chain_images: Vec<vk::Image>,

    swap_chain_image_view_list: Vec<vk::ImageView>,
    swap_chain_multi_sampling_images: Vec<SwapChainImage>,
    // when double passes are used, then the first pass does not write into the
    // surface images, but instead in completetly seperate images
    image_list_for_double_pass: Vec<CTexture>,
    multi_sampling_images_for_double_pass: Vec<SwapChainImage>,
    stencil_list_for_pass_transition: Vec<SwapChainImage>,
    stencil_format: vk::Format,
    framebuffer_list: Vec<vk::Framebuffer>,
    framebuffer_double_pass_list: Vec<vk::Framebuffer>,
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

    ash_vk: VulkanBackendAsh,

    vk_gpu: vk::PhysicalDevice,
    vk_graphics_queue_index: u32,
    device: Device,
    vk_graphics_queue: vk::Queue,
    vk_present_queue: vk::Queue,
    vk_present_surface: vk::SurfaceKHR,
    vk_swap_img_and_viewport_extent: SSwapImgViewportExtent,

    _debug_messenger: vk::DebugUtilsMessengerEXT,

    standard_pipeline: SPipelineContainer,
    standard_line_pipeline: SPipelineContainer,
    standard_stencil_only_pipeline: SPipelineContainer,
    standard_stencil_pipeline: SPipelineContainer,
    standard_3d_pipeline: SPipelineContainer,
    blur_pipeline: SPipelineContainer,
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

    current_sub_pass_index: usize,
    current_render_pass_type: RenderPassType,
    vk_render_pass_single_pass: vk::RenderPass,
    // render into a offscreen framebuffer first
    vk_render_pass_double_pass: vk::RenderPass,

    cur_frames: u32,
    cur_image_index: u32,

    canvas_width: f64,
    canvas_height: f64,

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

    logger: SystemLogGroup,

    runtime_threadpool: Arc<rayon::ThreadPool>,
}

pub struct ThreadVkBackendWrapper {
    backend: *mut VulkanBackend,
}

unsafe impl Send for ThreadVkBackendWrapper {}
unsafe impl Sync for ThreadVkBackendWrapper {}

impl VulkanBackend {
    /************************
     * ERROR MANAGEMENT HELPER
     ************************/
    // TODO fn ErroneousCleanup(&mut self )  { self.CleanupVulkanSDL(); }

    fn skip_frames_until_current_frame_is_used_again(&mut self) -> bool {
        // aggressivly try to get more memory
        unsafe { self.ash_vk.vk_device.device_wait_idle().unwrap() };
        for _ in 0..self.device.swap_chain_image_count + 1 {
            if !self.next_frame() {
                return false;
            }
        }

        true
    }

    /************************
     * COMMAND CALLBACKS
     ************************/
    fn command_cb(
        &mut self,
        cmd_param: &AllCommands,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        match &cmd_param {
            AllCommands::Render(render_cmd) => match render_cmd {
                CommandsRender::Clear(cmd) => self.cmd_clear(exec_buffer, cmd),
                CommandsRender::Render(cmd) => self.cmd_render(cmd, exec_buffer),
                CommandsRender::RenderTex3D(_) => todo!(),
                CommandsRender::RenderFirstPassBlurred(cmd) => {
                    self.cmd_render_first_subpass_blurred(cmd, exec_buffer)
                }
                CommandsRender::TileLayer(cmd) => self.cmd_render_tile_layer(cmd, exec_buffer),
                CommandsRender::BorderTile(cmd) => self.cmd_render_border_tile(cmd, exec_buffer),
                CommandsRender::BorderTileLine(cmd) => {
                    self.cmd_render_border_tile_line(cmd, exec_buffer)
                }
                CommandsRender::QuadLayer(cmd) => self.cmd_render_quad_layer(cmd, exec_buffer),
                CommandsRender::QuadContainerEx(cmd) => {
                    self.cmd_render_quad_container_ex(cmd, exec_buffer)
                }
                CommandsRender::QuadContainerSpriteMultiple(cmd) => {
                    self.cmd_render_quad_container_as_sprite_multiple(cmd, exec_buffer)
                }
            },
            AllCommands::Misc(misc_cmd) => match misc_cmd {
                Commands::TextureCreate(cmd) => self.cmd_texture_create(cmd),
                Commands::TextureDestroy(cmd) => self.cmd_texture_destroy(cmd),
                Commands::TextureUpdate(cmd) => self.cmd_texture_update(cmd),
                Commands::CreateBufferObject(cmd) => self.cmd_create_buffer_object(cmd),
                Commands::RecreateBufferObject(cmd) => self.cmd_recreate_buffer_object(cmd),
                Commands::DeleteBufferObject(cmd) => self.cmd_delete_buffer_object(cmd),
                Commands::IndicesRequiredNumNotify(cmd) => {
                    self.cmd_indices_required_num_notify(cmd)
                }
                Commands::Swap(_) => self.cmd_swap(),
                Commands::SwitchToDualPass => {
                    self.cmd_end_single_render_pass_and_start_double_render_pass()
                }
                Commands::NextSubpass => self.cmd_next_subpass(),
                Commands::UpdateViewport(cmd) => self.cmd_update_viewport(cmd),
                Commands::Multisampling => todo!(),
                Commands::VSync => todo!(),
                Commands::TrySwapAndScreenshot => todo!(),
                Commands::WindowCreateNtf => todo!(),
                Commands::WindowDestroyNtf => todo!(),
                _ => todo!(),
            },
        }
    }

    fn fill_execute_buffer(
        &mut self,
        cmd: &AllCommands,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
    ) {
        match &cmd {
            AllCommands::Render(render_cmd) => match render_cmd {
                CommandsRender::Clear(cmd) => self.cmd_clear_fill_execute_buffer(exec_buffer, cmd),
                CommandsRender::Render(cmd) => {
                    self.cmd_render_fill_execute_buffer(exec_buffer, cmd)
                }
                CommandsRender::RenderTex3D(_) => {}
                CommandsRender::RenderFirstPassBlurred(cmd) => {
                    self.cmd_render_first_subpass_blurred_fill_execute_buffer(exec_buffer, cmd)
                }
                CommandsRender::TileLayer(cmd) => {
                    self.cmd_render_tile_layer_fill_execute_buffer(exec_buffer, cmd)
                }
                CommandsRender::BorderTile(cmd) => {
                    self.cmd_render_border_tile_fill_execute_buffer(exec_buffer, cmd)
                }
                CommandsRender::BorderTileLine(cmd) => {
                    self.cmd_render_border_tile_line_fill_execute_buffer(exec_buffer, cmd)
                }
                CommandsRender::QuadLayer(cmd) => {
                    self.cmd_render_quad_layer_fill_execute_buffer(exec_buffer, cmd)
                }
                CommandsRender::QuadContainerEx(cmd) => {
                    self.cmd_render_quad_container_ex_fill_execute_buffer(exec_buffer, cmd)
                }
                CommandsRender::QuadContainerSpriteMultiple(cmd) => self
                    .cmd_render_quad_container_as_sprite_multiple_fill_execute_buffer(
                        exec_buffer,
                        cmd,
                    ),
            },
            AllCommands::Misc(misc_cmd) => match misc_cmd {
                Commands::TextureCreate(_cmd) => {}
                Commands::TextureDestroy(_) => {}
                Commands::TextureUpdate(_) => {}
                Commands::CreateBufferObject(_cmd) => {}
                Commands::RecreateBufferObject(_cmd) => {}
                Commands::DeleteBufferObject(_cmd) => {}
                Commands::IndicesRequiredNumNotify(_cmd) => {}
                Commands::Swap(_swap_cmd) => {}
                Commands::SwitchToDualPass => {}
                Commands::NextSubpass => {}
                Commands::UpdateViewport(cmd) => {
                    self.cmd_update_viewport_fill_execute_buffer(exec_buffer, cmd);
                }
                Commands::Multisampling => {}
                Commands::VSync => {}
                Commands::TrySwapAndScreenshot => {}
                Commands::WindowCreateNtf => {}
                Commands::WindowDestroyNtf => {}
                _ => todo!(),
            },
        }
    }

    /*****************************
     * VIDEO AND SCREENSHOT HELPER
     ******************************/
    // TODO dont unwrap in this function
    #[must_use]
    fn prepare_presented_image_data_image(
        &mut self,
        res_image_data: &mut &mut [u8],
        width: u32,
        height: u32,
    ) -> bool {
        let needs_new_img: bool = width != self.get_presented_img_data_helper_width
            || height != self.get_presented_img_data_helper_height;
        if self.get_presented_img_data_helper_image == vk::Image::null() || needs_new_img {
            if self.get_presented_img_data_helper_image != vk::Image::null() {
                self.delete_presented_image_data_image();
            }
            self.get_presented_img_data_helper_width = width;
            self.get_presented_img_data_helper_height = height;

            let mut image_info = vk::ImageCreateInfo::default();
            image_info.image_type = vk::ImageType::TYPE_2D;
            image_info.extent.width = width;
            image_info.extent.height = height;
            image_info.extent.depth = 1;
            image_info.mip_levels = 1;
            image_info.array_layers = 1;
            image_info.format = vk::Format::R8G8B8A8_UNORM;
            image_info.tiling = vk::ImageTiling::LINEAR;
            image_info.initial_layout = vk::ImageLayout::UNDEFINED;
            image_info.usage = vk::ImageUsageFlags::TRANSFER_DST;
            image_info.samples = vk::SampleCountFlags::TYPE_1;
            image_info.sharing_mode = vk::SharingMode::EXCLUSIVE;

            self.get_presented_img_data_helper_image =
                unsafe { self.ash_vk.vk_device.create_image(&image_info, None) }.unwrap();
            // Create memory to back up the image
            let mem_requirements = unsafe {
                self.ash_vk
                    .vk_device
                    .get_image_memory_requirements(self.get_presented_img_data_helper_image)
            };

            let mut mem_alloc_info = vk::MemoryAllocateInfo::default();
            mem_alloc_info.allocation_size = mem_requirements.size;
            mem_alloc_info.memory_type_index = self.device.mem.find_memory_type(
                self.vk_gpu,
                mem_requirements.memory_type_bits,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
            );

            self.get_presented_img_data_helper_mem.mem =
                unsafe { self.ash_vk.vk_device.allocate_memory(&mem_alloc_info, None) }.unwrap();
            if let Err(_) = unsafe {
                self.ash_vk.vk_device.bind_image_memory(
                    self.get_presented_img_data_helper_image,
                    self.get_presented_img_data_helper_mem.mem,
                    0,
                )
            } {
                return false;
            }

            if !self.device.image_barrier(
                self.get_presented_img_data_helper_image,
                0,
                1,
                0,
                1,
                vk::Format::R8G8B8A8_UNORM,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::GENERAL,
                self.cur_image_index,
            ) {
                return false;
            }

            let sub_resource = vk::ImageSubresource::builder()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .mip_level(0)
                .array_layer(0)
                .build();
            let sub_resource_layout = unsafe {
                self.ash_vk.vk_device.get_image_subresource_layout(
                    self.get_presented_img_data_helper_image,
                    sub_resource,
                )
            };

            self.get_presented_img_data_helper_mapped_memory = unsafe {
                self.ash_vk.vk_device.map_memory(
                    self.get_presented_img_data_helper_mem.mem,
                    0,
                    vk::WHOLE_SIZE,
                    vk::MemoryMapFlags::empty(),
                )
            }
            .unwrap() as *mut u8;
            self.get_presented_img_data_helper_mapped_layout_offset = sub_resource_layout.offset;
            self.get_presented_img_data_helper_mapped_layout_pitch = sub_resource_layout.row_pitch;
            self.get_presented_img_data_helper_mapped_memory = unsafe {
                self.get_presented_img_data_helper_mapped_memory
                    .offset(self.get_presented_img_data_helper_mapped_layout_offset as isize)
            };

            let mut fence_info = vk::FenceCreateInfo::default();
            fence_info.flags = vk::FenceCreateFlags::SIGNALED;
            self.get_presented_img_data_helper_fence =
                unsafe { self.ash_vk.vk_device.create_fence(&fence_info, None) }.unwrap();
        }
        *res_image_data = unsafe {
            std::slice::from_raw_parts_mut(
                self.get_presented_img_data_helper_mapped_memory,
                self.get_presented_img_data_helper_mem.size as usize
                    - self.get_presented_img_data_helper_mapped_layout_offset as usize,
            )
        };
        return true;
    }

    fn delete_presented_image_data_image(&mut self) {
        if self.get_presented_img_data_helper_image != vk::Image::null() {
            unsafe {
                self.ash_vk
                    .vk_device
                    .destroy_fence(self.get_presented_img_data_helper_fence, None);
            }

            self.get_presented_img_data_helper_fence = vk::Fence::null();

            unsafe {
                self.ash_vk
                    .vk_device
                    .destroy_image(self.get_presented_img_data_helper_image, None);
            }
            unsafe {
                self.ash_vk
                    .vk_device
                    .unmap_memory(self.get_presented_img_data_helper_mem.mem);
            }
            unsafe {
                self.ash_vk
                    .vk_device
                    .free_memory(self.get_presented_img_data_helper_mem.mem, None);
            }

            self.get_presented_img_data_helper_image = vk::Image::null();
            self.get_presented_img_data_helper_mem = Default::default();
            self.get_presented_img_data_helper_mapped_memory = std::ptr::null_mut();

            self.get_presented_img_data_helper_width = 0;
            self.get_presented_img_data_helper_height = 0;
        }
    }

    #[must_use]
    fn get_presented_image_data_impl(
        &mut self,
        width: &mut u32,
        height: &mut u32,
        dest_data_buff: &mut Vec<u8>,
        flip_img_data: bool,
        reset_alpha: bool,
    ) -> anyhow::Result<ImageFormat> {
        let mut is_b8_g8_r8_a8: bool = self.vk_surf_format.format == vk::Format::B8G8R8A8_UNORM;
        let uses_rgba_like_format: bool =
            self.vk_surf_format.format == vk::Format::R8G8B8A8_UNORM || is_b8_g8_r8_a8;
        if uses_rgba_like_format && self.last_presented_swap_chain_image_index != u32::MAX {
            let viewport = self
                .vk_swap_img_and_viewport_extent
                .get_presented_image_viewport();
            *width = viewport.width;
            *height = viewport.height;
            let format = ImageFormat::Rgba;

            let image_total_size: usize = *width as usize * *height as usize * 4;

            let mut res_image_data: &mut [u8] = &mut [];
            if !self.prepare_presented_image_data_image(&mut res_image_data, *width, *height) {
                return Err(anyhow!("Could not prepare presented image data"));
            }

            let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
            if !self
                .device
                .get_memory_command_buffer(&mut command_buffer_ptr, self.cur_image_index)
            {
                return Err(anyhow!("Could not get memory command buffer"));
            }
            let command_buffer = &unsafe { *command_buffer_ptr };

            let mut region = vk::BufferImageCopy::default();
            region.buffer_offset = 0;
            region.buffer_row_length = 0;
            region.buffer_image_height = 0;
            region.image_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
            region.image_subresource.mip_level = 0;
            region.image_subresource.base_array_layer = 0;
            region.image_subresource.layer_count = 1;
            region.image_offset = vk::Offset3D::builder().x(0).y(0).z(0).build();
            region.image_extent = vk::Extent3D::builder()
                .width(viewport.width)
                .height(viewport.height)
                .depth(1)
                .build();

            let swap_img =
                &mut self.vk_swap_chain_images[self.last_presented_swap_chain_image_index as usize];

            if !self.device.image_barrier(
                self.get_presented_img_data_helper_image,
                0,
                1,
                0,
                1,
                vk::Format::R8G8B8A8_UNORM,
                vk::ImageLayout::GENERAL,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                self.cur_image_index,
            ) {
                return Err(anyhow!("Image barrier failed for the helper image",));
            }
            if !self.device.image_barrier(
                *swap_img,
                0,
                1,
                0,
                1,
                self.vk_surf_format.format,
                vk::ImageLayout::PRESENT_SRC_KHR,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                self.cur_image_index,
            ) {
                return Err(anyhow!("Image barrier failed for the swapchain image",));
            }

            // If source and destination support blit we'll blit as this also does
            // automatic format conversion (e.g. from BGR to RGB)
            if self.device.optimal_swap_chain_image_blitting
                && self.device.linear_rgba_image_blitting
            {
                let mut blit_size = vk::Offset3D::default();
                blit_size.x = *width as i32;
                blit_size.y = *height as i32;
                blit_size.z = 1;
                let mut image_blit_region = vk::ImageBlit::default();
                image_blit_region.src_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
                image_blit_region.src_subresource.layer_count = 1;
                image_blit_region.src_offsets[1] = blit_size;
                image_blit_region.dst_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
                image_blit_region.dst_subresource.layer_count = 1;
                image_blit_region.dst_offsets[1] = blit_size;

                // Issue the blit command
                unsafe {
                    self.ash_vk.vk_device.cmd_blit_image(
                        *command_buffer,
                        *swap_img,
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        self.get_presented_img_data_helper_image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[image_blit_region],
                        vk::Filter::NEAREST,
                    )
                };

                // transformed to RGBA
                is_b8_g8_r8_a8 = false;
            } else {
                // Otherwise use image copy (requires us to manually flip components)
                let mut image_copy_region = vk::ImageCopy::default();
                image_copy_region.src_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
                image_copy_region.src_subresource.layer_count = 1;
                image_copy_region.dst_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
                image_copy_region.dst_subresource.layer_count = 1;
                image_copy_region.extent.width = *width;
                image_copy_region.extent.height = *height;
                image_copy_region.extent.depth = 1;

                // Issue the copy command
                unsafe {
                    self.ash_vk.vk_device.cmd_copy_image(
                        *command_buffer,
                        *swap_img,
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        self.get_presented_img_data_helper_image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[image_copy_region],
                    );
                }
            }

            if !self.device.image_barrier(
                self.get_presented_img_data_helper_image,
                0,
                1,
                0,
                1,
                vk::Format::R8G8B8A8_UNORM,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::GENERAL,
                self.cur_image_index,
            ) {
                return Err(anyhow!("Image barrier failed for the helper image",));
            }
            if !self.device.image_barrier(
                *swap_img,
                0,
                1,
                0,
                1,
                self.vk_surf_format.format,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
                self.cur_image_index,
            ) {
                return Err(anyhow!("Image barrier failed for the swap chain image"));
            }

            if let Err(_) = unsafe { self.ash_vk.vk_device.end_command_buffer(*command_buffer) } {
                return Err(anyhow!("Could not end command buffer."));
            }
            self.device.used_memory_command_buffer[self.cur_image_index as usize] = false;

            let mut submit_info = vk::SubmitInfo::default();

            submit_info.command_buffer_count = 1;
            submit_info.p_command_buffers = command_buffer;

            if let Err(_) = unsafe {
                self.ash_vk
                    .vk_device
                    .reset_fences(&[self.get_presented_img_data_helper_fence])
            } {
                return Err(anyhow!("Could not reset fences."));
            }
            if let Err(_) = unsafe {
                self.ash_vk.vk_device.queue_submit(
                    self.vk_graphics_queue,
                    &[submit_info],
                    self.get_presented_img_data_helper_fence,
                )
            } {
                return Err(anyhow!("Queue submit failed."));
            }
            if let Err(_) = unsafe {
                self.ash_vk.vk_device.wait_for_fences(
                    &[self.get_presented_img_data_helper_fence],
                    true,
                    u64::MAX,
                )
            } {
                return Err(anyhow!("Could not wait for fences."));
            }

            let mut mem_range = vk::MappedMemoryRange::default();
            mem_range.memory = self.get_presented_img_data_helper_mem.mem;
            mem_range.offset = self.get_presented_img_data_helper_mapped_layout_offset;
            mem_range.size = vk::WHOLE_SIZE;
            if let Err(_) = unsafe {
                self.ash_vk
                    .vk_device
                    .invalidate_mapped_memory_ranges(&[mem_range])
            } {
                return Err(anyhow!("Could not invalidate mapped memory ranges."));
            }

            let real_full_image_size: usize = image_total_size.max(
                *height as usize * self.get_presented_img_data_helper_mapped_layout_pitch as usize,
            );
            if dest_data_buff.len() < real_full_image_size + (*width * 4) as usize {
                dest_data_buff.resize(
                    real_full_image_size + (*width * 4) as usize,
                    Default::default(),
                ); // extra space for flipping
            }
            dest_data_buff
                .as_mut_slice()
                .copy_from_slice(res_image_data.split_at_mut(real_full_image_size).0);

            // pack image data together without any offset that the driver might
            // require
            if *width as u64 * 4 < self.get_presented_img_data_helper_mapped_layout_pitch {
                for y in 0..*height as usize {
                    let offset_image_packed: usize = y * *width as usize * 4;
                    let offset_image_unpacked: usize =
                        y * self.get_presented_img_data_helper_mapped_layout_pitch as usize;

                    let packed_part = dest_data_buff
                        .as_mut_slice()
                        .split_at_mut(offset_image_packed)
                        .1;

                    let (packed_part, unpacked_part) =
                        packed_part.split_at_mut(offset_image_unpacked - offset_image_packed);
                    packed_part.copy_from_slice(unpacked_part.split_at(*width as usize * 4).0);
                }
            }

            if is_b8_g8_r8_a8 || reset_alpha {
                // swizzle
                for y in 0..*height as usize {
                    for x in 0..*width as usize {
                        let img_off: usize = (y * *width as usize * 4) + (x * 4);
                        if is_b8_g8_r8_a8 {
                            let tmp = dest_data_buff[img_off];
                            dest_data_buff[img_off] = dest_data_buff[img_off + 2];
                            dest_data_buff[img_off + 2] = tmp;
                        }
                        dest_data_buff[img_off + 3] = 255;
                    }
                }
            }

            if flip_img_data {
                let (data_dest_real, temp_dest_copy_row) = dest_data_buff
                    .as_mut_slice()
                    .split_at_mut(*width as usize * *height as usize * 4);
                for y in 0..*height as usize / 2 {
                    temp_dest_copy_row.copy_from_slice(
                        data_dest_real
                            .split_at(y * *width as usize * 4)
                            .1
                            .split_at(*width as usize * 4)
                            .0,
                    );
                    let write_dest = data_dest_real.split_at_mut(y * *width as usize * 4).1;
                    let (write_dest, read_dest) = write_dest.split_at_mut(
                        (((*height as usize - y) - 1) * *width as usize * 4)
                            - (y * *width as usize * 4),
                    );
                    write_dest.copy_from_slice(read_dest.split_at(*width as usize * 4).0);
                    data_dest_real
                        .split_at_mut(((*height as usize - y) - 1) * *width as usize * 4)
                        .1
                        .copy_from_slice(temp_dest_copy_row.split_at(*width as usize * 4).0);
                }
            }

            Ok(format)
        } else {
            if !uses_rgba_like_format {
                // TODO: dbg_msg("vulkan", "swap chain image was not in a RGBA like format.");
            } else {
                // TODO: dbg_msg("vulkan", "swap chain image was not ready to be copied.");
            }
            Err(anyhow!(
                "Swap chain image was not ready to be copied. See logs for more detail.",
            ))
        }
    }

    /************************
     * SAMPLERS
     ************************/

    #[must_use]
    fn create_texture_samplers(&mut self) -> bool {
        let mut ret: bool = true;
        ret &= Device::create_texture_samplers_impl(
            &self.ash_vk.vk_device,
            self.device.limits.max_sampler_anisotropy,
            self.device.global_texture_lod_bias,
            &mut self.device.samplers[ESupportedSamplerTypes::Repeat as usize],
            vk::SamplerAddressMode::REPEAT,
            vk::SamplerAddressMode::REPEAT,
            vk::SamplerAddressMode::REPEAT,
        );
        ret &= Device::create_texture_samplers_impl(
            &self.ash_vk.vk_device,
            self.device.limits.max_sampler_anisotropy,
            self.device.global_texture_lod_bias,
            &mut self.device.samplers[ESupportedSamplerTypes::ClampToEdge as usize],
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
        );
        ret &= Device::create_texture_samplers_impl(
            &self.ash_vk.vk_device,
            self.device.limits.max_sampler_anisotropy,
            self.device.global_texture_lod_bias,
            &mut self.device.samplers[ESupportedSamplerTypes::Texture2DArray as usize],
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerAddressMode::MIRRORED_REPEAT,
        );
        return ret;
    }

    fn destroy_texture_samplers(&mut self) {
        unsafe {
            self.ash_vk.vk_device.destroy_sampler(
                self.device.samplers[ESupportedSamplerTypes::Repeat as usize],
                None,
            );
        }
        unsafe {
            self.ash_vk.vk_device.destroy_sampler(
                self.device.samplers[ESupportedSamplerTypes::ClampToEdge as usize],
                None,
            );
        }
        unsafe {
            self.ash_vk.vk_device.destroy_sampler(
                self.device.samplers[ESupportedSamplerTypes::Texture2DArray as usize],
                None,
            );
        }
    }

    #[must_use]
    fn create_descriptor_pools(&mut self, thread_count: usize) -> bool {
        self.device.standard_texture_descr_pool.is_uniform_pool = false;
        self.device.standard_texture_descr_pool.default_alloc_size = 1024;
        self.device.text_texture_descr_pool.is_uniform_pool = false;
        self.device.text_texture_descr_pool.default_alloc_size = 8;

        self.device
            .uniform_buffer_descr_pools
            .resize(thread_count, Default::default());
        for uniform_buffer_descr_pool in &mut self.device.uniform_buffer_descr_pools {
            uniform_buffer_descr_pool.is_uniform_pool = true;
            uniform_buffer_descr_pool.default_alloc_size = 512;
        }

        let mut ret = Device::allocate_descriptor_pool(
            &self.error,
            &self.ash_vk.vk_device,
            &mut self.device.standard_texture_descr_pool,
            StreamDataMax::MaxTextures as usize,
        );
        ret |= Device::allocate_descriptor_pool(
            &self.error,
            &self.ash_vk.vk_device,
            &mut self.device.text_texture_descr_pool,
            8,
        );

        for uniform_buffer_descr_pool in &mut self.device.uniform_buffer_descr_pools {
            ret |= Device::allocate_descriptor_pool(
                &self.error,
                &self.ash_vk.vk_device,
                uniform_buffer_descr_pool,
                64,
            );
        }

        return ret;
    }

    fn destroy_descriptor_pools(&mut self) {
        for descr_pool in &mut self.device.standard_texture_descr_pool.pools {
            unsafe {
                self.ash_vk
                    .vk_device
                    .destroy_descriptor_pool(descr_pool.pool, None);
            }
        }
        self.device
            .text_texture_descr_pool
            .pools
            .iter_mut()
            .for_each(|descr_pool| unsafe {
                self.ash_vk
                    .vk_device
                    .destroy_descriptor_pool(descr_pool.pool, None);
            });

        for uniform_buffer_descr_pool in &mut self.device.uniform_buffer_descr_pools {
            for descr_pool in &mut uniform_buffer_descr_pool.pools {
                unsafe {
                    self.ash_vk
                        .vk_device
                        .destroy_descriptor_pool(descr_pool.pool, None);
                }
            }
        }
        self.device.uniform_buffer_descr_pools.clear();
    }

    #[must_use]
    fn get_uniform_buffer_object(
        &mut self,
        render_thread_index: usize,
        requires_shared_stages_descriptor: bool,
        descr_set: &mut SDeviceDescriptorSet,
        _particle_count: usize,
        ptr_raw_data: *const c_void,
        data_size: usize,
        cur_image_index: u32,
    ) -> bool {
        if !self
            .device
            .get_uniform_buffer_object_impl::<SRenderSpriteInfo, 512, 128>(
                render_thread_index,
                requires_shared_stages_descriptor,
                descr_set,
                ptr_raw_data,
                data_size,
                cur_image_index,
            )
        {
            self.skip_frames_until_current_frame_is_used_again();
            // try again after memory was free'd
            self.device
                .get_uniform_buffer_object_impl::<SRenderSpriteInfo, 512, 128>(
                    render_thread_index,
                    requires_shared_stages_descriptor,
                    descr_set,
                    ptr_raw_data,
                    data_size,
                    cur_image_index,
                )
        } else {
            true
        }
    }

    /************************
     * SWAPPING MECHANISM
     ************************/

    fn start_render_thread(&mut self, thread_index: usize) {
        let list = &mut self.thread_command_lists[thread_index];
        if !list.is_empty() {
            self.thread_helper_had_commands[thread_index] = true;
            let thread = &mut self.render_threads[thread_index];
            let mut guard = thread.inner.lock().unwrap();
            guard.is_rendering = true;
            thread.cond.notify_one();
        }
    }

    fn finish_render_threads(&mut self) {
        if self.thread_count > 1 {
            // execute threads

            for thread_index in 0..self.thread_count - 1 {
                if !self.thread_helper_had_commands[thread_index] {
                    self.start_render_thread(thread_index);
                }
            }

            for thread_index in 0..self.thread_count - 1 {
                if self.thread_helper_had_commands[thread_index] {
                    let render_thread = &mut self.render_threads[thread_index];
                    self.thread_helper_had_commands[thread_index] = false;
                    let mut _guard = render_thread.inner.lock().unwrap();
                    _guard = render_thread
                        .cond
                        .wait_while(_guard, |p| {
                            return p.is_rendering;
                        })
                        .unwrap();
                    self.last_pipeline_per_thread[thread_index + 1] = vk::Pipeline::null();
                }
            }
        }
    }

    fn execute_memory_command_buffer(&mut self) {
        if self.device.used_memory_command_buffer[self.cur_image_index as usize] {
            let memory_command_buffer =
                &mut self.device.memory_command_buffers[self.cur_image_index as usize];
            unsafe {
                self.ash_vk
                    .vk_device
                    .end_command_buffer(*memory_command_buffer)
                    .unwrap();
            }

            let mut submit_info = vk::SubmitInfo::default();

            submit_info.command_buffer_count = 1;
            submit_info.p_command_buffers = memory_command_buffer;
            unsafe {
                self.ash_vk
                    .vk_device
                    .queue_submit(self.vk_graphics_queue, &[submit_info], vk::Fence::null())
                    .unwrap();
            }
            unsafe {
                self.ash_vk
                    .vk_device
                    .queue_wait_idle(self.vk_graphics_queue)
                    .unwrap();
            }

            self.device.used_memory_command_buffer[self.cur_image_index as usize] = false;
        }
    }

    fn upload_staging_buffers(&mut self) {
        if !self.device.non_flushed_staging_buffer_ranges.is_empty() {
            unsafe {
                self.ash_vk
                    .vk_device
                    .flush_mapped_memory_ranges(
                        self.device.non_flushed_staging_buffer_ranges.as_slice(),
                    )
                    .unwrap();
            }

            self.device.non_flushed_staging_buffer_ranges.clear();
        }
    }

    fn upload_non_flushed_buffers<const FLUSH_FOR_RENDERING: bool>(
        &mut self,
        cur_image_index: u32,
    ) {
        // streamed vertices
        Device::upload_streamed_buffer::<{ FLUSH_FOR_RENDERING }, _>(
            &self.ash_vk.vk_device,
            self.device.limits.non_coherent_mem_alignment,
            &mut self.device.streamed_vertex_buffer,
            cur_image_index,
        );

        // now the buffer objects
        for stream_uniform_buffer in &mut self.device.streamed_uniform_buffers {
            Device::upload_streamed_buffer::<{ FLUSH_FOR_RENDERING }, _>(
                &self.ash_vk.vk_device,
                self.device.limits.non_coherent_mem_alignment,
                stream_uniform_buffer,
                cur_image_index,
            );
        }

        self.upload_staging_buffers();
    }

    fn clear_frame_data(&mut self, frame_image_index: usize) {
        self.upload_staging_buffers();

        // clear pending buffers, that require deletion
        for buffer_pair in &mut self.device.frame_delayed_buffer_cleanups[frame_image_index] {
            if !buffer_pair.mapped_data.is_null() {
                unsafe {
                    self.ash_vk.vk_device.unmap_memory(buffer_pair.mem.mem);
                }
            }
            self.device.mem.clean_buffer_pair(
                frame_image_index,
                &mut buffer_pair.buffer,
                &mut buffer_pair.mem,
            );
        }
        self.device.frame_delayed_buffer_cleanups[frame_image_index].clear();

        // clear pending textures, that require deletion
        for texture in &mut self.device.frame_delayed_texture_cleanups[frame_image_index] {
            Device::destroy_texture(
                &mut self.device.frame_delayed_buffer_cleanups,
                &mut self.device.image_buffer_caches,
                &self.ash_vk.vk_device,
                texture,
                frame_image_index as u32,
            ); // TODO FrameImageIndex is a behaviour change, self.m_CurImageIndex was used before implictly
        }
        self.device.frame_delayed_texture_cleanups[frame_image_index].clear();

        self.device.staging_buffer_cache.cleanup(frame_image_index);
        self.device
            .staging_buffer_cache_image
            .cleanup(frame_image_index);
        self.device.vertex_buffer_cache.cleanup(frame_image_index);
        for image_buffer_cache in &mut self.device.image_buffer_caches {
            image_buffer_cache.1.cleanup(frame_image_index);
        }
        self.device
            .mem_allocator
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .free_mems_of_frame(frame_image_index as u32);
    }

    fn clear_frame_memory_usage(&mut self) {
        self.clear_frame_data(self.cur_image_index as usize);
        self.device.shrink_unused_caches();
    }

    fn start_render_pass(&mut self, render_pass_type: RenderPassType) -> bool {
        let command_buffer = &mut self.main_draw_command_buffers[self.cur_image_index as usize];
        let mut begin_info = vk::CommandBufferBeginInfo::default();
        begin_info.flags = vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT;

        unsafe {
            if self
                .ash_vk
                .vk_device
                .begin_command_buffer(*command_buffer, &begin_info)
                .is_err()
            {
                self.error.lock().unwrap().set_error(
                    EGFXErrorType::RenderRecording,
                    "Command buffer cannot be filled anymore.",
                );
                return false;
            }
        }

        let mut render_pass_info = vk::RenderPassBeginInfo::default();
        render_pass_info.render_pass = match render_pass_type {
            RenderPassType::Single => self.vk_render_pass_single_pass,
            RenderPassType::Dual => self.vk_render_pass_double_pass,
        };
        render_pass_info.framebuffer = match render_pass_type {
            RenderPassType::Single => self.framebuffer_list[self.cur_image_index as usize],
            RenderPassType::Dual => {
                self.framebuffer_double_pass_list[self.cur_image_index as usize]
            }
        };
        render_pass_info.render_area.offset = vk::Offset2D::default();
        render_pass_info.render_area.extent =
            self.vk_swap_img_and_viewport_extent.swap_image_viewport;

        let clear_color_val = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [
                        self.clear_color[0],
                        self.clear_color[1],
                        self.clear_color[2],
                        self.clear_color[3],
                    ],
                },
            },
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [
                        self.clear_color[0],
                        self.clear_color[1],
                        self.clear_color[2],
                        self.clear_color[3],
                    ],
                },
            },
            vk::ClearValue {
                color: vk::ClearColorValue {
                    int32: [0, 0, 0, 0],
                },
            },
        ];
        render_pass_info.clear_value_count = match render_pass_type {
            RenderPassType::Single => 1,
            RenderPassType::Dual => 3,
        };
        render_pass_info.p_clear_values = clear_color_val.as_ptr();

        unsafe {
            self.ash_vk.vk_device.cmd_begin_render_pass(
                *command_buffer,
                &render_pass_info,
                if self.thread_count > 1 {
                    vk::SubpassContents::SECONDARY_COMMAND_BUFFERS
                } else {
                    vk::SubpassContents::INLINE
                },
            );
        }

        for last_pipe in &mut self.last_pipeline_per_thread {
            *last_pipe = vk::Pipeline::null();
        }

        self.current_render_pass_type = render_pass_type;
        self.current_sub_pass_index = 0;

        true
    }

    #[must_use]
    fn finish_and_exec_render_threads(&mut self) -> bool {
        self.finish_render_threads();
        self.last_commands_in_pipe_thread_index = 0;

        self.upload_non_flushed_buffers::<true>(self.cur_image_index);

        let command_buffer = &mut self.main_draw_command_buffers[self.cur_image_index as usize];

        // render threads
        if self.thread_count > 1 {
            let mut threaded_commands_used_count: usize = 0;
            let render_thread_count: usize = self.thread_count - 1;
            for i in 0..render_thread_count {
                if self.used_thread_draw_command_buffer[i + 1][self.cur_image_index as usize] {
                    let graphic_thread_command_buffer =
                        &self.thread_draw_command_buffers[i + 1][self.cur_image_index as usize];
                    self.helper_thread_draw_command_buffers[threaded_commands_used_count] =
                        *graphic_thread_command_buffer;
                    threaded_commands_used_count += 1;

                    self.used_thread_draw_command_buffer[i + 1][self.cur_image_index as usize] =
                        false;
                }
            }
            if threaded_commands_used_count > 0 {
                unsafe {
                    self.ash_vk.vk_device.cmd_execute_commands(
                        *command_buffer,
                        self.helper_thread_draw_command_buffers
                            .split_at(threaded_commands_used_count)
                            .0,
                    );
                }
            }

            // special case if swap chain was not completed in one runbuffer call
            if self.used_thread_draw_command_buffer[0][self.cur_image_index as usize] {
                let graphic_thread_command_buffer =
                    &mut self.thread_draw_command_buffers[0][self.cur_image_index as usize];
                if let Err(_) = unsafe {
                    self.ash_vk
                        .vk_device
                        .end_command_buffer(*graphic_thread_command_buffer)
                } {
                    return false;
                }

                unsafe {
                    self.ash_vk
                        .vk_device
                        .cmd_execute_commands(*command_buffer, &[*graphic_thread_command_buffer]);
                }

                self.used_thread_draw_command_buffer[0][self.cur_image_index as usize] = false;
            }
        }
        true
    }

    #[must_use]
    fn end_render_pass(&mut self) -> bool {
        if !self.finish_and_exec_render_threads() {
            return false;
        }

        let command_buffer = &mut self.main_draw_command_buffers[self.cur_image_index as usize];
        unsafe { self.ash_vk.vk_device.cmd_end_render_pass(*command_buffer) };

        let res = unsafe { self.ash_vk.vk_device.end_command_buffer(*command_buffer) };
        if res.is_err() {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::RenderRecording,
                "Command buffer cannot be ended anymore.",
            );
            false
        } else {
            true
        }
    }

    #[must_use]
    fn cmd_end_single_render_pass_and_start_double_render_pass(&mut self) -> bool {
        if !self.end_render_pass() {
            return false;
        }
        if let Err(_) = unsafe { self.ash_vk.vk_device.device_wait_idle() } {
            return false;
        }
        if !self.start_render_pass(RenderPassType::Dual) {
            return false;
        }
        true
    }

    #[must_use]
    fn wait_frame(&mut self) -> bool {
        if !self.end_render_pass() {
            return false;
        }
        let command_buffer = &mut self.main_draw_command_buffers[self.cur_image_index as usize];

        let wait_semaphore = self.wait_semaphores[self.cur_frames as usize];

        let mut submit_info = vk::SubmitInfo::default();

        submit_info.command_buffer_count = 1;
        submit_info.p_command_buffers = command_buffer;

        let mut command_buffers: [vk::CommandBuffer; 2] = Default::default();

        if self.device.used_memory_command_buffer[self.cur_image_index as usize] {
            let memory_command_buffer =
                &mut self.device.memory_command_buffers[self.cur_image_index as usize];
            if let Err(_) = unsafe {
                self.ash_vk
                    .vk_device
                    .end_command_buffer(*memory_command_buffer)
            } {
                return false;
            }

            command_buffers[0] = *memory_command_buffer;
            command_buffers[1] = *command_buffer;
            submit_info.command_buffer_count = 2;
            submit_info.p_command_buffers = command_buffers.as_ptr();

            self.device.used_memory_command_buffer[self.cur_image_index as usize] = false;
        }

        let wait_semaphores: [vk::Semaphore; 1] = [wait_semaphore];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        submit_info.wait_semaphore_count = wait_semaphores.len() as u32;
        submit_info.p_wait_semaphores = wait_semaphores.as_ptr();
        submit_info.p_wait_dst_stage_mask = wait_stages.as_ptr();

        let signal_semaphores = [self.sig_semaphores[self.cur_frames as usize]];
        submit_info.signal_semaphore_count = signal_semaphores.len() as u32;
        submit_info.p_signal_semaphores = signal_semaphores.as_ptr();

        if let Err(_) = unsafe {
            self.ash_vk
                .vk_device
                .reset_fences(&[self.frame_fences[self.cur_frames as usize]])
        } {
            return false;
        }

        let queue_submit_res = unsafe {
            self.ash_vk.vk_device.queue_submit(
                self.vk_graphics_queue,
                &[submit_info],
                self.frame_fences[self.cur_frames as usize],
            )
        };
        if let Err(err) = queue_submit_res {
            let crit_error_msg = self.check_res.check_vulkan_critical_error(
                err,
                &self.error,
                &mut self.recreate_swap_chain,
            );
            if let Some(crit_err) = crit_error_msg {
                self.error.lock().unwrap().set_error_extra(
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

        let mut present_info = vk::PresentInfoKHR::default();

        present_info.wait_semaphore_count = signal_semaphores.len() as u32;
        present_info.p_wait_semaphores = signal_semaphores.as_ptr();

        let swap_chains = [self.vk_swap_chain_khr];
        present_info.swapchain_count = swap_chains.len() as u32;
        present_info.p_swapchains = swap_chains.as_ptr();

        present_info.p_image_indices = &mut self.cur_image_index;

        self.last_presented_swap_chain_image_index = self.cur_image_index;

        let queue_present_res = unsafe {
            self.ash_vk
                .vk_swap_chain_ash
                .queue_present(self.vk_present_queue, &present_info)
        };
        if queue_present_res.is_err()
            && queue_present_res.unwrap_err() != vk::Result::SUBOPTIMAL_KHR
        {
            let crit_error_msg = self.check_res.check_vulkan_critical_error(
                queue_present_res.unwrap_err(),
                &self.error,
                &mut self.recreate_swap_chain,
            );
            if let Some(crit_err) = crit_error_msg {
                self.error.lock().unwrap().set_error_extra(
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
    fn prepare_frame(&mut self) -> bool {
        if self.recreate_swap_chain {
            self.recreate_swap_chain = false;
            if is_verbose(&*self.dbg) {
                self.logger
                    .log(LogLevel::Debug)
                    .msg("recreating swap chain requested by user (prepare frame).");
            }
            self.recreate_swap_chain();
        }

        let acq_result = unsafe {
            self.ash_vk.vk_swap_chain_ash.acquire_next_image(
                self.vk_swap_chain_khr,
                u64::MAX,
                self.sig_semaphores[self.cur_frames as usize],
                vk::Fence::null(),
            )
        };
        if acq_result.is_err() || acq_result.unwrap().1 {
            if (acq_result.is_err() && acq_result.unwrap_err() == vk::Result::ERROR_OUT_OF_DATE_KHR)
                || self.recreate_swap_chain
            {
                self.recreate_swap_chain = false;
                if is_verbose(&*self.dbg) {
                    self.logger.log(LogLevel::Debug).msg(
                        "recreating swap chain requested by acquire next image (prepare frame).",
                    );
                }
                self.recreate_swap_chain();
                return self.prepare_frame();
            } else {
                if acq_result.is_ok() && acq_result.as_ref().unwrap().1 {
                    self.logger
                        .log(LogLevel::Debug)
                        .msg("acquire next image failed ");
                }
                let res = if acq_result.is_err() {
                    acq_result.unwrap_err()
                } else {
                    vk::Result::SUBOPTIMAL_KHR
                };

                let crit_error_msg = self.check_res.check_vulkan_critical_error(
                    res,
                    &self.error,
                    &mut self.recreate_swap_chain,
                );
                if let Some(crit_err) = crit_error_msg {
                    self.error.lock().unwrap().set_error_extra(
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
        self.cur_image_index = acq_result.unwrap().0;
        std::mem::swap(
            &mut self.wait_semaphores[self.cur_frames as usize],
            &mut self.sig_semaphores[self.cur_frames as usize],
        );

        if self.image_fences[self.cur_image_index as usize] != vk::Fence::null() {
            if let Err(_) = unsafe {
                self.ash_vk.vk_device.wait_for_fences(
                    &[self.image_fences[self.cur_image_index as usize]],
                    true,
                    u64::MAX,
                )
            } {
                return false;
            }
        }
        self.image_fences[self.cur_image_index as usize] =
            self.frame_fences[self.cur_frames as usize];

        // next frame
        self.cur_frame += 1;
        self.image_last_frame_check[self.cur_image_index as usize] = self.cur_frame;

        // check if older frames weren't used in a long time
        for frame_image_index in 0..self.image_last_frame_check.len() {
            let last_frame = self.image_last_frame_check[frame_image_index];
            if self.cur_frame - last_frame > self.device.swap_chain_image_count as u64 {
                if self.image_fences[frame_image_index] != vk::Fence::null() {
                    if let Err(_) = unsafe {
                        self.ash_vk.vk_device.wait_for_fences(
                            &[self.image_fences[frame_image_index]],
                            true,
                            u64::MAX,
                        )
                    } {
                        return false;
                    }
                    self.clear_frame_data(frame_image_index);
                    self.image_fences[frame_image_index] = vk::Fence::null();
                }
                self.image_last_frame_check[frame_image_index] = self.cur_frame;
            }
        }

        // clear frame's memory data
        self.clear_frame_memory_usage();

        // clear frame
        if let Err(_) = unsafe {
            self.ash_vk.vk_device.reset_command_buffer(
                *&mut self.main_draw_command_buffers[self.cur_image_index as usize],
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )
        } {
            return false;
        }

        if !self.start_render_pass(RenderPassType::Single) {
            return false;
        }

        return true;
    }

    #[must_use]
    fn pure_memory_frame(&mut self) -> bool {
        self.execute_memory_command_buffer();

        // reset streamed data
        self.upload_non_flushed_buffers::<false>(self.cur_image_index);

        self.clear_frame_memory_usage();

        return true;
    }

    #[must_use]
    pub fn next_frame(&mut self) -> bool {
        if !self.rendering_paused {
            if !self.wait_frame() {
                return false;
            }
            if !self.prepare_frame() {
                return false;
            }
        }
        // else only execute the memory command buffer
        else {
            if !self.pure_memory_frame() {
                return false;
            }
        }

        return true;
    }

    /************************
     * TEXTURES
     ************************/
    #[must_use]
    fn update_texture(
        &mut self,
        texture_slot: u128,
        format: vk::Format,
        data: &mut Vec<u8>,
        mut x_off: i64,
        mut y_off: i64,
        mut width: usize,
        mut height: usize,
        color_channel_count: usize,
    ) -> bool {
        let image_size: usize = width * height * color_channel_count;
        let staging_allocation = Device::get_staging_buffer_image(
            &mut self.device.mem,
            &mut self.device.staging_buffer_cache_image,
            &self.device.limits,
            data,
            image_size as u64,
            self.cur_image_index,
        );
        if let Err(_) = staging_allocation {
            return false;
        }
        let mut staging_buffer = staging_allocation.unwrap();

        let tex = self.device.textures.get(&texture_slot).unwrap();

        if tex.rescale_count > 0 {
            for _i in 0..tex.rescale_count {
                width >>= 1;
                height >>= 1;

                x_off /= 2;
                y_off /= 2;
            }

            let mut tmp_data = resize(
                &self.runtime_threadpool,
                data,
                width,
                height,
                width,
                height,
                vulkan_format_to_image_color_channel_count(format),
            );
            std::mem::swap(data, &mut tmp_data);
        }

        let tex_img = tex.img;
        if !self.device.image_barrier(
            tex_img,
            0,
            tex.mip_map_count as usize,
            0,
            1,
            format,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            self.cur_image_index,
        ) {
            return false;
        }
        if !self.device.copy_buffer_to_image(
            staging_buffer.buffer,
            staging_buffer.heap_data.offset_to_align as u64,
            tex_img,
            x_off as i32,
            y_off as i32,
            width as u32,
            height as u32,
            1,
            self.cur_image_index,
        ) {
            return false;
        }

        let tex = self.device.textures.get(&texture_slot).unwrap();
        if tex.mip_map_count > 1 {
            if !self.device.build_mipmaps(
                tex.img,
                format,
                width,
                height,
                1,
                tex.mip_map_count as usize,
                self.cur_image_index,
            ) {
                return false;
            }
        } else {
            if !self.device.image_barrier(
                tex.img,
                0,
                1,
                0,
                1,
                format,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                self.cur_image_index,
            ) {
                return false;
            }
        }

        self.device
            .upload_and_free_staging_image_mem_block(&mut staging_buffer, self.cur_image_index);

        return true;
    }

    #[must_use]
    fn create_texture_cmd(
        &mut self,
        slot: u128,
        mut width: usize,
        mut height: usize,
        depth: usize,
        is_3d_tex: bool,
        pixel_size: usize,
        tex_format: vk::Format,
        _store_format: vk::Format,
        tex_flags: TexFlags,
        upload_data: VulkanDeviceInternalMemory,
    ) -> bool {
        let image_index = slot;
        let image_color_channels = vulkan_format_to_image_color_channel_count(tex_format);

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
            // TODO split resize for 3d textures
            let tmp_data = resize(
                &self.runtime_threadpool,
                upload_data.mem,
                width,
                height,
                width,
                height,
                image_color_channels,
            );
            // should be safe since we only downscale
            upload_data.mem.copy_from_slice(tmp_data.as_slice());
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

        let mut texture = CTexture::default();

        texture.width = width;
        texture.height = height;
        texture.depth = depth;
        texture.rescale_count = rescale_count;
        texture.mip_map_count = mip_map_level_count as u32;

        if !is_3d_tex {
            if !self.device.create_texture_image(
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
            let img_view = self.device.create_texture_image_view(
                texture.img,
                img_format,
                vk::ImageViewType::TYPE_2D,
                1,
                mip_map_level_count,
            );
            texture.img_view = img_view.unwrap(); // TODO: err handling
            let mut img_sampler = self
                .device
                .get_texture_sampler(ESupportedSamplerTypes::Repeat);
            texture.samplers[0] = img_sampler;
            img_sampler = self
                .device
                .get_texture_sampler(ESupportedSamplerTypes::ClampToEdge);
            texture.samplers[1] = img_sampler;

            if !self
                .device
                .create_new_textured_standard_descriptor_sets(0, &mut texture)
            {
                return false;
            }
            if !self
                .device
                .create_new_textured_standard_descriptor_sets(1, &mut texture)
            {
                return false;
            }
        } else {
            let image_3d_width = width as usize;
            let image_3d_height = height as usize;

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

            if !self.device.create_texture_image(
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
            let img_format = tex_format;
            let img_view = self.device.create_texture_image_view(
                texture.img_3d,
                img_format,
                vk::ImageViewType::TYPE_2D_ARRAY,
                depth,
                mip_map_level_count,
            );
            texture.img_3d_view = img_view.unwrap(); // TODO: err handling;
            let img_sampler = self
                .device
                .get_texture_sampler(ESupportedSamplerTypes::Texture2DArray);
            texture.sampler_3d = img_sampler;

            if !self
                .device
                .create_new_3d_textured_standard_descriptor_sets(image_index, &mut texture)
            {
                return false;
            }
        }

        self.device.textures.insert(image_index, texture); // TODO better fix
        return true;
    }

    /************************
     * RENDER STATES
     ************************/

    fn get_state_matrix(state: &State, matrix: &mut [f32; 4 * 2]) {
        *matrix = [
            // column 1
            2.0 / (state.canvas_br.x - state.canvas_tl.x),
            0.0,
            // column 2
            0.0,
            2.0 / (state.canvas_br.y - state.canvas_tl.y),
            // column 3
            0.0,
            0.0,
            // column 4
            -((state.canvas_tl.x + state.canvas_br.x) / (state.canvas_br.x - state.canvas_tl.x)),
            -((state.canvas_tl.y + state.canvas_br.y) / (state.canvas_br.y - state.canvas_tl.y)),
        ];
    }

    #[must_use]
    fn get_is_textured(state: &State) -> bool {
        if let Some(_) = state.texture_index {
            return true;
        }
        return false;
    }

    fn get_address_mode_index(state: &State) -> usize {
        return if state.wrap_mode == WrapType::Repeat {
            EVulkanBackendAddressModes::Repeat as usize
        } else {
            EVulkanBackendAddressModes::ClampEdges as usize
        };
    }

    fn get_blend_mode_index(state: &State) -> usize {
        return if state.blend_mode == BlendType::Additive {
            EVulkanBackendBlendModes::Additative as usize
        } else {
            if state.blend_mode == BlendType::None {
                EVulkanBackendBlendModes::None as usize
            } else {
                EVulkanBackendBlendModes::Alpha as usize
            }
        };
    }

    fn get_dynamic_mode_index_from_state(&self, state: &State) -> usize {
        return if state.clip_enable
            || self.has_dynamic_viewport
            || self.vk_swap_img_and_viewport_extent.has_forced_viewport
        {
            EVulkanBackendClipModes::DynamicScissorAndViewport as usize
        } else {
            EVulkanBackendClipModes::None as usize
        };
    }

    fn get_dynamic_mode_index_from_exec_buffer(exec_buffer: &SRenderCommandExecuteBuffer) -> usize {
        return if exec_buffer.has_dynamic_state {
            EVulkanBackendClipModes::DynamicScissorAndViewport as usize
        } else {
            EVulkanBackendClipModes::None as usize
        };
    }

    fn get_pipeline<'a>(
        container: &'a mut SPipelineContainer,
        is_textured: bool,
        blend_mode_index: usize,
        dynamic_index: usize,
        render_pass_type_index: usize,
        sub_pass_index: usize,
    ) -> &'a mut vk::Pipeline {
        return &mut container.pipelines[blend_mode_index][dynamic_index][is_textured as usize]
            [render_pass_type_index][sub_pass_index];
    }

    fn get_pipe_layout<'a>(
        container: &'a mut SPipelineContainer,
        is_textured: bool,
        blend_mode_index: usize,
        dynamic_index: usize,
        render_pass_type_index: usize,
        sub_pass_index: usize,
    ) -> &'a mut vk::PipelineLayout {
        return &mut container.pipeline_layouts[blend_mode_index][dynamic_index]
            [is_textured as usize][render_pass_type_index][sub_pass_index];
    }

    fn get_pipeline_and_layout<'a>(
        container: &'a SPipelineContainer,
        is_textured: bool,
        blend_mode_index: usize,
        dynamic_index: usize,
        render_pass_type_index: usize,
        sub_pass_index: usize,
    ) -> (&'a vk::Pipeline, &'a vk::PipelineLayout) {
        return (
            &container.pipelines[blend_mode_index][dynamic_index][is_textured as usize]
                [render_pass_type_index][sub_pass_index],
            &container.pipeline_layouts[blend_mode_index][dynamic_index][is_textured as usize]
                [render_pass_type_index][sub_pass_index],
        );
    }

    fn get_pipeline_and_layout_mut<'a>(
        container: &'a mut SPipelineContainer,
        is_textured: bool,
        blend_mode_index: usize,
        dynamic_index: usize,
        render_pass_type: RenderPassType,
        sub_pass_index: usize,
    ) -> (&'a mut vk::Pipeline, &'a mut vk::PipelineLayout) {
        return (
            &mut container.pipelines[blend_mode_index][dynamic_index][is_textured as usize]
                [render_pass_type as usize][sub_pass_index],
            &mut container.pipeline_layouts[blend_mode_index][dynamic_index][is_textured as usize]
                [render_pass_type as usize][sub_pass_index],
        );
    }

    fn get_standard_pipe_and_layout<'a>(
        standard_line_pipeline: &'a SPipelineContainer,
        standard_pipeline: &'a SPipelineContainer,
        standard_stencil_only_pipeline: &'a SPipelineContainer,
        standard_stencil_pipeline: &'a SPipelineContainer,
        is_line_geometry: bool,
        is_textured: bool,
        blend_mode_index: usize,
        dynamic_index: usize,
        render_pass_type_index: usize,
        stencil_type: StencilOpType,
        sub_pass_index: usize,
    ) -> (&'a vk::Pipeline, &'a vk::PipelineLayout) {
        if is_line_geometry {
            Self::get_pipeline_and_layout(
                standard_line_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
                render_pass_type_index,
                sub_pass_index,
            )
        } else {
            match stencil_type {
                StencilOpType::AlwaysPass => Self::get_pipeline_and_layout(
                    standard_stencil_only_pipeline,
                    is_textured,
                    blend_mode_index,
                    dynamic_index,
                    render_pass_type_index,
                    sub_pass_index,
                ),
                StencilOpType::OnlyWhenPassed | StencilOpType::OnlyWhenNotPassed => {
                    Self::get_pipeline_and_layout(
                        standard_stencil_pipeline,
                        is_textured,
                        blend_mode_index,
                        dynamic_index,
                        render_pass_type_index,
                        sub_pass_index,
                    )
                }
                StencilOpType::None => Self::get_pipeline_and_layout(
                    standard_pipeline,
                    is_textured,
                    blend_mode_index,
                    dynamic_index,
                    render_pass_type_index,
                    sub_pass_index,
                ),
            }
        }
    }

    fn get_tile_layer_pipe_layout(
        &mut self,
        layout_type: i32, // TODO: name the types
        is_textured: bool,
        blend_mode_index: usize,
        dynamic_index: usize,
        render_pass_type_index: usize,
        sub_pass_index: usize,
    ) -> &mut vk::PipelineLayout {
        if layout_type == 0 {
            return Self::get_pipe_layout(
                &mut self.tile_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
                render_pass_type_index,
                sub_pass_index,
            );
        } else if layout_type == 1 {
            return Self::get_pipe_layout(
                &mut self.tile_border_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
                render_pass_type_index,
                sub_pass_index,
            );
        } else {
            return Self::get_pipe_layout(
                &mut self.tile_border_line_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
                render_pass_type_index,
                sub_pass_index,
            );
        }
    }

    fn get_tile_layer_pipe(
        &mut self,
        pipe_type: i32, // TODO: name the types
        is_textured: bool,
        blend_mode_index: usize,
        dynamic_index: usize,
        render_pass_type_index: usize,
        sub_pass_index: usize,
    ) -> &mut vk::Pipeline {
        if pipe_type == 0 {
            return Self::get_pipeline(
                &mut self.tile_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
                render_pass_type_index,
                sub_pass_index,
            );
        } else if pipe_type == 1 {
            return Self::get_pipeline(
                &mut self.tile_border_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
                render_pass_type_index,
                sub_pass_index,
            );
        } else {
            return Self::get_pipeline(
                &mut self.tile_border_line_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
                render_pass_type_index,
                sub_pass_index,
            );
        }
    }

    fn get_state_indices(
        exec_buffer: &SRenderCommandExecuteBuffer,
        state: &State,
        is_textured: &mut bool,
        blend_mode_index: &mut usize,
        dynamic_index: &mut usize,
        address_mode_index: &mut usize,
        render_pass_type_index: &mut usize,
        sub_pass_index: &mut usize,
    ) {
        *is_textured = Self::get_is_textured(state);
        *address_mode_index = Self::get_address_mode_index(state);
        *blend_mode_index = Self::get_blend_mode_index(state);
        *dynamic_index = Self::get_dynamic_mode_index_from_exec_buffer(exec_buffer);
        *render_pass_type_index = exec_buffer.render_pass_index;
        *sub_pass_index = exec_buffer.sub_pass_index;
    }

    fn exec_buffer_fill_dynamic_states(
        &self,
        state: &State,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
    ) {
        let dynamic_state_index: usize = self.get_dynamic_mode_index_from_state(state);
        if dynamic_state_index == EVulkanBackendClipModes::DynamicScissorAndViewport as usize {
            let mut viewport = vk::Viewport::default();
            if self.has_dynamic_viewport {
                viewport.x = self.dynamic_viewport_offset.x as f32;
                viewport.y = self.dynamic_viewport_offset.y as f32;
                viewport.width = self.dynamic_viewport_size.width as f32;
                viewport.height = self.dynamic_viewport_size.height as f32;
                viewport.min_depth = 0.0;
                viewport.max_depth = 1.0;
            }
            // else check if there is a forced viewport
            else if self.vk_swap_img_and_viewport_extent.has_forced_viewport {
                viewport.x = 0.0;
                viewport.y = 0.0;
                viewport.width = self.vk_swap_img_and_viewport_extent.forced_viewport.width as f32;
                viewport.height =
                    self.vk_swap_img_and_viewport_extent.forced_viewport.height as f32;
                viewport.min_depth = 0.0;
                viewport.max_depth = 1.0;
            } else {
                viewport.x = 0.0;
                viewport.y = 0.0;
                viewport.width = self
                    .vk_swap_img_and_viewport_extent
                    .swap_image_viewport
                    .width as f32;
                viewport.height = self
                    .vk_swap_img_and_viewport_extent
                    .swap_image_viewport
                    .height as f32;
                viewport.min_depth = 0.0;
                viewport.max_depth = 1.0;
            }

            let mut scissor = vk::Rect2D::default();
            // convert from OGL to vulkan clip

            // the scissor always assumes the presented viewport, because the
            // front-end keeps the calculation for the forced viewport in sync
            let scissor_viewport = self
                .vk_swap_img_and_viewport_extent
                .get_presented_image_viewport();
            if state.clip_enable {
                let scissor_y: i32 =
                    scissor_viewport.height as i32 - (state.clip_y as i32 + state.clip_h as i32);
                let scissor_h = state.clip_h as i32;
                scissor.offset = vk::Offset2D {
                    x: state.clip_x as i32,
                    y: scissor_y,
                };
                scissor.extent = vk::Extent2D {
                    width: state.clip_w as u32,
                    height: scissor_h as u32,
                };
            } else {
                scissor.offset = vk::Offset2D::default();
                scissor.extent = vk::Extent2D {
                    width: scissor_viewport.width as u32,
                    height: scissor_viewport.height as u32,
                };
            }

            // if there is a dynamic viewport make sure the scissor data is scaled
            // down to that
            if self.has_dynamic_viewport {
                scissor.offset.x = ((scissor.offset.x as f32 / scissor_viewport.width as f32)
                    * self.dynamic_viewport_size.width as f32)
                    as i32
                    + self.dynamic_viewport_offset.x;
                scissor.offset.y = ((scissor.offset.y as f32 / scissor_viewport.height as f32)
                    * self.dynamic_viewport_size.height as f32)
                    as i32
                    + self.dynamic_viewport_offset.y;
                scissor.extent.width = ((scissor.extent.width / scissor_viewport.width) as f32
                    * self.dynamic_viewport_size.width as f32)
                    as u32;
                scissor.extent.height = ((scissor.extent.height / scissor_viewport.height) as f32
                    * self.dynamic_viewport_size.height as f32)
                    as u32;
            }

            viewport.x = viewport.x.clamp(0.0, f32::MAX);
            viewport.y = viewport.y.clamp(0.0, f32::MAX);

            scissor.offset.x = scissor.offset.x.clamp(0, i32::MAX);
            scissor.offset.y = scissor.offset.y.clamp(0, i32::MAX);

            exec_buffer.has_dynamic_state = true;
            exec_buffer.viewport = viewport;
            exec_buffer.scissor = scissor;
        } else {
            exec_buffer.has_dynamic_state = false;
        }
    }

    fn bind_pipeline(
        device: &ash::Device,
        last_pipeline: &mut Vec<vk::Pipeline>,
        render_thread_index: usize,
        command_buffer: vk::CommandBuffer,
        exec_buffer: &SRenderCommandExecuteBuffer,
        binding_pipe: vk::Pipeline,
        _state: &State,
    ) {
        if last_pipeline[render_thread_index] != binding_pipe {
            unsafe {
                device.cmd_bind_pipeline(
                    command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    binding_pipe,
                );
            }
            last_pipeline[render_thread_index] = binding_pipe;
        }

        let dynamic_state_index: usize = Self::get_dynamic_mode_index_from_exec_buffer(exec_buffer);
        if dynamic_state_index == EVulkanBackendClipModes::DynamicScissorAndViewport as usize {
            unsafe {
                device.cmd_set_viewport(command_buffer, 0, &[exec_buffer.viewport]);
            }
            unsafe {
                device.cmd_set_scissor(command_buffer, 0, &[exec_buffer.scissor]);
            }
        }
    }

    /**************************
     * RENDERING IMPLEMENTATION
     ***************************/

    fn render_tile_layer_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        draw_calls: usize,
        state: &State,
        buffer_object_index: u128,
    ) {
        let buffer_object = self
            .device
            .buffer_objects
            .get(&buffer_object_index)
            .unwrap();

        exec_buffer.buffer = buffer_object.cur_buffer;
        exec_buffer.buffer_off = buffer_object.cur_buffer_offset;

        let is_textured: bool = Self::get_is_textured(state);
        if is_textured {
            exec_buffer.descriptors[0] = self
                .device
                .textures
                .get(&state.texture_index.unwrap())
                .unwrap()
                .vk_standard_3d_textured_descr_set
                .clone();
        }

        exec_buffer.index_buffer = self.render_index_buffer;

        exec_buffer.estimated_render_call_count = draw_calls;

        self.exec_buffer_fill_dynamic_states(state, exec_buffer);
    }

    #[must_use]
    fn render_tile_layer(
        &mut self,
        exec_buffer: &SRenderCommandExecuteBuffer,
        state: &State,
        layer_type: i32, // TODO: name the type
        color: &GlColorf,
        dir: &vec2,
        off: &vec2,
        jump_index: i32,
        indices_draw_num: usize,
        indices_offsets: &[usize],
        draw_counts: &[usize],
        instance_count: usize,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::get_state_matrix(state, &mut m);

        let mut is_textured: bool = Default::default();
        let mut blend_mode_index: usize = Default::default();
        let mut dynamic_index: usize = Default::default();
        let mut address_mode_index: usize = Default::default();
        let mut render_pass_type_index: usize = Default::default();
        let mut sub_pass_index: usize = Default::default();
        Self::get_state_indices(
            exec_buffer,
            state,
            &mut is_textured,
            &mut blend_mode_index,
            &mut dynamic_index,
            &mut address_mode_index,
            &mut render_pass_type_index,
            &mut sub_pass_index,
        );
        let pipe_layout = *self.get_tile_layer_pipe_layout(
            layer_type,
            is_textured,
            blend_mode_index,
            dynamic_index,
            render_pass_type_index,
            sub_pass_index,
        );
        let pipe_line = *self.get_tile_layer_pipe(
            layer_type,
            is_textured,
            blend_mode_index,
            dynamic_index,
            render_pass_type_index,
            sub_pass_index,
        );

        let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.get_graphic_command_buffer(
            &mut command_buffer_ptr,
            exec_buffer.thread_index as usize,
            exec_buffer.sub_pass_index,
        ) {
            return false;
        }
        let command_buffer = unsafe { &mut *command_buffer_ptr };

        Self::bind_pipeline(
            &self.ash_vk.vk_device,
            &mut self.last_pipeline_per_thread,
            exec_buffer.thread_index as usize,
            *command_buffer,
            exec_buffer,
            pipe_line,
            state,
        );

        let vertex_buffers = [exec_buffer.buffer];
        let offsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            self.ash_vk.vk_device.cmd_bind_vertex_buffers(
                *command_buffer,
                0,
                &vertex_buffers,
                &offsets,
            );
        }

        if is_textured {
            unsafe {
                self.ash_vk.vk_device.cmd_bind_descriptor_sets(
                    *command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipe_layout,
                    0,
                    &[exec_buffer.descriptors[0].descriptor],
                    &[],
                );
            }
        }

        let mut vertex_push_constants = SUniformTileGPosBorder::default();
        let mut vertex_push_constant_size: usize = std::mem::size_of::<SUniformTileGPos>();
        let frag_push_constant_size: usize = std::mem::size_of::<SUniformTileGVertColor>();

        unsafe {
            libc::memcpy(
                vertex_push_constants.base.base.pos.as_mut_ptr() as *mut c_void,
                m.as_ptr() as *const c_void,
                m.len() * std::mem::size_of::<f32>(),
            );
        }
        let frag_push_constants: SUniformTileGVertColor = *color;

        if layer_type == 1 {
            vertex_push_constants.base.dir = *dir;
            vertex_push_constants.base.offset = *off;
            vertex_push_constants.jump_index = jump_index;
            vertex_push_constant_size = std::mem::size_of::<SUniformTileGPosBorder>();
        } else if layer_type == 2 {
            vertex_push_constants.base.dir = *dir;
            vertex_push_constants.base.offset = *off;
            vertex_push_constant_size = std::mem::size_of::<SUniformTileGPosBorderLine>();
        }

        unsafe {
            self.ash_vk.vk_device.cmd_push_constants(
                *command_buffer,
                pipe_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                std::slice::from_raw_parts(
                    (&vertex_push_constants) as *const _ as *const u8,
                    vertex_push_constant_size,
                ),
            );
        }
        unsafe {
            self.ash_vk.vk_device.cmd_push_constants(
                *command_buffer,
                pipe_layout,
                vk::ShaderStageFlags::FRAGMENT,
                (std::mem::size_of::<SUniformTileGPosBorder>()
                    + std::mem::size_of::<SUniformTileGVertColorAlign>()) as u32,
                std::slice::from_raw_parts(
                    &frag_push_constants as *const _ as *const u8,
                    frag_push_constant_size,
                ),
            );
        }

        let draw_count: usize = indices_draw_num as usize;
        unsafe {
            self.ash_vk.vk_device.cmd_bind_index_buffer(
                *command_buffer,
                exec_buffer.index_buffer,
                0,
                vk::IndexType::UINT32,
            );
        }
        for i in 0..draw_count {
            let index_offset =
                (indices_offsets[i] as usize / std::mem::size_of::<u32>()) as vk::DeviceSize;

            unsafe {
                self.ash_vk.vk_device.cmd_draw_indexed(
                    *command_buffer,
                    draw_counts[i] as u32,
                    instance_count as u32,
                    index_offset as u32,
                    0,
                    0,
                );
            }
        }

        return true;
    }

    #[must_use]
    fn render_standard<TName, const IS_3D_TEXTURED: bool>(
        &mut self,
        exec_buffer: &SRenderCommandExecuteBuffer,
        state: &State,
        prim_type: PrimType,
        primitive_count: usize,
        stencil_type: StencilOpType,
        has_push_const: bool,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::get_state_matrix(state, &mut m);

        let is_line_geometry: bool = prim_type == PrimType::Lines;

        let mut is_textured: bool = Default::default();
        let mut blend_mode_index: usize = Default::default();
        let mut dynamic_index: usize = Default::default();
        let mut address_mode_index: usize = Default::default();
        let mut render_pass_type_index: usize = Default::default();
        let mut sub_pass_index: usize = Default::default();
        Self::get_state_indices(
            exec_buffer,
            state,
            &mut is_textured,
            &mut blend_mode_index,
            &mut dynamic_index,
            &mut address_mode_index,
            &mut render_pass_type_index,
            &mut sub_pass_index,
        );
        let (pipeline_ref, pipe_layout_ref) = if IS_3D_TEXTURED {
            Self::get_pipeline_and_layout(
                &mut self.standard_3d_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
                render_pass_type_index,
                sub_pass_index,
            )
        } else {
            Self::get_standard_pipe_and_layout(
                &mut self.standard_line_pipeline,
                &mut self.standard_pipeline,
                &mut self.standard_stencil_only_pipeline,
                &mut self.standard_stencil_pipeline,
                is_line_geometry,
                is_textured,
                blend_mode_index,
                dynamic_index,
                render_pass_type_index,
                stencil_type,
                sub_pass_index,
            )
        };
        let (pipeline, pipe_layout) = (*pipeline_ref, *pipe_layout_ref);

        self.render_standard_impl::<TName, { IS_3D_TEXTURED }>(
            exec_buffer,
            state,
            prim_type,
            primitive_count,
            &m,
            is_textured,
            pipeline,
            pipe_layout,
            has_push_const,
            false,
        )
    }

    #[must_use]
    fn render_blur<TName>(
        &mut self,
        exec_buffer: &SRenderCommandExecuteBuffer,
        state: &State,
        prim_type: PrimType,
        primitive_count: usize,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::get_state_matrix(state, &mut m);

        let mut is_textured: bool = Default::default();
        let mut blend_mode_index: usize = Default::default();
        let mut dynamic_index: usize = Default::default();
        let mut address_mode_index: usize = Default::default();
        let mut render_pass_type_index: usize = Default::default();
        let mut sub_pass_index: usize = Default::default();
        Self::get_state_indices(
            exec_buffer,
            state,
            &mut is_textured,
            &mut blend_mode_index,
            &mut dynamic_index,
            &mut address_mode_index,
            &mut render_pass_type_index,
            &mut sub_pass_index,
        );
        let (pipeline_ref, pipe_layout_ref) = Self::get_pipeline_and_layout(
            &mut self.blur_pipeline,
            is_textured,
            blend_mode_index,
            dynamic_index,
            render_pass_type_index,
            sub_pass_index,
        );
        let (pipeline, pipe_layout) = (*pipeline_ref, *pipe_layout_ref);

        self.render_standard_impl::<TName, false>(
            exec_buffer,
            state,
            prim_type,
            primitive_count,
            &m,
            is_textured,
            pipeline,
            pipe_layout,
            false,
            true,
        )
    }

    #[must_use]
    fn render_standard_impl<TName, const IS_3D_TEXTURED: bool>(
        &mut self,
        exec_buffer: &SRenderCommandExecuteBuffer,
        state: &State,
        prim_type: PrimType,
        primitive_count: usize,
        m: &[f32],
        is_textured: bool,
        pipeline: vk::Pipeline,
        pipe_layout: vk::PipelineLayout,
        has_push_const: bool,
        as_blur: bool,
    ) -> bool {
        let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.get_graphic_command_buffer(
            &mut command_buffer_ptr,
            exec_buffer.thread_index as usize,
            exec_buffer.sub_pass_index,
        ) {
            return false;
        }
        let command_buffer = unsafe { &mut *command_buffer_ptr };

        Self::bind_pipeline(
            &self.ash_vk.vk_device,
            &mut self.last_pipeline_per_thread,
            exec_buffer.thread_index as usize,
            *command_buffer,
            exec_buffer,
            pipeline,
            state,
        );

        let mut vert_per_prim: usize = 2;
        let mut is_indexed: bool = false;
        if prim_type == PrimType::Quads {
            vert_per_prim = 4;
            is_indexed = true;
        } else if prim_type == PrimType::Triangles {
            vert_per_prim = 3;
        }

        let vertex_buffers = [exec_buffer.buffer];
        let buffer_offsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            self.ash_vk.vk_device.cmd_bind_vertex_buffers(
                *command_buffer,
                0,
                vertex_buffers.as_slice(),
                buffer_offsets.as_slice(),
            );
        }

        if is_indexed {
            unsafe {
                self.ash_vk.vk_device.cmd_bind_index_buffer(
                    *command_buffer,
                    exec_buffer.index_buffer,
                    0,
                    vk::IndexType::UINT32,
                );
            }
        }
        if is_textured {
            unsafe {
                self.ash_vk.vk_device.cmd_bind_descriptor_sets(
                    *command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipe_layout.clone(),
                    0,
                    &[exec_buffer.descriptors[0].descriptor],
                    &[],
                );
            }
        }

        if has_push_const {
            unsafe {
                self.ash_vk.vk_device.cmd_push_constants(
                    *command_buffer,
                    pipe_layout.clone(),
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    std::slice::from_raw_parts(
                        m.as_ptr() as *const _ as *const u8,
                        m.len() * std::mem::size_of::<f32>(),
                    ),
                );
            }
        }
        if as_blur {
            let blur_push = SUniformGBlur {
                texture_size: vec2::new(
                    self.vk_swap_img_and_viewport_extent
                        .swap_image_viewport
                        .width as f32,
                    self.vk_swap_img_and_viewport_extent
                        .swap_image_viewport
                        .height as f32,
                ),
                blur_radius: 5.0,
            };
            unsafe {
                self.ash_vk.vk_device.cmd_push_constants(
                    *command_buffer,
                    pipe_layout.clone(),
                    vk::ShaderStageFlags::FRAGMENT,
                    0,
                    std::slice::from_raw_parts(
                        &blur_push as *const _ as *const u8,
                        std::mem::size_of::<SUniformGBlur>(),
                    ),
                );
            }
        }

        if is_indexed {
            unsafe {
                self.ash_vk.vk_device.cmd_draw_indexed(
                    *command_buffer,
                    (primitive_count * 6) as u32,
                    1,
                    0,
                    0,
                    0,
                );
            }
        } else {
            unsafe {
                self.ash_vk.vk_device.cmd_draw(
                    *command_buffer,
                    (primitive_count as usize * vert_per_prim as usize) as u32,
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
    fn get_vulkan_extensions(
        window: &winit::window::Window,
    ) -> Result<Vec<String>, ArrayString<4096>> {
        let mut vk_extensions = Vec::<String>::new();

        let ext_list_res = ash_window::enumerate_required_extensions(window.raw_display_handle());
        if let Err(err) = ext_list_res {
            let mut res =
                ArrayString::from_str("Could not get instance extensions from SDL: ").unwrap();
            res.push_str(&err.to_string());
            return Err(res);
        }
        let ext_list = ext_list_res.unwrap();

        for ext in ext_list {
            let ext_name = unsafe { CStr::from_ptr(*ext).to_str().unwrap().to_string() };
            vk_extensions.push(ext_name);
        }

        return Ok(vk_extensions);
    }

    fn our_vklayers(dbg: EDebugGFXModes) -> std::collections::BTreeSet<String> {
        let mut our_layers: std::collections::BTreeSet<String> = Default::default();

        if dbg == EDebugGFXModes::Minimum || dbg == EDebugGFXModes::All {
            our_layers.insert("VK_LAYER_KHRONOS_validation".to_string());
            // deprecated, but VK_LAYER_KHRONOS_validation was released after
            // vulkan 1.1
            our_layers.insert("VK_LAYER_LUNARG_standard_validation".to_string());
        }

        return our_layers;
    }

    fn our_device_extensions() -> std::collections::BTreeSet<String> {
        let mut our_ext: std::collections::BTreeSet<String> = Default::default();
        our_ext.insert(vk::KhrSwapchainFn::name().to_str().unwrap().to_string());
        return our_ext;
    }

    fn our_image_usages() -> Vec<vk::ImageUsageFlags> {
        let mut img_usages: Vec<vk::ImageUsageFlags> = Default::default();

        img_usages.push(vk::ImageUsageFlags::COLOR_ATTACHMENT);
        img_usages.push(vk::ImageUsageFlags::TRANSFER_SRC);

        return img_usages;
    }

    #[must_use]
    fn get_vulkan_layers(
        dbg: EDebugGFXModes,
        entry: &ash::Entry,
    ) -> Result<Vec<String>, ArrayString<4096>> {
        let res = entry.enumerate_instance_layer_properties();
        if res.is_err() {
            return Err(ArrayString::from_str("Could not get vulkan layers.").unwrap());
        }
        let mut vk_instance_layers = res.unwrap();

        let req_layer_names = Self::our_vklayers(dbg);
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
    fn create_vulkan_instance(
        dbg: EDebugGFXModes,
        entry: &ash::Entry,
        error: &Arc<Mutex<Error>>,
        vk_layers: &Vec<String>,
        vk_extensions: &Vec<String>,
        try_debug_extensions: bool,
    ) -> Result<ash::Instance, ArrayString<4096>> {
        let mut layers_cstr: Vec<*const libc::c_char> = Default::default();
        let mut layers_cstr_helper: Vec<CString> = Default::default();
        layers_cstr.reserve(vk_layers.len());
        for layer in vk_layers {
            layers_cstr_helper
                .push(unsafe { CString::from_vec_unchecked(layer.as_bytes().to_vec()) });
            layers_cstr.push(layers_cstr_helper.last().unwrap().as_ptr());
        }

        let mut ext_cstr: Vec<*const libc::c_char> = Default::default();
        let mut ext_cstr_helper: Vec<CString> = Default::default();
        ext_cstr.reserve(vk_extensions.len() + 1);
        for ext in vk_extensions {
            ext_cstr_helper.push(unsafe { CString::from_vec_unchecked(ext.as_bytes().to_vec()) });
            ext_cstr.push(ext_cstr_helper.last().unwrap().as_ptr());
        }

        if try_debug_extensions && (dbg == EDebugGFXModes::Minimum || dbg == EDebugGFXModes::All) {
            // debug message support
            ext_cstr.push(vk::ExtDebugUtilsFn::name().as_ptr());
        }

        let mut vk_app_info = vk::ApplicationInfo::default();
        vk_app_info.p_application_name = APP_NAME.as_ptr() as *const i8;
        vk_app_info.application_version = 1;
        vk_app_info.p_engine_name = APP_VK_NAME.as_ptr() as *const i8;
        vk_app_info.engine_version = 1;
        vk_app_info.api_version = vk::API_VERSION_1_1;

        let mut ptr_ext = std::ptr::null();
        let mut features = vk::ValidationFeaturesEXT::default();
        let enabled_exts = [
            vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
            vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
        ];
        if try_debug_extensions
            && (dbg == EDebugGFXModes::AffectsPerformance || dbg == EDebugGFXModes::All)
        {
            features.enabled_validation_feature_count = enabled_exts.len() as u32;
            features.p_enabled_validation_features = enabled_exts.as_ptr();

            ptr_ext = &features;
        }

        let mut vk_instance_info = vk::InstanceCreateInfo::default();
        vk_instance_info.p_next = ptr_ext as *const c_void;
        vk_instance_info.flags = vk::InstanceCreateFlags::empty();
        vk_instance_info.p_application_info = &vk_app_info;
        vk_instance_info.enabled_extension_count = ext_cstr.len() as u32;
        vk_instance_info.pp_enabled_extension_names = ext_cstr.as_ptr();
        vk_instance_info.enabled_layer_count = layers_cstr.len() as u32;
        vk_instance_info.pp_enabled_layer_names = layers_cstr.as_ptr();

        let mut try_again: bool = false;

        let res = unsafe { entry.create_instance(&vk_instance_info, None) };
        if let Err(res_err) = res {
            let mut check_res = CheckResult::default();
            let mut recreate_swap_chain_dummy = false;
            let crit_error_msg = check_res.check_vulkan_critical_error(
                res_err,
                error,
                &mut recreate_swap_chain_dummy,
            );
            if let Some(_err_crit) = crit_error_msg {
                return Err(ArrayString::from_str("Creating instance failed.").unwrap());
            } else if res.is_err()
                && (res_err == vk::Result::ERROR_LAYER_NOT_PRESENT
                    || res_err == vk::Result::ERROR_EXTENSION_NOT_PRESENT)
            {
                try_again = true;
            }
        }

        if try_again && try_debug_extensions {
            return Self::create_vulkan_instance(
                dbg,
                entry,
                error,
                vk_layers,
                vk_extensions,
                false,
            );
        }

        Ok(res.unwrap())
    }

    fn vk_gputype_to_graphics_gputype(vk_gpu_type: vk::PhysicalDeviceType) -> ETWGraphicsGPUType {
        if vk_gpu_type == vk::PhysicalDeviceType::DISCRETE_GPU {
            return ETWGraphicsGPUType::Discrete;
        } else if vk_gpu_type == vk::PhysicalDeviceType::INTEGRATED_GPU {
            return ETWGraphicsGPUType::Integrated;
        } else if vk_gpu_type == vk::PhysicalDeviceType::VIRTUAL_GPU {
            return ETWGraphicsGPUType::Virtual;
        } else if vk_gpu_type == vk::PhysicalDeviceType::CPU {
            return ETWGraphicsGPUType::CPU;
        }

        return ETWGraphicsGPUType::CPU;
    }

    // from:
    // https://github.com/SaschaWillems/vulkan.gpuinfo.org/blob/5c3986798afc39d736b825bf8a5fbf92b8d9ed49/includes/functions.php#L364
    fn get_driver_verson(driver_version: u32, vendor_id: u32) -> String {
        // NVIDIA
        if vendor_id == 4318 {
            format!(
                "{}.{}.{}.{}",
                (driver_version >> 22) & 0x3ff,
                (driver_version >> 14) & 0x0ff,
                (driver_version >> 6) & 0x0ff,
                (driver_version) & 0x003f
            )
        }
        // windows only
        else if vendor_id == 0x8086 {
            format!("{}.{}", (driver_version >> 14), (driver_version) & 0x3fff)
        } else {
            // Use Vulkan version conventions if vendor mapping is not available
            format!(
                "{}.{}.{}",
                (driver_version >> 22),
                (driver_version >> 12) & 0x3ff,
                driver_version & 0xfff
            )
        }
    }

    #[must_use]
    fn select_gpu(
        instance: &ash::Instance,
        dbg: EDebugGFXModes,
        logger: &SystemLogGroup,
    ) -> Result<
        (
            TTWGraphicsGPUList,
            Limits,
            Config,
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
        let mut device_list = res.unwrap();

        let renderer_name;
        let vendor_name;
        let version_name;
        let mut gpu_list = TTWGraphicsGPUList::default();

        let mut index: usize = 0;
        let mut device_prop_list = Vec::<vk::PhysicalDeviceProperties>::new();
        device_prop_list.resize(device_list.len(), Default::default());
        gpu_list.gpus.reserve(device_list.len());

        let mut found_device_index: usize = 0;
        let mut found_gpu_type: usize = ETWGraphicsGPUType::Invalid as usize;

        let mut auto_gpu_type = ETWGraphicsGPUType::Invalid;

        let is_auto_gpu: bool = true; // TODO str_comp("auto" /* TODO: g_Config.m_GfxGPUName */, "auto") == 0;

        for cur_device in &mut device_list {
            device_prop_list[index] =
                unsafe { instance.get_physical_device_properties(*cur_device) };

            let device_prop = &mut device_prop_list[index];

            let gpu_type = Self::vk_gputype_to_graphics_gputype(device_prop.device_type);

            let mut new_gpu = STWGraphicGPUItem::default();
            new_gpu.name = unsafe {
                CStr::from_ptr(device_prop.device_name.as_ptr())
                    .to_str()
                    .unwrap()
                    .to_string()
            };
            new_gpu.gpu_type = gpu_type as u32;
            gpu_list.gpus.push(new_gpu);

            index += 1;

            let dev_api_major: i32 = vk::api_version_major(device_prop.api_version) as i32;
            let dev_api_minor: i32 = vk::api_version_minor(device_prop.api_version) as i32;

            if (gpu_type as usize) < auto_gpu_type as usize
                && (dev_api_major > VK_BACKEND_MAJOR as i32
                    || (dev_api_major == VK_BACKEND_MAJOR as i32
                        && dev_api_minor >= VK_BACKEND_MINOR as i32))
            {
                gpu_list.auto_gpu.name = unsafe {
                    CStr::from_ptr(device_prop.device_name.as_ptr())
                        .to_str()
                        .unwrap()
                        .to_string()
                };
                gpu_list.auto_gpu.gpu_type = gpu_type as u32;

                auto_gpu_type = gpu_type;
            }

            if ((is_auto_gpu && (gpu_type as usize) < found_gpu_type)
                || unsafe {
                    CStr::from_ptr(device_prop.device_name.as_ptr())
                        .to_str()
                        .unwrap()
                        .to_string()
                        == "auto" /* TODO: g_Config.m_GfxGPUName */
                })
                && (dev_api_major > VK_BACKEND_MAJOR as i32
                    || (dev_api_major == VK_BACKEND_MAJOR as i32
                        && dev_api_minor >= VK_BACKEND_MINOR as i32))
            {
                found_device_index = index;
                found_gpu_type = gpu_type as usize;
            }
        }

        if found_device_index == 0 {
            found_device_index = 1;
        }

        let device_prop = &mut device_prop_list[found_device_index - 1];

        let dev_api_major: i32 = vk::api_version_major(device_prop.api_version) as i32;
        let dev_api_minor: i32 = vk::api_version_minor(device_prop.api_version) as i32;
        let dev_api_patch: i32 = vk::api_version_patch(device_prop.api_version) as i32;

        renderer_name = unsafe {
            CStr::from_ptr(device_prop.device_name.as_ptr())
                .to_str()
                .unwrap()
                .to_string()
        };
        let vendor_name_str: &str;
        match device_prop.vendor_id {
            0x1002 => vendor_name_str = "AMD",
            0x1010 => vendor_name_str = "ImgTec",
            0x106B => vendor_name_str = "Apple",
            0x10DE => vendor_name_str = "NVIDIA",
            0x13B5 => vendor_name_str = "ARM",
            0x5143 => vendor_name_str = "Qualcomm",
            0x8086 => vendor_name_str = "INTEL",
            0x10005 => vendor_name_str = "Mesa",
            _ => {
                logger
                    .log(LogLevel::Info)
                    .msg("unknown gpu vendor ")
                    .msg_var(&device_prop.vendor_id);
                vendor_name_str = "unknown"
            }
        }

        let mut limits = Limits::default();
        vendor_name = vendor_name_str.to_string();
        version_name = format!(
            "Vulkan {}.{}.{} (driver: {})",
            dev_api_major,
            dev_api_minor,
            dev_api_patch,
            Self::get_driver_verson(device_prop.driver_version, device_prop.vendor_id)
        );

        // get important device limits
        limits.non_coherent_mem_alignment = device_prop.limits.non_coherent_atom_size;
        limits.optimal_image_copy_mem_alignment =
            device_prop.limits.optimal_buffer_copy_offset_alignment;
        limits.max_texture_size = device_prop.limits.max_image_dimension2_d;
        limits.max_sampler_anisotropy = device_prop.limits.max_sampler_anisotropy as u32;

        limits.min_uniform_align = device_prop.limits.min_uniform_buffer_offset_alignment as u32;
        limits.max_multi_sample = device_prop.limits.framebuffer_color_sample_counts;

        if is_verbose_mode(dbg) {
            logger
                .log(LogLevel::Debug)
                .msg("device prop: non-coherent align: ")
                .msg_var(&limits.non_coherent_mem_alignment)
                .msg(", optimal image copy align: ")
                .msg_var(&limits.optimal_image_copy_mem_alignment)
                .msg(", max texture size: ")
                .msg_var(&limits.max_texture_size)
                .msg(", max sampler anisotropy: ")
                .msg_var(&limits.max_sampler_anisotropy);
            logger
                .log(LogLevel::Debug)
                .msg("device prop: min uniform align: ")
                .msg_var(&limits.min_uniform_align)
                .msg(", multi sample: ")
                .msg_var(&(limits.max_multi_sample.as_raw()));
        }

        let cur_device = device_list[found_device_index - 1];

        let queue_prop_list =
            unsafe { instance.get_physical_device_queue_family_properties(cur_device) };
        if queue_prop_list.len() == 0 {
            return Err(ArrayString::from_str("No vulkan queue family properties found.").unwrap());
        }

        let mut queue_node_index: u32 = u32::MAX;
        for i in 0..queue_prop_list.len() {
            if queue_prop_list[i].queue_count > 0
                && !(queue_prop_list[i].queue_flags & vk::QueueFlags::GRAPHICS).is_empty()
            {
                queue_node_index = i as u32;
            }
            /*if(vQueuePropList[i].queue_count > 0 && (vQueuePropList[i].queue_flags &
            vk::QueueFlags::COMPUTE))
            {
                QueueNodeIndex = i;
            }*/
        }

        if queue_node_index == u32::MAX {
            return Err(ArrayString::from_str(
                "No vulkan queue found that matches the requirements: graphics queue.",
            )
            .unwrap());
        }

        Ok((
            gpu_list,
            limits,
            Config::default(),
            renderer_name,
            vendor_name,
            version_name,
            cur_device,
            queue_node_index,
        ))
    }

    #[must_use]
    fn create_logical_device(
        phy_gpu: &vk::PhysicalDevice,
        graphics_queue_index: u32,
        instance: &ash::Instance,
        layers: &Vec<String>,
    ) -> Result<ash::Device, ArrayString<4096>> {
        let mut layer_cnames = Vec::<*const libc::c_char>::new();
        let mut layer_cnames_helper = Vec::<CString>::new();
        layer_cnames.reserve(layers.len());
        layer_cnames_helper.reserve(layers.len());
        for layer in layers {
            let mut bytes = layer.clone().into_bytes();
            bytes.push(0);
            layer_cnames_helper.push(CString::from_vec_with_nul(bytes).unwrap());
            layer_cnames.push(layer_cnames_helper.last().unwrap().as_ptr());
        }

        let res = unsafe { instance.enumerate_device_extension_properties(*phy_gpu) };
        if res.is_err() {
            return Err(ArrayString::from_str(
                "Querying logical device extension properties failed.",
            )
            .unwrap());
        }
        let mut dev_prop_list = res.unwrap();

        let mut dev_prop_cnames = Vec::<*const libc::c_char>::new();
        let mut dev_prop_cnames_helper = Vec::<CString>::new();
        let our_dev_ext = Self::our_device_extensions();

        for cur_ext_prop in &mut dev_prop_list {
            let ext_name = unsafe {
                CStr::from_ptr(cur_ext_prop.extension_name.as_ptr())
                    .to_str()
                    .unwrap()
                    .to_string()
            };
            let it = our_dev_ext.get(&ext_name);
            if let Some(str) = it {
                dev_prop_cnames_helper
                    .push(unsafe { CString::from_vec_unchecked(str.as_bytes().to_vec()) });
                dev_prop_cnames.push(dev_prop_cnames_helper.last().unwrap().as_ptr());
            }
        }

        let mut vk_queue_create_info = vk::DeviceQueueCreateInfo::default();
        vk_queue_create_info.queue_family_index = graphics_queue_index;
        vk_queue_create_info.queue_count = 1;
        let queue_prio = 1.0;
        vk_queue_create_info.p_queue_priorities = &queue_prio;
        vk_queue_create_info.flags = vk::DeviceQueueCreateFlags::default();

        let mut vk_create_info = vk::DeviceCreateInfo::default();
        vk_create_info.queue_create_info_count = 1;
        vk_create_info.p_queue_create_infos = &vk_queue_create_info;
        vk_create_info.pp_enabled_extension_names = layer_cnames.as_ptr();
        vk_create_info.enabled_extension_count = layer_cnames.len() as u32;
        vk_create_info.pp_enabled_extension_names = dev_prop_cnames.as_ptr();
        vk_create_info.enabled_extension_count = dev_prop_cnames.len() as u32;
        vk_create_info.p_enabled_features = std::ptr::null();
        vk_create_info.flags = vk::DeviceCreateFlags::empty();

        let res = unsafe { instance.create_device(*phy_gpu, &vk_create_info, None) };
        if res.is_err() {
            return Err(ArrayString::from_str("Logical device could not be created.").unwrap());
        }
        Ok(res.unwrap())
    }

    #[must_use]
    fn create_surface(
        entry: &ash::Entry,
        raw_window: &winit::window::Window,
        surface: &ash::extensions::khr::Surface,
        instance: &ash::Instance,
        phy_gpu: &vk::PhysicalDevice,
        queue_family_index: u32,
    ) -> Result<vk::SurfaceKHR, ArrayString<4096>> {
        let surf_res = unsafe {
            ash_window::create_surface(
                entry,
                instance,
                raw_window.raw_display_handle(),
                raw_window.raw_window_handle(),
                None,
            )
        };
        if let Err(err) = surf_res {
            // TODO dbg_msg("vulkan", "error from sdl: %s", SDL_GetError());
            let mut res =
                ArrayString::from_str("Creating a vulkan surface for the SDL window failed: ")
                    .unwrap();
            res.push_str(&err.to_string());
            return Err(res);
        }
        let surface_khr = surf_res.unwrap();

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

    fn destroy_surface(&mut self) {
        unsafe {
            self.ash_vk
                .surface
                .destroy_surface(self.vk_present_surface, None)
        };
    }

    #[must_use]
    fn get_presentation_mode(&mut self, vk_io_mode: &mut vk::PresentModeKHR) -> bool {
        let res = unsafe {
            self.ash_vk
                .surface
                .get_physical_device_surface_present_modes(self.vk_gpu, self.vk_present_surface)
        };
        if res.is_err() {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::Init,
                "The device surface presentation modes could not be fetched.",
            );
            return false;
        }

        let present_mode_list = res.unwrap();

        *vk_io_mode = /*TODO!: g_Config.*/ if self.gfx_vsync { vk::PresentModeKHR::FIFO } else { vk::PresentModeKHR::IMMEDIATE };
        for mode in &present_mode_list {
            if mode == vk_io_mode {
                return true;
            }
        }

        // TODO dbg_msg("vulkan", "warning: requested presentation mode was not available. falling back to mailbox / fifo relaxed.");
        *vk_io_mode = /*TODO!: g_Config.*/ if self.gfx_vsync { vk::PresentModeKHR::FIFO_RELAXED } else { vk::PresentModeKHR::MAILBOX };
        for mode in &present_mode_list {
            if mode == vk_io_mode {
                return true;
            }
        }

        // TODO dbg_msg("vulkan", "warning: requested presentation mode was not available. using first available.");
        if present_mode_list.len() > 0 {
            *vk_io_mode = present_mode_list[0];
        }

        return true;
    }

    #[must_use]
    fn get_surface_properties(
        &mut self,
        vk_surf_capabilities: &mut vk::SurfaceCapabilitiesKHR,
    ) -> bool {
        let capabilities_res = unsafe {
            self.ash_vk
                .surface
                .get_physical_device_surface_capabilities(self.vk_gpu, self.vk_present_surface)
        };
        if let Err(_) = capabilities_res {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::Init,
                "The device surface capabilities could not be fetched.",
            );
            return false;
        }
        *vk_surf_capabilities = capabilities_res.unwrap();
        return true;
    }

    fn get_number_of_swap_images(&mut self, vk_capabilities: &vk::SurfaceCapabilitiesKHR) -> u32 {
        let img_number = vk_capabilities.min_image_count + 1;
        if is_verbose(&*self.dbg) {
            self.logger
                .log(LogLevel::Debug)
                .msg("minimal swap image count ")
                .msg_var(&vk_capabilities.min_image_count);
        }
        return if vk_capabilities.max_image_count > 0
            && img_number > vk_capabilities.max_image_count
        {
            vk_capabilities.max_image_count
        } else {
            img_number
        };
    }

    fn get_swap_image_size(
        &mut self,
        vk_capabilities: &vk::SurfaceCapabilitiesKHR,
    ) -> SSwapImgViewportExtent {
        let mut ret_size = vk::Extent2D {
            width: self.canvas_width as u32,
            height: self.canvas_height as u32,
        };

        if vk_capabilities.current_extent.width == u32::MAX {
            ret_size.width = ret_size.width.clamp(
                vk_capabilities.min_image_extent.width,
                vk_capabilities.max_image_extent.width,
            );
            ret_size.height = ret_size.height.clamp(
                vk_capabilities.min_image_extent.height,
                vk_capabilities.max_image_extent.height,
            );
        } else {
            ret_size = vk_capabilities.current_extent;
        }

        let mut auto_viewport_extent = ret_size;
        let mut uses_forced_viewport: bool = false;
        // keep this in sync with graphics_threaded AdjustViewport's check
        if auto_viewport_extent.height > 4 * auto_viewport_extent.width / 5 {
            auto_viewport_extent.height = 4 * auto_viewport_extent.width / 5;
            uses_forced_viewport = true;
        }

        let mut ext = SSwapImgViewportExtent::default();
        ext.swap_image_viewport = ret_size;
        ext.forced_viewport = auto_viewport_extent;
        ext.has_forced_viewport = uses_forced_viewport;

        return ext;
    }

    #[must_use]
    fn get_image_usage(
        &mut self,
        vk_capabilities: &vk::SurfaceCapabilitiesKHR,
        vk_out_usage: &mut vk::ImageUsageFlags,
    ) -> bool {
        let out_img_usages = Self::our_image_usages();
        if out_img_usages.is_empty() {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::Init,
                "Framebuffer image attachment types not supported.",
            );
            return false;
        }

        *vk_out_usage = out_img_usages[0];

        for img_usage in &out_img_usages {
            let img_usage_flags = *img_usage & vk_capabilities.supported_usage_flags;
            if img_usage_flags != *img_usage {
                self.error.lock().unwrap().set_error(
                    EGFXErrorType::Init,
                    "Framebuffer image attachment types not supported.",
                );
                return false;
            }

            *vk_out_usage = *vk_out_usage | *img_usage;
        }

        return true;
    }

    fn get_transform(vk_capabilities: &vk::SurfaceCapabilitiesKHR) -> vk::SurfaceTransformFlagsKHR {
        if !(vk_capabilities.supported_transforms & vk::SurfaceTransformFlagsKHR::IDENTITY)
            .is_empty()
        {
            return vk::SurfaceTransformFlagsKHR::IDENTITY;
        }
        return vk_capabilities.current_transform;
    }

    #[must_use]
    fn get_format(&mut self) -> bool {
        let _surf_formats: u32 = 0;
        let res = unsafe {
            self.ash_vk
                .surface
                .get_physical_device_surface_formats(self.vk_gpu, self.vk_present_surface)
        };
        if res.is_err() && *res.as_ref().unwrap_err() != vk::Result::INCOMPLETE {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::Init,
                "The device surface format fetching failed.",
            );
            return false;
        }

        if res.is_err() && *res.as_ref().unwrap_err() == vk::Result::INCOMPLETE {
            // TODO dbg_msg("vulkan", "warning: not all surface formats are requestable with your current settings.");
            // TODO!  SetError(EGFXErrorType::GFX_ERROR_TYPE_INIT, ("The device surface format fetching failed."));
            return false;
        }

        let surf_format_list = res.unwrap();

        if surf_format_list.len() == 1 && surf_format_list[0].format == vk::Format::UNDEFINED {
            self.vk_surf_format.format = vk::Format::B8G8R8A8_UNORM;
            self.vk_surf_format.color_space = vk::ColorSpaceKHR::SRGB_NONLINEAR;
            // TODO dbg_msg("vulkan", "warning: surface format was undefined. This can potentially cause bugs.");
            return true;
        }

        for find_format in &surf_format_list {
            if find_format.format == vk::Format::B8G8R8A8_UNORM
                && find_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            {
                self.vk_surf_format = *find_format;
                return true;
            } else if find_format.format == vk::Format::R8G8B8A8_UNORM
                && find_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            {
                self.vk_surf_format = *find_format;
                return true;
            }
        }

        // TODO dbg_msg("vulkan", "warning: surface format was not RGBA(or variants of it). This can potentially cause weird looking images(too bright etc.).");
        self.vk_surf_format = surf_format_list[0];
        return true;
    }

    #[must_use]
    fn create_swap_chain(&mut self, old_swap_chain: &mut vk::SwapchainKHR) -> bool {
        let mut vksurf_cap = vk::SurfaceCapabilitiesKHR::default();
        if !self.get_surface_properties(&mut vksurf_cap) {
            return false;
        }

        let mut present_mode = vk::PresentModeKHR::IMMEDIATE;
        if !self.get_presentation_mode(&mut present_mode) {
            return false;
        }

        let swap_img_count = self.get_number_of_swap_images(&vksurf_cap);

        self.vk_swap_img_and_viewport_extent = self.get_swap_image_size(&vksurf_cap);

        let mut usage_flags = vk::ImageUsageFlags::default();
        if !self.get_image_usage(&vksurf_cap, &mut usage_flags) {
            return false;
        }

        let transform_flag_bits = Self::get_transform(&vksurf_cap);

        if !self.get_format() {
            return false;
        }

        *old_swap_chain = self.vk_swap_chain_khr;

        let mut swap_info = vk::SwapchainCreateInfoKHR::default();
        swap_info.flags = vk::SwapchainCreateFlagsKHR::empty();
        swap_info.surface = self.vk_present_surface;
        swap_info.min_image_count = swap_img_count;
        swap_info.image_format = self.vk_surf_format.format;
        swap_info.image_color_space = self.vk_surf_format.color_space;
        swap_info.image_extent = self.vk_swap_img_and_viewport_extent.swap_image_viewport;
        swap_info.image_array_layers = 1;
        swap_info.image_usage = usage_flags;
        swap_info.image_sharing_mode = vk::SharingMode::EXCLUSIVE;
        swap_info.queue_family_index_count = 0;
        swap_info.p_queue_family_indices = std::ptr::null();
        swap_info.pre_transform = transform_flag_bits;
        swap_info.composite_alpha = vk::CompositeAlphaFlagsKHR::OPAQUE;
        swap_info.present_mode = present_mode;
        swap_info.clipped = vk::TRUE;
        swap_info.old_swapchain = *old_swap_chain;

        self.vk_swap_chain_khr = vk::SwapchainKHR::default();
        let res = unsafe {
            self.ash_vk
                .vk_swap_chain_ash
                .create_swapchain(&swap_info, None)
        };
        if res.is_err() {
            let crit_error_msg = self.check_res.check_vulkan_critical_error(
                res.unwrap_err(),
                &self.error,
                &mut self.recreate_swap_chain,
            );
            if let Some(crit_err) = crit_error_msg {
                self.error.lock().unwrap().set_error_extra(
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

    fn destroy_swap_chain(&mut self, force_destroy: bool) {
        if force_destroy {
            unsafe {
                self.ash_vk
                    .vk_swap_chain_ash
                    .destroy_swapchain(self.vk_swap_chain_khr, None);
            }
            self.vk_swap_chain_khr = vk::SwapchainKHR::null();
        }
    }

    #[must_use]
    fn get_swap_chain_image_handles(&mut self) -> bool {
        let res = unsafe {
            self.ash_vk
                .vk_swap_chain_ash
                .get_swapchain_images(self.vk_swap_chain_khr)
        };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .set_error(EGFXErrorType::Init, "Could not get swap chain images.");
            return false;
        }

        self.vk_swap_chain_images = res.unwrap();
        self.device.swap_chain_image_count = self.vk_swap_chain_images.len() as u32;

        return true;
    }

    fn clear_swap_chain_image_handles(&mut self) {
        self.vk_swap_chain_images.clear();
    }

    fn get_device_queue(
        device: &ash::Device,
        graphics_queue_index: u32,
    ) -> Result<(vk::Queue, vk::Queue), ArrayString<4096>> {
        Ok((
            unsafe { device.get_device_queue(graphics_queue_index, 0) },
            unsafe { device.get_device_queue(graphics_queue_index, 0) },
        ))
    }

    unsafe extern "system" fn vk_debug_callback(
        message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
        _message_type: vk::DebugUtilsMessageTypeFlagsEXT,
        ptr_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
        _ptr_raw_user: *mut c_void,
    ) -> vk::Bool32 {
        if !(message_severity & vk::DebugUtilsMessageSeverityFlagsEXT::ERROR).is_empty() {
            println!("[vulkan debug] error: {}", unsafe {
                CStr::from_ptr((*ptr_callback_data).p_message)
                    .to_str()
                    .unwrap()
            });
        } else {
            println!("[vulkan debug] {}", unsafe {
                CStr::from_ptr((*ptr_callback_data).p_message)
                    .to_str()
                    .unwrap()
            });
        }

        return vk::FALSE;
    }

    fn create_debug_utils_messenger_ext(
        entry: &ash::Entry,
        instance: &ash::Instance,
        create_info: &vk::DebugUtilsMessengerCreateInfoEXT,
        allocator: Option<&vk::AllocationCallbacks>,
    ) -> vk::DebugUtilsMessengerEXT {
        let dbg_utils = ash::extensions::ext::DebugUtils::new(entry, instance);
        let res = unsafe { dbg_utils.create_debug_utils_messenger(create_info, allocator) };
        if let Err(_res) = res {
            return vk::DebugUtilsMessengerEXT::null();
        }
        res.unwrap()
    }

    fn destroy_debug_utils_messenger_ext(_debug_messenger: &mut vk::DebugUtilsMessengerEXT) {
        /* TODO! let func = unsafe { self.m_VKEntry.get_instance_proc_addr(self.m_VKInstance, "vkDestroyDebugUtilsMessengerEXT") as Option<vk::PFN_vkDestroyDebugUtilsMessengerEXT> };
        if let Some(f) = func
        {
            f(self.m_VKInstance, DebugMessenger, std::ptr::null());
        }*/
    }

    fn setup_debug_callback(
        entry: &ash::Entry,
        instance: &ash::Instance,
        logger: &SystemLogGroup,
    ) -> Result<vk::DebugUtilsMessengerEXT, ArrayString<4096>> {
        let mut create_info = vk::DebugUtilsMessengerCreateInfoEXT::default();
        create_info.message_severity = vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
            | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
            | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR;
        create_info.message_type = vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE; // | vk::DEBUG_UTILS_MESSAGE_TYPE_GENERAL_BIT_EXT <- too annoying
        create_info.pfn_user_callback = Some(Self::vk_debug_callback);

        let res_dbg = Self::create_debug_utils_messenger_ext(entry, instance, &create_info, None);
        if res_dbg == vk::DebugUtilsMessengerEXT::null() {
            logger
                .log(LogLevel::Info)
                .msg("didn't find vulkan debug layer.");
            return Err(ArrayString::from_str("Debug extension could not be loaded.").unwrap());
        } else {
            logger
                .log(LogLevel::Info)
                .msg("enabled vulkan debug context.");
        }
        return Ok(res_dbg);
    }

    fn unregister_debug_callback(&mut self) {
        if self._debug_messenger != vk::DebugUtilsMessengerEXT::null() {
            Self::destroy_debug_utils_messenger_ext(&mut self._debug_messenger);
        }
    }

    #[must_use]
    fn create_image_views(&mut self) -> bool {
        let swap_chain_count = self.device.swap_chain_image_count;
        let img_format = self.vk_surf_format.format;
        let image_views = &mut self.swap_chain_image_view_list;
        let images = &mut self.vk_swap_chain_images;

        image_views.resize(swap_chain_count as usize, Default::default());

        for i in 0..swap_chain_count {
            let res = self.device.create_image_view(
                images[i as usize],
                img_format,
                vk::ImageViewType::TYPE_2D,
                1,
                1,
                vk::ImageAspectFlags::COLOR,
            );
            if res.is_err() {
                self.error.lock().unwrap().set_error(
                    EGFXErrorType::Init,
                    "Could not create image views for the swap chain framebuffers.",
                );
                return false;
            }
            image_views[i as usize] = res.unwrap();
        }

        true
    }

    fn destroy_image_views(&mut self) {
        for imgage_view in &mut self.swap_chain_image_view_list {
            unsafe {
                self.ash_vk.vk_device.destroy_image_view(*imgage_view, None);
            }
        }

        self.swap_chain_image_view_list.clear();
    }

    #[must_use]
    fn create_images_for_second_pass(&mut self) -> bool {
        let swap_chain_count = self.device.swap_chain_image_count as usize;

        self.image_list_for_double_pass
            .resize(swap_chain_count, Default::default());

        let mut res = true;
        self.image_list_for_double_pass
            .iter_mut()
            .for_each(|img_second_pass| {
                let mut img = vk::Image::default();
                let mut img_mem = SMemoryImageBlock::<IMAGE_BUFFER_CACHE_ID>::default();
                if !self.device.create_image_ex(
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
                    &mut img,
                    &mut img_mem,
                    vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::INPUT_ATTACHMENT
                        | vk::ImageUsageFlags::SAMPLED,
                    None,
                    self.cur_image_index,
                ) {
                    res = false;
                }

                img_second_pass.img = img;
                img_second_pass.img_mem = img_mem;
                img_second_pass.img_view = self
                    .device
                    .create_image_view(
                        img,
                        self.vk_surf_format.format,
                        vk::ImageViewType::TYPE_2D,
                        1,
                        1,
                        vk::ImageAspectFlags::COLOR,
                    )
                    .unwrap(); // TODO: error handling

                img_second_pass.samplers[0] = self
                    .device
                    .get_texture_sampler(ESupportedSamplerTypes::Repeat);
                img_second_pass.samplers[1] = self
                    .device
                    .get_texture_sampler(ESupportedSamplerTypes::ClampToEdge);

                if !self
                    .device
                    .create_new_textured_standard_descriptor_sets(0, img_second_pass)
                {
                    self.error.lock().unwrap().set_error(
                        EGFXErrorType::Init,
                        "Could not create image descriptors for double pass images.",
                    );
                    res = false;
                }
                if !self
                    .device
                    .create_new_textured_standard_descriptor_sets(1, img_second_pass)
                {
                    self.error.lock().unwrap().set_error(
                        EGFXErrorType::Init,
                        "Could not create image descriptors for double pass images.",
                    );
                    res = false;
                }
            });
        res
    }

    fn destroy_images_for_second_pass(&mut self) {
        self.image_list_for_double_pass.iter_mut().for_each(|img| {
            Device::destroy_textured_standard_descriptor_sets(&self.ash_vk.vk_device, img, 0);
            Device::destroy_textured_standard_descriptor_sets(&self.ash_vk.vk_device, img, 1);
            unsafe {
                self.ash_vk.vk_device.destroy_image(img.img, None);
            }
            unsafe {
                self.ash_vk.vk_device.destroy_image_view(img.img_view, None);
            }
            Device::free_image_mem_block(
                &mut self.device.frame_delayed_buffer_cleanups,
                &mut self.device.image_buffer_caches,
                &mut img.img_mem,
                self.cur_image_index,
            );
        });

        self.image_list_for_double_pass.clear()
    }

    #[must_use]
    fn create_multi_sampler_image_attachments_impl(&mut self, second_pass: bool) -> bool {
        let has_multi_sampling = if second_pass {
            self.has_multi_sampling_in_second_pass()
        } else {
            self.has_multi_sampling()
        };
        let multi_sampling_count = if second_pass {
            self.device.config.multi_sampling_second_pass_count
        } else {
            self.device.config.multi_sampling_count
        };
        let multi_sampling_images = if second_pass {
            &mut self.multi_sampling_images_for_double_pass
        } else {
            &mut self.swap_chain_multi_sampling_images
        };
        multi_sampling_images.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );
        if has_multi_sampling {
            for i in 0..self.device.swap_chain_image_count {
                let mut img = vk::Image::default();
                let mut img_mem = SMemoryImageBlock::<IMAGE_BUFFER_CACHE_ID>::default();
                if !self.device.create_image_ex(
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
                    &mut img,
                    &mut img_mem,
                    vk::ImageUsageFlags::TRANSIENT_ATTACHMENT
                        | vk::ImageUsageFlags::COLOR_ATTACHMENT,
                    Some(multi_sampling_count),
                    self.cur_image_index,
                ) {
                    return false;
                }
                multi_sampling_images[i as usize].image = img;
                multi_sampling_images[i as usize].img_mem = img_mem;
                multi_sampling_images[i as usize].img_view = self
                    .device
                    .create_image_view(
                        multi_sampling_images[i as usize].image,
                        self.vk_surf_format.format,
                        vk::ImageViewType::TYPE_2D,
                        1,
                        1,
                        vk::ImageAspectFlags::COLOR,
                    )
                    .unwrap(); // TODO: err handling
            }
        }

        return true;
    }

    #[must_use]
    fn create_multi_sampler_image_attachments(&mut self) -> bool {
        self.create_multi_sampler_image_attachments_impl(false)
    }

    #[must_use]
    fn create_multi_sampler_image_attachments_for_second_pass(&mut self) -> bool {
        self.create_multi_sampler_image_attachments_impl(true)
    }

    fn destroy_multi_sampler_image_attachments_impl(&mut self, second_pass: bool) {
        let has_multi_sampling = if second_pass {
            self.has_multi_sampling_in_second_pass()
        } else {
            self.has_multi_sampling()
        };
        let multi_sampling_images = if second_pass {
            &mut self.multi_sampling_images_for_double_pass
        } else {
            &mut self.swap_chain_multi_sampling_images
        };
        if has_multi_sampling {
            for i in 0..multi_sampling_images.len() {
                unsafe {
                    self.ash_vk
                        .vk_device
                        .destroy_image(multi_sampling_images[i as usize].image, None);
                }
                unsafe {
                    self.ash_vk
                        .vk_device
                        .destroy_image_view(multi_sampling_images[i as usize].img_view, None);
                }
                Device::free_image_mem_block(
                    &mut self.device.frame_delayed_buffer_cleanups,
                    &mut self.device.image_buffer_caches,
                    &mut multi_sampling_images[i as usize].img_mem,
                    self.cur_image_index,
                );
            }
        }
        multi_sampling_images.clear();
    }

    fn destroy_multi_sampler_image_attachments(&mut self) {
        self.destroy_multi_sampler_image_attachments_impl(false)
    }

    fn destroy_multi_sampler_image_attachments_for_second_pass(&mut self) {
        self.destroy_multi_sampler_image_attachments_impl(true)
    }

    #[must_use]
    fn create_stencil_attachments_for_pass_transition(&mut self) -> bool {
        let has_multi_sampling = self.has_multi_sampling();
        let multi_sampling_count = if has_multi_sampling {
            Some(self.device.config.multi_sampling_count)
        } else {
            None
        };
        let stencil_images = &mut self.stencil_list_for_pass_transition;
        stencil_images.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );

        // determine stencil image format
        self.stencil_format = [
            vk::Format::S8_UINT,
            vk::Format::D32_SFLOAT_S8_UINT,
            vk::Format::D24_UNORM_S8_UINT,
            vk::Format::D16_UNORM_S8_UINT,
        ]
        .into_iter()
        .find(|format| {
            let props = unsafe {
                self.ash_vk
                    .vk_instance
                    .get_physical_device_format_properties(self.vk_gpu, *format)
            };

            let tiling = vk::ImageTiling::OPTIMAL;
            let features = vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT;
            if tiling == vk::ImageTiling::LINEAR && props.linear_tiling_features.contains(features)
            {
                true
            } else if tiling == vk::ImageTiling::OPTIMAL
                && props.optimal_tiling_features.contains(features)
            {
                true
            } else {
                false
            }
        })
        .unwrap();

        for i in 0..self.device.swap_chain_image_count {
            let mut img = vk::Image::default();
            let mut img_mem = SMemoryImageBlock::<IMAGE_BUFFER_CACHE_ID>::default();
            if !self.device.create_image_ex(
                self.vk_swap_img_and_viewport_extent
                    .swap_image_viewport
                    .width,
                self.vk_swap_img_and_viewport_extent
                    .swap_image_viewport
                    .height,
                1,
                1,
                self.stencil_format,
                vk::ImageTiling::OPTIMAL,
                &mut img,
                &mut img_mem,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                multi_sampling_count,
                self.cur_image_index,
            ) {
                return false;
            }
            stencil_images[i as usize].image = img;
            stencil_images[i as usize].img_mem = img_mem;
            stencil_images[i as usize].img_view = self
                .device
                .create_image_view(
                    stencil_images[i as usize].image,
                    self.stencil_format,
                    vk::ImageViewType::TYPE_2D,
                    1,
                    1,
                    vk::ImageAspectFlags::STENCIL,
                )
                .unwrap(); // TODO: err handling
        }
        true
    }

    fn destroy_stencil_attachments_for_pass_transition(&mut self) {
        let stencil_images = &mut self.stencil_list_for_pass_transition;
        for i in 0..stencil_images.len() {
            unsafe {
                self.ash_vk
                    .vk_device
                    .destroy_image(stencil_images[i as usize].image, None);
            }
            unsafe {
                self.ash_vk
                    .vk_device
                    .destroy_image_view(stencil_images[i as usize].img_view, None);
            }
            Device::free_image_mem_block(
                &mut self.device.frame_delayed_buffer_cleanups,
                &mut self.device.image_buffer_caches,
                &mut stencil_images[i as usize].img_mem,
                self.cur_image_index,
            );
        }
        stencil_images.clear();
    }

    #[must_use]
    fn create_render_pass_impl(
        &mut self,
        clear_attachs: bool,
        has_multi_sampling: bool,
        has_multi_sampling_in_second_pass: bool,
        format: vk::Format,
        double_pass: bool,
    ) -> anyhow::Result<vk::RenderPass> {
        let has_multi_sampling_targets = has_multi_sampling;
        let has_multi_sampling_in_second_pass_targets = has_multi_sampling_in_second_pass;
        let mut multi_sampling_color_attachment = vk::AttachmentDescription::default();
        multi_sampling_color_attachment.format = format;
        multi_sampling_color_attachment.samples =
            Device::get_sample_count(self.device.config.multi_sampling_count, &self.device.limits);
        multi_sampling_color_attachment.load_op = if clear_attachs {
            vk::AttachmentLoadOp::CLEAR
        } else {
            vk::AttachmentLoadOp::DONT_CARE
        };
        multi_sampling_color_attachment.store_op = vk::AttachmentStoreOp::DONT_CARE;
        multi_sampling_color_attachment.stencil_load_op = vk::AttachmentLoadOp::DONT_CARE;
        multi_sampling_color_attachment.stencil_store_op = vk::AttachmentStoreOp::DONT_CARE;
        multi_sampling_color_attachment.initial_layout = vk::ImageLayout::UNDEFINED;
        multi_sampling_color_attachment.final_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        let mut color_attachment = vk::AttachmentDescription::default();
        color_attachment.format = format;
        color_attachment.samples = vk::SampleCountFlags::TYPE_1;
        color_attachment.load_op = if clear_attachs && !has_multi_sampling_targets {
            vk::AttachmentLoadOp::CLEAR
        } else {
            vk::AttachmentLoadOp::DONT_CARE
        };
        color_attachment.store_op = vk::AttachmentStoreOp::STORE;
        color_attachment.stencil_load_op = vk::AttachmentLoadOp::DONT_CARE;
        color_attachment.stencil_store_op = vk::AttachmentStoreOp::DONT_CARE;
        color_attachment.initial_layout = vk::ImageLayout::UNDEFINED;
        color_attachment.final_layout = vk::ImageLayout::PRESENT_SRC_KHR;

        let mut stencil_attachment = vk::AttachmentDescription::default();
        stencil_attachment.format = self.stencil_format;
        stencil_attachment.samples = if !has_multi_sampling_in_second_pass_targets {
            vk::SampleCountFlags::TYPE_1
        } else {
            Device::get_sample_count(
                self.device.config.multi_sampling_second_pass_count,
                &self.device.limits,
            )
        };
        stencil_attachment.load_op = vk::AttachmentLoadOp::DONT_CARE;
        stencil_attachment.store_op = vk::AttachmentStoreOp::DONT_CARE;
        stencil_attachment.stencil_load_op = vk::AttachmentLoadOp::CLEAR;
        stencil_attachment.stencil_store_op = vk::AttachmentStoreOp::STORE;
        stencil_attachment.initial_layout = vk::ImageLayout::UNDEFINED;
        stencil_attachment.final_layout = vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL;

        let mut color_attachment_in_second_pass = color_attachment.clone();
        color_attachment_in_second_pass.load_op = if clear_attachs && !has_multi_sampling {
            vk::AttachmentLoadOp::CLEAR
        } else {
            vk::AttachmentLoadOp::DONT_CARE
        };
        color_attachment_in_second_pass.store_op = vk::AttachmentStoreOp::DONT_CARE;
        color_attachment_in_second_pass.final_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
        let mut multi_sampling_color_attachment_in_second_pass =
            multi_sampling_color_attachment.clone();
        multi_sampling_color_attachment_in_second_pass.samples = Device::get_sample_count(
            self.device.config.multi_sampling_second_pass_count,
            &self.device.limits,
        );

        let mut color_attachment_ref = vk::AttachmentReference::default();
        color_attachment_ref.layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        let mut multi_sampling_color_attachment_ref = vk::AttachmentReference::default();
        multi_sampling_color_attachment_ref.layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        let mut color_attachment_in_second_pass_ref = vk::AttachmentReference::default();
        color_attachment_in_second_pass_ref.layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        let mut color_attachment_from_first_pass_as_input_ref = vk::AttachmentReference::default();
        color_attachment_from_first_pass_as_input_ref.layout =
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

        let mut multi_sampling_color_attachment_in_second_pass_ref =
            vk::AttachmentReference::default();
        multi_sampling_color_attachment_in_second_pass_ref.layout =
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        let mut stencil_buffer_in_second_pass_ref = vk::AttachmentReference::default();
        stencil_buffer_in_second_pass_ref.layout =
            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL;

        let mut attachments: [vk::AttachmentDescription; 6] = Default::default();
        let mut attachment_count = 0;
        attachments[attachment_count] = color_attachment;
        color_attachment_ref.attachment = attachment_count as u32;
        attachment_count += 1;
        if has_multi_sampling_targets {
            attachments[attachment_count] = multi_sampling_color_attachment;
            multi_sampling_color_attachment_ref.attachment = attachment_count as u32;
            attachment_count += 1;
        }
        if double_pass {
            attachments[attachment_count] = color_attachment_in_second_pass;
            color_attachment_in_second_pass_ref.attachment = attachment_count as u32;
            color_attachment_from_first_pass_as_input_ref.attachment = attachment_count as u32;
            attachment_count += 1;
            attachments[attachment_count] = stencil_attachment;
            stencil_buffer_in_second_pass_ref.attachment = attachment_count as u32;
            attachment_count += 1;
            if has_multi_sampling_in_second_pass_targets {
                attachments[attachment_count] = multi_sampling_color_attachment_in_second_pass;
                multi_sampling_color_attachment_in_second_pass_ref.attachment =
                    attachment_count as u32;
                attachment_count += 1;
            }
        }

        let mut subpasses = [
            vk::SubpassDescription::default(),
            vk::SubpassDescription::default(),
        ];
        subpasses[0].pipeline_bind_point = vk::PipelineBindPoint::GRAPHICS;
        subpasses[0].color_attachment_count = 1;
        subpasses[0].p_color_attachments = if has_multi_sampling_targets {
            &multi_sampling_color_attachment_ref
        } else {
            &color_attachment_ref
        };
        subpasses[0].p_resolve_attachments = if has_multi_sampling_targets {
            &color_attachment_ref
        } else {
            std::ptr::null()
        };

        // double pass
        subpasses[1] = subpasses[0].clone();
        if double_pass {
            subpasses[0].p_color_attachments = if has_multi_sampling_targets {
                &multi_sampling_color_attachment_in_second_pass_ref
            } else {
                &color_attachment_in_second_pass_ref
            };
            subpasses[0].p_resolve_attachments = if has_multi_sampling_targets {
                &color_attachment_in_second_pass_ref
            } else {
                std::ptr::null()
            };

            subpasses[1].input_attachment_count = 1;
            subpasses[1].p_input_attachments = &color_attachment_from_first_pass_as_input_ref;
            subpasses[1].p_depth_stencil_attachment = &stencil_buffer_in_second_pass_ref;
            subpasses[1].p_color_attachments = if has_multi_sampling_in_second_pass_targets {
                &multi_sampling_color_attachment_ref
            } else {
                &color_attachment_ref
            };
            subpasses[1].p_resolve_attachments = if has_multi_sampling_in_second_pass_targets {
                &color_attachment_ref
            } else {
                std::ptr::null()
            };
        }

        let mut dependencies = [
            vk::SubpassDependency::default(),
            vk::SubpassDependency::default(),
        ];
        dependencies[0].src_subpass = vk::SUBPASS_EXTERNAL;
        dependencies[0].dst_subpass = 0;
        dependencies[0].src_stage_mask = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
        dependencies[0].src_access_mask = vk::AccessFlags::empty();
        dependencies[0].dst_stage_mask = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
        dependencies[0].dst_access_mask = vk::AccessFlags::COLOR_ATTACHMENT_WRITE;
        dependencies[0].dependency_flags = vk::DependencyFlags::BY_REGION;

        // for multiple passes
        dependencies[1].src_subpass = 0;
        dependencies[1].dst_subpass = 1;
        dependencies[1].src_stage_mask = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
        dependencies[1].src_access_mask = vk::AccessFlags::COLOR_ATTACHMENT_WRITE;
        dependencies[1].dst_stage_mask = vk::PipelineStageFlags::FRAGMENT_SHADER;
        dependencies[1].dst_access_mask = vk::AccessFlags::SHADER_READ;
        dependencies[1].dependency_flags = vk::DependencyFlags::BY_REGION;

        let mut create_render_pass_info = vk::RenderPassCreateInfo::default();
        create_render_pass_info.attachment_count = attachment_count as u32;
        create_render_pass_info.p_attachments = attachments.as_ptr();
        create_render_pass_info.subpass_count = if double_pass { 2 } else { 1 };
        create_render_pass_info.p_subpasses = subpasses.as_ptr();
        create_render_pass_info.dependency_count = if double_pass {
            dependencies.len() as u32
        } else {
            1
        };
        create_render_pass_info.p_dependencies = dependencies.as_ptr();

        let res = unsafe {
            self.ash_vk
                .vk_device
                .create_render_pass(&create_render_pass_info, None)
        };
        match res {
            Ok(res) => Ok(res),
            Err(err) => Err(anyhow!(format!("Creating the render pass failed: {}", err))),
        }
    }

    #[must_use]
    fn create_render_pass(&mut self, clear_attachs: bool) -> bool {
        let has_multi_sampling = self.has_multi_sampling();
        let has_multi_sampling_in_second_pass = self.has_multi_sampling_in_second_pass();
        match self.create_render_pass_impl(
            clear_attachs,
            has_multi_sampling,
            has_multi_sampling_in_second_pass,
            self.vk_surf_format.format,
            false,
        ) {
            Ok(render_pass) => {
                self.vk_render_pass_single_pass = render_pass;
                true
            }
            Err(err) => {
                self.error
                    .lock()
                    .unwrap()
                    .set_error(EGFXErrorType::Init, &err.to_string());
                false
            }
        }
    }

    #[must_use]
    fn create_render_pass_double(&mut self, clear_attachs: bool) -> bool {
        let has_multi_sampling = self.has_multi_sampling();
        let has_multi_sampling_in_second_pass = self.has_multi_sampling_in_second_pass();
        match self.create_render_pass_impl(
            clear_attachs,
            has_multi_sampling,
            has_multi_sampling_in_second_pass,
            self.vk_surf_format.format,
            true,
        ) {
            Ok(render_pass) => {
                self.vk_render_pass_double_pass = render_pass;
                true
            }
            Err(err) => {
                self.error
                    .lock()
                    .unwrap()
                    .set_error(EGFXErrorType::Init, &err.to_string());
                false
            }
        }
    }

    fn destroy_render_pass(&mut self) {
        unsafe {
            self.ash_vk
                .vk_device
                .destroy_render_pass(self.vk_render_pass_single_pass, None);
        }
    }

    fn destroy_render_pass_double_pass(&mut self) {
        unsafe {
            self.ash_vk
                .vk_device
                .destroy_render_pass(self.vk_render_pass_double_pass, None);
        }
    }

    #[must_use]
    fn create_framebuffers_impl(&mut self, double_pass: bool) -> bool {
        let has_multi_sampling_in_second_pass_targets = self.has_multi_sampling_in_second_pass();
        let has_multi_sampling_targets = self.has_multi_sampling();
        let framebuffer_list = if double_pass {
            &mut self.framebuffer_double_pass_list
        } else {
            &mut self.framebuffer_list
        };
        framebuffer_list.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );

        for i in 0..self.device.swap_chain_image_count {
            let mut attachments: [vk::ImageView; 5] = Default::default();
            let mut attachment_count = 0;
            attachments[attachment_count] = self.swap_chain_image_view_list[i as usize];
            attachment_count += 1;
            if has_multi_sampling_targets {
                attachments[attachment_count] =
                    self.swap_chain_multi_sampling_images[i as usize].img_view;
                attachment_count += 1;
            }
            if double_pass {
                attachments[attachment_count] =
                    self.image_list_for_double_pass[i as usize].img_view;
                attachment_count += 1;
                attachments[attachment_count] =
                    self.stencil_list_for_pass_transition[i as usize].img_view;
                attachment_count += 1;
                if has_multi_sampling_in_second_pass_targets {
                    attachments[attachment_count] =
                        self.multi_sampling_images_for_double_pass[i as usize].img_view;
                    attachment_count += 1;
                }
            } else {
            }

            let mut framebuffer_info = vk::FramebufferCreateInfo::default();
            framebuffer_info.render_pass = if double_pass {
                self.vk_render_pass_double_pass
            } else {
                self.vk_render_pass_single_pass
            };
            framebuffer_info.attachment_count = attachment_count as u32;
            framebuffer_info.p_attachments = attachments.as_ptr();
            framebuffer_info.width = self
                .vk_swap_img_and_viewport_extent
                .swap_image_viewport
                .width;
            framebuffer_info.height = self
                .vk_swap_img_and_viewport_extent
                .swap_image_viewport
                .height;
            framebuffer_info.layers = 1;

            let res = unsafe {
                self.ash_vk
                    .vk_device
                    .create_framebuffer(&framebuffer_info, None)
            };
            if res.is_err() {
                self.error
                    .lock()
                    .unwrap()
                    .set_error(EGFXErrorType::Init, "Creating the framebuffers failed.");
                return false;
            }
            framebuffer_list[i as usize] = res.unwrap();
        }

        return true;
    }

    fn create_framebuffers(&mut self) -> bool {
        self.create_framebuffers_impl(false)
    }

    fn create_framebuffers_double_pass(&mut self) -> bool {
        self.create_framebuffers_impl(true)
    }

    fn destroy_framebuffers(&mut self) {
        for frame_buffer in &mut self.framebuffer_list {
            unsafe {
                self.ash_vk
                    .vk_device
                    .destroy_framebuffer(*frame_buffer, None);
            }
        }

        self.framebuffer_list.clear();
    }

    fn destroy_framebuffers_double_pass(&mut self) {
        for frame_buffer in &mut self.framebuffer_double_pass_list {
            unsafe {
                self.ash_vk
                    .vk_device
                    .destroy_framebuffer(*frame_buffer, None);
            }
        }

        self.framebuffer_double_pass_list.clear();
    }

    #[must_use]
    fn create_shader_module(
        &mut self,
        code: &Vec<u8>,
        shader_module: &mut vk::ShaderModule,
    ) -> bool {
        let mut create_info = vk::ShaderModuleCreateInfo::default();
        create_info.code_size = code.len();
        create_info.p_code = code.as_ptr() as _;

        let res = unsafe {
            self.ash_vk
                .vk_device
                .create_shader_module(&create_info, None)
        };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .set_error(EGFXErrorType::Init, "Shader module was not created.");
            return false;
        }
        *shader_module = res.unwrap();

        return true;
    }

    #[must_use]
    fn create_descriptor_set_layouts(&mut self) -> bool {
        let mut sampler_layout_binding = vk::DescriptorSetLayoutBinding::default();
        sampler_layout_binding.binding = 0;
        sampler_layout_binding.descriptor_count = 1;
        sampler_layout_binding.descriptor_type = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        sampler_layout_binding.p_immutable_samplers = std::ptr::null();
        sampler_layout_binding.stage_flags = vk::ShaderStageFlags::FRAGMENT;

        let layout_bindings = [sampler_layout_binding];

        let mut layout_info = vk::DescriptorSetLayoutCreateInfo::default();
        layout_info.binding_count = layout_bindings.len() as u32;
        layout_info.p_bindings = layout_bindings.as_ptr();

        let res = unsafe {
            self.ash_vk
                .vk_device
                .create_descriptor_set_layout(&layout_info, None)
        };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .set_error(EGFXErrorType::Init, "Creating descriptor layout failed.");
            return false;
        }
        self.device.standard_textured_descriptor_set_layout = res.unwrap();

        let res = unsafe {
            self.ash_vk
                .vk_device
                .create_descriptor_set_layout(&layout_info, None)
        };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .set_error(EGFXErrorType::Init, "Creating descriptor layout failed.");
            return false;
        }
        self.device.standard_3d_textured_descriptor_set_layout = res.unwrap();
        return true;
    }

    fn destroy_descriptor_set_layouts(&mut self) {
        unsafe {
            self.ash_vk.vk_device.destroy_descriptor_set_layout(
                self.device.standard_textured_descriptor_set_layout,
                None,
            );
        }
        unsafe {
            self.ash_vk.vk_device.destroy_descriptor_set_layout(
                self.device.standard_3d_textured_descriptor_set_layout,
                None,
            );
        }
    }

    #[must_use]
    fn load_shader(&mut self, file_name: &str) -> Result<Vec<u8>, ArrayString<4096>> {
        let it = self.shader_files.get(file_name);
        if let Some(f) = it {
            Ok(f.binary.clone())
        } else {
            let mut res = ArrayString::from_str("Shader file was not loaded: ").unwrap();
            res.push_str(file_name);
            Err(res)
        }
    }

    #[must_use]
    fn create_shaders(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        shader_stages: &mut [vk::PipelineShaderStageCreateInfo; 2],
        shader_module: &mut SShaderModule,
    ) -> bool {
        let shader_loaded: bool = true;

        let vert_data_buff = self.load_shader(vert_name).unwrap();
        let frag_data_buff = self.load_shader(frag_name).unwrap();

        shader_module.vk_device = self.ash_vk.vk_device.clone();

        if !shader_loaded {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::Init,
                "A shader file could not load correctly.",
            );
            return false;
        }

        if !self.create_shader_module(&vert_data_buff, &mut shader_module.vert_shader_module) {
            return false;
        }

        if !self.create_shader_module(&frag_data_buff, &mut shader_module.frag_shader_module) {
            return false;
        }

        let vert_shader_stage_info = &mut shader_stages[0];
        *vert_shader_stage_info = vk::PipelineShaderStageCreateInfo::default();
        vert_shader_stage_info.stage = vk::ShaderStageFlags::VERTEX;
        vert_shader_stage_info.module = shader_module.vert_shader_module;
        vert_shader_stage_info.p_name = SHADER_MAIN_FUNC_NAME.as_ptr() as *const i8;

        let frag_shader_stage_info = &mut shader_stages[1];
        *frag_shader_stage_info = vk::PipelineShaderStageCreateInfo::default();
        frag_shader_stage_info.stage = vk::ShaderStageFlags::FRAGMENT;
        frag_shader_stage_info.module = shader_module.frag_shader_module;
        frag_shader_stage_info.p_name = SHADER_MAIN_FUNC_NAME.as_ptr() as *const i8;
        return true;
    }

    fn get_standard_pipeline_info(
        &mut self,
        input_assembly: &mut vk::PipelineInputAssemblyStateCreateInfo,
        viewport: &mut vk::Viewport,
        scissor: &mut vk::Rect2D,
        viewport_state: &mut vk::PipelineViewportStateCreateInfo,
        rasterizer: &mut vk::PipelineRasterizationStateCreateInfo,
        multisampling: &mut vk::PipelineMultisampleStateCreateInfo,
        color_blend_attachment: &mut vk::PipelineColorBlendAttachmentState,
        color_blending: &mut vk::PipelineColorBlendStateCreateInfo,
        blend_mode: EVulkanBackendBlendModes,
        stencil_only: StencilOpType,
    ) -> bool {
        input_assembly.topology = vk::PrimitiveTopology::TRIANGLE_LIST;
        input_assembly.primitive_restart_enable = vk::FALSE;

        viewport.x = 0.0;
        viewport.y = 0.0;
        viewport.width = self
            .vk_swap_img_and_viewport_extent
            .swap_image_viewport
            .width as f32;
        viewport.height = self
            .vk_swap_img_and_viewport_extent
            .swap_image_viewport
            .height as f32;
        viewport.min_depth = 0.0;
        viewport.max_depth = 1.0;

        scissor.offset = vk::Offset2D { x: 0, y: 0 };
        scissor.extent = self.vk_swap_img_and_viewport_extent.swap_image_viewport;

        viewport_state.viewport_count = 1;
        viewport_state.p_viewports = viewport;
        viewport_state.scissor_count = 1;
        viewport_state.p_scissors = scissor;

        rasterizer.depth_clamp_enable = vk::FALSE;
        rasterizer.rasterizer_discard_enable = vk::FALSE;
        rasterizer.polygon_mode = vk::PolygonMode::FILL;
        rasterizer.line_width = 1.0;
        rasterizer.cull_mode = vk::CullModeFlags::NONE;
        rasterizer.front_face = vk::FrontFace::CLOCKWISE;
        rasterizer.depth_bias_enable = vk::FALSE;

        multisampling.sample_shading_enable = vk::FALSE;
        multisampling.rasterization_samples =
            Device::get_sample_count(self.device.config.multi_sampling_count, &self.device.limits);

        color_blend_attachment.color_write_mask = if let StencilOpType::AlwaysPass = stencil_only {
            vk::ColorComponentFlags::empty()
        } else {
            vk::ColorComponentFlags::R
                | vk::ColorComponentFlags::G
                | vk::ColorComponentFlags::B
                | vk::ColorComponentFlags::A
        };

        color_blend_attachment.blend_enable = if blend_mode == EVulkanBackendBlendModes::None {
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

        color_blend_attachment.src_color_blend_factor = src_blend_factor_color;
        color_blend_attachment.dst_color_blend_factor = dst_blend_factor_color;
        color_blend_attachment.color_blend_op = vk::BlendOp::ADD;
        color_blend_attachment.src_alpha_blend_factor = src_blend_factor_alpha;
        color_blend_attachment.dst_alpha_blend_factor = dst_blend_factor_alpha;
        color_blend_attachment.alpha_blend_op = vk::BlendOp::ADD;

        color_blending.logic_op_enable = vk::FALSE;
        color_blending.logic_op = vk::LogicOp::COPY;
        color_blending.attachment_count = 1;
        color_blending.p_attachments = color_blend_attachment;
        color_blending.blend_constants[0] = 0.0;
        color_blending.blend_constants[1] = 0.0;
        color_blending.blend_constants[2] = 0.0;
        color_blending.blend_constants[3] = 0.0;

        return true;
    }

    #[must_use]
    fn create_graphics_pipeline_ex<const FORCE_REQUIRE_DESCRIPTORS: bool>(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut SPipelineContainer,
        stride: u32,
        input_attributes: &mut [vk::VertexInputAttributeDescription],
        set_layouts: &mut [vk::DescriptorSetLayout],
        push_constants: &[vk::PushConstantRange],
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        is_line_prim: bool,
        render_pass_type: RenderPassType,
        stencil_type: StencilOpType,
        sub_pass_index: usize,
    ) -> bool {
        let mut shader_stages: [vk::PipelineShaderStageCreateInfo; 2] = Default::default();
        let mut module = SShaderModule::new(&self.ash_vk.vk_device);
        if !self.create_shaders(vert_name, frag_name, &mut shader_stages, &mut module) {
            return false;
        }

        let has_sampler: bool = tex_mode == EVulkanBackendTextureModes::Textured;

        let mut vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default();
        let mut binding_description = vk::VertexInputBindingDescription::default();
        binding_description.binding = 0;
        binding_description.stride = stride;
        binding_description.input_rate = vk::VertexInputRate::VERTEX;

        vertex_input_info.vertex_binding_description_count = 1;
        vertex_input_info.vertex_attribute_description_count = input_attributes.len() as u32;
        vertex_input_info.p_vertex_binding_descriptions = &binding_description;
        vertex_input_info.p_vertex_attribute_descriptions = input_attributes.as_ptr();

        let mut input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default();
        let mut viewport = vk::Viewport::default();
        let mut scissor = vk::Rect2D::default();
        let mut viewport_state = vk::PipelineViewportStateCreateInfo::default();
        let mut rasterizer = vk::PipelineRasterizationStateCreateInfo::default();
        let mut multisampling = vk::PipelineMultisampleStateCreateInfo::default();
        let mut color_blend_attachment = vk::PipelineColorBlendAttachmentState::default();
        let mut color_blending = vk::PipelineColorBlendStateCreateInfo::default();
        let mut stencil_state = vk::PipelineDepthStencilStateCreateInfo::default();

        self.get_standard_pipeline_info(
            &mut input_assembly,
            &mut viewport,
            &mut scissor,
            &mut viewport_state,
            &mut rasterizer,
            &mut multisampling,
            &mut color_blend_attachment,
            &mut color_blending,
            blend_mode,
            stencil_type,
        );
        input_assembly.topology = if is_line_prim {
            vk::PrimitiveTopology::LINE_LIST
        } else {
            vk::PrimitiveTopology::TRIANGLE_LIST
        };

        let mut pipeline_layout_info = vk::PipelineLayoutCreateInfo::default();
        pipeline_layout_info.set_layout_count = if has_sampler || FORCE_REQUIRE_DESCRIPTORS {
            set_layouts.len() as u32
        } else {
            0
        };
        pipeline_layout_info.p_set_layouts =
            if (has_sampler || FORCE_REQUIRE_DESCRIPTORS) && !set_layouts.is_empty() {
                set_layouts.as_ptr()
            } else {
                std::ptr::null()
            };

        pipeline_layout_info.push_constant_range_count = push_constants.len() as u32;
        pipeline_layout_info.p_push_constant_ranges = if !push_constants.is_empty() {
            push_constants.as_ptr()
        } else {
            std::ptr::null()
        };

        let (pipeline, pipe_layout) = Self::get_pipeline_and_layout_mut(
            pipe_container,
            has_sampler,
            blend_mode as usize,
            (dynamic_mode) as usize,
            render_pass_type,
            sub_pass_index,
        );

        let res = unsafe {
            self.ash_vk
                .vk_device
                .create_pipeline_layout(&pipeline_layout_info, None)
        };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .set_error(EGFXErrorType::Init, "Creating pipeline layout failed.");
            return false;
        }
        *pipe_layout = res.unwrap();

        let mut pipeline_info = vk::GraphicsPipelineCreateInfo::default();
        pipeline_info.stage_count = shader_stages.len() as u32;
        pipeline_info.p_stages = shader_stages.as_ptr();
        pipeline_info.p_vertex_input_state = &vertex_input_info;
        pipeline_info.p_input_assembly_state = &input_assembly;
        pipeline_info.p_viewport_state = &viewport_state;
        pipeline_info.p_rasterization_state = &rasterizer;
        pipeline_info.p_multisample_state = &multisampling;
        pipeline_info.p_color_blend_state = &color_blending;
        match stencil_type {
            StencilOpType::AlwaysPass => {
                stencil_state.stencil_test_enable = vk::TRUE;
                stencil_state.front.compare_op = vk::CompareOp::ALWAYS;
                stencil_state.front.fail_op = vk::StencilOp::REPLACE;
                stencil_state.front.pass_op = vk::StencilOp::REPLACE;
                stencil_state.front.depth_fail_op = vk::StencilOp::REPLACE;
                stencil_state.front.compare_mask = 0xFF;
                stencil_state.front.write_mask = 0xFF;
                stencil_state.front.reference = 0x1;
                stencil_state.back = stencil_state.front;
                pipeline_info.p_depth_stencil_state = &stencil_state;
            }
            StencilOpType::OnlyWhenPassed => {
                stencil_state.stencil_test_enable = vk::TRUE;
                stencil_state.front.compare_op = vk::CompareOp::EQUAL;
                stencil_state.front.fail_op = vk::StencilOp::KEEP;
                stencil_state.front.pass_op = vk::StencilOp::KEEP;
                stencil_state.front.depth_fail_op = vk::StencilOp::KEEP;
                stencil_state.front.compare_mask = 0xFF;
                stencil_state.front.write_mask = 0xFF;
                stencil_state.front.reference = 0x1;
                stencil_state.back = stencil_state.front;
                pipeline_info.p_depth_stencil_state = &stencil_state;
            }
            StencilOpType::OnlyWhenNotPassed => {
                stencil_state.stencil_test_enable = vk::TRUE;
                stencil_state.front.compare_op = vk::CompareOp::NOT_EQUAL;
                stencil_state.front.fail_op = vk::StencilOp::KEEP;
                stencil_state.front.pass_op = vk::StencilOp::KEEP;
                stencil_state.front.depth_fail_op = vk::StencilOp::KEEP;
                stencil_state.front.compare_mask = 0xFF;
                stencil_state.front.write_mask = 0xFF;
                stencil_state.front.reference = 0x1;
                stencil_state.back = stencil_state.front;
                pipeline_info.p_depth_stencil_state = &stencil_state;
            }
            StencilOpType::None => {
                // nothing to do
                if let RenderPassType::Dual = render_pass_type {
                    stencil_state.stencil_test_enable = vk::FALSE;
                    pipeline_info.p_depth_stencil_state = &stencil_state;
                }
            }
        }
        pipeline_info.layout = *pipe_layout;
        pipeline_info.render_pass = match render_pass_type {
            RenderPassType::Single => self.vk_render_pass_single_pass,
            RenderPassType::Dual => self.vk_render_pass_double_pass,
        };
        pipeline_info.subpass = match render_pass_type {
            RenderPassType::Single => 0,
            RenderPassType::Dual => sub_pass_index as u32,
        };
        pipeline_info.base_pipeline_handle = vk::Pipeline::null();

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        let mut dynamic_state_create = vk::PipelineDynamicStateCreateInfo::default();
        dynamic_state_create.dynamic_state_count = dynamic_states.len() as u32;
        dynamic_state_create.p_dynamic_states = dynamic_states.as_ptr();

        if dynamic_mode == EVulkanBackendClipModes::DynamicScissorAndViewport {
            pipeline_info.p_dynamic_state = &dynamic_state_create;
        }

        let res = unsafe {
            self.ash_vk.vk_device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[pipeline_info],
                None,
            )
        };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .set_error(EGFXErrorType::Init, "Creating the graphic pipeline failed.");
            return false;
        }
        *pipeline = res.unwrap()[0]; // TODO correct?

        return true;
    }

    #[must_use]
    fn create_graphics_pipeline<const FORCE_REQUIRE_DESCRIPTORS: bool>(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut SPipelineContainer,
        stride: u32,
        input_attributes: &mut [vk::VertexInputAttributeDescription],
        set_layouts: &mut [vk::DescriptorSetLayout],
        push_constants: &mut [vk::PushConstantRange],
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
        stencil_only: StencilOpType,
        sub_pass_index: usize,
    ) -> bool {
        return self.create_graphics_pipeline_ex::<{ FORCE_REQUIRE_DESCRIPTORS }>(
            vert_name,
            frag_name,
            pipe_container,
            stride,
            input_attributes,
            set_layouts,
            push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            false,
            render_pass_type,
            stencil_only,
            sub_pass_index,
        );
    }

    #[must_use]
    fn create_standard_graphics_pipeline_impl(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut SPipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        is_line_prim: bool,
        render_pass_type: RenderPassType,
        stencil_only: StencilOpType,
        sub_pass_index: usize,
    ) -> bool {
        let mut attribute_descriptors: [vk::VertexInputAttributeDescription; 3] =
            Default::default();

        attribute_descriptors[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        attribute_descriptors[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: (std::mem::size_of::<f32>() * 2) as u32,
        };
        attribute_descriptors[2] = vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * (2 + 2)) as u32,
        };

        let mut set_layouts = [self.device.standard_textured_descriptor_set_layout];

        let push_constants: &[vk::PushConstantRange] = if let StencilOpType::OnlyWhenPassed
        | StencilOpType::OnlyWhenNotPassed =
            stencil_only
        {
            &[]
        } else {
            &[vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX,
                offset: 0,
                size: std::mem::size_of::<SUniformGPos>() as u32,
            }]
        };

        return self.create_graphics_pipeline_ex::<false>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &mut attribute_descriptors,
            &mut set_layouts,
            push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            is_line_prim,
            render_pass_type,
            stencil_only,
            sub_pass_index,
        );
    }

    #[must_use]
    fn create_standard_graphics_pipeline(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        has_sampler: bool,
        is_line_pipe: bool,
        stencil_only: StencilOpType,
    ) -> bool {
        let mut ret: bool = true;

        let tex_mode = if has_sampler {
            EVulkanBackendTextureModes::Textured
        } else {
            EVulkanBackendTextureModes::NotTextured
        };

        let mut pipe_container = if is_line_pipe {
            self.standard_line_pipeline.clone()
        } else {
            match stencil_only {
                StencilOpType::AlwaysPass => self.standard_stencil_only_pipeline.clone(),
                StencilOpType::OnlyWhenPassed | StencilOpType::OnlyWhenNotPassed => {
                    self.standard_stencil_pipeline.clone()
                }
                StencilOpType::None => self.standard_pipeline.clone(),
            }
        };
        for s in 0..MAX_SUB_PASS_COUNT {
            for n in 0..RENDER_PASS_TYPE_COUNT {
                for i in 0..EVulkanBackendBlendModes::Count as usize {
                    for j in 0..EVulkanBackendClipModes::Count as usize {
                        ret &= self.create_standard_graphics_pipeline_impl(
                            vert_name,
                            frag_name,
                            &mut pipe_container,
                            tex_mode,
                            EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                            EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                            is_line_pipe,
                            RenderPassType::from_u32(n as u32).unwrap(),
                            stencil_only,
                            s,
                        );
                    }
                }
            }
        }

        let cont = if is_line_pipe {
            &mut self.standard_line_pipeline
        } else {
            match stencil_only {
                StencilOpType::AlwaysPass => &mut self.standard_stencil_only_pipeline,
                StencilOpType::OnlyWhenPassed | StencilOpType::OnlyWhenNotPassed => {
                    &mut self.standard_stencil_pipeline
                }
                StencilOpType::None => &mut self.standard_pipeline,
            }
        };
        *cont = pipe_container;

        return ret;
    }

    #[must_use]
    fn create_standard_3d_graphics_pipeline_impl(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut SPipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
        sub_pass_index: usize,
    ) -> bool {
        let mut attribute_descriptors: [vk::VertexInputAttributeDescription; 3] =
            Default::default();

        attribute_descriptors[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        attribute_descriptors[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * 2) as u32,
        };
        attribute_descriptors[2] = vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: (std::mem::size_of::<f32>() * 2 + std::mem::size_of::<u8>() * 4) as u32,
        };

        let mut set_layouts = [self.device.standard_3d_textured_descriptor_set_layout];

        let mut push_constants = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: std::mem::size_of::<SUniformGPos>() as u32,
        }];

        return self.create_graphics_pipeline::<false>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * 2
                + std::mem::size_of::<u8>() * 4
                + std::mem::size_of::<f32>() * 3) as u32,
            &mut attribute_descriptors,
            &mut set_layouts,
            &mut push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
            sub_pass_index,
        );
    }

    #[must_use]
    fn create_standard_3d_graphics_pipeline(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        has_sampler: bool,
    ) -> bool {
        let mut ret: bool = true;

        let tex_mode = if has_sampler {
            EVulkanBackendTextureModes::Textured
        } else {
            EVulkanBackendTextureModes::NotTextured
        };

        let mut pipe_container = self.standard_3d_pipeline.clone();
        for s in 0..MAX_SUB_PASS_COUNT {
            for n in 0..RENDER_PASS_TYPE_COUNT {
                for i in 0..EVulkanBackendBlendModes::Count as usize {
                    for j in 0..EVulkanBackendClipModes::Count as usize {
                        ret &= self.create_standard_3d_graphics_pipeline_impl(
                            vert_name,
                            frag_name,
                            &mut pipe_container,
                            tex_mode,
                            EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                            EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                            RenderPassType::from_u32(n as u32).unwrap(),
                            s,
                        );
                    }
                }
            }
        }
        self.standard_3d_pipeline = pipe_container;

        return ret;
    }

    #[must_use]
    fn create_blur_graphics_pipeline_impl(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut SPipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
        sub_pass_index: usize,
    ) -> bool {
        let mut attribute_descriptors: [vk::VertexInputAttributeDescription; 2] =
            Default::default();

        attribute_descriptors[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        attribute_descriptors[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: (std::mem::size_of::<f32>() * 2) as u32,
        };

        let mut set_layouts = [self.device.standard_textured_descriptor_set_layout];

        let mut push_constants = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: std::mem::size_of::<SUniformGBlur>() as u32,
        }];

        return self.create_graphics_pipeline_ex::<false>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &mut attribute_descriptors,
            &mut set_layouts,
            &mut push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            false,
            render_pass_type,
            StencilOpType::OnlyWhenPassed,
            sub_pass_index,
        );
    }

    #[must_use]
    fn create_blur_graphics_pipeline(&mut self, vert_name: &str, frag_name: &str) -> bool {
        let mut ret: bool = true;

        let tex_mode = EVulkanBackendTextureModes::Textured;

        let mut pipe_container = self.blur_pipeline.clone();
        for s in 0..MAX_SUB_PASS_COUNT {
            for n in 0..RENDER_PASS_TYPE_COUNT {
                for i in 0..EVulkanBackendBlendModes::Count as usize {
                    for j in 0..EVulkanBackendClipModes::Count as usize {
                        ret &= self.create_blur_graphics_pipeline_impl(
                            vert_name,
                            frag_name,
                            &mut pipe_container,
                            tex_mode,
                            EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                            EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                            RenderPassType::from_u32(n as u32).unwrap(),
                            s,
                        );
                    }
                }
            }
        }

        self.blur_pipeline = pipe_container;

        return ret;
    }

    #[must_use]
    fn create_text_descriptor_set_layout(&mut self) -> bool {
        let mut sampler_layout_binding = vk::DescriptorSetLayoutBinding::default();
        sampler_layout_binding.binding = 0;
        sampler_layout_binding.descriptor_count = 1;
        sampler_layout_binding.descriptor_type = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        sampler_layout_binding.p_immutable_samplers = std::ptr::null();
        sampler_layout_binding.stage_flags = vk::ShaderStageFlags::FRAGMENT;

        let mut sampler_layout_binding2 = sampler_layout_binding.clone();
        sampler_layout_binding2.binding = 1;

        let layout_bindings = [sampler_layout_binding, sampler_layout_binding2];
        let mut layout_info = vk::DescriptorSetLayoutCreateInfo::default();
        layout_info.binding_count = layout_bindings.len() as u32;
        layout_info.p_bindings = layout_bindings.as_ptr();

        let res = unsafe {
            self.ash_vk
                .vk_device
                .create_descriptor_set_layout(&layout_info, None)
        };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .set_error(EGFXErrorType::Init, "Creating descriptor layout failed.");
            return false;
        }
        self.device.text_descriptor_set_layout = res.unwrap();

        return true;
    }

    fn destroy_text_descriptor_set_layout(&mut self) {
        unsafe {
            self.ash_vk
                .vk_device
                .destroy_descriptor_set_layout(self.device.text_descriptor_set_layout, None);
        }
    }

    #[must_use]
    fn create_text_graphics_pipeline_impl(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut SPipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
        sub_pass_index: usize,
    ) -> bool {
        let mut attribute_descriptors: [vk::VertexInputAttributeDescription; 3] =
            Default::default();
        attribute_descriptors[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        attribute_descriptors[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: (std::mem::size_of::<f32>() * 2) as u32,
        };
        attribute_descriptors[2] = vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * (2 + 2)) as u32,
        };

        let mut set_layouts = [self.device.text_descriptor_set_layout];

        let mut push_constants = [
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

        return self.create_graphics_pipeline::<false>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &mut attribute_descriptors,
            &mut set_layouts,
            &mut push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
            sub_pass_index,
        );
    }

    #[must_use]
    fn create_text_graphics_pipeline(&mut self, vert_name: &str, frag_name: &str) -> bool {
        let mut ret: bool = true;

        let tex_mode = EVulkanBackendTextureModes::Textured;

        let mut pipe_container = self.text_pipeline.clone();
        for s in 0..MAX_SUB_PASS_COUNT {
            for n in 0..RENDER_PASS_TYPE_COUNT {
                for i in 0..EVulkanBackendBlendModes::Count as usize {
                    for j in 0..EVulkanBackendClipModes::Count as usize {
                        ret &= self.create_text_graphics_pipeline_impl(
                            vert_name,
                            frag_name,
                            &mut pipe_container,
                            tex_mode,
                            EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                            EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                            RenderPassType::from_u32(n as u32).unwrap(),
                            s,
                        );
                    }
                }
            }
        }
        self.text_pipeline = pipe_container;

        return ret;
    }

    #[must_use]
    fn create_tile_graphics_pipeline_impl<const HAS_SAMPLER: bool>(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_type: i32, // TODO: use a type instead
        pipe_container: &mut SPipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
        sub_pass_index: usize,
    ) -> bool {
        let mut attribute_descriptors: [vk::VertexInputAttributeDescription; 2] =
            Default::default();
        attribute_descriptors[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        if HAS_SAMPLER {
            attribute_descriptors[1] = vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: (std::mem::size_of::<f32>() * 2) as u32,
            };
        }

        let mut set_layouts = [self.device.standard_3d_textured_descriptor_set_layout];

        let mut vert_push_constant_size = std::mem::size_of::<SUniformTileGPos>();
        if pipe_type == 1 {
            vert_push_constant_size = std::mem::size_of::<SUniformTileGPosBorder>();
        } else if pipe_type == 2 {
            vert_push_constant_size = std::mem::size_of::<SUniformTileGPosBorderLine>();
        }

        let frag_push_constant_size = std::mem::size_of::<SUniformTileGVertColor>();

        let mut push_constants: [vk::PushConstantRange; 2] = Default::default();
        push_constants[0] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: vert_push_constant_size as u32,
        };
        push_constants[1] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            offset: (std::mem::size_of::<SUniformTileGPosBorder>()
                + std::mem::size_of::<SUniformTileGVertColorAlign>()) as u32,
            size: frag_push_constant_size as u32,
        };

        return self.create_graphics_pipeline::<false>(
            vert_name,
            frag_name,
            pipe_container,
            if HAS_SAMPLER {
                (std::mem::size_of::<f32>() * (2 + 3)) as u32
            } else {
                (std::mem::size_of::<f32>() * 2) as u32
            },
            &mut attribute_descriptors
                .split_at_mut(if HAS_SAMPLER { 2 } else { 1 })
                .0,
            &mut set_layouts,
            &mut push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
            sub_pass_index,
        );
    }

    #[must_use]
    fn create_tile_graphics_pipeline<const HAS_SAMPLER: bool>(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_type: i32, // TODO: use a type
    ) -> bool {
        let mut ret: bool = true;

        let tex_mode = if HAS_SAMPLER {
            EVulkanBackendTextureModes::Textured
        } else {
            EVulkanBackendTextureModes::NotTextured
        };

        let mut pipe_container = if pipe_type == 0 {
            self.tile_pipeline.clone()
        } else {
            if pipe_type == 1 {
                self.tile_border_pipeline.clone()
            } else {
                self.tile_border_line_pipeline.clone()
            }
        };
        for s in 0..MAX_SUB_PASS_COUNT {
            for n in 0..RENDER_PASS_TYPE_COUNT {
                for i in 0..EVulkanBackendBlendModes::Count as usize {
                    for j in 0..EVulkanBackendClipModes::Count as usize {
                        ret &= self.create_tile_graphics_pipeline_impl::<HAS_SAMPLER>(
                            vert_name,
                            frag_name,
                            pipe_type,
                            &mut pipe_container,
                            tex_mode,
                            EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                            EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                            RenderPassType::from_u32(n as u32).unwrap(),
                            s,
                        );
                    }
                }
            }
        }

        let cont = if pipe_type == 0 {
            &mut self.tile_pipeline
        } else {
            if pipe_type == 1 {
                &mut self.tile_border_pipeline
            } else {
                &mut self.tile_border_line_pipeline
            }
        };
        *cont = pipe_container;

        return ret;
    }

    #[must_use]
    fn create_prim_ex_graphics_pipeline_impl(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        rotationless: bool,
        pipe_container: &mut SPipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
        sub_pass_index: usize,
    ) -> bool {
        let mut attribute_descriptors: [vk::VertexInputAttributeDescription; 3] =
            Default::default();
        attribute_descriptors[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        attribute_descriptors[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: (std::mem::size_of::<f32>() * 2) as u32,
        };
        attribute_descriptors[2] = vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * (2 + 2)) as u32,
        };

        let mut set_layouts = [self.device.standard_textured_descriptor_set_layout];
        let mut vert_push_constant_size = std::mem::size_of::<SUniformPrimExGPos>();
        if rotationless {
            vert_push_constant_size = std::mem::size_of::<SUniformPrimExGPosRotationless>();
        }

        let frag_push_constant_size = std::mem::size_of::<SUniformPrimExGVertColor>();

        let mut push_constants: [vk::PushConstantRange; 2] = Default::default();
        push_constants[0] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: vert_push_constant_size as u32,
        };
        push_constants[1] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            offset: (std::mem::size_of::<SUniformPrimExGPos>()
                + std::mem::size_of::<SUniformPrimExGVertColorAlign>()) as u32,
            size: frag_push_constant_size as u32,
        };

        return self.create_graphics_pipeline::<false>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &mut attribute_descriptors,
            &mut set_layouts,
            &mut push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
            sub_pass_index,
        );
    }

    #[must_use]
    fn create_prim_ex_graphics_pipeline(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        has_sampler: bool,
        rotationless: bool,
    ) -> bool {
        let mut ret: bool = true;

        let tex_mode = if has_sampler {
            EVulkanBackendTextureModes::Textured
        } else {
            EVulkanBackendTextureModes::NotTextured
        };

        let mut pipe_container = if rotationless {
            self.prim_ex_rotationless_pipeline.clone()
        } else {
            self.prim_ex_pipeline.clone()
        };
        for s in 0..MAX_SUB_PASS_COUNT {
            for n in 0..RENDER_PASS_TYPE_COUNT {
                for i in 0..EVulkanBackendBlendModes::Count as usize {
                    for j in 0..EVulkanBackendClipModes::Count as usize {
                        ret &= self.create_prim_ex_graphics_pipeline_impl(
                            vert_name,
                            frag_name,
                            rotationless,
                            &mut pipe_container,
                            tex_mode,
                            EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                            EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                            RenderPassType::from_u32(n as u32).unwrap(),
                            s,
                        );
                    }
                }
            }
        }
        let cont = if rotationless {
            &mut self.prim_ex_rotationless_pipeline
        } else {
            &mut self.prim_ex_pipeline
        };
        *cont = pipe_container;

        return ret;
    }

    #[must_use]
    fn create_sprite_multi_graphics_pipeline_impl(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut SPipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
        sub_pass_index: usize,
    ) -> bool {
        let mut attribute_descriptors: [vk::VertexInputAttributeDescription; 3] =
            Default::default();
        attribute_descriptors[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        attribute_descriptors[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: (std::mem::size_of::<f32>() * 2) as u32,
        };
        attribute_descriptors[2] = vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * (2 + 2)) as u32,
        };

        let mut set_layouts = [
            self.device.standard_textured_descriptor_set_layout,
            self.device.sprite_multi_uniform_descriptor_set_layout,
        ];

        let vert_push_constant_size = std::mem::size_of::<SUniformSpriteMultiGPos>() as u32;
        let frag_push_constant_size = std::mem::size_of::<SUniformSpriteMultiGVertColor>() as u32;

        let mut push_constants: [vk::PushConstantRange; 2] = Default::default();
        push_constants[0] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: vert_push_constant_size,
        };
        push_constants[1] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            offset: (std::mem::size_of::<SUniformSpriteMultiGPos>()
                + std::mem::size_of::<SUniformSpriteMultiGVertColorAlign>())
                as u32,
            size: frag_push_constant_size,
        };

        return self.create_graphics_pipeline::<false>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &mut attribute_descriptors,
            &mut set_layouts,
            &mut push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
            sub_pass_index,
        );
    }

    #[must_use]
    fn create_sprite_multi_graphics_pipeline(&mut self, vert_name: &str, frag_name: &str) -> bool {
        let mut ret: bool = true;

        let tex_mode = EVulkanBackendTextureModes::Textured;

        let mut pipe_container = self.sprite_multi_pipeline.clone();
        for s in 0..MAX_SUB_PASS_COUNT {
            for n in 0..RENDER_PASS_TYPE_COUNT {
                for i in 0..EVulkanBackendBlendModes::Count as usize {
                    for j in 0..EVulkanBackendClipModes::Count as usize {
                        ret &= self.create_sprite_multi_graphics_pipeline_impl(
                            vert_name,
                            frag_name,
                            &mut pipe_container,
                            tex_mode,
                            EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                            EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                            RenderPassType::from_u32(n as u32).unwrap(),
                            s,
                        );
                    }
                }
            }
        }
        self.sprite_multi_pipeline = pipe_container;

        return ret;
    }

    #[must_use]
    fn create_sprite_multi_push_graphics_pipeline_impl(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut SPipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
        sub_pass_index: usize,
    ) -> bool {
        let mut attribute_descriptors: [vk::VertexInputAttributeDescription; 3] =
            Default::default();
        attribute_descriptors[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };
        attribute_descriptors[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: (std::mem::size_of::<f32>() * 2) as u32,
        };
        attribute_descriptors[2] = vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * (2 + 2)) as u32,
        };

        let mut set_layouts = [self.device.standard_textured_descriptor_set_layout];

        let vert_push_constant_size = std::mem::size_of::<SUniformSpriteMultiPushGPos>();
        let frag_push_constant_size = std::mem::size_of::<SUniformSpriteMultiPushGVertColor>();

        let mut push_constants: [vk::PushConstantRange; 2] = Default::default();
        push_constants[0] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: vert_push_constant_size as u32,
        };
        push_constants[1] = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            offset: (std::mem::size_of::<SUniformSpriteMultiPushGPos>()) as u32,
            size: frag_push_constant_size as u32,
        };

        return self.create_graphics_pipeline::<false>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &mut attribute_descriptors,
            &mut set_layouts,
            &mut push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
            sub_pass_index,
        );
    }

    #[must_use]
    fn create_sprite_multi_push_graphics_pipeline(
        &mut self,
        vert_name: &str,
        frag_name: &str,
    ) -> bool {
        let mut ret: bool = true;

        let tex_mode = EVulkanBackendTextureModes::Textured;

        let mut pipe_container = self.sprite_multi_push_pipeline.clone();
        for s in 0..MAX_SUB_PASS_COUNT {
            for n in 0..RENDER_PASS_TYPE_COUNT {
                for i in 0..EVulkanBackendBlendModes::Count as usize {
                    for j in 0..EVulkanBackendClipModes::Count as usize {
                        ret &= self.create_sprite_multi_push_graphics_pipeline_impl(
                            vert_name,
                            frag_name,
                            &mut pipe_container,
                            tex_mode,
                            EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                            EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                            RenderPassType::from_u32(n as u32).unwrap(),
                            s,
                        );
                    }
                }
            }
        }
        self.sprite_multi_push_pipeline = pipe_container;

        return ret;
    }

    #[must_use]
    fn create_quad_graphics_pipeline_impl<const IS_TEXTURED: bool>(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut SPipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
        sub_pass_index: usize,
    ) -> bool {
        let mut attribute_descriptors: [vk::VertexInputAttributeDescription; 3] =
            Default::default();
        attribute_descriptors[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32B32A32_SFLOAT,
            offset: 0,
        };
        attribute_descriptors[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * 4) as u32,
        };
        if IS_TEXTURED {
            attribute_descriptors[2] = vk::VertexInputAttributeDescription {
                location: 2,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: (std::mem::size_of::<f32>() * 4 + std::mem::size_of::<u8>() * 4) as u32,
            };
        }

        let mut set_layouts: [vk::DescriptorSetLayout; 2] = Default::default();
        if IS_TEXTURED {
            set_layouts[0] = self.device.standard_textured_descriptor_set_layout;
            set_layouts[1] = self.device.quad_uniform_descriptor_set_layout;
        } else {
            set_layouts[0] = self.device.quad_uniform_descriptor_set_layout;
        }

        let push_constant_size = std::mem::size_of::<SUniformQuadGPos>();

        let mut push_constants = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: push_constant_size as u32,
        }];

        return self.create_graphics_pipeline::<true>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * 4
                + std::mem::size_of::<u8>() * 4
                + (if IS_TEXTURED {
                    std::mem::size_of::<f32>() * 2
                } else {
                    0
                })) as u32,
            &mut attribute_descriptors
                .split_at_mut(if IS_TEXTURED { 3 } else { 2 })
                .0,
            &mut set_layouts.split_at_mut(if IS_TEXTURED { 2 } else { 1 }).0,
            &mut push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
            sub_pass_index,
        );
    }

    #[must_use]
    fn create_quad_graphics_pipeline<const HAS_SAMPLER: bool>(
        &mut self,
        vert_name: &str,
        frag_name: &str,
    ) -> bool {
        let mut ret: bool = true;

        let tex_mode = if HAS_SAMPLER {
            EVulkanBackendTextureModes::Textured
        } else {
            EVulkanBackendTextureModes::NotTextured
        };

        let mut pipe_container = self.quad_pipeline.clone();
        for s in 0..MAX_SUB_PASS_COUNT {
            for n in 0..RENDER_PASS_TYPE_COUNT {
                for i in 0..EVulkanBackendBlendModes::Count as usize {
                    for j in 0..EVulkanBackendClipModes::Count as usize {
                        ret &= self.create_quad_graphics_pipeline_impl::<HAS_SAMPLER>(
                            vert_name,
                            frag_name,
                            &mut pipe_container,
                            tex_mode,
                            EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                            EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                            RenderPassType::from_u32(n as u32).unwrap(),
                            s,
                        );
                    }
                }
            }
        }
        self.quad_pipeline = pipe_container;

        return ret;
    }

    #[must_use]
    fn create_quad_push_graphics_pipeline_impl<const IS_TEXTURED: bool>(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut SPipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
        sub_pass_index: usize,
    ) -> bool {
        let mut attribute_descriptors: [vk::VertexInputAttributeDescription; 3] =
            Default::default();
        attribute_descriptors[0] = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32B32A32_SFLOAT,
            offset: 0,
        };
        attribute_descriptors[1] = vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: (std::mem::size_of::<f32>() * 4) as u32,
        };
        if IS_TEXTURED {
            attribute_descriptors[2] = vk::VertexInputAttributeDescription {
                location: 2,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: (std::mem::size_of::<f32>() * 4 + std::mem::size_of::<u8>() * 4) as u32,
            };
        }

        let mut set_layouts = [self.device.standard_textured_descriptor_set_layout];

        let push_constant_size = std::mem::size_of::<SUniformQuadPushGPos>();

        let mut push_constants = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: push_constant_size as u32,
        }];

        return self.create_graphics_pipeline::<false>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * 4
                + std::mem::size_of::<u8>() * 4
                + (if IS_TEXTURED {
                    std::mem::size_of::<f32>() * 2
                } else {
                    0
                })) as u32,
            &mut attribute_descriptors
                .split_at_mut(if IS_TEXTURED { 3 } else { 2 })
                .0,
            &mut set_layouts,
            &mut push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
            sub_pass_index,
        );
    }

    #[must_use]
    fn create_quad_push_graphics_pipeline<const HAS_SAMPLER: bool>(
        &mut self,
        vert_name: &str,
        frag_name: &str,
    ) -> bool {
        let mut ret: bool = true;

        let tex_mode = if HAS_SAMPLER {
            EVulkanBackendTextureModes::Textured
        } else {
            EVulkanBackendTextureModes::NotTextured
        };

        let mut pipe_container = self.quad_push_pipeline.clone();
        for s in 0..MAX_SUB_PASS_COUNT {
            for n in 0..RENDER_PASS_TYPE_COUNT {
                for i in 0..EVulkanBackendBlendModes::Count as usize {
                    for j in 0..EVulkanBackendClipModes::Count as usize {
                        ret &= self.create_quad_push_graphics_pipeline_impl::<HAS_SAMPLER>(
                            vert_name,
                            frag_name,
                            &mut pipe_container,
                            tex_mode,
                            EVulkanBackendBlendModes::from_u32(i as u32).unwrap(),
                            EVulkanBackendClipModes::from_u32(j as u32).unwrap(),
                            RenderPassType::from_u32(n as u32).unwrap(),
                            s,
                        );
                    }
                }
            }
        }
        self.quad_push_pipeline = pipe_container;

        return ret;
    }

    #[must_use]
    fn create_command_pool(&mut self) -> bool {
        let mut create_pool_info = vk::CommandPoolCreateInfo::default();
        create_pool_info.queue_family_index = self.vk_graphics_queue_index;
        create_pool_info.flags = vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER;

        self.command_pools
            .resize(self.thread_count, Default::default());
        for i in 0..self.thread_count {
            let res = unsafe {
                self.ash_vk
                    .vk_device
                    .create_command_pool(&create_pool_info, None)
            };
            if res.is_err() {
                self.error
                    .lock()
                    .unwrap()
                    .set_error(EGFXErrorType::Init, "Creating the command pool failed.");
                return false;
            }
            self.command_pools[i] = res.unwrap();
        }
        return true;
    }

    fn destroy_command_pool(&mut self) {
        for i in 0..self.thread_count {
            unsafe {
                self.ash_vk
                    .vk_device
                    .destroy_command_pool(self.command_pools[i], None);
            }
        }
    }

    #[must_use]
    fn create_command_buffers(&mut self) -> bool {
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
            for thread_draw_command_buffers in &mut self.thread_draw_command_buffers {
                thread_draw_command_buffers.resize(
                    self.device.swap_chain_image_count as usize,
                    Default::default(),
                );
            }
            for used_thread_draw_command_buffer in &mut self.used_thread_draw_command_buffer {
                used_thread_draw_command_buffer
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

        let mut alloc_info = vk::CommandBufferAllocateInfo::default();
        alloc_info.command_pool = self.command_pools[0];
        alloc_info.level = vk::CommandBufferLevel::PRIMARY;
        alloc_info.command_buffer_count = self.main_draw_command_buffers.len() as u32;

        let res = unsafe { self.ash_vk.vk_device.allocate_command_buffers(&alloc_info) };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .set_error(EGFXErrorType::Init, "Allocating command buffers failed.");
            return false;
        }
        self.main_draw_command_buffers = res.unwrap();

        alloc_info.command_buffer_count = self.device.memory_command_buffers.len() as u32;

        let res = unsafe { self.ash_vk.vk_device.allocate_command_buffers(&alloc_info) };
        if res.is_err() {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::Init,
                "Allocating memory command buffers failed.",
            );
            return false;
        }
        self.device.memory_command_buffers = res.unwrap();

        if self.thread_count > 1 {
            let mut count: usize = 0;
            for thread_draw_command_buffers in &mut self.thread_draw_command_buffers {
                alloc_info.command_pool = self.command_pools[count];
                count += 1;
                alloc_info.command_buffer_count = thread_draw_command_buffers.len() as u32;
                alloc_info.level = vk::CommandBufferLevel::SECONDARY;
                let res = unsafe { self.ash_vk.vk_device.allocate_command_buffers(&alloc_info) };
                if res.is_err() {
                    self.error.lock().unwrap().set_error(
                        EGFXErrorType::Init,
                        "Allocating thread command buffers failed.",
                    );
                    return false;
                }
                *thread_draw_command_buffers = res.unwrap();
            }
        }

        return true;
    }

    fn destroy_command_buffer(&mut self) {
        if self.thread_count > 1 {
            let mut count: usize = 0;
            for thread_draw_command_buffers in &self.thread_draw_command_buffers {
                unsafe {
                    self.ash_vk.vk_device.free_command_buffers(
                        self.command_pools[count],
                        thread_draw_command_buffers.as_slice(),
                    );
                }
                count += 1;
            }
        }

        unsafe {
            self.ash_vk.vk_device.free_command_buffers(
                self.command_pools[0],
                self.device.memory_command_buffers.as_slice(),
            );
        }
        unsafe {
            self.ash_vk.vk_device.free_command_buffers(
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
    fn create_sync_objects(&mut self) -> bool {
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

        let create_semaphore_info = vk::SemaphoreCreateInfo::default();

        let mut fence_info = vk::FenceCreateInfo::default();
        fence_info.flags = vk::FenceCreateFlags::SIGNALED;

        for i in 0..self.device.swap_chain_image_count {
            let res = unsafe {
                self.ash_vk
                    .vk_device
                    .create_semaphore(&create_semaphore_info, None)
            };
            let res2 = unsafe {
                self.ash_vk
                    .vk_device
                    .create_semaphore(&create_semaphore_info, None)
            };
            let res3 = unsafe {
                self.ash_vk
                    .vk_device
                    .create_semaphore(&create_semaphore_info, None)
            };
            let res4 = unsafe { self.ash_vk.vk_device.create_fence(&fence_info, None) };
            if res.is_err() || res2.is_err() || res3.is_err() || res4.is_err() {
                self.error.lock().unwrap().set_error(
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

    fn destroy_sync_objects(&mut self) {
        for i in 0..self.device.swap_chain_image_count {
            unsafe {
                self.ash_vk
                    .vk_device
                    .destroy_semaphore(self.wait_semaphores[i as usize], None);
            }
            unsafe {
                self.ash_vk
                    .vk_device
                    .destroy_semaphore(self.sig_semaphores[i as usize], None);
            }
            unsafe {
                self.ash_vk
                    .vk_device
                    .destroy_semaphore(self.memory_sempahores[i as usize], None);
            }
            unsafe {
                self.ash_vk
                    .vk_device
                    .destroy_fence(self.frame_fences[i as usize], None);
            }
        }

        self.wait_semaphores.clear();
        self.sig_semaphores.clear();

        self.memory_sempahores.clear();

        self.frame_fences.clear();
        self.image_fences.clear();
    }

    fn destroy_buffer_of_frame(mem: &mut Memory, image_index: usize, buffer: &mut SFrameBuffers) {
        mem.clean_buffer_pair(image_index, &mut buffer.buffer, &mut buffer.buffer_mem);
    }

    fn destroy_uni_buffer_of_frame(
        mem: &mut Memory,
        device: &ash::Device,
        image_index: usize,
        buffer: &mut SFrameUniformBuffers,
    ) {
        mem.clean_buffer_pair(
            image_index,
            &mut buffer.base.buffer,
            &mut buffer.base.buffer_mem,
        );
        for descr_set in &mut buffer.uniform_sets {
            if descr_set.descriptor != vk::DescriptorSet::null() {
                Device::destroy_uniform_descriptor_sets(device, descr_set, 1);
            }
        }
    }

    /*************
     * SWAP CHAIN
     **************/

    fn cleanup_vulkan_swap_chain(&mut self, force_swap_chain_destruct: bool) {
        self.standard_pipeline.destroy(&self.ash_vk.vk_device);
        self.standard_line_pipeline.destroy(&self.ash_vk.vk_device);
        self.standard_stencil_only_pipeline
            .destroy(&self.ash_vk.vk_device);
        self.standard_stencil_pipeline
            .destroy(&self.ash_vk.vk_device);
        self.standard_3d_pipeline.destroy(&self.ash_vk.vk_device);
        self.blur_pipeline.destroy(&self.ash_vk.vk_device);
        self.text_pipeline.destroy(&self.ash_vk.vk_device);
        self.tile_pipeline.destroy(&self.ash_vk.vk_device);
        self.tile_border_pipeline.destroy(&self.ash_vk.vk_device);
        self.tile_border_line_pipeline
            .destroy(&self.ash_vk.vk_device);
        self.prim_ex_pipeline.destroy(&self.ash_vk.vk_device);
        self.prim_ex_rotationless_pipeline
            .destroy(&self.ash_vk.vk_device);
        self.sprite_multi_pipeline.destroy(&self.ash_vk.vk_device);
        self.sprite_multi_push_pipeline
            .destroy(&self.ash_vk.vk_device);
        self.quad_pipeline.destroy(&self.ash_vk.vk_device);
        self.quad_push_pipeline.destroy(&self.ash_vk.vk_device);

        self.destroy_framebuffers_double_pass();
        self.destroy_framebuffers();

        self.destroy_render_pass();
        self.destroy_render_pass_double_pass();

        self.destroy_stencil_attachments_for_pass_transition();

        self.destroy_multi_sampler_image_attachments();
        self.destroy_multi_sampler_image_attachments_for_second_pass();

        self.destroy_images_for_second_pass();

        self.destroy_image_views();
        self.clear_swap_chain_image_handles();

        self.destroy_swap_chain(force_swap_chain_destruct);

        self.swap_chain_created = false;
    }

    fn cleanup_vulkan<const IS_LAST_CLEANUP: bool>(&mut self) {
        if IS_LAST_CLEANUP {
            if self.swap_chain_created {
                self.cleanup_vulkan_swap_chain(true);
            }

            // clean all images, buffers, buffer containers
            for (_, mut texture) in &mut self.device.textures.drain() {
                if texture.vk_text_descr_set.descriptor != vk::DescriptorSet::null()
                    && is_verbose(&*self.dbg)
                {
                    // TODO  dbg_msg("vulkan", "text textures not cleared over cmd.");
                }
                Device::destroy_texture(
                    &mut self.device.frame_delayed_buffer_cleanups,
                    &mut self.device.image_buffer_caches,
                    &self.ash_vk.vk_device,
                    &mut texture,
                    self.cur_image_index,
                );
            }

            for (_, mut buffer_object) in self.device.buffer_objects.drain() {
                Device::free_vertex_mem_block(
                    &mut self.device.frame_delayed_buffer_cleanups,
                    &mut self.device.vertex_buffer_cache,
                    &mut buffer_object.buffer_object.mem,
                    self.cur_image_index,
                );
            }
        }

        self.image_last_frame_check.clear();

        self.last_pipeline_per_thread.clear();

        self.device
            .streamed_vertex_buffer
            .destroy(&mut |image_index, buffer| {
                Self::destroy_buffer_of_frame(&mut self.device.mem, image_index, buffer);
            });
        for i in 0..self.thread_count {
            self.device.streamed_uniform_buffers[i].destroy(&mut |image_index, buffer| {
                Self::destroy_uni_buffer_of_frame(
                    &mut self.device.mem,
                    &self.device.ash_vk.device,
                    image_index,
                    buffer,
                );
            });
        }
        self.device.streamed_vertex_buffer = Default::default();
        self.device.streamed_uniform_buffers.clear();

        for i in 0..self.device.swap_chain_image_count {
            self.clear_frame_data(i as usize);
        }

        self.device.frame_delayed_buffer_cleanups.clear();
        self.device.frame_delayed_texture_cleanups.clear();

        self.device
            .staging_buffer_cache
            .destroy_frame_data(self.device.swap_chain_image_count as usize);
        self.device
            .staging_buffer_cache_image
            .destroy_frame_data(self.device.swap_chain_image_count as usize);
        self.device
            .vertex_buffer_cache
            .destroy_frame_data(self.device.swap_chain_image_count as usize);
        for image_buffer_cache in &mut self.device.image_buffer_caches {
            image_buffer_cache
                .1
                .destroy_frame_data(self.device.swap_chain_image_count as usize);
        }

        if IS_LAST_CLEANUP {
            self.device
                .staging_buffer_cache
                .destroy(&self.ash_vk.vk_device);
            self.device
                .staging_buffer_cache_image
                .destroy(&self.ash_vk.vk_device);
            self.device
                .vertex_buffer_cache
                .destroy(&self.ash_vk.vk_device);
            for image_buffer_cache in &mut self.device.image_buffer_caches {
                image_buffer_cache.1.destroy(&self.ash_vk.vk_device);
            }

            self.device.image_buffer_caches.clear();

            self.destroy_texture_samplers();
            self.destroy_descriptor_pools();

            self.delete_presented_image_data_image();
        }

        self.destroy_sync_objects();
        self.destroy_command_buffer();

        if IS_LAST_CLEANUP {
            self.destroy_command_pool();
        }

        if IS_LAST_CLEANUP {
            self.device.destroy_uniform_descriptor_set_layouts();
            self.destroy_text_descriptor_set_layout();
            self.destroy_descriptor_set_layouts();
        }
    }

    fn cleanup_vulkan_sdl(&mut self) {
        if self.ash_vk.vk_instance.handle() != vk::Instance::null() {
            self.destroy_surface();
            unsafe {
                self.ash_vk.vk_device.destroy_device(None);
            }

            let dbg_val = self.dbg.load(std::sync::atomic::Ordering::Relaxed);
            if dbg_val == EDebugGFXModes::Minimum as u8 || dbg_val == EDebugGFXModes::All as u8 {
                self.unregister_debug_callback();
            }
            unsafe { self.ash_vk.vk_instance.destroy_instance(None) };
        }
    }

    fn recreate_swap_chain(&mut self) -> i32 {
        let mut ret: i32 = 0;
        unsafe { self.ash_vk.vk_device.device_wait_idle().unwrap() };

        if is_verbose(&*self.dbg) {
            self.logger
                .log(LogLevel::Info)
                .msg("recreating swap chain.");
        }

        let mut old_swap_chain = vk::SwapchainKHR::null();
        let old_swap_chain_image_count: u32 = self.device.swap_chain_image_count;

        if self.swap_chain_created {
            self.cleanup_vulkan_swap_chain(false);
        }

        // set new multi sampling if it was requested
        if self.next_multi_sampling_count != u32::MAX {
            self.device.config.multi_sampling_count = self.next_multi_sampling_count;
            self.next_multi_sampling_count = u32::MAX;
        }
        if self.next_multi_sampling_second_pass_count != u32::MAX {
            self.device.config.multi_sampling_second_pass_count =
                self.next_multi_sampling_second_pass_count;
            self.next_multi_sampling_second_pass_count = u32::MAX;
        }

        if !self.swap_chain_created {
            ret = self.init_vulkan_swap_chain(&mut old_swap_chain);
        }

        if old_swap_chain_image_count != self.device.swap_chain_image_count {
            self.cleanup_vulkan::<false>();
            self.init_vulkan::<false>();
        }

        if old_swap_chain != vk::SwapchainKHR::null() {
            // TODO! unsafe {self.m_VKDevice.DestroySwapchainKHR( OldSwapChain, std::ptr::null());}
        }

        if ret != 0 && is_verbose(&*self.dbg) {
            // TODO  dbg_msg("vulkan", "recreating swap chain failed.");
        }

        return ret;
    }

    fn init_vulkan_sdl(
        window: &winit::window::Window,
        _canvas_width: f64,
        _canvas_height: f64,
        dbg: EDebugGFXModes,
        error: &Arc<Mutex<Error>>,
        logger: &SystemLogGroup,
    ) -> Result<
        (
            ash::Entry,
            ash::Instance,
            ash::Device,
            TTWGraphicsGPUList,
            Limits,
            Config,
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

        let extensions_res = Self::get_vulkan_extensions(window);
        if let Err(err) = extensions_res {
            return Err(err);
        }
        let mut extensions = extensions_res.unwrap();

        let layers_res = Self::get_vulkan_layers(dbg, &entry);
        if let Err(err) = layers_res {
            return Err(err);
        }
        let mut layers = layers_res.unwrap();

        let instance_res =
            Self::create_vulkan_instance(dbg, &entry, error, &mut layers, &mut extensions, true);
        if let Err(err) = instance_res {
            return Err(err);
        }
        let instance = instance_res.unwrap();

        let mut _dbg_callback = vk::DebugUtilsMessengerEXT::null();
        if dbg == EDebugGFXModes::Minimum || dbg == EDebugGFXModes::All {
            let dbg_res = Self::setup_debug_callback(&entry, &instance, logger);
            if let Ok(dbg) = dbg_res {
                _dbg_callback = dbg;
            }

            for vk_layer in &mut layers {
                logger
                    .log(LogLevel::Info)
                    .msg("Validation layer: ")
                    .msg(vk_layer.as_str());
            }
        }

        let gpu_res = Self::select_gpu(&instance, dbg, logger);
        if let Err(err) = gpu_res {
            return Err(err);
        }
        let (
            gpu_list,
            limits,
            config,
            renderer_name,
            vendor_name,
            version_name,
            physical_gpu,
            graphics_queue_index,
        ) = gpu_res.unwrap();

        let device_res =
            Self::create_logical_device(&physical_gpu, graphics_queue_index, &instance, &layers);
        if let Err(err) = device_res {
            return Err(err);
        }
        let device = device_res.unwrap();

        let dev_queue_res = Self::get_device_queue(&device, graphics_queue_index);
        if let Err(err) = dev_queue_res {
            return Err(err);
        }
        let (graphics_queue, presentation_queue) = dev_queue_res.unwrap();

        let surface = ash::extensions::khr::Surface::new(&entry, &instance);

        let surf_res = Self::create_surface(
            &entry,
            window,
            &surface,
            &instance,
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
            config,
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
    fn has_multi_sampling(&mut self) -> bool {
        return Device::get_sample_count(
            self.device.config.multi_sampling_count,
            &self.device.limits,
        ) != vk::SampleCountFlags::TYPE_1;
    }

    #[must_use]
    fn has_multi_sampling_in_second_pass(&mut self) -> bool {
        return Device::get_sample_count(
            self.device.config.multi_sampling_second_pass_count,
            &self.device.limits,
        ) != vk::SampleCountFlags::TYPE_1;
    }

    fn init_vulkan_swap_chain(&mut self, old_swap_chain: &mut vk::SwapchainKHR) -> i32 {
        *old_swap_chain = vk::SwapchainKHR::null();
        if !self.create_swap_chain(old_swap_chain) {
            return -1;
        }

        if !self.get_swap_chain_image_handles() {
            return -1;
        }

        if !self.create_image_views() {
            return -1;
        }

        if !self.create_multi_sampler_image_attachments() {
            return -1;
        }

        if !self.create_images_for_second_pass() {
            return -1;
        }

        if !self.create_multi_sampler_image_attachments_for_second_pass() {
            return -1;
        }

        if !self.create_stencil_attachments_for_pass_transition() {
            return -1;
        }

        self.last_presented_swap_chain_image_index = u32::MAX;

        if !self.create_render_pass(true) {
            return -1;
        }

        if !self.create_render_pass_double(true) {
            return -1;
        }

        if !self.create_framebuffers() {
            return -1;
        }

        if !self.create_framebuffers_double_pass() {
            return -1;
        }

        if !self.create_standard_graphics_pipeline(
            "shader/vulkan/prim.vert.spv",
            "shader/vulkan/prim.frag.spv",
            false,
            false,
            StencilOpType::None,
        ) {
            return -1;
        }

        if !self.create_standard_graphics_pipeline(
            "shader/vulkan/prim_textured.vert.spv",
            "shader/vulkan/prim_textured.frag.spv",
            true,
            false,
            StencilOpType::None,
        ) {
            return -1;
        }

        if !self.create_standard_graphics_pipeline(
            "shader/vulkan/prim.vert.spv",
            "shader/vulkan/prim.frag.spv",
            false,
            true,
            StencilOpType::None,
        ) {
            return -1;
        }

        // stencil only pipeline, does not write to any color attachments
        if !self.create_standard_graphics_pipeline(
            "shader/vulkan/prim.vert.spv",
            "shader/vulkan/prim.frag.spv",
            false,
            false,
            StencilOpType::AlwaysPass,
        ) {
            return -1;
        }

        if !self.create_standard_graphics_pipeline(
            "shader/vulkan/full.vert.spv",
            "shader/vulkan/prim_textured.frag.spv",
            true,
            false,
            StencilOpType::OnlyWhenPassed,
        ) {
            return -1;
        }

        if !self.create_standard_graphics_pipeline(
            "shader/vulkan/full.vert.spv",
            "shader/vulkan/prim_no_alpha.frag.spv",
            true,
            false,
            StencilOpType::OnlyWhenNotPassed,
        ) {
            return -1;
        }

        if !self.create_blur_graphics_pipeline(
            "shader/vulkan/full.vert.spv",
            "shader/vulkan/blur.frag.spv",
        ) {
            return -1;
        }

        if !self.create_standard_3d_graphics_pipeline(
            "shader/vulkan/prim3d.vert.spv",
            "shader/vulkan/prim3d.frag.spv",
            false,
        ) {
            return -1;
        }

        if !self.create_standard_3d_graphics_pipeline(
            "shader/vulkan/prim3d_textured.vert.spv",
            "shader/vulkan/prim3d_textured.frag.spv",
            true,
        ) {
            return -1;
        }

        if !self.create_text_graphics_pipeline(
            "shader/vulkan/text.vert.spv",
            "shader/vulkan/text.frag.spv",
        ) {
            return -1;
        }

        if !self.create_tile_graphics_pipeline::<false>(
            "shader/vulkan/tile.vert.spv",
            "shader/vulkan/tile.frag.spv",
            0,
        ) {
            return -1;
        }

        if !self.create_tile_graphics_pipeline::<true>(
            "shader/vulkan/tile_textured.vert.spv",
            "shader/vulkan/tile_textured.frag.spv",
            0,
        ) {
            return -1;
        }

        if !self.create_tile_graphics_pipeline::<false>(
            "shader/vulkan/tile_border.vert.spv",
            "shader/vulkan/tile_border.frag.spv",
            1,
        ) {
            return -1;
        }

        if !self.create_tile_graphics_pipeline::<true>(
            "shader/vulkan/tile_border_textured.vert.spv",
            "shader/vulkan/tile_border_textured.frag.spv",
            1,
        ) {
            return -1;
        }

        if !self.create_tile_graphics_pipeline::<false>(
            "shader/vulkan/tile_border_line.vert.spv",
            "shader/vulkan/tile_border_line.frag.spv",
            2,
        ) {
            return -1;
        }

        if !self.create_tile_graphics_pipeline::<true>(
            "shader/vulkan/tile_border_line_textured.vert.spv",
            "shader/vulkan/tile_border_line_textured.frag.spv",
            2,
        ) {
            return -1;
        }

        if !self.create_prim_ex_graphics_pipeline(
            "shader/vulkan/primex_rotationless.vert.spv",
            "shader/vulkan/primex_rotationless.frag.spv",
            false,
            true,
        ) {
            return -1;
        }

        if !self.create_prim_ex_graphics_pipeline(
            "shader/vulkan/primex_tex_rotationless.vert.spv",
            "shader/vulkan/primex_tex_rotationless.frag.spv",
            true,
            true,
        ) {
            return -1;
        }

        if !self.create_prim_ex_graphics_pipeline(
            "shader/vulkan/primex.vert.spv",
            "shader/vulkan/primex.frag.spv",
            false,
            false,
        ) {
            return -1;
        }

        if !self.create_prim_ex_graphics_pipeline(
            "shader/vulkan/primex_tex.vert.spv",
            "shader/vulkan/primex_tex.frag.spv",
            true,
            false,
        ) {
            return -1;
        }

        if !self.create_sprite_multi_graphics_pipeline(
            "shader/vulkan/spritemulti.vert.spv",
            "shader/vulkan/spritemulti.frag.spv",
        ) {
            return -1;
        }

        if !self.create_sprite_multi_push_graphics_pipeline(
            "shader/vulkan/spritemulti_push.vert.spv",
            "shader/vulkan/spritemulti_push.frag.spv",
        ) {
            return -1;
        }

        if !self.create_quad_graphics_pipeline::<false>(
            "shader/vulkan/quad.vert.spv",
            "shader/vulkan/quad.frag.spv",
        ) {
            return -1;
        }

        if !self.create_quad_graphics_pipeline::<true>(
            "shader/vulkan/quad_textured.vert.spv",
            "shader/vulkan/quad_textured.frag.spv",
        ) {
            return -1;
        }

        if !self.create_quad_push_graphics_pipeline::<false>(
            "shader/vulkan/quad_push.vert.spv",
            "shader/vulkan/quad_push.frag.spv",
        ) {
            return -1;
        }

        if !self.create_quad_push_graphics_pipeline::<true>(
            "shader/vulkan/quad_push_textured.vert.spv",
            "shader/vulkan/quad_push_textured.frag.spv",
        ) {
            return -1;
        }

        self.swap_chain_created = true;
        return 0;
    }

    fn init_vulkan_without_io(&mut self) -> i32 {
        if !self.create_descriptor_set_layouts() {
            return -1;
        }

        if !self.create_text_descriptor_set_layout() {
            return -1;
        }

        if !self
            .device
            .create_sprite_multi_uniform_descriptor_set_layout()
        {
            return -1;
        }

        if !self.device.create_quad_uniform_descriptor_set_layout() {
            return -1;
        }

        return 0;
    }

    fn init_vulkan_with_io<const IS_FIRST_INITIALIZATION: bool>(&mut self) -> i32 {
        if IS_FIRST_INITIALIZATION {
            if !self.create_descriptor_pools(self.thread_count) {
                return -1;
            }

            if !self.create_texture_samplers() {
                return -1;
            }
        }

        if IS_FIRST_INITIALIZATION {
            let mut old_swap_chain = vk::SwapchainKHR::null();
            if self.init_vulkan_swap_chain(&mut old_swap_chain) != 0 {
                return -1;
            }
        }

        if IS_FIRST_INITIALIZATION {
            if !self.create_command_pool() {
                return -1;
            }
        }

        if !self.create_command_buffers() {
            return -1;
        }

        if !self.create_sync_objects() {
            return -1;
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
        self.device
            .staging_buffer_cache
            .init(self.device.swap_chain_image_count as usize);
        self.device
            .staging_buffer_cache_image
            .init(self.device.swap_chain_image_count as usize);
        self.device
            .vertex_buffer_cache
            .init(self.device.swap_chain_image_count as usize);
        for image_buffer_cache in &mut self.device.image_buffer_caches {
            image_buffer_cache
                .1
                .init(self.device.swap_chain_image_count as usize);
        }

        self.image_last_frame_check
            .resize(self.device.swap_chain_image_count as usize, 0);

        if IS_FIRST_INITIALIZATION {
            // check if image format supports linear blitting
            let mut format_properties = unsafe {
                self.ash_vk
                    .vk_instance
                    .get_physical_device_format_properties(self.vk_gpu, vk::Format::R8G8B8A8_UNORM)
            };
            if !(format_properties.optimal_tiling_features
                & vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR)
                .is_empty()
            {
                self.device.allows_linear_blitting = true;
            }
            if !(format_properties.optimal_tiling_features & vk::FormatFeatureFlags::BLIT_SRC)
                .is_empty()
                && !(format_properties.optimal_tiling_features & vk::FormatFeatureFlags::BLIT_DST)
                    .is_empty()
            {
                self.device.optimal_rgba_image_blitting = true;
            }
            // check if image format supports blitting to linear tiled images
            if !(format_properties.linear_tiling_features & vk::FormatFeatureFlags::BLIT_DST)
                .is_empty()
            {
                self.device.linear_rgba_image_blitting = true;
            }

            format_properties = unsafe {
                self.ash_vk
                    .vk_instance
                    .get_physical_device_format_properties(self.vk_gpu, self.vk_surf_format.format)
            };
            if !(format_properties.optimal_tiling_features & vk::FormatFeatureFlags::BLIT_SRC)
                .is_empty()
            {
                self.device.optimal_swap_chain_image_blitting = true;
            }
        }

        return 0;
    }

    fn init_vulkan<const IS_FIRST_INITIALIZATION: bool>(&mut self) -> i32 {
        let res = self.init_vulkan_without_io();
        if res != 0 {
            return res;
        }

        let res = self.init_vulkan_with_io::<{ IS_FIRST_INITIALIZATION }>();
        if res != 0 {
            return res;
        }

        return 0;
    }

    #[must_use]
    fn get_graphic_command_buffer(
        &mut self,
        ptr_draw_command_buffer: &mut *mut vk::CommandBuffer,
        render_thread_index: usize,
        sub_pass_index: usize,
    ) -> bool {
        if self.thread_count < 2 {
            *ptr_draw_command_buffer =
                &mut self.main_draw_command_buffers[self.cur_image_index as usize];
            return true;
        } else {
            let draw_command_buffer = &mut self.thread_draw_command_buffers[render_thread_index]
                [self.cur_image_index as usize];
            if !self.used_thread_draw_command_buffer[render_thread_index]
                [self.cur_image_index as usize]
            {
                self.used_thread_draw_command_buffer[render_thread_index]
                    [self.cur_image_index as usize] = true;

                if let Err(_) = unsafe {
                    self.ash_vk.vk_device.reset_command_buffer(
                        *draw_command_buffer,
                        vk::CommandBufferResetFlags::RELEASE_RESOURCES,
                    )
                } {
                    return false;
                }

                let mut begin_info = vk::CommandBufferBeginInfo::default();
                begin_info.flags = vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT
                    | vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE;

                let mut inheret_info = vk::CommandBufferInheritanceInfo::default();
                inheret_info.framebuffer = self.framebuffer_list[self.cur_image_index as usize];
                inheret_info.occlusion_query_enable = vk::FALSE;
                inheret_info.render_pass = match self.current_render_pass_type {
                    RenderPassType::Single => self.vk_render_pass_single_pass,
                    RenderPassType::Dual => self.vk_render_pass_double_pass,
                };
                inheret_info.subpass = sub_pass_index as u32;

                begin_info.p_inheritance_info = &inheret_info;

                let begin_res = unsafe {
                    self.ash_vk
                        .vk_device
                        .begin_command_buffer(*draw_command_buffer, &begin_info)
                };
                if let Err(_) = begin_res {
                    self.error.lock().unwrap().set_error(
                        EGFXErrorType::RenderRecording,
                        "Thread draw command buffer cannot be filled anymore.",
                    );
                    return false;
                }
            }
            *ptr_draw_command_buffer = draw_command_buffer;
            return true;
        }
    }

    /************************
     * COMMAND IMPLEMENTATION
     ************************/
    #[must_use]
    fn cmd_texture_update(&mut self, cmd: &CommandTextureUpdate) -> bool {
        let index_tex = cmd.texture_index;

        // TODO: useless copy?
        let mut data = cmd.data.clone();

        if !self.update_texture(
            index_tex,
            vk::Format::R8G8B8A8_UNORM,
            &mut data,
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
    fn cmd_texture_destroy(&mut self, cmd: &CommandTextureDestroy) -> bool {
        let image_index = cmd.texture_index;
        let texture = &mut self.device.textures.remove(&image_index).unwrap();

        self.device.frame_delayed_texture_cleanups[self.cur_image_index as usize]
            .push(texture.clone());

        *texture = CTexture::default();

        return true;
    }

    #[must_use]
    fn cmd_texture_create(&mut self, cmd: &CommandTextureCreate) -> bool {
        let texture_index = cmd.texture_index;
        let width = cmd.width;
        let height = cmd.height;
        let depth = cmd.depth;
        let pixel_size = cmd.pixel_size;
        let format = cmd.format;
        let store_format = cmd.store_format;
        let flags = cmd.flags;

        let mut data_opt = cmd.data.borrow_mut();
        let mut data: Option<GraphicsBackendMemory> = None;
        std::mem::swap(&mut data, &mut data_opt);

        let data_mem = data.unwrap();
        let data_mem = self
            .device
            .mem_allocator
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .memory_to_internal_memory(
                data_mem,
                GraphicsMemoryAllocationType::Texture,
                self.cur_image_index,
            );

        if !self.create_texture_cmd(
            texture_index,
            width as usize,
            height as usize,
            depth,
            cmd.is_3d_tex,
            pixel_size as usize,
            texture_format_to_vulkan_format(format),
            texture_format_to_vulkan_format(store_format),
            flags,
            data_mem,
        ) {
            return false;
        }

        return true;
    }

    fn cmd_next_subpass(&mut self) -> bool {
        self.finish_render_threads();

        let command_buffer = &mut self.main_draw_command_buffers[self.cur_image_index as usize];
        unsafe {
            self.ash_vk.vk_device.cmd_next_subpass(
                *command_buffer,
                if self.thread_count > 1 {
                    vk::SubpassContents::SECONDARY_COMMAND_BUFFERS
                } else {
                    vk::SubpassContents::INLINE
                },
            )
        };

        self.current_sub_pass_index += 1;
        true
    }

    fn cmd_clear_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &CommandClear,
    ) {
        if !cmd.force_clear {
            let color_changed: bool = self.clear_color[0] != cmd.color.r
                || self.clear_color[1] != cmd.color.g
                || self.clear_color[2] != cmd.color.b
                || self.clear_color[3] != cmd.color.a;
            self.clear_color[0] = cmd.color.r;
            self.clear_color[1] = cmd.color.g;
            self.clear_color[2] = cmd.color.b;
            self.clear_color[3] = cmd.color.a;
            if color_changed {
                exec_buffer.clear_color_in_render_thread = true;
            }
        } else {
            exec_buffer.clear_color_in_render_thread = true;
        }
        exec_buffer.estimated_render_call_count = 0;
    }

    #[must_use]
    fn cmd_clear(&mut self, exec_buffer: &SRenderCommandExecuteBuffer, cmd: &CommandClear) -> bool {
        if exec_buffer.clear_color_in_render_thread {
            let clear_attachments = [vk::ClearAttachment {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                color_attachment: 0,
                clear_value: vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [cmd.color.r, cmd.color.g, cmd.color.b, cmd.color.a],
                    },
                },
            }];
            let clear_rects = [vk::ClearRect {
                rect: vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.vk_swap_img_and_viewport_extent.swap_image_viewport,
                },
                base_array_layer: 0,
                layer_count: 1,
            }];

            let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
            if !self.get_graphic_command_buffer(
                &mut command_buffer_ptr,
                exec_buffer.thread_index as usize,
                exec_buffer.sub_pass_index,
            ) {
                return false;
            }
            let command_buffer = unsafe { &mut *command_buffer_ptr };
            unsafe {
                self.ash_vk.vk_device.cmd_clear_attachments(
                    *command_buffer,
                    &clear_attachments,
                    &clear_rects,
                );
            }
        }

        return true;
    }

    fn cmd_render_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &CommandRender,
    ) {
        let is_textured: bool = Self::get_is_textured(&cmd.state);
        if is_textured {
            let address_mode_index: usize = Self::get_address_mode_index(&cmd.state);
            exec_buffer.descriptors[0] = self
                .device
                .textures
                .get(&cmd.state.texture_index.unwrap())
                .unwrap()
                .vk_standard_textured_descr_sets[address_mode_index]
                .clone();
        }

        exec_buffer.index_buffer = self.index_buffer;

        exec_buffer.estimated_render_call_count = 1;

        self.exec_buffer_fill_dynamic_states(&cmd.state, exec_buffer);

        let cur_stream_buffer = self
            .device
            .streamed_vertex_buffer
            .get_current_buffer(self.cur_image_index as usize);
        exec_buffer.buffer = cur_stream_buffer.buffer;
        exec_buffer.buffer_off = cur_stream_buffer.offset_in_buffer
            + self.cur_stream_vertex_byte_offset
            + cmd.vertices_offset * std::mem::size_of::<GlVertex>();
    }

    #[must_use]
    fn cmd_render(
        &mut self,
        cmd: &CommandRender,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        return self.render_standard::<GlVertex, false>(
            exec_buffer,
            &cmd.state,
            cmd.prim_type,
            cmd.prim_count,
            StencilOpType::None,
            true,
        );
    }

    fn cmd_render_first_subpass_blurred_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &CommandRender,
    ) {
        self.cmd_render_fill_execute_buffer(exec_buffer, cmd);
    }

    #[must_use]
    fn cmd_render_first_subpass_blurred(
        &mut self,
        cmd: &CommandRender,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        let mut exec_buffer_real = exec_buffer.clone();
        let mut state_real = cmd.state.clone();
        state_real.clear_texture();
        exec_buffer_real.descriptors = Default::default();
        // draw the vertices and fill stencil buffer
        let mut res = self.render_standard::<GlVertex, false>(
            &exec_buffer_real,
            &state_real,
            cmd.prim_type,
            cmd.prim_count,
            StencilOpType::AlwaysPass,
            true,
        );

        struct FakeTexture {}
        impl SharedIndexGetIndexUnsafe for FakeTexture {
            fn get_index_unsafe(&self) -> u128 {
                0
            }
        }
        state_real.set_texture(&FakeTexture {});
        exec_buffer_real.descriptors = self.image_list_for_double_pass
            [self.cur_image_index as usize]
            .vk_standard_textured_descr_sets
            .clone();
        // draw where the stencil buffer triggered
        res &= self.render_blur::<GlVertex>(&exec_buffer_real, &state_real, PrimType::Triangles, 1);
        // then draw the rest of the first pass
        // where the stencil buffer didn't trigger
        self.render_standard::<GlVertex, false>(
            &exec_buffer_real,
            &state_real,
            PrimType::Triangles,
            1,
            StencilOpType::OnlyWhenNotPassed,
            false,
        ) && res
    }

    /*
                    void Cmd_RenderTex3D_FillExecuteBuffer(exec_buffer: &mut SRenderCommandExecuteBuffer, cmd: &CommandRenderTex3D)
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

                    #[must_use] fn Cmd_RenderTex3D(cmd: &CommandRenderTex3D, exec_buffer: &SRenderCommandExecuteBuffer ) { return RenderStandard<CCommandBuffer::SVertexTex3DStream, true>(&mut self,exec_buffer, cmd.state, cmd.m_PrimType, cmd.m_pVertices, cmd.m_PrimCount); } -> bool
    */
    fn cmd_update_viewport_fill_execute_buffer(
        &self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        _cmd: &CommandUpdateViewport,
    ) {
        exec_buffer.estimated_render_call_count = 0;
    }

    #[must_use]
    fn cmd_update_viewport(&mut self, cmd: &CommandUpdateViewport) -> bool {
        if cmd.by_resize {
            if is_verbose(&*self.dbg) {
                self.logger
                    .log(LogLevel::Debug)
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
                self.canvas_width = cmd.width as f64;
                self.canvas_height = cmd.height as f64;
                self.recreate_swap_chain = true;
            }
        } else {
            let viewport = self
                .vk_swap_img_and_viewport_extent
                .get_presented_image_viewport();
            if cmd.x != 0
                || cmd.y != 0
                || cmd.width != viewport.width
                || cmd.height != viewport.height
            {
                self.has_dynamic_viewport = true;

                // convert viewport from OGL to vulkan
                let viewport_y: i32 = viewport.height as i32 - (cmd.y + cmd.height as i32);
                let viewport_h = cmd.height;
                self.dynamic_viewport_offset = vk::Offset2D {
                    x: cmd.x,
                    y: viewport_y,
                };
                self.dynamic_viewport_size = vk::Extent2D {
                    width: cmd.width,
                    height: viewport_h,
                };
            } else {
                self.has_dynamic_viewport = false;
            }
        }

        return true;
    }
    /*
                #[must_use] fn Cmd_VSync(&mut self,cmd: &CommandVSync) -> bool
                {
                    if(IsVerbose(&*self.dbg))
                    {
                        dbg_msg("vulkan", "queueing swap chain recreation because vsync was changed");
                    }
                    self.m_RecreateSwapChain = true;
                    *cmd.m_pRetOk = true;

                    return true;
                }

                #[must_use] fn Cmd_MultiSampling(&mut self,cmd: &CommandMultiSampling) -> bool
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

                #[must_use] fn Cmd_Finish(&mut self,cmd: &CommandFinish) -> bool
                {
                    // just ignore it with vulkan
                    return true;
                }
    */
    #[must_use]
    fn cmd_swap(&mut self) -> bool {
        return self.next_frame();
    }

    #[must_use]
    fn cmd_create_buffer_object(&mut self, cmd: &CommandCreateBufferObject) -> bool {
        let mut upload_data = None;
        let mut cmd_data = cmd.upload_data.borrow_mut();
        std::mem::swap(&mut upload_data, &mut *cmd_data);

        let upload_data_size = upload_data.as_ref().unwrap().as_slice().len() as vk::DeviceSize;

        let data_mem = upload_data.unwrap();
        let data_mem = self
            .device
            .mem_allocator
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .memory_to_internal_memory(
                data_mem,
                GraphicsMemoryAllocationType::Buffer,
                self.cur_image_index,
            );

        if !self.device.create_buffer_object(
            cmd.buffer_index,
            data_mem,
            upload_data_size,
            self.cur_image_index,
        ) {
            return false;
        }

        return true;
    }

    #[must_use]
    fn cmd_recreate_buffer_object(&mut self, cmd: &CommandRecreateBufferObject) -> bool {
        self.device
            .delete_buffer_object(cmd.buffer_index, self.cur_image_index);

        let mut upload_data = None;
        let mut cmd_data = cmd.upload_data.borrow_mut();
        std::mem::swap(&mut upload_data, &mut *cmd_data);

        let upload_data_size = upload_data.as_ref().unwrap().as_slice().len() as vk::DeviceSize;

        let data_mem = upload_data.unwrap();
        let data_mem = self
            .device
            .mem_allocator
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .memory_to_internal_memory(
                data_mem,
                GraphicsMemoryAllocationType::Buffer,
                self.cur_image_index,
            );

        return self.device.create_buffer_object(
            cmd.buffer_index,
            data_mem,
            upload_data_size,
            self.cur_image_index,
        );
    }

    #[must_use]
    fn cmd_delete_buffer_object(&mut self, cmd: &CommandDeleteBufferObject) -> bool {
        let buffer_index = cmd.buffer_index;
        self.device
            .delete_buffer_object(buffer_index, self.cur_image_index);

        return true;
    }

    #[must_use]
    fn cmd_indices_required_num_notify(&mut self, cmd: &CommandIndicesRequiredNumNotify) -> bool {
        let indices_count: usize = cmd.required_indices_num;
        if self.cur_render_index_primitive_count < indices_count / 6 {
            self.device.frame_delayed_buffer_cleanups[self.cur_image_index as usize].push(
                VKDelayedBufferCleanupItem {
                    buffer: self.render_index_buffer,
                    mem: self.render_index_buffer_memory.clone(),
                    ..Default::default()
                },
            );
            let mut upload_indices = Vec::<u32>::new();
            upload_indices.resize(indices_count, Default::default());
            let mut primitive_count: u32 = 0;
            for i in (0..indices_count).step_by(6) {
                upload_indices[i] = primitive_count;
                upload_indices[i + 1] = primitive_count + 1;
                upload_indices[i + 2] = primitive_count + 2;
                upload_indices[i + 3] = primitive_count;
                upload_indices[i + 4] = primitive_count + 2;
                upload_indices[i + 5] = primitive_count + 3;
                primitive_count += 4;
            }
            if !self.device.create_index_buffer(
                upload_indices.as_ptr() as *const c_void,
                upload_indices.len() * std::mem::size_of::<u32>(),
                &mut self.render_index_buffer,
                &mut self.render_index_buffer_memory,
                self.cur_image_index,
            ) {
                return false;
            }
            self.cur_render_index_primitive_count = indices_count / 6;
        }

        return true;
    }

    fn cmd_render_tile_layer_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &CommandRenderTileLayer,
    ) {
        self.render_tile_layer_fill_execute_buffer(
            exec_buffer,
            cmd.indices_draw_num,
            &cmd.state,
            cmd.buffer_object_index,
        );
    }

    #[must_use]
    fn cmd_render_tile_layer(
        &mut self,
        cmd: &CommandRenderTileLayer,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        let layer_type: i32 = 0;
        let dir = vec2::default();
        let off = vec2::default();
        let jump_index: i32 = 0;
        return self.render_tile_layer(
            exec_buffer,
            &cmd.state,
            layer_type,
            &cmd.color,
            &dir,
            &off,
            jump_index,
            cmd.indices_draw_num,
            &cmd.indices_offsets,
            &cmd.draw_count,
            1,
        );
    }

    fn cmd_render_border_tile_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &CommandRenderBorderTile,
    ) {
        self.render_tile_layer_fill_execute_buffer(
            exec_buffer,
            1,
            &cmd.state,
            cmd.buffer_object_index,
        );
    }

    #[must_use]
    fn cmd_render_border_tile(
        &mut self,
        cmd: &CommandRenderBorderTile,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        let layer_type: i32 = 1; // TODO: use type
        let dir = cmd.dir;
        let off = cmd.offset;
        let draw_num = 6;
        return self.render_tile_layer(
            exec_buffer,
            &cmd.state,
            layer_type,
            &cmd.color,
            &dir,
            &off,
            cmd.jump_index,
            1,
            &[cmd.indices_offset],
            &[draw_num],
            cmd.draw_num,
        );
    }

    fn cmd_render_border_tile_line_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &CommandRenderBorderTileLine,
    ) {
        self.render_tile_layer_fill_execute_buffer(
            exec_buffer,
            1,
            &cmd.state,
            cmd.buffer_object_index,
        );
    }

    #[must_use]
    fn cmd_render_border_tile_line(
        &mut self,
        cmd: &CommandRenderBorderTileLine,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        let layer_type: i32 = 2; // TODO: use type
        let dir = cmd.dir;
        let off = cmd.offset;
        return self.render_tile_layer(
            exec_buffer,
            &cmd.state,
            layer_type,
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

    fn cmd_render_quad_layer_fill_execute_buffer(
        &self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &CommandRenderQuadLayer,
    ) {
        let buffer_object = self
            .device
            .buffer_objects
            .get(&cmd.buffer_object_index)
            .unwrap();

        exec_buffer.buffer = buffer_object.cur_buffer;
        exec_buffer.buffer_off = buffer_object.cur_buffer_offset;

        let is_textured: bool = Self::get_is_textured(&cmd.state);
        if is_textured {
            let address_mode_index: usize = Self::get_address_mode_index(&cmd.state);
            exec_buffer.descriptors[0] = self
                .device
                .textures
                .get(&cmd.state.texture_index.unwrap())
                .unwrap()
                .vk_standard_textured_descr_sets[address_mode_index]
                .clone();
        }

        exec_buffer.index_buffer = self.render_index_buffer;

        exec_buffer.estimated_render_call_count =
            ((cmd.quad_num - 1) / GRAPHICS_MAX_QUADS_RENDER_COUNT) + 1;

        self.exec_buffer_fill_dynamic_states(&cmd.state, exec_buffer);
    }

    #[must_use]
    fn cmd_render_quad_layer(
        &mut self,
        cmd: &CommandRenderQuadLayer,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::get_state_matrix(&cmd.state, &mut m);

        let can_be_pushed: bool = cmd.quad_num == 1;

        let mut is_textured: bool = Default::default();
        let mut blend_mode_index: usize = Default::default();
        let mut dynamic_index: usize = Default::default();
        let mut address_mode_index: usize = Default::default();
        let mut render_pass_type_index: usize = Default::default();
        let mut sub_pass_index: usize = Default::default();
        Self::get_state_indices(
            exec_buffer,
            &cmd.state,
            &mut is_textured,
            &mut blend_mode_index,
            &mut dynamic_index,
            &mut address_mode_index,
            &mut render_pass_type_index,
            &mut sub_pass_index,
        );
        let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.get_graphic_command_buffer(
            &mut command_buffer_ptr,
            exec_buffer.thread_index as usize,
            exec_buffer.sub_pass_index,
        ) {
            return false;
        }
        let command_buffer = unsafe { &*command_buffer_ptr };
        let (pipeline, pipe_layout) = Self::get_pipeline_and_layout(
            if can_be_pushed {
                &self.quad_push_pipeline
            } else {
                &self.quad_pipeline
            },
            is_textured,
            blend_mode_index as usize,
            dynamic_index as usize,
            render_pass_type_index,
            sub_pass_index,
        );
        let (pipeline, pipe_layout) = (*pipeline, *pipe_layout);

        Self::bind_pipeline(
            &self.ash_vk.vk_device,
            &mut self.last_pipeline_per_thread,
            exec_buffer.thread_index,
            *command_buffer,
            exec_buffer,
            pipeline,
            &cmd.state,
        );

        let vertex_buffers = [exec_buffer.buffer];
        let buffer_offsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            self.ash_vk.vk_device.cmd_bind_vertex_buffers(
                *command_buffer,
                0,
                &vertex_buffers,
                &buffer_offsets,
            );
        }

        unsafe {
            self.ash_vk.vk_device.cmd_bind_index_buffer(
                *command_buffer,
                exec_buffer.index_buffer,
                0,
                vk::IndexType::UINT32,
            );
        }

        if is_textured {
            unsafe {
                self.ash_vk.vk_device.cmd_bind_descriptor_sets(
                    *command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipe_layout,
                    0,
                    &[exec_buffer.descriptors[0].descriptor],
                    &[],
                );
            }
        }

        if can_be_pushed {
            let mut push_constant_vertex = SUniformQuadPushGPos::default();

            unsafe {
                libc::memcpy(
                    &mut push_constant_vertex.bo_push as *mut SUniformQuadPushGBufferObject
                        as *mut c_void,
                    &cmd.quad_info[0] as *const SQuadRenderInfo as *const c_void,
                    std::mem::size_of::<SUniformQuadPushGBufferObject>(),
                )
            };

            push_constant_vertex.pos = m;
            push_constant_vertex.quad_offset = cmd.quad_offset as i32;

            unsafe {
                self.ash_vk.vk_device.cmd_push_constants(
                    *command_buffer,
                    pipe_layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    std::slice::from_raw_parts(
                        &push_constant_vertex as *const SUniformQuadPushGPos as *const u8,
                        std::mem::size_of::<SUniformQuadPushGPos>(),
                    ),
                );
            }
        } else {
            let mut push_constant_vertex = SUniformQuadGPos::default();
            push_constant_vertex.pos = m;
            push_constant_vertex.quad_offset = cmd.quad_offset as i32;

            unsafe {
                self.ash_vk.vk_device.cmd_push_constants(
                    *command_buffer,
                    pipe_layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    std::slice::from_raw_parts(
                        &push_constant_vertex as *const SUniformQuadGPos as *const u8,
                        std::mem::size_of::<SUniformQuadGPos>(),
                    ),
                );
            }
        }

        let mut draw_count = cmd.quad_num;
        let mut render_offset: usize = 0;

        while draw_count > 0 {
            let real_draw_count = if draw_count > GRAPHICS_MAX_QUADS_RENDER_COUNT {
                GRAPHICS_MAX_QUADS_RENDER_COUNT
            } else {
                draw_count
            };

            let index_offset = (cmd.quad_offset + render_offset) * 6;
            if !can_be_pushed {
                // create uniform buffer
                let mut uni_descr_set = SDeviceDescriptorSet::default();
                if !self.get_uniform_buffer_object(
                    exec_buffer.thread_index,
                    true,
                    &mut uni_descr_set,
                    real_draw_count,
                    &cmd.quad_info[render_offset] as *const SQuadRenderInfo as *const c_void,
                    real_draw_count * std::mem::size_of::<SQuadRenderInfo>(),
                    self.cur_image_index,
                ) {
                    return false;
                }

                unsafe {
                    self.ash_vk.vk_device.cmd_bind_descriptor_sets(
                        *command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipe_layout,
                        if is_textured { 1 } else { 0 },
                        &[uni_descr_set.descriptor],
                        &[],
                    );
                }
                if render_offset > 0 {
                    let quad_offset: i32 = (cmd.quad_offset + render_offset) as i32;
                    unsafe {
                        self.ash_vk.vk_device.cmd_push_constants(
                            *command_buffer,
                            pipe_layout,
                            vk::ShaderStageFlags::VERTEX,
                            (std::mem::size_of::<SUniformQuadGPos>() - std::mem::size_of::<i32>())
                                as u32,
                            std::slice::from_raw_parts(
                                &quad_offset as *const i32 as *const u8,
                                std::mem::size_of::<i32>(),
                            ),
                        );
                    }
                }
            }

            unsafe {
                self.ash_vk.vk_device.cmd_draw_indexed(
                    *command_buffer,
                    (real_draw_count * 6) as u32,
                    1,
                    index_offset as u32,
                    0,
                    0,
                );
            }

            render_offset += real_draw_count;
            draw_count -= real_draw_count;
        }

        return true;
    }

    /*
                fn Cmd_RenderText_FillExecuteBuffer(exec_buffer: &mut SRenderCommandExecuteBuffer, cmd: &CommandRenderText)
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

                #[must_use] fn Cmd_RenderText(&mut self,cmd: &CommandRenderText, exec_buffer: &SRenderCommandExecuteBuffer ) -> bool
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

                    let vertex_buffers = [exec_buffer.m_Buffer];
                    let buffer_offsets = [exec_buffer.m_BufferOff as vk::DeviceSize];
                    unsafe { self.m_VKDevice.cmd_bind_vertex_buffers(CommandBuffer, 0, 1, vertex_buffers.as_ptr(), buffer_offsets.as_ptr()); }

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

    fn buffer_object_fill_execute_buffer(
        &self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        state: &State,
        buffer_object_index: u128,
        draw_calls: usize,
    ) {
        let buffer_object = self
            .device
            .buffer_objects
            .get(&buffer_object_index)
            .unwrap();

        exec_buffer.buffer = buffer_object.cur_buffer;
        exec_buffer.buffer_off = buffer_object.cur_buffer_offset;

        let is_textured: bool = Self::get_is_textured(state);
        if is_textured {
            let address_mode_index: usize = Self::get_address_mode_index(state);
            exec_buffer.descriptors[0] = self
                .device
                .textures
                .get(&state.texture_index.unwrap())
                .unwrap()
                .vk_standard_textured_descr_sets[address_mode_index]
                .clone();
        }

        exec_buffer.index_buffer = self.render_index_buffer;

        exec_buffer.estimated_render_call_count = draw_calls;

        self.exec_buffer_fill_dynamic_states(&state, exec_buffer);
    }

    fn cmd_render_quad_container_ex_fill_execute_buffer(
        &self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &CommandRenderQuadContainerEx,
    ) {
        self.buffer_object_fill_execute_buffer(exec_buffer, &cmd.state, cmd.buffer_object_index, 1);
    }

    #[must_use]
    fn cmd_render_quad_container_ex(
        &mut self,
        cmd: &CommandRenderQuadContainerEx,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::get_state_matrix(&cmd.state, &mut m);

        let is_rotationless: bool = !(cmd.rotation != 0.0);
        let mut is_textured: bool = Default::default();
        let mut blend_mode_index: usize = Default::default();
        let mut dynamic_index: usize = Default::default();
        let mut address_mode_index: usize = Default::default();
        let mut render_pass_type_index: usize = Default::default();
        let mut sub_pass_index: usize = Default::default();
        Self::get_state_indices(
            exec_buffer,
            &cmd.state,
            &mut is_textured,
            &mut blend_mode_index,
            &mut dynamic_index,
            &mut address_mode_index,
            &mut render_pass_type_index,
            &mut sub_pass_index,
        );
        let mut command_buffer: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.get_graphic_command_buffer(
            &mut command_buffer,
            exec_buffer.thread_index as usize,
            exec_buffer.sub_pass_index,
        ) {
            return false;
        }
        let command_buffer = unsafe { &mut *command_buffer };
        let (pipeline, pipe_layout) = Self::get_pipeline_and_layout(
            if is_rotationless {
                &self.prim_ex_rotationless_pipeline
            } else {
                &self.prim_ex_pipeline
            },
            is_textured,
            blend_mode_index as usize,
            dynamic_index as usize,
            render_pass_type_index,
            sub_pass_index,
        );

        Self::bind_pipeline(
            &self.ash_vk.vk_device,
            &mut self.last_pipeline_per_thread,
            exec_buffer.thread_index,
            *command_buffer,
            exec_buffer,
            *pipeline,
            &cmd.state,
        );

        let vertex_buffers = [exec_buffer.buffer];
        let buffer_offsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            self.ash_vk.vk_device.cmd_bind_vertex_buffers(
                *command_buffer,
                0,
                &vertex_buffers,
                &buffer_offsets,
            );
        }

        let index_offset = cmd.offset as vk::DeviceSize;

        unsafe {
            self.ash_vk.vk_device.cmd_bind_index_buffer(
                *command_buffer,
                exec_buffer.index_buffer,
                index_offset,
                vk::IndexType::UINT32,
            );
        }

        if is_textured {
            unsafe {
                self.ash_vk.vk_device.cmd_bind_descriptor_sets(
                    *command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    *pipe_layout,
                    0,
                    &[exec_buffer.descriptors[0].descriptor],
                    &[],
                );
            }
        }

        let mut push_constant_vertex = SUniformPrimExGPos::default();
        let mut vertex_push_constant_size: usize = std::mem::size_of::<SUniformPrimExGPos>();

        let push_constant_color: SUniformPrimExGVertColor = cmd.vertex_color;
        push_constant_vertex.base.pos = m;

        if !is_rotationless {
            push_constant_vertex.rotation = cmd.rotation;
            push_constant_vertex.center = cmd.center;
        } else {
            vertex_push_constant_size = std::mem::size_of::<SUniformPrimExGPosRotationless>();
        }

        unsafe {
            self.ash_vk.vk_device.cmd_push_constants(
                *command_buffer,
                *pipe_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                std::slice::from_raw_parts(
                    &push_constant_vertex as *const SUniformPrimExGPos as *const u8,
                    vertex_push_constant_size,
                ),
            );
        }
        unsafe {
            self.ash_vk.vk_device.cmd_push_constants(
                *command_buffer,
                *pipe_layout,
                vk::ShaderStageFlags::FRAGMENT,
                (std::mem::size_of::<SUniformPrimExGPos>()
                    + std::mem::size_of::<SUniformPrimExGVertColorAlign>()) as u32,
                std::slice::from_raw_parts(
                    &push_constant_color as *const ColorRGBA as *const u8,
                    std::mem::size_of::<SUniformPrimExGVertColor>(),
                ),
            );
        }

        unsafe {
            self.ash_vk.vk_device.cmd_draw_indexed(
                *command_buffer,
                (cmd.draw_num) as u32,
                1,
                0,
                0,
                0,
            );
        }

        return true;
    }

    fn cmd_render_quad_container_as_sprite_multiple_fill_execute_buffer(
        &self,
        exec_buffer: &mut SRenderCommandExecuteBuffer,
        cmd: &CommandRenderQuadContainerAsSpriteMultiple,
    ) {
        self.buffer_object_fill_execute_buffer(
            exec_buffer,
            &cmd.state,
            cmd.buffer_object_index,
            ((cmd.draw_count - 1) / GRAPHICS_MAX_PARTICLES_RENDER_COUNT) + 1,
        );
    }

    #[must_use]
    fn cmd_render_quad_container_as_sprite_multiple(
        &mut self,
        cmd: &CommandRenderQuadContainerAsSpriteMultiple,
        exec_buffer: &SRenderCommandExecuteBuffer,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::get_state_matrix(&cmd.state, &mut m);

        let can_be_pushed: bool = cmd.draw_count <= 1;

        let mut is_textured: bool = Default::default();
        let mut blend_mode_index: usize = Default::default();
        let mut dynamic_index: usize = Default::default();
        let mut address_mode_index: usize = Default::default();
        let mut render_pass_type_index: usize = Default::default();
        let mut sub_pass_index: usize = Default::default();
        Self::get_state_indices(
            exec_buffer,
            &cmd.state,
            &mut is_textured,
            &mut blend_mode_index,
            &mut dynamic_index,
            &mut address_mode_index,
            &mut render_pass_type_index,
            &mut sub_pass_index,
        );
        let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.get_graphic_command_buffer(
            &mut command_buffer_ptr,
            exec_buffer.thread_index as usize,
            exec_buffer.sub_pass_index,
        ) {
            return false;
        }
        let command_buffer = unsafe { &mut *command_buffer_ptr };
        let (pipeline, pipe_layout) = Self::get_pipeline_and_layout(
            if can_be_pushed {
                &self.sprite_multi_push_pipeline
            } else {
                &self.sprite_multi_pipeline
            },
            is_textured,
            blend_mode_index as usize,
            dynamic_index as usize,
            render_pass_type_index,
            sub_pass_index,
        );
        let (pipeline, pipe_layout) = (*pipeline, *pipe_layout);

        Self::bind_pipeline(
            &self.ash_vk.vk_device,
            &mut self.last_pipeline_per_thread,
            exec_buffer.thread_index,
            *command_buffer,
            exec_buffer,
            pipeline,
            &cmd.state,
        );

        let vertex_buffers = [exec_buffer.buffer];
        let buffer_offsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            self.ash_vk.vk_device.cmd_bind_vertex_buffers(
                *command_buffer,
                0,
                &vertex_buffers,
                &buffer_offsets,
            );
        }

        let index_offset = cmd.offset as vk::DeviceSize;
        unsafe {
            self.ash_vk.vk_device.cmd_bind_index_buffer(
                *command_buffer,
                exec_buffer.index_buffer,
                index_offset,
                vk::IndexType::UINT32,
            );
        }

        unsafe {
            self.ash_vk.vk_device.cmd_bind_descriptor_sets(
                *command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipe_layout,
                0,
                &[exec_buffer.descriptors[0].descriptor],
                &[],
            );
        }

        if can_be_pushed {
            let mut push_constant_vertex = SUniformSpriteMultiPushGPos::default();

            let push_constant_color: SUniformSpriteMultiPushGVertColor = cmd.vertex_color;

            push_constant_vertex.base.pos = m;
            push_constant_vertex.base.center = cmd.center;

            for i in 0..cmd.draw_count {
                push_constant_vertex.psr[i] = vec4 {
                    x: cmd.render_info[i].pos.x,
                    y: cmd.render_info[i].pos.y,
                    z: cmd.render_info[i].scale,
                    w: cmd.render_info[i].rotation,
                };
            }
            unsafe {
                self.ash_vk.vk_device.cmd_push_constants(
                    *command_buffer,
                    pipe_layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    std::slice::from_raw_parts(
                        &push_constant_vertex as *const SUniformSpriteMultiPushGPos as *const u8,
                        std::mem::size_of::<SUniformSpriteMultiPushGPosBase>()
                            + std::mem::size_of::<vec4>() * cmd.draw_count,
                    ),
                );
            }
            unsafe {
                self.ash_vk.vk_device.cmd_push_constants(
                    *command_buffer,
                    pipe_layout,
                    vk::ShaderStageFlags::FRAGMENT,
                    std::mem::size_of::<SUniformSpriteMultiPushGPos>() as u32,
                    std::slice::from_raw_parts(
                        &push_constant_color as *const ColorRGBA as *const u8,
                        std::mem::size_of::<SUniformSpriteMultiPushGVertColor>(),
                    ),
                );
            }
        } else {
            let mut push_constant_vertex = SUniformSpriteMultiGPos::default();

            let push_constant_color: SUniformSpriteMultiGVertColor = cmd.vertex_color;

            push_constant_vertex.pos = m;
            push_constant_vertex.center = cmd.center;

            unsafe {
                self.ash_vk.vk_device.cmd_push_constants(
                    *command_buffer,
                    pipe_layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    std::slice::from_raw_parts(
                        &push_constant_vertex as *const SUniformSpriteMultiGPos as *const u8,
                        std::mem::size_of::<SUniformSpriteMultiGPos>(),
                    ),
                );
            }
            unsafe {
                self.ash_vk.vk_device.cmd_push_constants(
                    *command_buffer,
                    pipe_layout,
                    vk::ShaderStageFlags::FRAGMENT,
                    (std::mem::size_of::<SUniformSpriteMultiGPos>()
                        + std::mem::size_of::<SUniformSpriteMultiGVertColorAlign>())
                        as u32,
                    std::slice::from_raw_parts(
                        &push_constant_color as *const SUniformSpriteMultiGVertColor as *const u8,
                        std::mem::size_of::<SUniformSpriteMultiGVertColor>(),
                    ),
                );
            }
        }

        let rsp_count: usize = 512;
        let mut draw_count = cmd.draw_count;
        let mut render_offset: usize = 0;

        while draw_count > 0 {
            let uniform_count = if draw_count > rsp_count {
                rsp_count
            } else {
                draw_count
            };

            if !can_be_pushed {
                // create uniform buffer
                let mut uni_descr_set = SDeviceDescriptorSet::default();
                if !self.get_uniform_buffer_object(
                    exec_buffer.thread_index,
                    false,
                    &mut uni_descr_set,
                    uniform_count,
                    &cmd.render_info[render_offset] as *const SRenderSpriteInfo as *const c_void,
                    uniform_count * std::mem::size_of::<SRenderSpriteInfo>(),
                    self.cur_image_index,
                ) {
                    return false;
                }

                unsafe {
                    self.ash_vk.vk_device.cmd_bind_descriptor_sets(
                        *command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipe_layout,
                        1,
                        &[uni_descr_set.descriptor],
                        &[],
                    );
                }
            }

            unsafe {
                self.ash_vk.vk_device.cmd_draw_indexed(
                    *command_buffer,
                    (cmd.draw_num) as u32,
                    uniform_count as u32,
                    0,
                    0,
                    0,
                );
            }

            render_offset += uniform_count;
            draw_count -= uniform_count;
        }

        return true;
    }
    /*
            #[must_use] fn Cmd_WindowCreateNtf(&mut self,cmd: &CommandWindowCreateNtf) -> bool
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

            #[must_use] fn Cmd_WindowDestroyNtf(&mut self,cmd: &CommandWindowDestroyNtf) -> bool
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
        window: &winit::window::Window,
        _gpu_list: &mut TTWGraphicsGPUList,
        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,

        canvas_width: f64,
        canvas_height: f64,

        runtime_threadpool: &Arc<rayon::ThreadPool>,

        sys: &System,

        options: &Options,
    ) -> Result<Pin<Box<Self>>, ArrayString<4096>> {
        let dbg_mode = options.dbg_gfx; // TODO config / options
        let dbg = Arc::new(AtomicU8::new(dbg_mode as u8));
        let error = Arc::new(Mutex::new(Error::default()));
        let logger = sys.log.logger("vulkan");
        let vk_res = Self::init_vulkan_sdl(
            window,
            canvas_width,
            canvas_height,
            dbg_mode,
            &error,
            &logger,
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
            config,
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

        let render_threads: Vec<Arc<SRenderThread>> = Default::default();
        let thread_command_lists: Vec<Vec<SRenderCommandExecuteBuffer>> = Default::default();
        let thread_helper_had_commands: Vec<bool> = Default::default();

        let swap_chain = ash::extensions::khr::Swapchain::new(&instance, &device);

        let mut res = Box::pin(Self {
            dbg: dbg.clone(),
            gfx_vsync: Default::default(),
            shader_files: Default::default(),
            // m_pGPUList: gpu_list,
            next_multi_sampling_count: Default::default(),
            next_multi_sampling_second_pass_count: Default::default(),
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
            cur_stream_vertex_byte_offset: Default::default(),
            cur_render_call_count_in_pipe: Default::default(),
            commands_in_pipe: Default::default(),
            render_calls_in_pipe: Default::default(),
            last_commands_in_pipe_thread_index: Default::default(),
            render_threads,
            swap_chain_image_view_list: Default::default(),
            swap_chain_multi_sampling_images: Default::default(),
            image_list_for_double_pass: Default::default(),
            multi_sampling_images_for_double_pass: Default::default(),
            stencil_list_for_pass_transition: Default::default(),
            stencil_format: Default::default(),
            framebuffer_list: Default::default(),
            framebuffer_double_pass_list: Default::default(),
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

            ash_vk: VulkanBackendAsh {
                vk_instance: instance.clone(),
                _vk_entry: entry.clone(),
                surface: ash_surface,
                vk_device: device.clone(),
                vk_swap_chain_ash: swap_chain,
            },

            vk_gpu: phy_gpu,
            vk_graphics_queue_index: graphics_queue_index,
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
                config,
                &sys.log,
            ),
            vk_graphics_queue: graphics_queue,
            vk_present_queue: presentation_queue,
            vk_present_surface: surface,
            vk_swap_img_and_viewport_extent: Default::default(),
            _debug_messenger: Default::default(),
            standard_pipeline: Default::default(),
            standard_line_pipeline: Default::default(),
            standard_stencil_only_pipeline: Default::default(),
            standard_stencil_pipeline: Default::default(),
            standard_3d_pipeline: Default::default(),
            blur_pipeline: Default::default(),
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
            current_render_pass_type: RenderPassType::Single,
            current_sub_pass_index: 0,
            vk_render_pass_single_pass: Default::default(),
            vk_render_pass_double_pass: Default::default(),
            vk_surf_format: Default::default(),
            vk_swap_chain_khr: Default::default(),
            vk_swap_chain_images: Default::default(),
            cur_frames: Default::default(),
            cur_image_index: Default::default(),
            canvas_width,
            canvas_height,
            //m_pWindow: window.clone(),
            clear_color: Default::default(),
            thread_command_lists,
            thread_helper_had_commands,
            //m_aCommandCallbacks: Default::default(),
            error: error,
            check_res: Default::default(),

            logger,

            runtime_threadpool: runtime_threadpool.clone(),
        });

        // start threads
        assert!(
            thread_count != 2,
            "Either use 1 main thread or at least 2 extra rendering threads."
        );
        if thread_count > 1 {
            res.thread_command_lists
                .resize(thread_count - 1, Default::default());
            res.thread_helper_had_commands
                .resize(thread_count - 1, false);
            for thread_command_list in &mut res.thread_command_lists {
                thread_command_list.reserve(256);
            }

            for _ in 0..thread_count - 1 {
                let render_thread = Arc::new(SRenderThread {
                    inner: Mutex::new(SRenderThreadInner {
                        is_rendering: false,
                        thread: None,
                        finished: false,
                        started: false,
                    }),
                    cond: Condvar::new(),
                });
                res.render_threads.push(render_thread);
            }
            for i in 0..thread_count - 1 {
                let unsafe_vk_backend = ThreadVkBackendWrapper {
                    backend: &mut *res.as_mut(),
                };
                let render_thread = &res.render_threads[i];
                let mut g = render_thread.inner.lock().unwrap();
                g.thread = Some(std::thread::spawn(move || {
                    Self::run_thread(unsafe_vk_backend, i)
                }));
                // wait until thread started
                let _g = render_thread
                    .cond
                    .wait_while(g, |render_thread| !render_thread.started)
                    .unwrap();
            }
        }

        Ok(res)
    }
    /*
            #[must_use] fn Cmd_PostShutdown(&mut self,const CCommandProcessorFragment_GLBase::CommandPostShutdown *pCommand) -> bool
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

    fn run_thread(selfi_ptr: ThreadVkBackendWrapper, thread_index: usize) {
        let selfi = unsafe { &mut *selfi_ptr.backend };
        let thread = &selfi.render_threads[thread_index];
        let mut guard = thread.inner.lock().unwrap();
        guard.started = true;
        thread.cond.notify_one();

        while !guard.finished {
            guard = thread
                .cond
                .wait_while(guard, |thread| -> bool {
                    return !thread.is_rendering && !thread.finished;
                })
                .unwrap();
            thread.cond.notify_one();

            // set this to true, if you want to benchmark the render thread times
            let benchmark_render_threads = false;
            let _thread_render_time = Duration::from_nanos(0);
            /*TODO! if(IsVerbose(&*self.dbg) && s_BenchmarkRenderThreads)
            {
                ThreadRenderTime = time_get_nanoseconds();
            }*/

            if !guard.finished {
                let mut has_error_from_cmd: bool = false;
                for _next_cmd in &selfi.thread_command_lists[thread_index] {
                    // TODO! if (!self.CommandCB(&NextCmd.0, &NextCmd.1))
                    {
                        // an error occured, the thread will not continue execution
                        has_error_from_cmd = true;
                        break;
                    }
                }
                selfi.thread_command_lists[thread_index].clear();

                if !has_error_from_cmd
                    && selfi.used_thread_draw_command_buffer[thread_index + 1]
                        [selfi.cur_image_index as usize]
                {
                    let graphic_thread_command_buffer = &mut selfi.thread_draw_command_buffers
                        [thread_index + 1][selfi.cur_image_index as usize];
                    unsafe {
                        selfi
                            .ash_vk
                            .vk_device
                            .end_command_buffer(*graphic_thread_command_buffer)
                            .unwrap();
                    }
                }
            }

            if is_verbose(&*selfi.dbg) && benchmark_render_threads {
                //self.sys.log ("vulkan").msg("render thread ").msg(ThreadIndex).msg(" took ").msg(time_get_nanoseconds() - ThreadRenderTime).msg(" ns to finish");
            }

            guard.is_rendering = false;
        }
    }

    pub fn get_mt_backend(&self) -> VulkanBackendMt {
        VulkanBackendMt {
            mem_allocator: self.device.mem_allocator.clone(),
        }
    }
}

const STREAM_DATA_MEMORY_BLOCK_SIZE: usize =
    StreamDataMax::MaxVertices as usize * std::mem::size_of::<GlVertex>();

impl DriverBackendInterface for VulkanBackend {
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

        self.device.global_texture_lod_bias = -500; // TODO! g_Config.m_GfxGLTextureLODBIAS;

        self.device.config.multi_sampling_count =
            0 /* TODO! g_Config.m_GfxFsaaSamples */ & 0xFFFFFFFE; // ignore the uneven bit, only even multi sampling works
        self.device.config.multi_sampling_second_pass_count =
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

        let mut indices_upload: Vec<u32> = Vec::new();
        indices_upload.reserve(StreamDataMax::MaxVertices as usize / 4 * 6);
        let mut primitive_count: u32 = 0;
        for _ in (0..(StreamDataMax::MaxVertices as usize / 4 * 6) as usize).step_by(6) {
            indices_upload.push(primitive_count);
            indices_upload.push(primitive_count + 1);
            indices_upload.push(primitive_count + 2);
            indices_upload.push(primitive_count);
            indices_upload.push(primitive_count + 2);
            indices_upload.push(primitive_count + 3);
            primitive_count += 4;
        }

        if !self.prepare_frame() {
            return Err(ArrayString::from_str("Failed to prepare frame.").unwrap());
        }

        // TODO: ??? looks completely stupid.. better handle all errors instead
        if self.error.lock().unwrap().has_error {
            return Err(ArrayString::from_str("This is a stupid call.").unwrap());
        }

        if !self.device.create_index_buffer(
            indices_upload.as_mut_ptr() as *mut c_void,
            std::mem::size_of::<u32>() * indices_upload.len(),
            &mut self.index_buffer,
            &mut self.index_buffer_memory,
            0,
        ) {
            return Err(ArrayString::from_str("Failed to create index buffer.").unwrap());
        }
        if !self.device.create_index_buffer(
            indices_upload.as_mut_ptr() as *mut c_void,
            std::mem::size_of::<u32>() * indices_upload.len(),
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

    fn destroy(mut self) {
        unsafe { self.ash_vk.vk_device.device_wait_idle().unwrap() };

        self.device
            .destroy_index_buffer(&mut self.index_buffer, &mut self.index_buffer_memory);
        self.device.destroy_index_buffer(
            &mut self.render_index_buffer,
            &mut self.render_index_buffer_memory,
        );

        self.cleanup_vulkan::<true>();
        self.cleanup_vulkan_sdl();
    }

    #[must_use]
    fn get_presented_image_data(
        &mut self,
        width: &mut u32,
        height: &mut u32,
        dest_data_buffer: &mut Vec<u8>,
    ) -> anyhow::Result<ImageFormat> {
        self.get_presented_image_data_impl(width, height, dest_data_buffer, false, false)
    }

    #[must_use]
    fn run_command(&mut self, cmd: &AllCommands) -> ERunCommandReturnTypes {
        /* TODO! no locking pls if(self.m_HasError)
        {
            // ignore all further commands
            return ERunCommandReturnTypes::RUN_COMMAND_COMMAND_ERROR;
        }*/

        //let CallbackObj = &mut  self.m_aCommandCallbacks[CommandBufferCMDOff(CCommandBuffer::ECommandBufferCMD(Cmd))];
        let mut buffer = SRenderCommandExecuteBuffer::default();
        buffer.raw_command = cmd;
        buffer.thread_index = 0;
        buffer.render_pass_index = self.current_render_pass_type as usize;
        buffer.sub_pass_index = self.current_sub_pass_index;

        if self.cur_command_in_pipe + 1 == self.commands_in_pipe {
            self.last_commands_in_pipe_thread_index = usize::MAX;
        }

        let mut can_start_thread: bool = false;
        if let AllCommands::Render(_) = cmd {
            let force_single_thread: bool = self.last_commands_in_pipe_thread_index == usize::MAX;

            let potentially_next_thread: usize =
                ((self.cur_command_in_pipe * (self.thread_count - 1)) / self.commands_in_pipe) + 1;
            if potentially_next_thread - 1 > self.last_commands_in_pipe_thread_index {
                can_start_thread = true;
                self.last_commands_in_pipe_thread_index = potentially_next_thread - 1;
            }
            buffer.thread_index = if self.thread_count > 1 && !force_single_thread {
                self.last_commands_in_pipe_thread_index + 1
            } else {
                0
            };
            self.fill_execute_buffer(&cmd, &mut buffer);
            self.cur_render_call_count_in_pipe += buffer.estimated_render_call_count;
        }
        let mut is_misc_cmd = false;
        if let AllCommands::Misc(_) = cmd {
            is_misc_cmd = true;
        }
        if is_misc_cmd || (buffer.thread_index == 0 && !self.rendering_paused) {
            if !self.command_cb(&cmd, &buffer) {
                // an error occured, stop this command and ignore all further commands
                return ERunCommandReturnTypes::CmdError;
            }
        } else if !self.rendering_paused {
            if can_start_thread {
                self.start_render_thread(self.last_commands_in_pipe_thread_index - 1);
            }
            self.thread_command_lists[buffer.thread_index as usize - 1].push(buffer);
        }

        self.cur_command_in_pipe += 1;
        return ERunCommandReturnTypes::CmdHandled;
    }

    fn start_commands(
        &mut self,
        _backend_buffer: &BackendCommands,
        stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>,
        command_count: usize,
        estimated_render_call_count: usize,
    ) {
        self.commands_in_pipe = command_count;
        self.render_calls_in_pipe = estimated_render_call_count;
        self.cur_command_in_pipe = 0;
        self.cur_render_call_count_in_pipe = 0;
        self.device.update_stream_vertex_buffer(
            STREAM_DATA_MEMORY_BLOCK_SIZE,
            stream_data.borrow().vertices_count() * std::mem::size_of::<GlVertex>(),
            self.cur_image_index,
        );
    }

    fn end_commands(&mut self) -> Result<&'static mut [GlVertex], ()> {
        self.finish_render_threads();
        self.commands_in_pipe = 0;
        self.render_calls_in_pipe = 0;

        let mut vk_buffer: vk::Buffer = Default::default();
        let mut vk_buffer_mem: SDeviceMemoryBlock = Default::default();
        let mut buffer_off: usize = 0;
        let mut memory_ptr: *mut u8 = std::ptr::null_mut();
        if !self.device.create_stream_vertex_buffer(
            &mut vk_buffer,
            &mut vk_buffer_mem,
            &mut buffer_off,
            &mut memory_ptr,
            STREAM_DATA_MEMORY_BLOCK_SIZE,
            self.cur_image_index,
        ) {
            return Err(());
        }

        self.cur_stream_vertex_byte_offset = buffer_off;
        Ok(unsafe {
            std::slice::from_raw_parts_mut(
                memory_ptr.offset(buffer_off as isize) as *mut GlVertex,
                StreamDataMax::MaxVertices as usize,
            )
        })
    }
}

#[derive(Debug)]
pub struct VulkanBackendMt {
    pub mem_allocator: Arc<std::sync::Mutex<Option<VulkanAllocator>>>,
}

#[derive(Debug)]
pub struct VulkanBackendDellocator {
    pub mem_allocator: Arc<std::sync::Mutex<Option<VulkanAllocator>>>,
}

impl GraphicsBackendMemoryStaticCleaner for VulkanBackendDellocator {
    fn destroy(&self, mem: &'static mut [u8]) {
        self.mem_allocator
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .free_mem_raw(mem.as_ptr() as *mut c_void);
    }
}

impl GraphicsBackendMtInterface for VulkanBackendMt {
    fn mem_alloc(
        &self,
        alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> GraphicsBackendMemory {
        let buffer_data: *const c_void = std::ptr::null();
        let allocator_clone = self.mem_allocator.clone();
        let mut mem_allocator = self.mem_allocator.lock().unwrap();
        let allocator = mem_allocator.as_mut().unwrap();
        match alloc_type {
            GraphicsMemoryAllocationType::Buffer => {
                let res_block = allocator
                    .get_staging_buffer(buffer_data, req_size as vk::DeviceSize, u32::MAX)
                    .unwrap();
                GraphicsBackendMemory::Static(GraphicsBackendMemoryStatic {
                    mem: Some(unsafe {
                        std::slice::from_raw_parts_mut(res_block.mapped_buffer as *mut u8, req_size)
                    }),
                    deallocator: Some(Box::new(VulkanBackendDellocator {
                        mem_allocator: allocator_clone,
                    })),
                })
            }
            GraphicsMemoryAllocationType::Texture => {
                let res_block = allocator
                    .get_staging_buffer_image(buffer_data, req_size as vk::DeviceSize, u32::MAX)
                    .unwrap();
                GraphicsBackendMemory::Static(GraphicsBackendMemoryStatic {
                    mem: Some(unsafe {
                        std::slice::from_raw_parts_mut(res_block.mapped_buffer as *mut u8, req_size)
                    }),
                    deallocator: Some(Box::new(VulkanBackendDellocator {
                        mem_allocator: allocator_clone,
                    })),
                })
            }
        }
    }
}
