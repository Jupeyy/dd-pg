use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::CStr,
    num::NonZeroUsize,
    os::raw::c_void,
    pin::Pin,
    rc::Rc,
    sync::{
        atomic::{AtomicU32, AtomicU64, AtomicU8},
        Arc, Condvar, Mutex,
    },
};

use base_log::log::{LogLevel, SystemLogGroup, SystemLogInterface};
use graphics_backend_traits::{
    traits::{DriverBackendInterface, GraphicsBackendMtInterface},
    types::BackendCommands,
};
use graphics_base_traits::traits::GraphicsStreamDataInterface;

use anyhow::anyhow;
use graphics_types::{
    command_buffer::{
        AllCommands, CommandClear, CommandCreateBufferObject, CommandDeleteBufferObject,
        CommandIndicesRequiredNumNotify, CommandRecreateBufferObject, CommandRender,
        CommandRenderBorderTile, CommandRenderBorderTileLine, CommandRenderQuadContainer,
        CommandRenderQuadContainerAsSpriteMultiple, CommandRenderQuadLayer, CommandRenderTileLayer,
        CommandTextureCreate, CommandTextureDestroy, CommandTextureUpdate, CommandUpdateViewport,
        Commands, CommandsRender, CommandsRenderMap, CommandsRenderQuadContainer,
        CommandsRenderStream, PrimType, SBackendCapabilites, SQuadRenderInfo, SRenderSpriteInfo,
        StreamDataMax, GRAPHICS_MAX_PARTICLES_RENDER_COUNT, GRAPHICS_MAX_QUADS_RENDER_COUNT,
    },
    rendering::{BlendType, ColorRGBA, GlColorf, GlVertex, State, WrapType, WRAP_TYPE_COUNT},
    types::{
        GraphicsBackendMemory, GraphicsBackendMemoryStatic, GraphicsBackendMemoryStaticCleaner,
        GraphicsMemoryAllocationType, ImageFormat,
    },
};
use num_traits::FromPrimitive;

use ash::vk::{self};

use crate::{
    backends::vulkan::{
        barriers::image_barrier,
        vulkan_types::{SRenderThreadInner, RENDER_PASS_TYPE_COUNT},
    },
    window::{BackendSurface, BackendSwapchain, BackendWindow},
};

use base::{benchmark::Benchmark, shared_index::SharedIndexGetIndexUnsafe, system::System};
use config::config::EDebugGFXModes;
use math::math::vector::{vec2, vec4};

const SHADER_MAIN_FUNC_NAME: [u8; 5] = [b'm', b'a', b'i', b'n', b'\0'];

use super::{
    buffer::Buffer,
    command_buffer::CommandBuffers,
    command_pool::{AutoCommandBuffer, AutoCommandBufferType, CommandPool},
    common::{
        tex_format_to_image_color_channel_count, texture_format_to_vulkan_format, EGFXErrorType,
        TTWGraphicsGPUList,
    },
    descriptor_set::DescriptorSet,
    fence::Fence,
    frame::Frame,
    image::{Image, ImageFakeForSwapchainImgs},
    instance::Instance,
    logical_device::LogicalDevice,
    mapped_memory::MappedMemory,
    memory_block::SDeviceMemoryBlock,
    phy_device::PhyDevice,
    queue::Queue,
    render_pass::{RenderSetup, RenderSetupGroup},
    semaphore::Semaphore,
    streamed_uniform::StreamedUniform,
    utils::copy_color_attachment_to_present_src,
    vulkan_allocator::{
        VulkanAllocator, VulkanAllocatorImageCacheEntryData, VulkanDeviceInternalMemory,
    },
    vulkan_dbg::is_verbose,
    vulkan_device::Device,
    vulkan_error::{CheckResult, Error},
    vulkan_limits::Limits,
    vulkan_types::{
        CTexture, EMemoryBlockUsage, ESupportedSamplerTypes, EVulkanBackendAddressModes,
        EVulkanBackendBlendModes, EVulkanBackendClipModes, EVulkanBackendTextureModes,
        PipelineContainer, RenderCommandExecuteBuffer, RenderPassType, SDeviceDescriptorPools,
        SRenderThread, SShaderFileCache, SShaderModule, SSwapImgViewportExtent, SwapChainImageBase,
        SwapChainImageFull, TThreadCommandGroup, TextureData,
    },
    vulkan_uniform::{
        SUniformGBlur, SUniformGPos, SUniformPrimExGPos, SUniformPrimExGPosRotationless,
        SUniformPrimExGVertColor, SUniformPrimExGVertColorAlign, SUniformQuadGPos,
        SUniformQuadPushGBufferObject, SUniformQuadPushGPos, SUniformSpriteMultiGPos,
        SUniformSpriteMultiGVertColor, SUniformSpriteMultiGVertColorAlign,
        SUniformSpriteMultiPushGPos, SUniformSpriteMultiPushGPosBase,
        SUniformSpriteMultiPushGVertColor, SUniformTileGPos, SUniformTileGPosBorder,
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

pub struct VulkanBackendAsh {
    vk_swap_chain_ash: BackendSwapchain,
    instance: Arc<Instance>,
    surface: BackendSurface,
    vk_device: Arc<LogicalDevice>,
}

impl std::fmt::Debug for VulkanBackendAsh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanBackendAsh").finish()
    }
}

#[derive(Debug, Default)]
pub struct VulkanFetchFramebuffer {
    get_presented_img_data_helper_mem: Option<Arc<SDeviceMemoryBlock>>,
    get_presented_img_data_helper_image: Option<Arc<Image>>,
    get_presented_img_data_helper_mapped_memory: Option<Arc<MappedMemory>>,
    get_presented_img_data_helper_mapped_layout_offset: vk::DeviceSize,
    get_presented_img_data_helper_mapped_layout_pitch: vk::DeviceSize,
    get_presented_img_data_helper_width: u32,
    get_presented_img_data_helper_height: u32,
    get_presented_img_data_helper_fence: vk::Fence,
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

    recreate_swap_chain: bool,
    swap_chain_created: bool,
    rendering_paused: bool,
    has_dynamic_viewport: bool,
    dynamic_viewport_offset: vk::Offset2D,

    dynamic_viewport_size: vk::Extent2D,

    index_buffer: Option<Arc<Buffer>>,
    index_buffer_memory: Option<Arc<SDeviceMemoryBlock>>,

    render_index_buffer: Option<Arc<Buffer>>,
    render_index_buffer_memory: Option<Arc<SDeviceMemoryBlock>>,
    cur_render_index_primitive_count: usize,

    fetch_frame_buffer: VulkanFetchFramebuffer,

    thread_count: usize,

    cur_render_call_count_in_pipe: usize,

    cur_stream_vertex_byte_offset: usize,
    commands_in_pipe: usize,
    render_calls_in_pipe: usize,

    last_render_thread_index: usize,

    render_threads: Vec<Arc<SRenderThread>>,
    render_thread_infos: Vec<(Arc<AtomicU32>, Arc<AtomicU32>)>,

    main_render_command_buffer: Option<AutoCommandBuffer>,
    frame: Arc<spin::Mutex<Frame>>,

    // swapped by use case
    wait_semaphores: Vec<Arc<Semaphore>>,
    sig_semaphores: Vec<Arc<Semaphore>>,

    memory_sempahores: Vec<Arc<Semaphore>>,

    frame_fences: Vec<Arc<Fence>>,
    image_fences: Vec<Option<Arc<Fence>>>,

    order_id_gen: usize,
    cur_frame: u64,
    image_last_frame_check: Vec<u64>,

    last_presented_swap_chain_image_index: u32,

    ash_vk: VulkanBackendAsh,

    vk_gpu: Arc<PhyDevice>,
    device: Device,
    queue: Arc<spin::Mutex<Queue>>,
    vk_swap_img_and_viewport_extent: SSwapImgViewportExtent,

    debug_messenger: vk::DebugUtilsMessengerEXT,

    command_pool: Rc<CommandPool>,

    render: RenderSetupGroup,

    cur_frames: u32,
    cur_image_index: u32,

    canvas_width: f64,
    canvas_height: f64,

    // TODO! m_pWindow: sdl2::video::Window,
    clear_color: [f32; 4],

    current_command_group: TThreadCommandGroup,
    command_groups: Vec<TThreadCommandGroup>,

    /************************
     * ERROR MANAGEMENT
     ************************/
    error: Arc<std::sync::Mutex<Error>>,
    check_res: CheckResult,

    logger: SystemLogGroup,

    _runtime_threadpool: Arc<rayon::ThreadPool>,
}

pub struct ThreadVkBackendWrapper {
    render: *const RenderSetupGroup,
}

unsafe impl Send for ThreadVkBackendWrapper {}
unsafe impl Sync for ThreadVkBackendWrapper {}

impl VulkanBackend {
    /************************
     * ERROR MANAGEMENT HELPER
     ************************/
    // TODO fn ErroneousCleanup(&mut self )  { self.CleanupVulkanSDL(); }

    fn skip_frames_until_current_frame_is_used_again(&mut self) -> anyhow::Result<()> {
        // aggressivly try to get more memory
        unsafe { self.ash_vk.vk_device.device.device_wait_idle().unwrap() };
        for _ in 0..self.device.swap_chain_image_count + 1 {
            self.next_frame()?;
        }

        Ok(())
    }

    /************************
     * COMMAND CALLBACKS
     ************************/
    fn command_cb_render(
        device: &LogicalDevice,
        render: &RenderSetupGroup,
        streamed_uniform: &Arc<spin::Mutex<StreamedUniform>>,
        frame_index: u32,
        thread_index: usize,
        render_pass_type: RenderPassType,
        cmd_param: &CommandsRender,
        exec_buffer: RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
    ) -> bool {
        match cmd_param {
            CommandsRender::Clear(cmd) => {
                Self::cmd_clear(device, &exec_buffer, command_buffer, cmd).is_ok()
            }
            CommandsRender::Stream(cmd) => match cmd {
                CommandsRenderStream::Render(cmd) => Self::cmd_render(
                    device,
                    render,
                    render_pass_type,
                    cmd,
                    &exec_buffer,
                    command_buffer,
                ),
                CommandsRenderStream::RenderTex3D(_) => todo!(),
                CommandsRenderStream::RenderBlurred {
                    cmd,
                    blur_radius,
                    blur_horizontal,
                    blur_color,
                } => Self::cmd_render_blurred(
                    device,
                    render,
                    render_pass_type,
                    frame_index,
                    cmd,
                    exec_buffer,
                    command_buffer,
                    *blur_radius,
                    *blur_horizontal,
                    *blur_color,
                ),
                CommandsRenderStream::RenderStencil { cmd } => Self::cmd_render_for_stencil(
                    device,
                    render,
                    render_pass_type,
                    cmd,
                    &exec_buffer,
                    command_buffer,
                ),
                CommandsRenderStream::RenderStencilNotPased { cmd, clear_stencil } => {
                    Self::cmd_render_where_stencil_did_not_pass(
                        device,
                        render,
                        render_pass_type,
                        frame_index,
                        cmd,
                        exec_buffer,
                        command_buffer,
                        *clear_stencil,
                    )
                }
            },
            CommandsRender::Map(cmd) => match cmd {
                CommandsRenderMap::TileLayer(cmd) => Self::cmd_render_tile_layer(
                    device,
                    render,
                    render_pass_type,
                    cmd,
                    &exec_buffer,
                    command_buffer,
                ),
                CommandsRenderMap::BorderTile(cmd) => Self::cmd_render_border_tile(
                    device,
                    render,
                    render_pass_type,
                    cmd,
                    &exec_buffer,
                    command_buffer,
                ),
                CommandsRenderMap::BorderTileLine(cmd) => Self::cmd_render_border_tile_line(
                    device,
                    render,
                    render_pass_type,
                    cmd,
                    &exec_buffer,
                    command_buffer,
                ),
                CommandsRenderMap::QuadLayer(cmd) => Self::cmd_render_quad_layer(
                    device,
                    render,
                    render_pass_type,
                    thread_index,
                    streamed_uniform,
                    frame_index,
                    cmd,
                    &exec_buffer,
                    command_buffer,
                ),
            },
            CommandsRender::QuadContainer(cmd) => match cmd {
                CommandsRenderQuadContainer::Render(cmd) => Self::cmd_render_quad_container_ex(
                    device,
                    render,
                    render_pass_type,
                    cmd,
                    &exec_buffer,
                    command_buffer,
                ),
                CommandsRenderQuadContainer::RenderAsSpriteMultiple(cmd) => {
                    Self::cmd_render_quad_container_as_sprite_multiple(
                        device,
                        render,
                        render_pass_type,
                        thread_index,
                        streamed_uniform,
                        frame_index,
                        cmd,
                        &exec_buffer,
                        command_buffer,
                    )
                }
            },
        }
    }

    fn command_cb_misc(&mut self, cmd_param: Commands) -> anyhow::Result<()> {
        match cmd_param {
            Commands::TextureCreate(cmd) => self.cmd_texture_create(cmd),
            Commands::TextureDestroy(cmd) => self.cmd_texture_destroy(&cmd),
            Commands::TextureUpdate(cmd) => self.cmd_texture_update(&cmd),
            Commands::CreateBufferObject(cmd) => self.cmd_create_buffer_object(cmd),
            Commands::RecreateBufferObject(cmd) => self.cmd_recreate_buffer_object(cmd),
            Commands::DeleteBufferObject(cmd) => self.cmd_delete_buffer_object(&cmd),
            Commands::IndicesRequiredNumNotify(cmd) => self.cmd_indices_required_num_notify(&cmd),
            Commands::Swap => self.cmd_swap(),
            Commands::NextSwitchPass => self.cmd_switch_to_switching_passes(),
            Commands::UpdateViewport(cmd) => self.cmd_update_viewport(&cmd),
            Commands::Multisampling => todo!(),
            Commands::VSync => todo!(),
            Commands::TrySwapAndScreenshot => todo!(),
            Commands::WindowCreateNtf => todo!(),
            Commands::WindowDestroyNtf => todo!(),
        }
    }

    fn fill_execute_buffer(
        &mut self,
        cmd: &CommandsRender,
        exec_buffer: &mut RenderCommandExecuteBuffer,
    ) {
        match &cmd {
            CommandsRender::Clear(cmd) => self.cmd_clear_fill_execute_buffer(exec_buffer, cmd),
            CommandsRender::Stream(cmd) => match cmd {
                CommandsRenderStream::Render(cmd) => {
                    self.cmd_render_fill_execute_buffer(exec_buffer, cmd)
                }
                CommandsRenderStream::RenderTex3D(_) => {}
                CommandsRenderStream::RenderBlurred { cmd, .. } => {
                    self.cmd_render_blurred_fill_execute_buffer(exec_buffer, cmd)
                }
                CommandsRenderStream::RenderStencil { cmd } => {
                    self.cmd_render_for_stencil_fill_execute_buffer(exec_buffer, cmd)
                }
                CommandsRenderStream::RenderStencilNotPased { cmd, .. } => {
                    self.cmd_render_where_stencil_did_not_pass_fill_execute_buffer(exec_buffer, cmd)
                }
            },
            CommandsRender::Map(cmd) => match cmd {
                CommandsRenderMap::TileLayer(cmd) => {
                    self.cmd_render_tile_layer_fill_execute_buffer(exec_buffer, cmd)
                }
                CommandsRenderMap::BorderTile(cmd) => {
                    self.cmd_render_border_tile_fill_execute_buffer(exec_buffer, cmd)
                }
                CommandsRenderMap::BorderTileLine(cmd) => {
                    self.cmd_render_border_tile_line_fill_execute_buffer(exec_buffer, cmd)
                }
                CommandsRenderMap::QuadLayer(cmd) => {
                    self.cmd_render_quad_layer_fill_execute_buffer(exec_buffer, cmd)
                }
            },
            CommandsRender::QuadContainer(cmd) => match cmd {
                CommandsRenderQuadContainer::Render(cmd) => {
                    self.cmd_render_quad_container_ex_fill_execute_buffer(exec_buffer, cmd)
                }
                CommandsRenderQuadContainer::RenderAsSpriteMultiple(cmd) => self
                    .cmd_render_quad_container_as_sprite_multiple_fill_execute_buffer(
                        exec_buffer,
                        cmd,
                    ),
            },
        }
    }

    /*****************************
     * VIDEO AND SCREENSHOT HELPER
     ******************************/
    // TODO dont unwrap in this function
    fn prepare_presented_image_data_image(
        &mut self,
        res_image_data: &mut &mut [u8],
        width: u32,
        height: u32,
    ) -> anyhow::Result<()> {
        let needs_new_img: bool = width
            != self.fetch_frame_buffer.get_presented_img_data_helper_width
            || height != self.fetch_frame_buffer.get_presented_img_data_helper_height;
        if self
            .fetch_frame_buffer
            .get_presented_img_data_helper_image
            .is_none()
            || needs_new_img
        {
            if self
                .fetch_frame_buffer
                .get_presented_img_data_helper_image
                .is_some()
            {
                self.delete_presented_image_data_image();
            }
            self.fetch_frame_buffer.get_presented_img_data_helper_width = width;
            self.fetch_frame_buffer.get_presented_img_data_helper_height = height;

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

            self.fetch_frame_buffer.get_presented_img_data_helper_image =
                Some(Image::new(self.ash_vk.vk_device.clone(), image_info).unwrap());
            // Create memory to back up the image
            let mem_requirements = unsafe {
                self.ash_vk.vk_device.device.get_image_memory_requirements(
                    self.fetch_frame_buffer
                        .get_presented_img_data_helper_image
                        .as_ref()
                        .unwrap()
                        .image,
                )
            };

            let mut mem_alloc_info = vk::MemoryAllocateInfo::default();
            mem_alloc_info.allocation_size = mem_requirements.size;
            mem_alloc_info.memory_type_index = self.device.mem.find_memory_type(
                self.vk_gpu.cur_device,
                mem_requirements.memory_type_bits,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
            );

            self.fetch_frame_buffer.get_presented_img_data_helper_mem = Some(
                SDeviceMemoryBlock::new(
                    self.ash_vk.vk_device.clone(),
                    mem_alloc_info,
                    EMemoryBlockUsage::Texture,
                )
                .unwrap(),
            );
            self.fetch_frame_buffer
                .get_presented_img_data_helper_image
                .as_mut()
                .unwrap()
                .bind(
                    self.fetch_frame_buffer
                        .get_presented_img_data_helper_mem
                        .clone()
                        .unwrap(),
                    0,
                )?;

            self.device.image_barrier(
                self.fetch_frame_buffer
                    .get_presented_img_data_helper_image
                    .as_ref()
                    .unwrap()
                    .as_ref(),
                0,
                1,
                0,
                1,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::GENERAL,
                self.cur_image_index,
            )?;

            let sub_resource = vk::ImageSubresource::builder()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .mip_level(0)
                .array_layer(0)
                .build();
            let sub_resource_layout = unsafe {
                self.ash_vk.vk_device.device.get_image_subresource_layout(
                    self.fetch_frame_buffer
                        .get_presented_img_data_helper_image
                        .as_ref()
                        .unwrap()
                        .image,
                    sub_resource,
                )
            };

            self.fetch_frame_buffer
                .get_presented_img_data_helper_mapped_memory = Some(
                MappedMemory::new(
                    self.ash_vk.vk_device.clone(),
                    self.fetch_frame_buffer
                        .get_presented_img_data_helper_mem
                        .as_ref()
                        .unwrap()
                        .clone(),
                    self.fetch_frame_buffer
                        .get_presented_img_data_helper_mapped_layout_offset,
                )
                .unwrap(),
            );
            self.fetch_frame_buffer
                .get_presented_img_data_helper_mapped_layout_offset = sub_resource_layout.offset;
            self.fetch_frame_buffer
                .get_presented_img_data_helper_mapped_layout_pitch = sub_resource_layout.row_pitch;

            let mut fence_info = vk::FenceCreateInfo::default();
            fence_info.flags = vk::FenceCreateFlags::SIGNALED;
            self.fetch_frame_buffer.get_presented_img_data_helper_fence =
                unsafe { self.ash_vk.vk_device.device.create_fence(&fence_info, None) }.unwrap();
        }
        *res_image_data = unsafe {
            std::slice::from_raw_parts_mut(
                self.fetch_frame_buffer
                    .get_presented_img_data_helper_mapped_memory
                    .as_ref()
                    .unwrap()
                    .get_mem(),
                self.fetch_frame_buffer
                    .get_presented_img_data_helper_mem
                    .as_ref()
                    .unwrap()
                    .size as usize
                    - self
                        .fetch_frame_buffer
                        .get_presented_img_data_helper_mapped_layout_offset
                        as usize,
            )
        };
        Ok(())
    }

    fn delete_presented_image_data_image(&mut self) {
        if self
            .fetch_frame_buffer
            .get_presented_img_data_helper_image
            .is_some()
        {
            unsafe {
                self.ash_vk.vk_device.device.destroy_fence(
                    self.fetch_frame_buffer.get_presented_img_data_helper_fence,
                    None,
                );
            }

            self.fetch_frame_buffer.get_presented_img_data_helper_fence = vk::Fence::null();

            self.fetch_frame_buffer.get_presented_img_data_helper_image = None;
            self.fetch_frame_buffer.get_presented_img_data_helper_mem = Default::default();
            self.fetch_frame_buffer
                .get_presented_img_data_helper_mapped_memory = None;

            self.fetch_frame_buffer.get_presented_img_data_helper_width = 0;
            self.fetch_frame_buffer.get_presented_img_data_helper_height = 0;
        }
    }

    fn get_presented_image_data_impl(
        &mut self,
        width: &mut u32,
        height: &mut u32,
        dest_data_buff: &mut Vec<u8>,
        flip_img_data: bool,
        reset_alpha: bool,
    ) -> anyhow::Result<ImageFormat> {
        let mut is_b8_g8_r8_a8: bool =
            self.render.get().vk_surf_format.format == vk::Format::B8G8R8A8_UNORM;
        let uses_rgba_like_format: bool =
            self.render.get().vk_surf_format.format == vk::Format::R8G8B8A8_UNORM || is_b8_g8_r8_a8;
        if uses_rgba_like_format && self.last_presented_swap_chain_image_index != u32::MAX {
            let viewport = self
                .vk_swap_img_and_viewport_extent
                .get_presented_image_viewport();
            *width = viewport.width;
            *height = viewport.height;
            let format = ImageFormat::Rgba;

            let image_total_size: usize = *width as usize * *height as usize * 4;

            let mut res_image_data: &mut [u8] = &mut [];
            self.prepare_presented_image_data_image(&mut res_image_data, *width, *height)
                .map_err(|err| anyhow!("Could not prepare presented image data: {err}"))?;

            let mut command_buffer_ptr: *const vk::CommandBuffer = std::ptr::null_mut();
            self.device
                .get_memory_command_buffer(&mut command_buffer_ptr, self.cur_image_index)
                .map_err(|err| anyhow!("Could not get memory command buffer: {err}"))?;
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

            let final_layout = self.ash_vk.vk_device.final_layout();
            let swap_img = &self.render.get().native.swap_chain_images
                [self.last_presented_swap_chain_image_index as usize];

            self.device
                .image_barrier(
                    self.fetch_frame_buffer
                        .get_presented_img_data_helper_image
                        .as_ref()
                        .unwrap()
                        .as_ref(),
                    0,
                    1,
                    0,
                    1,
                    vk::ImageLayout::GENERAL,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    self.cur_image_index,
                )
                .map_err(|err| anyhow!("Image barrier failed for the helper image: {err}"))?;
            self.device
                .image_barrier(
                    &ImageFakeForSwapchainImgs { img: *swap_img },
                    0,
                    1,
                    0,
                    1,
                    final_layout,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    self.cur_image_index,
                )
                .map_err(|err| anyhow!("Image barrier failed for the swapchain image: {err}"))?;

            // If source and destination support blit we'll blit as this also does
            // automatic format conversion (e.g. from BGR to RGB)
            if self
                .ash_vk
                .vk_device
                .phy_device
                .config
                .read()
                .unwrap()
                .optimal_swap_chain_image_blitting
                && self
                    .ash_vk
                    .vk_device
                    .phy_device
                    .config
                    .read()
                    .unwrap()
                    .linear_rgba_image_blitting
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
                    self.ash_vk.vk_device.device.cmd_blit_image(
                        *command_buffer,
                        *swap_img,
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        self.fetch_frame_buffer
                            .get_presented_img_data_helper_image
                            .as_ref()
                            .unwrap()
                            .image,
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
                    self.ash_vk.vk_device.device.cmd_copy_image(
                        *command_buffer,
                        *swap_img,
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        self.fetch_frame_buffer
                            .get_presented_img_data_helper_image
                            .as_ref()
                            .unwrap()
                            .image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[image_copy_region],
                    );
                }
            }

            self.device
                .image_barrier(
                    self.fetch_frame_buffer
                        .get_presented_img_data_helper_image
                        .as_ref()
                        .unwrap()
                        .as_ref(),
                    0,
                    1,
                    0,
                    1,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::ImageLayout::GENERAL,
                    self.cur_image_index,
                )
                .map_err(|err| anyhow!("Image barrier failed for the helper image: {err}"))?;
            self.device
                .image_barrier(
                    &ImageFakeForSwapchainImgs { img: *swap_img },
                    0,
                    1,
                    0,
                    1,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    self.ash_vk.vk_device.final_layout(),
                    self.cur_image_index,
                )
                .map_err(|err| anyhow!("Image barrier failed for the swap chain image: {err}"))?;

            unsafe {
                self.ash_vk
                    .vk_device
                    .device
                    .end_command_buffer(*command_buffer)
                    .map_err(|err| anyhow!("Could not end command buffer: {err}"))?;
            }
            self.device.used_memory_command_buffer[self.cur_image_index as usize] = false;

            let mut submit_info = vk::SubmitInfo::default();

            submit_info.command_buffer_count = 1;
            submit_info.p_command_buffers = command_buffer;

            unsafe {
                self.ash_vk
                    .vk_device
                    .device
                    .reset_fences(&[self.fetch_frame_buffer.get_presented_img_data_helper_fence])
            }
            .map_err(|err| anyhow!("Could not reset fences: {err}"))?;
            unsafe {
                let queue = self.queue.lock();
                self.ash_vk.vk_device.device.queue_submit(
                    queue.graphics_queue,
                    &[submit_info],
                    self.fetch_frame_buffer.get_presented_img_data_helper_fence,
                )
            }
            .map_err(|err| anyhow!("Queue submit failed: {err}"))?;
            unsafe {
                self.ash_vk.vk_device.device.wait_for_fences(
                    &[self.fetch_frame_buffer.get_presented_img_data_helper_fence],
                    true,
                    u64::MAX,
                )
            }
            .map_err(|err| anyhow!("Could not wait for fences: {err}"))?;

            let mut mem_range = vk::MappedMemoryRange::default();
            mem_range.memory = self
                .fetch_frame_buffer
                .get_presented_img_data_helper_mem
                .as_ref()
                .unwrap()
                .mem;
            mem_range.offset = self
                .fetch_frame_buffer
                .get_presented_img_data_helper_mapped_layout_offset;
            mem_range.size = vk::WHOLE_SIZE;
            unsafe {
                self.ash_vk
                    .vk_device
                    .device
                    .invalidate_mapped_memory_ranges(&[mem_range])
            }
            .map_err(|err| anyhow!("Could not invalidate mapped memory ranges: {err}"))?;

            let real_full_image_size: usize = image_total_size.max(
                *height as usize
                    * self
                        .fetch_frame_buffer
                        .get_presented_img_data_helper_mapped_layout_pitch
                        as usize,
            );
            if dest_data_buff.len() < real_full_image_size + (*width * 4) as usize {
                dest_data_buff.resize(
                    real_full_image_size + (*width * 4) as usize,
                    Default::default(),
                ); // extra space for flipping
            }
            dest_data_buff
                .as_mut_slice()
                .split_at_mut(real_full_image_size)
                .0
                .copy_from_slice(res_image_data.split_at_mut(real_full_image_size).0);

            // pack image data together without any offset
            // that the driver might require
            if *width as u64 * 4
                < self
                    .fetch_frame_buffer
                    .get_presented_img_data_helper_mapped_layout_pitch
            {
                for y in 0..*height as usize {
                    let offset_image_packed: usize = y * *width as usize * 4;
                    let offset_image_unpacked: usize = y * self
                        .fetch_frame_buffer
                        .get_presented_img_data_helper_mapped_layout_pitch
                        as usize;

                    let (img_part, help_part) = dest_data_buff
                        .as_mut_slice()
                        .split_at_mut(real_full_image_size);

                    let unpacked_part = img_part.split_at(offset_image_unpacked).1;
                    help_part.copy_from_slice(unpacked_part.split_at(*width as usize * 4).0);

                    let packed_part = img_part.split_at_mut(offset_image_packed).1;
                    packed_part
                        .split_at_mut(*width as usize * 4)
                        .0
                        .copy_from_slice(help_part);
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

            dest_data_buff.resize(*width as usize * *height as usize * 4, Default::default());

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
    fn create_texture_samplers(
        device: &Arc<LogicalDevice>,
        limits: &Limits,
        global_texture_lod_bias: f64,
    ) -> anyhow::Result<(vk::Sampler, vk::Sampler, vk::Sampler)> {
        Ok((
            Device::create_texture_samplers_impl(
                &device.device,
                limits.max_sampler_anisotropy,
                global_texture_lod_bias,
                vk::SamplerAddressMode::REPEAT,
                vk::SamplerAddressMode::REPEAT,
                vk::SamplerAddressMode::REPEAT,
            )?,
            Device::create_texture_samplers_impl(
                &device.device,
                limits.max_sampler_anisotropy,
                global_texture_lod_bias,
                vk::SamplerAddressMode::CLAMP_TO_EDGE,
                vk::SamplerAddressMode::CLAMP_TO_EDGE,
                vk::SamplerAddressMode::CLAMP_TO_EDGE,
            )?,
            Device::create_texture_samplers_impl(
                &device.device,
                limits.max_sampler_anisotropy,
                global_texture_lod_bias,
                vk::SamplerAddressMode::CLAMP_TO_EDGE,
                vk::SamplerAddressMode::CLAMP_TO_EDGE,
                vk::SamplerAddressMode::MIRRORED_REPEAT,
            )?,
        ))
    }

    fn destroy_texture_samplers(&mut self) {
        unsafe {
            self.ash_vk.vk_device.device.destroy_sampler(
                self.device.samplers[ESupportedSamplerTypes::Repeat as usize],
                None,
            );
        }
        unsafe {
            self.ash_vk.vk_device.device.destroy_sampler(
                self.device.samplers[ESupportedSamplerTypes::ClampToEdge as usize],
                None,
            );
        }
        unsafe {
            self.ash_vk.vk_device.device.destroy_sampler(
                self.device.samplers[ESupportedSamplerTypes::Texture2DArray as usize],
                None,
            );
        }
    }

    fn create_descriptor_pools(
        device: &Arc<LogicalDevice>,
    ) -> anyhow::Result<SDeviceDescriptorPools> {
        let mut pool = SDeviceDescriptorPools {
            pools: Default::default(),
            default_alloc_size: 1024,
            is_uniform_pool: false,
        };
        VulkanAllocator::allocate_descriptor_pool(
            device,
            &mut pool,
            StreamDataMax::MaxTextures as usize,
        )?;
        Ok(pool)
    }

    fn destroy_descriptor_pools(&mut self) {
        self.device.standard_texture_descr_pool.pools.clear();
    }

    fn get_uniform_buffer_object(
        stream_uniform: &Arc<spin::Mutex<StreamedUniform>>,
        render_thread_index: usize,
        requires_shared_stages_descriptor: bool,
        _particle_count: usize,
        ptr_raw_data: *const c_void,
        data_size: usize,
        cur_image_index: u32,
    ) -> anyhow::Result<Arc<DescriptorSet>> {
        Ok(stream_uniform
            .lock()
            .get_uniform_buffer_object_impl::<SRenderSpriteInfo, 512, 128>(
                render_thread_index,
                requires_shared_stages_descriptor,
                ptr_raw_data,
                data_size,
                cur_image_index,
            )?)
    }

    /************************
     * SWAPPING MECHANISM
     ************************/
    fn start_render_thread(&mut self, thread_index: usize) {
        if !self.command_groups.is_empty() {
            let thread = &mut self.render_threads[thread_index];
            let mut guard = thread.inner.lock().unwrap();
            guard.is_rendering = true;
            guard.command_groups.append(&mut self.command_groups);
            thread.cond.notify_one();
        }
    }

    fn finish_render_threads(&mut self) {
        // execute threads
        let mut thread_index = self.last_render_thread_index;
        while !self.command_groups.is_empty() {
            self.start_render_thread(thread_index % self.thread_count);
            thread_index += 1;
        }

        for thread_index in 0..self.thread_count {
            let render_thread = &mut self.render_threads[thread_index];
            let mut _guard = render_thread.inner.lock().unwrap();
            _guard = render_thread
                .cond
                .wait_while(_guard, |p| p.is_rendering)
                .unwrap();
        }
    }

    fn execute_memory_command_buffer(&mut self) {
        if self.device.used_memory_command_buffer[self.cur_image_index as usize] {
            let memory_command_buffer = &self
                .device
                .memory_command_buffers
                .as_ref()
                .unwrap()
                .command_buffers[self.cur_image_index as usize];
            unsafe {
                self.ash_vk
                    .vk_device
                    .device
                    .end_command_buffer(*memory_command_buffer)
                    .unwrap();
            }

            let mut submit_info = vk::SubmitInfo::default();

            submit_info.command_buffer_count = 1;
            submit_info.p_command_buffers = memory_command_buffer;
            unsafe {
                let queue = self.queue.lock();
                self.ash_vk
                    .vk_device
                    .device
                    .queue_submit(queue.graphics_queue, &[submit_info], vk::Fence::null())
                    .unwrap();
            }
            unsafe {
                let queue = self.queue.lock();
                self.ash_vk
                    .vk_device
                    .device
                    .queue_wait_idle(queue.graphics_queue)
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
                    .device
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
            &self.ash_vk.vk_device.device,
            self.device.vk_gpu.limits.non_coherent_mem_alignment,
            &mut self.device.streamed_vertex_buffer,
            cur_image_index,
        );

        // now the buffer objects
        for stream_uniform_buffer in &mut self.device.streamed_uniform.lock().buffers {
            Device::upload_streamed_buffer::<{ FLUSH_FOR_RENDERING }, _>(
                &self.ash_vk.vk_device.device,
                self.device.vk_gpu.limits.non_coherent_mem_alignment,
                stream_uniform_buffer,
                cur_image_index,
            );
        }

        self.upload_staging_buffers();
    }

    fn clear_frame_data(&mut self, frame_image_index: usize) {
        self.upload_staging_buffers();

        self.device
            .mem_allocator
            .lock()
            .clear_frame_data(frame_image_index);
    }

    fn clear_frame_memory_usage(&mut self) {
        self.clear_frame_data(self.cur_image_index as usize);
        self.device.mem_allocator.lock().shrink_unused_caches();
    }

    fn command_buffer_start_render_pass(
        device: &Arc<LogicalDevice>,
        render: &RenderSetupGroup,
        swap_chain_extent_info: &SSwapImgViewportExtent,
        clear_color: &[f32; 4],
        cur_image_index: u32,
        render_pass_type: RenderPassType,
        command_buffer: vk::CommandBuffer,
    ) -> anyhow::Result<()> {
        let mut render_pass_info = vk::RenderPassBeginInfo::default();
        render_pass_info.render_pass = match render_pass_type {
            RenderPassType::Single => render.get().native.render_pass.pass,
            RenderPassType::Switching1 => render.get().switching.passes[0].render_pass.pass,
            RenderPassType::Switching2 => render.get().switching.passes[1].render_pass.pass,
        };
        render_pass_info.framebuffer = match render_pass_type {
            RenderPassType::Single => {
                render.get().native.framebuffer_list[cur_image_index as usize]
            }
            RenderPassType::Switching1 => {
                render.get().switching.passes[0].framebuffer_list[cur_image_index as usize]
            }
            RenderPassType::Switching2 => {
                render.get().switching.passes[1].framebuffer_list[cur_image_index as usize]
            }
        };
        render_pass_info.render_area.offset = vk::Offset2D::default();
        render_pass_info.render_area.extent = swap_chain_extent_info.swap_image_viewport;

        let clear_color_val = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [
                        clear_color[0],
                        clear_color[1],
                        clear_color[2],
                        clear_color[3],
                    ],
                },
            },
            vk::ClearValue {
                color: vk::ClearColorValue {
                    int32: [0, 0, 0, 0],
                },
            },
        ];
        let clear_color_val_switching_pass = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [
                        clear_color[0],
                        clear_color[1],
                        clear_color[2],
                        clear_color[3],
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
            RenderPassType::Switching1 => 2,
            RenderPassType::Switching2 => 2,
        };
        render_pass_info.p_clear_values = match render_pass_type {
            RenderPassType::Single => clear_color_val.as_ptr(),
            RenderPassType::Switching1 | RenderPassType::Switching2 => {
                clear_color_val_switching_pass.as_ptr()
            }
        };

        unsafe {
            device.device.cmd_begin_render_pass(
                command_buffer,
                &render_pass_info,
                vk::SubpassContents::SECONDARY_COMMAND_BUFFERS,
            );
        }

        Ok(())
    }

    fn start_new_render_pass(&mut self, render_pass_type: RenderPassType) -> anyhow::Result<()> {
        self.new_command_group(
            self.current_command_group.render_pass_index + 1,
            render_pass_type,
        );

        Ok(())
    }

    fn cmd_switch_to_switching_passes(&mut self) -> anyhow::Result<()> {
        match self.current_command_group.render_pass {
            RenderPassType::Single | RenderPassType::Switching2 => {
                self.start_new_render_pass(RenderPassType::Switching1)?;
            }
            RenderPassType::Switching1 => {
                self.start_new_render_pass(RenderPassType::Switching2)?;
            }
        }
        Ok(())
    }

    /// returns if any render pass at all was started
    fn collect_frame(&mut self) -> anyhow::Result<()> {
        let frame = self.frame.lock();
        let main_command_buffer = frame.render.main_command_buffer;

        let mut did_at_least_one_render_pass = false;
        let mut cur_render_pass_type = RenderPassType::Single;
        for render_pass in &frame.render.passes {
            if matches!(
                render_pass.render_pass_type,
                RenderPassType::Switching1 | RenderPassType::Switching2
            ) {
                let img = if let RenderPassType::Switching1 = render_pass.render_pass_type {
                    &mut self.render.get_mut().switching.passes[1].surface.image_list
                        [self.cur_image_index as usize]
                } else {
                    &mut self.render.get_mut().switching.passes[0].surface.image_list
                        [self.cur_image_index as usize]
                };
                // transition the current frame image to shader_read
                image_barrier(
                    &self.ash_vk.vk_device,
                    main_command_buffer,
                    img.base.image.as_ref(),
                    0,
                    1,
                    0,
                    1,
                    if img.base.layout_is_undefined {
                        vk::ImageLayout::UNDEFINED
                    } else {
                        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
                    },
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                )
                .map_err(|err| {
                    anyhow!("could not transition image for swapping framebuffer: {err}")
                })?;
                img.base.layout_is_undefined = false;
                if let RenderPassType::Switching1 = render_pass.render_pass_type {
                    &mut self.render.get_mut().switching.passes[0].surface.image_list
                        [self.cur_image_index as usize]
                } else {
                    &mut self.render.get_mut().switching.passes[1].surface.image_list
                        [self.cur_image_index as usize]
                }
                .base
                .layout_is_undefined = false;

                // transition the stencil buffer if needed
                let stencil = &mut self
                    .render
                    .get_mut()
                    .switching
                    .stencil_list_for_pass_transition[self.cur_image_index as usize];

                if stencil.layout_is_undefined {
                    image_barrier(
                        &self.ash_vk.vk_device,
                        main_command_buffer,
                        stencil.image.as_ref(),
                        0,
                        1,
                        0,
                        1,
                        vk::ImageLayout::UNDEFINED,
                        vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                    )
                    .map_err(|err| {
                        anyhow!("could not transition image for swapping framebuffer: {err}")
                    })?;
                    stencil.layout_is_undefined = false;
                }
            }

            // start the render pass
            Self::command_buffer_start_render_pass(
                &self.ash_vk.vk_device,
                &self.render,
                &self.vk_swap_img_and_viewport_extent,
                &self.clear_color,
                self.cur_image_index,
                render_pass.render_pass_type,
                main_command_buffer,
            )?;
            did_at_least_one_render_pass = true;

            // collect commands
            for (index, subpass) in render_pass.subpasses.iter().enumerate() {
                if index != 0 {
                    unsafe {
                        self.ash_vk.vk_device.device.cmd_next_subpass(
                            main_command_buffer,
                            vk::SubpassContents::SECONDARY_COMMAND_BUFFERS,
                        )
                    };
                }
                // collect in order
                let mut buffers: Vec<vk::CommandBuffer> = Default::default();
                buffers.extend(subpass.command_buffers.values().map(|buffer| *buffer));
                unsafe {
                    self.ash_vk
                        .vk_device
                        .device
                        .cmd_execute_commands(main_command_buffer, &buffers);
                }
            }

            // end render pass
            unsafe {
                self.ash_vk
                    .vk_device
                    .device
                    .cmd_end_render_pass(main_command_buffer)
            };

            if matches!(
                render_pass.render_pass_type,
                RenderPassType::Switching1 | RenderPassType::Switching2
            ) {
                let img = if let RenderPassType::Switching1 = render_pass.render_pass_type {
                    &self.render.get().switching.passes[1].surface.image_list
                        [self.cur_image_index as usize]
                } else {
                    &self.render.get().switching.passes[0].surface.image_list
                        [self.cur_image_index as usize]
                };
                // transition the current frame image to shader_read
                image_barrier(
                    &self.ash_vk.vk_device,
                    main_command_buffer,
                    img.base.image.as_ref(),
                    0,
                    1,
                    0,
                    1,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                )
                .map_err(|err| {
                    anyhow!("could not transition image for swapping framebuffer: {err}")
                })?;
            }

            cur_render_pass_type = render_pass.render_pass_type;
        }

        if !did_at_least_one_render_pass {
            // fake (empty) render pass
            Self::command_buffer_start_render_pass(
                &self.ash_vk.vk_device,
                &self.render,
                &self.vk_swap_img_and_viewport_extent,
                &self.clear_color,
                self.cur_image_index,
                RenderPassType::Single,
                main_command_buffer,
            )?;
            unsafe {
                self.ash_vk
                    .vk_device
                    .device
                    .cmd_end_render_pass(main_command_buffer)
            };
        }

        if let RenderPassType::Switching1 | RenderPassType::Switching2 = cur_render_pass_type {
            // copy to presentation render pass
            let img = if let RenderPassType::Switching1 = cur_render_pass_type {
                &self.render.get().switching.passes[0].surface.image_list
                    [self.cur_image_index as usize]
            } else {
                &self.render.get().switching.passes[1].surface.image_list
                    [self.cur_image_index as usize]
            };

            copy_color_attachment_to_present_src(
                &self.ash_vk.vk_device,
                main_command_buffer,
                img.base.image.as_ref(),
                &ImageFakeForSwapchainImgs {
                    img: self.render.get().native.swap_chain_images[self.cur_image_index as usize],
                },
                self.vk_swap_img_and_viewport_extent
                    .swap_image_viewport
                    .width,
                self.vk_swap_img_and_viewport_extent
                    .swap_image_viewport
                    .height,
            )?;
        }

        Ok(())
    }

    fn new_command_group(&mut self, render_pass_index: usize, render_pass_type: RenderPassType) {
        if !self.current_command_group.cmds.is_empty() {
            self.command_groups
                .push(std::mem::take(&mut self.current_command_group));
        }

        self.start_render_thread(self.last_render_thread_index);
        self.last_render_thread_index = (self.last_render_thread_index + 1) % self.thread_count;

        self.order_id_gen += 1;
        self.current_command_group.render_pass_index = render_pass_index;
        self.current_command_group.in_order_id = self.order_id_gen;
        self.current_command_group.render_pass = render_pass_type;
        self.current_command_group.cur_frame_index = self.cur_image_index;
    }

    fn wait_frame(&mut self) -> anyhow::Result<()> {
        let command_buffer = self
            .main_render_command_buffer
            .as_ref()
            .ok_or(anyhow!("main render command buffer was None"))?
            .command_buffer;

        // make sure even the current unhandled commands get handled
        if !self.current_command_group.cmds.is_empty() {
            self.command_groups
                .push(std::mem::take(&mut self.current_command_group));
        }

        self.finish_render_threads();
        self.upload_non_flushed_buffers::<true>(self.cur_image_index);

        self.collect_frame()?;

        self.main_render_command_buffer = None;

        let wait_semaphore = self.wait_semaphores[self.cur_frames as usize].semaphore;

        let mut submit_info = vk::SubmitInfo::default();

        let mut command_buffers: [vk::CommandBuffer; 2] = Default::default();
        command_buffers[0] = command_buffer;

        submit_info.command_buffer_count = 1;
        submit_info.p_command_buffers = command_buffers.as_ptr();

        if self.device.used_memory_command_buffer[self.cur_image_index as usize] {
            let memory_command_buffer = &self
                .device
                .memory_command_buffers
                .as_ref()
                .ok_or(anyhow!("memory command buffer was None"))?
                .command_buffers[self.cur_image_index as usize];
            unsafe {
                self.ash_vk
                    .vk_device
                    .device
                    .end_command_buffer(*memory_command_buffer)
                    .map_err(|err| anyhow!("ending memory command buffer failed {err}"))
            }?;

            command_buffers[0] = *memory_command_buffer;
            command_buffers[1] = command_buffer;
            submit_info.command_buffer_count = 2;
            submit_info.p_command_buffers = command_buffers.as_ptr();

            self.device.used_memory_command_buffer[self.cur_image_index as usize] = false;
        }

        let wait_semaphores = [wait_semaphore];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        submit_info.wait_semaphore_count = wait_semaphores.len() as u32;
        submit_info.p_wait_semaphores = wait_semaphores.as_ptr();
        submit_info.p_wait_dst_stage_mask = wait_stages.as_ptr();

        let signal_semaphores = [self.sig_semaphores[self.cur_frames as usize].semaphore];
        submit_info.signal_semaphore_count = signal_semaphores.len() as u32;
        submit_info.p_signal_semaphores = signal_semaphores.as_ptr();

        let timeline_submit_info: vk::TimelineSemaphoreSubmitInfo;

        if self.device.is_headless {
            let wait_counter = unsafe {
                self.ash_vk
                    .vk_device
                    .device
                    .get_semaphore_counter_value(wait_semaphore)
                    .unwrap()
            };
            let signal_counter = unsafe {
                self.ash_vk
                    .vk_device
                    .device
                    .get_semaphore_counter_value(signal_semaphores[0])
                    .unwrap()
            };
            timeline_submit_info = vk::TimelineSemaphoreSubmitInfo::builder()
                .wait_semaphore_values(&[wait_counter])
                .signal_semaphore_values(&[signal_counter + 1])
                .build();
            submit_info.p_next = &timeline_submit_info as *const _ as *const _;
        }

        unsafe {
            self.ash_vk
                .vk_device
                .device
                .reset_fences(&[self.frame_fences[self.cur_frames as usize].fence])
                .map_err(|err| anyhow!("could not reset fences {err}"))
        }?;

        let queue_submit_res = unsafe {
            let queue = self.queue.lock();
            self.ash_vk.vk_device.device.queue_submit(
                queue.graphics_queue,
                &[submit_info],
                self.frame_fences[self.cur_frames as usize].fence,
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
                return Err(anyhow!("Submitting to graphics queue failed: {crit_err}"));
            }
        }

        std::mem::swap(
            &mut self.wait_semaphores[self.cur_frames as usize],
            &mut self.sig_semaphores[self.cur_frames as usize],
        );

        let mut present_info = vk::PresentInfoKHR::default();

        present_info.wait_semaphore_count = signal_semaphores.len() as u32;
        present_info.p_wait_semaphores = signal_semaphores.as_ptr();

        present_info.p_image_indices = &mut self.cur_image_index;

        self.last_presented_swap_chain_image_index = self.cur_image_index;

        let queue_present_res = unsafe {
            let queue = self.queue.lock();
            self.ash_vk
                .vk_swap_chain_ash
                .queue_present(queue.present_queue, present_info)
        };
        if matches!(queue_present_res, Err(_))
            && !matches!(queue_present_res, Err(vk::Result::SUBOPTIMAL_KHR))
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
                return Err(anyhow!("Presenting graphics queue failed: {crit_err}"));
            }
        }

        self.cur_frames = (self.cur_frames + 1) % self.device.swap_chain_image_count;
        Ok(())
    }

    fn prepare_frame(&mut self) -> anyhow::Result<()> {
        if self.recreate_swap_chain {
            self.recreate_swap_chain = false;
            if is_verbose(&self.dbg) {
                self.logger
                    .log(LogLevel::Debug)
                    .msg("recreating swap chain requested by user (prepare frame).");
            }
            self.recreate_swap_chain();
        }

        let acq_result = unsafe {
            self.ash_vk.vk_swap_chain_ash.acquire_next_image(
                u64::MAX,
                self.sig_semaphores[self.cur_frames as usize].semaphore,
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
                self.prepare_frame()?;
            } else {
                if let Err(err) = acq_result {
                    self.logger
                        .log(LogLevel::Debug)
                        .msg("acquire next image failed ")
                        .msg_var(&err);
                }
                let res = match acq_result {
                    Err(err) => err,
                    Ok(_) => vk::Result::SUBOPTIMAL_KHR,
                };

                let crit_error_msg = self.check_res.check_vulkan_critical_error(
                    res,
                    &self.error,
                    &mut self.recreate_swap_chain,
                );
                if let Some(crit_err) = crit_error_msg {
                    return Err(anyhow!(format!("Acquiring next image failed: {crit_err}")));
                } else if res == vk::Result::ERROR_SURFACE_LOST_KHR {
                    self.rendering_paused = true;
                    return Ok(());
                }
            }
        }
        self.cur_image_index = acq_result.unwrap().0;
        std::mem::swap(
            &mut self.wait_semaphores[self.cur_frames as usize],
            &mut self.sig_semaphores[self.cur_frames as usize],
        );

        if let Some(img_fence) = &self.image_fences[self.cur_image_index as usize] {
            unsafe {
                self.ash_vk
                    .vk_device
                    .device
                    .wait_for_fences(&[img_fence.fence], true, u64::MAX)
            }?;
        }
        self.image_fences[self.cur_image_index as usize] =
            Some(self.frame_fences[self.cur_frames as usize].clone());

        // next frame
        self.cur_frame += 1;
        self.order_id_gen = 0;
        self.image_last_frame_check[self.cur_image_index as usize] = self.cur_frame;
        self.current_command_group = Default::default();
        self.current_command_group.render_pass = RenderPassType::Single;
        self.current_command_group.cur_frame_index = self.cur_image_index;

        // check if older frames weren't used in a long time
        for frame_image_index in 0..self.image_last_frame_check.len() {
            let last_frame = self.image_last_frame_check[frame_image_index];
            if self.cur_frame - last_frame > self.device.swap_chain_image_count as u64 {
                if let Some(img_fence) = &self.image_fences[frame_image_index] {
                    unsafe {
                        self.ash_vk.vk_device.device.wait_for_fences(
                            &[img_fence.fence],
                            true,
                            u64::MAX,
                        )
                    }?;
                    self.clear_frame_data(frame_image_index);
                    self.image_fences[frame_image_index] = None;
                }
                self.image_last_frame_check[frame_image_index] = self.cur_frame;
            }
        }

        // clear frame's memory data
        self.clear_frame_memory_usage();
        self.ash_vk
            .vk_device
            .memory_allocator
            .lock()
            .set_frame_index(self.cur_image_index as usize);
        self.device
            .mem_allocator
            .lock()
            .set_frame_index(self.cur_image_index as usize);

        self.command_pool
            .set_frame_index(self.cur_image_index as usize);
        for thread in &self.render_thread_infos {
            thread
                .0
                .store(self.cur_image_index, std::sync::atomic::Ordering::SeqCst);
        }

        // clear frame
        self.frame.lock().clear();
        self.main_render_command_buffer = Some(
            self.command_pool
                .get_render_buffer(AutoCommandBufferType::Primary, &self.frame)?,
        );
        self.frame.lock().render.main_command_buffer = self
            .main_render_command_buffer
            .as_ref()
            .unwrap()
            .command_buffer;

        Ok(())
    }

    fn pure_memory_frame(&mut self) -> anyhow::Result<()> {
        self.execute_memory_command_buffer();

        // reset streamed data
        self.upload_non_flushed_buffers::<false>(self.cur_image_index);

        self.clear_frame_memory_usage();

        Ok(())
    }

    pub fn next_frame(&mut self) -> anyhow::Result<()> {
        if !self.rendering_paused {
            self.wait_frame()?;
            self.prepare_frame()?;
        }
        // else only execute the memory command buffer
        else {
            self.pure_memory_frame()?;
        }

        Ok(())
    }

    /************************
     * TEXTURES
     ************************/
    fn update_texture(
        &mut self,
        texture_slot: u128,
        format: vk::Format,
        data: &Vec<u8>,
        x_off: i64,
        y_off: i64,
        width: usize,
        height: usize,
        color_channel_count: usize,
    ) -> anyhow::Result<()> {
        let image_size: usize = width * height * color_channel_count;
        let mut staging_allocation = self.device.mem_allocator.lock().get_staging_buffer_image(
            &self.device.mem,
            &self.device.vk_gpu.limits,
            data,
            image_size as u64,
        );
        if let Err(_) = staging_allocation {
            self.skip_frames_until_current_frame_is_used_again()?;
            staging_allocation = self.device.mem_allocator.lock().get_staging_buffer_image(
                &self.device.mem,
                &self.device.vk_gpu.limits,
                data,
                image_size as u64,
            );
        }
        let staging_buffer = staging_allocation?;

        let tex = self.device.textures.get(&texture_slot).unwrap();
        match &tex.data {
            TextureData::Tex2D { img, .. } => {
                let img = img.clone();
                let mip_map_count = tex.mip_map_count;
                self.device
                    .image_barrier(
                        img.as_ref(),
                        0,
                        tex.mip_map_count as usize,
                        0,
                        1,
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        self.cur_image_index,
                    )
                    .map_err(|err| {
                        anyhow!("updating texture failed when transitioning to transfer dst: {err}")
                    })?;
                self.device
                    .copy_buffer_to_image(
                        staging_buffer.buffer.as_ref().unwrap(),
                        staging_buffer.heap_data.offset_to_align as u64,
                        &img,
                        x_off as i32,
                        y_off as i32,
                        width as u32,
                        height as u32,
                        1,
                        self.cur_image_index,
                    )
                    .map_err(|err| {
                        anyhow!("texture updating failed while copying buffer to image: {err}")
                    })?;

                if mip_map_count > 1 {
                    self.device
                        .build_mipmaps(
                            &img,
                            format,
                            width,
                            height,
                            1,
                            mip_map_count as usize,
                            self.cur_image_index,
                        )
                        .map_err(|err| {
                            anyhow!("updating texture failed when building mipmaps: {err}")
                        })?;
                } else {
                    self.device.image_barrier(
                        img.as_ref(),
                        0,
                        1,
                        0,
                        1,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        self.cur_image_index,
                    ).map_err(|err| anyhow!("updating texture failed when transitioning back from transfer dst: {err}"))?;
                }
            }
            TextureData::Tex3D { .. } => panic!("not implemented for 3d textures"),
        }

        self.device
            .upload_and_free_staging_image_mem_block(staging_buffer);

        Ok(())
    }

    fn create_texture_cmd(
        &mut self,
        slot: u128,
        pixel_size: usize,
        tex_format: vk::Format,
        _store_format: vk::Format,
        upload_data: VulkanDeviceInternalMemory,
    ) -> anyhow::Result<()> {
        let image_index = slot;

        let VulkanAllocatorImageCacheEntryData {
            width,
            height,
            depth,
            is_3d_tex,
            mip_map_count,
            ..
        } = self
            .device
            .mem_allocator
            .lock()
            .mem_image_cache_entry(upload_data.mem.as_mut_ptr());

        let texture_data = if !is_3d_tex {
            match self.device.create_texture_image(
                image_index,
                upload_data,
                tex_format,
                width,
                height,
                depth,
                pixel_size,
                mip_map_count,
                self.cur_image_index,
            ) {
                Ok((img, img_mem)) => {
                    let img_format = tex_format;
                    let img_view = self.device.create_texture_image_view(
                        &img,
                        img_format,
                        vk::ImageViewType::TYPE_2D,
                        1,
                        mip_map_count,
                    );
                    let img_view = img_view.unwrap(); // TODO: err handling

                    let mut samplers: [vk::Sampler; WRAP_TYPE_COUNT] = Default::default();
                    samplers[0] = self
                        .device
                        .get_texture_sampler(ESupportedSamplerTypes::Repeat);
                    samplers[1] = self
                        .device
                        .get_texture_sampler(ESupportedSamplerTypes::ClampToEdge);

                    let descriptors = self.device.create_new_textured_standard_descriptor_sets(
                        img_view.image_view,
                        &samplers,
                    )?;
                    TextureData::Tex2D {
                        img,
                        img_mem,
                        img_view,
                        samplers,
                        vk_standard_textured_descr_sets: descriptors,
                    }
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        } else {
            let image_3d_width = width;
            let image_3d_height = height;

            let (img_3d, img_mem_3d) = self.device.create_texture_image(
                image_index,
                upload_data,
                tex_format,
                image_3d_width,
                image_3d_height,
                depth,
                pixel_size,
                mip_map_count,
                self.cur_image_index,
            )?;
            let img_format = tex_format;
            let img_view = self.device.create_texture_image_view(
                &img_3d,
                img_format,
                vk::ImageViewType::TYPE_2D_ARRAY,
                depth,
                mip_map_count,
            );
            let img_3d_view = img_view.unwrap(); // TODO: err handling;
            let sampler_3d = self
                .device
                .get_texture_sampler(ESupportedSamplerTypes::Texture2DArray);

            let descr = self
                .device
                .create_new_3d_textured_standard_descriptor_sets(
                    img_3d_view.image_view,
                    sampler_3d,
                )?;

            TextureData::Tex3D {
                img_3d,
                img_3d_mem: img_mem_3d,
                img_3d_view,
                sampler_3d,
                vk_standard_3d_textured_descr_set: descr,
            }
        };

        let texture = CTexture {
            data: texture_data,
            width,
            height,
            depth,
            mip_map_count: mip_map_count as u32,
        };

        self.device.textures.insert(image_index, texture); // TODO better fix
        Ok(())
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
        false
    }

    fn get_address_mode_index(state: &State) -> usize {
        if state.wrap_mode == WrapType::Repeat {
            EVulkanBackendAddressModes::Repeat as usize
        } else {
            EVulkanBackendAddressModes::ClampEdges as usize
        }
    }

    fn get_blend_mode_index(state: &State) -> usize {
        if state.blend_mode == BlendType::Additive {
            EVulkanBackendBlendModes::Additative as usize
        } else {
            if state.blend_mode == BlendType::None {
                EVulkanBackendBlendModes::None as usize
            } else {
                EVulkanBackendBlendModes::Alpha as usize
            }
        }
    }

    fn get_dynamic_mode_index_from_state(&self, state: &State) -> usize {
        if state.clip_enable
            || self.has_dynamic_viewport
            || self.vk_swap_img_and_viewport_extent.has_forced_viewport
        {
            EVulkanBackendClipModes::DynamicScissorAndViewport as usize
        } else {
            EVulkanBackendClipModes::None as usize
        }
    }

    fn get_dynamic_mode_index_from_exec_buffer(exec_buffer: &RenderCommandExecuteBuffer) -> usize {
        if exec_buffer.has_dynamic_state {
            EVulkanBackendClipModes::DynamicScissorAndViewport as usize
        } else {
            EVulkanBackendClipModes::None as usize
        }
    }

    fn get_pipeline(
        container: &PipelineContainer,
        is_textured: bool,
        blend_mode_index: usize,
        dynamic_index: usize,
    ) -> &vk::Pipeline {
        &container.pipelines[blend_mode_index][dynamic_index][is_textured as usize]
    }

    fn get_pipe_layout(
        container: &PipelineContainer,
        is_textured: bool,
        blend_mode_index: usize,
        dynamic_index: usize,
    ) -> &vk::PipelineLayout {
        &container.pipeline_layouts[blend_mode_index][dynamic_index][is_textured as usize]
    }

    fn get_pipeline_and_layout(
        container: &PipelineContainer,
        is_textured: bool,
        blend_mode_index: usize,
        dynamic_index: usize,
    ) -> (&vk::Pipeline, &vk::PipelineLayout) {
        (
            &container.pipelines[blend_mode_index][dynamic_index][is_textured as usize],
            &container.pipeline_layouts[blend_mode_index][dynamic_index][is_textured as usize],
        )
    }

    fn get_pipeline_and_layout_mut(
        container: &mut PipelineContainer,
        is_textured: bool,
        blend_mode_index: usize,
        dynamic_index: usize,
    ) -> (&mut vk::Pipeline, &mut vk::PipelineLayout) {
        (
            &mut container.pipelines[blend_mode_index][dynamic_index][is_textured as usize],
            &mut container.pipeline_layouts[blend_mode_index][dynamic_index][is_textured as usize],
        )
    }

    fn get_standard_pipe_and_layout<'a>(
        standard_line_pipeline: &'a PipelineContainer,
        standard_pipeline: &'a PipelineContainer,
        standard_stencil_only_pipeline: &'a PipelineContainer,
        standard_stencil_when_passed_pipeline: &'a PipelineContainer,
        standard_stencil_pipeline: &'a PipelineContainer,
        is_line_geometry: bool,
        is_textured: bool,
        blend_mode_index: usize,
        dynamic_index: usize,
        stencil_type: StencilOpType,
    ) -> (&'a vk::Pipeline, &'a vk::PipelineLayout) {
        if is_line_geometry {
            Self::get_pipeline_and_layout(
                standard_line_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
            )
        } else {
            match stencil_type {
                StencilOpType::AlwaysPass => Self::get_pipeline_and_layout(
                    standard_stencil_only_pipeline,
                    is_textured,
                    blend_mode_index,
                    dynamic_index,
                ),
                StencilOpType::OnlyWhenPassed => Self::get_pipeline_and_layout(
                    standard_stencil_when_passed_pipeline,
                    is_textured,
                    blend_mode_index,
                    dynamic_index,
                ),
                StencilOpType::OnlyWhenNotPassed => Self::get_pipeline_and_layout(
                    standard_stencil_pipeline,
                    is_textured,
                    blend_mode_index,
                    dynamic_index,
                ),
                StencilOpType::None => Self::get_pipeline_and_layout(
                    standard_pipeline,
                    is_textured,
                    blend_mode_index,
                    dynamic_index,
                ),
            }
        }
    }

    fn get_tile_layer_pipe_layout(
        render: &RenderSetupGroup,
        layout_type: i32, // TODO: name the types
        is_textured: bool,
        blend_mode_index: usize,
        dynamic_index: usize,
        render_pass_type: RenderPassType,
    ) -> &vk::PipelineLayout {
        if layout_type == 0 {
            return Self::get_pipe_layout(
                &render.get().sub_render_pass(render_pass_type).tile_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
            );
        } else if layout_type == 1 {
            return Self::get_pipe_layout(
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .tile_border_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
            );
        } else {
            return Self::get_pipe_layout(
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .tile_border_line_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
            );
        }
    }

    fn get_tile_layer_pipe(
        render: &RenderSetupGroup,
        pipe_type: i32, // TODO: name the types
        is_textured: bool,
        blend_mode_index: usize,
        dynamic_index: usize,
        render_pass_type: RenderPassType,
    ) -> &vk::Pipeline {
        if pipe_type == 0 {
            return Self::get_pipeline(
                &render.get().sub_render_pass(render_pass_type).tile_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
            );
        } else if pipe_type == 1 {
            return Self::get_pipeline(
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .tile_border_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
            );
        } else {
            return Self::get_pipeline(
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .tile_border_line_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
            );
        }
    }

    fn get_state_indices(
        exec_buffer: &RenderCommandExecuteBuffer,
        state: &State,
        is_textured: &mut bool,
        blend_mode_index: &mut usize,
        dynamic_index: &mut usize,
        address_mode_index: &mut usize,
    ) {
        *is_textured = Self::get_is_textured(state);
        *address_mode_index = Self::get_address_mode_index(state);
        *blend_mode_index = Self::get_blend_mode_index(state);
        *dynamic_index = Self::get_dynamic_mode_index_from_exec_buffer(exec_buffer);
    }

    fn exec_buffer_fill_dynamic_states(
        &self,
        state: &State,
        exec_buffer: &mut RenderCommandExecuteBuffer,
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
                    scissor_viewport.height as i32 - (state.clip_y + state.clip_h as i32);
                let scissor_h = state.clip_h as i32;
                scissor.offset = vk::Offset2D {
                    x: state.clip_x,
                    y: scissor_y,
                };
                scissor.extent = vk::Extent2D {
                    width: state.clip_w,
                    height: scissor_h as u32,
                };
            } else {
                scissor.offset = vk::Offset2D::default();
                scissor.extent = vk::Extent2D {
                    width: scissor_viewport.width,
                    height: scissor_viewport.height,
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
        command_buffer: vk::CommandBuffer,
        exec_buffer: &RenderCommandExecuteBuffer,
        binding_pipe: vk::Pipeline,
        _state: &State,
    ) {
        unsafe {
            device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                binding_pipe,
            );
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
        exec_buffer: &mut RenderCommandExecuteBuffer,
        draw_calls: usize,
        state: &State,
        buffer_object_index: u128,
    ) {
        let buffer_object = self
            .device
            .buffer_objects
            .get(&buffer_object_index)
            .unwrap();

        exec_buffer.buffer = buffer_object.cur_buffer.buffer;
        exec_buffer.buffer_off = buffer_object.cur_buffer_offset;

        let is_textured: bool = Self::get_is_textured(state);
        if is_textured {
            exec_buffer.descriptors[0] = Some(
                self.device
                    .textures
                    .get(&state.texture_index.unwrap())
                    .unwrap()
                    .data
                    .unwrap_3d_descr()
                    .clone(),
            );
        }

        exec_buffer.index_buffer = self.render_index_buffer.as_ref().unwrap().buffer;

        exec_buffer.estimated_render_call_count = draw_calls;

        self.exec_buffer_fill_dynamic_states(state, exec_buffer);
    }

    #[must_use]
    fn render_tile_layer(
        device: &LogicalDevice,
        render: &RenderSetupGroup,
        render_pass_type: RenderPassType,
        exec_buffer: &RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
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
        Self::get_state_indices(
            exec_buffer,
            state,
            &mut is_textured,
            &mut blend_mode_index,
            &mut dynamic_index,
            &mut address_mode_index,
        );
        let pipe_layout = *Self::get_tile_layer_pipe_layout(
            render,
            layer_type,
            is_textured,
            blend_mode_index,
            dynamic_index,
            render_pass_type,
        );
        let pipe_line = *Self::get_tile_layer_pipe(
            render,
            layer_type,
            is_textured,
            blend_mode_index,
            dynamic_index,
            render_pass_type,
        );

        Self::bind_pipeline(
            &device.device,
            command_buffer.command_buffer,
            exec_buffer,
            pipe_line,
            state,
        );

        let vertex_buffers = [exec_buffer.buffer];
        let offsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            device.device.cmd_bind_vertex_buffers(
                command_buffer.command_buffer,
                0,
                &vertex_buffers,
                &offsets,
            );
        }

        if is_textured {
            unsafe {
                device.device.cmd_bind_descriptor_sets(
                    command_buffer.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipe_layout,
                    0,
                    &[exec_buffer.descriptors[0].as_ref().unwrap().set()],
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
            device.device.cmd_push_constants(
                command_buffer.command_buffer,
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
            device.device.cmd_push_constants(
                command_buffer.command_buffer,
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

        let draw_count: usize = indices_draw_num;
        unsafe {
            device.device.cmd_bind_index_buffer(
                command_buffer.command_buffer,
                exec_buffer.index_buffer,
                0,
                vk::IndexType::UINT32,
            );
        }
        for i in 0..draw_count {
            let index_offset = (indices_offsets[i] / std::mem::size_of::<u32>()) as vk::DeviceSize;

            unsafe {
                device.device.cmd_draw_indexed(
                    command_buffer.command_buffer,
                    draw_counts[i] as u32,
                    instance_count as u32,
                    index_offset as u32,
                    0,
                    0,
                );
            }
        }

        true
    }

    #[must_use]
    fn render_standard<TName, const IS_3D_TEXTURED: bool>(
        device: &LogicalDevice,
        render: &RenderSetupGroup,
        render_pass_type: RenderPassType,
        exec_buffer: &RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
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
        Self::get_state_indices(
            exec_buffer,
            state,
            &mut is_textured,
            &mut blend_mode_index,
            &mut dynamic_index,
            &mut address_mode_index,
        );
        let (pipeline_ref, pipe_layout_ref) = if IS_3D_TEXTURED {
            Self::get_pipeline_and_layout(
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .standard_3d_pipeline,
                is_textured,
                blend_mode_index,
                dynamic_index,
            )
        } else {
            Self::get_standard_pipe_and_layout(
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .standard_line_pipeline,
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .standard_pipeline,
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .standard_stencil_only_pipeline,
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .standard_stencil_when_passed_pipeline,
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .standard_stencil_pipeline,
                is_line_geometry,
                is_textured,
                blend_mode_index,
                dynamic_index,
                stencil_type,
            )
        };
        let (pipeline, pipe_layout) = (*pipeline_ref, *pipe_layout_ref);

        Self::render_standard_impl::<TName, { IS_3D_TEXTURED }>(
            device,
            exec_buffer,
            command_buffer,
            state,
            prim_type,
            primitive_count,
            &m,
            is_textured,
            pipeline,
            pipe_layout,
            has_push_const,
            false,
            0.0,
            false,
            vec4::default(),
        )
    }

    #[must_use]
    fn render_blur<TName>(
        device: &LogicalDevice,
        render: &RenderSetupGroup,
        render_pass_type: RenderPassType,
        exec_buffer: &RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
        state: &State,
        prim_type: PrimType,
        primitive_count: usize,
        blur_radius: f32,
        blur_horizontal: bool,
        blur_color: vec4,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::get_state_matrix(state, &mut m);

        let mut is_textured: bool = Default::default();
        let mut blend_mode_index: usize = Default::default();
        let mut dynamic_index: usize = Default::default();
        let mut address_mode_index: usize = Default::default();
        Self::get_state_indices(
            exec_buffer,
            state,
            &mut is_textured,
            &mut blend_mode_index,
            &mut dynamic_index,
            &mut address_mode_index,
        );
        let (pipeline_ref, pipe_layout_ref) = Self::get_pipeline_and_layout(
            &render.get().sub_render_pass(render_pass_type).blur_pipeline,
            is_textured,
            blend_mode_index,
            dynamic_index,
        );
        let (pipeline, pipe_layout) = (*pipeline_ref, *pipe_layout_ref);

        Self::render_standard_impl::<TName, false>(
            device,
            exec_buffer,
            command_buffer,
            state,
            prim_type,
            primitive_count,
            &m,
            is_textured,
            pipeline,
            pipe_layout,
            false,
            true,
            blur_radius,
            blur_horizontal,
            blur_color,
        )
    }

    #[must_use]
    fn render_standard_impl<TName, const IS_3D_TEXTURED: bool>(
        device: &LogicalDevice,
        exec_buffer: &RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
        state: &State,
        prim_type: PrimType,
        primitive_count: usize,
        m: &[f32],
        is_textured: bool,
        pipeline: vk::Pipeline,
        pipe_layout: vk::PipelineLayout,
        has_push_const: bool,
        as_blur: bool,
        blur_radius: f32,
        blur_horizontal: bool,
        blur_color: vec4,
    ) -> bool {
        Self::bind_pipeline(
            &device.device,
            command_buffer.command_buffer,
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
            device.device.cmd_bind_vertex_buffers(
                command_buffer.command_buffer,
                0,
                vertex_buffers.as_slice(),
                buffer_offsets.as_slice(),
            );
        }

        if is_indexed {
            unsafe {
                device.device.cmd_bind_index_buffer(
                    command_buffer.command_buffer,
                    exec_buffer.index_buffer,
                    0,
                    vk::IndexType::UINT32,
                );
            }
        }
        if is_textured {
            unsafe {
                device.device.cmd_bind_descriptor_sets(
                    command_buffer.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipe_layout.clone(),
                    0,
                    &[exec_buffer.descriptors[0].as_ref().unwrap().set()],
                    &[],
                );
            }
        }

        if has_push_const {
            unsafe {
                device.device.cmd_push_constants(
                    command_buffer.command_buffer,
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
                    exec_buffer.viewport_size.width as f32,
                    exec_buffer.viewport_size.height as f32,
                ),
                blur_radius,
                blur_horizontal: if blur_horizontal { 1 } else { 0 },
                color: blur_color,
            };
            unsafe {
                device.device.cmd_push_constants(
                    command_buffer.command_buffer,
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
                device.device.cmd_draw_indexed(
                    command_buffer.command_buffer,
                    (primitive_count * 6) as u32,
                    1,
                    0,
                    0,
                    0,
                );
            }
        } else {
            unsafe {
                device.device.cmd_draw(
                    command_buffer.command_buffer,
                    (primitive_count * vert_per_prim) as u32,
                    1,
                    0,
                    0,
                );
            }
        }

        true
    }

    /************************
     * VULKAN SETUP CODE
     ************************/

    fn our_image_usages() -> Vec<vk::ImageUsageFlags> {
        let mut img_usages: Vec<vk::ImageUsageFlags> = Default::default();

        img_usages.push(vk::ImageUsageFlags::COLOR_ATTACHMENT);
        img_usages.push(vk::ImageUsageFlags::TRANSFER_SRC);
        img_usages.push(vk::ImageUsageFlags::TRANSFER_DST);

        img_usages
    }

    fn create_surface(
        entry: &ash::Entry,
        raw_window: &BackendWindow,
        surface: &mut BackendSurface,
        instance: &ash::Instance,
        phy_gpu: &vk::PhysicalDevice,
        queue_family_index: u32,
        device_instance: &Device,
    ) -> anyhow::Result<()> {
        unsafe { surface.create_vk_surface(entry, instance, raw_window, device_instance) }?;

        let is_supported =
            unsafe { surface.get_physical_device_surface_support(*phy_gpu, queue_family_index) }?;
        if !is_supported {
            return Err(anyhow!("The device surface does not support presenting the framebuffer to a screen. (maybe the wrong GPU was selected?)"));
        }

        Ok(())
    }

    fn destroy_surface(&mut self) {
        unsafe { self.ash_vk.surface.destroy_vk_surface(&mut self.device) };
    }

    fn get_presentation_mode(&mut self) -> anyhow::Result<vk::PresentModeKHR> {
        let present_mode_list = unsafe {
            self.ash_vk
                .surface
                .get_physical_device_surface_present_modes(self.vk_gpu.cur_device)
        }
        .map_err(|err| anyhow!("get_physical_device_surface_present_modes failed: {err}"))?;

        let mut vk_io_mode = if self.gfx_vsync {
            vk::PresentModeKHR::FIFO
        } else {
            vk::PresentModeKHR::IMMEDIATE
        };
        for mode in &present_mode_list {
            if *mode == vk_io_mode {
                return Ok(vk_io_mode);
            }
        }

        // TODO dbg_msg("vulkan", "warning: requested presentation mode was not available. falling back to mailbox / fifo relaxed.");
        vk_io_mode = if self.gfx_vsync {
            vk::PresentModeKHR::FIFO_RELAXED
        } else {
            vk::PresentModeKHR::MAILBOX
        };
        for mode in &present_mode_list {
            if *mode == vk_io_mode {
                return Ok(vk_io_mode);
            }
        }

        self.logger
            .log(LogLevel::Warning)
            .msg("requested presentation mode was not available. using first available.");
        if present_mode_list.len() > 0 {
            vk_io_mode = present_mode_list[0];
        } else {
            return Err(anyhow!("List of presentation modes was empty."));
        }

        Ok(vk_io_mode)
    }

    #[must_use]
    fn get_surface_properties(
        &mut self,
        vk_surf_capabilities: &mut vk::SurfaceCapabilitiesKHR,
    ) -> bool {
        let capabilities_res = unsafe {
            self.ash_vk
                .surface
                .get_physical_device_surface_capabilities(self.vk_gpu.cur_device)
        };
        if let Err(_) = capabilities_res {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::Init,
                "The device surface capabilities could not be fetched.",
            );
            return false;
        }
        *vk_surf_capabilities = capabilities_res.unwrap();
        true
    }

    fn get_number_of_swap_images(&mut self, vk_capabilities: &vk::SurfaceCapabilitiesKHR) -> u32 {
        let img_number = vk_capabilities.min_image_count + 1;
        if is_verbose(&*self.dbg) {
            self.logger
                .log(LogLevel::Debug)
                .msg("minimal swap image count ")
                .msg_var(&vk_capabilities.min_image_count);
        }
        if vk_capabilities.max_image_count > 0 && img_number > vk_capabilities.max_image_count {
            vk_capabilities.max_image_count
        } else {
            img_number
        }
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

        let auto_viewport_extent = ret_size;
        let uses_forced_viewport: bool = false;
        // keep this in sync with graphics_threaded AdjustViewport's check
        /* TODO: i'd say we don't need this anymore: egui is quite ok with weird resolutions if auto_viewport_extent.height > 4 * auto_viewport_extent.width / 5 {
            auto_viewport_extent.height = 4 * auto_viewport_extent.width / 5;
            uses_forced_viewport = true;
        }*/

        let mut ext = SSwapImgViewportExtent::default();
        ext.swap_image_viewport = ret_size;
        ext.forced_viewport = auto_viewport_extent;
        ext.has_forced_viewport = uses_forced_viewport;

        ext
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

        true
    }

    fn get_transform(vk_capabilities: &vk::SurfaceCapabilitiesKHR) -> vk::SurfaceTransformFlagsKHR {
        if !(vk_capabilities.supported_transforms & vk::SurfaceTransformFlagsKHR::IDENTITY)
            .is_empty()
        {
            return vk::SurfaceTransformFlagsKHR::IDENTITY;
        }
        vk_capabilities.current_transform
    }

    #[must_use]
    fn get_format(&mut self) -> bool {
        let _surf_formats: u32 = 0;
        let res = unsafe {
            self.ash_vk
                .surface
                .get_physical_device_surface_formats(self.vk_gpu.cur_device)
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
            self.render.get_mut().vk_surf_format.format = vk::Format::B8G8R8A8_UNORM;
            self.render.get_mut().vk_surf_format.color_space = vk::ColorSpaceKHR::SRGB_NONLINEAR;
            // TODO dbg_msg("vulkan", "warning: surface format was undefined. This can potentially cause bugs.");
            return true;
        }

        for find_format in &surf_format_list {
            if (find_format.format == vk::Format::B8G8R8A8_UNORM
                && find_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
                || (find_format.format == vk::Format::R8G8B8A8_UNORM
                    && find_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
            {
                self.render.get_mut().vk_surf_format = *find_format;
                return true;
            }
        }

        // TODO dbg_msg("vulkan", "warning: surface format was not RGBA(or variants of it). This can potentially cause weird looking images(too bright etc.).");
        self.render.get_mut().vk_surf_format = surf_format_list[0];
        true
    }

    fn create_swap_chain(&mut self, old_swap_chain: &mut vk::SwapchainKHR) -> anyhow::Result<()> {
        let mut vksurf_cap = vk::SurfaceCapabilitiesKHR::default();
        if !self.get_surface_properties(&mut vksurf_cap) {
            return Err(anyhow!("Could not get surface properties"));
        }

        let present_mode = self.get_presentation_mode()?;

        let swap_img_count = self.get_number_of_swap_images(&vksurf_cap);

        self.vk_swap_img_and_viewport_extent = self.get_swap_image_size(&vksurf_cap);

        let mut usage_flags = vk::ImageUsageFlags::default();
        if !self.get_image_usage(&vksurf_cap, &mut usage_flags) {
            return Err(anyhow!("Could not get image usage"));
        }

        let transform_flag_bits = Self::get_transform(&vksurf_cap);

        if !self.get_format() {
            return Err(anyhow!("Could image format"));
        }

        let mut swap_info = vk::SwapchainCreateInfoKHR::default();
        swap_info.flags = vk::SwapchainCreateFlagsKHR::empty();

        swap_info.min_image_count = swap_img_count;
        swap_info.image_format = self.render.get().vk_surf_format.format;
        swap_info.image_color_space = self.render.get().vk_surf_format.color_space;
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

        let res = unsafe {
            self.ash_vk
                .vk_swap_chain_ash
                .create_swapchain(&self.ash_vk.surface, swap_info)
        };

        if let Err(err) = res {
            let crit_error_msg = self.check_res.check_vulkan_critical_error(
                err,
                &self.error,
                &mut self.recreate_swap_chain,
            );
            if let Some(crit_err) = crit_error_msg {
                self.error.lock().unwrap().set_error_extra(
                    EGFXErrorType::Init,
                    "Creating the swap chain failed.",
                    Some(crit_err),
                );
                return Err(anyhow!("Creating the swap chain failed {crit_err}"));
            } else if res.unwrap_err() == vk::Result::ERROR_NATIVE_WINDOW_IN_USE_KHR {
                return Err(anyhow!("Window was in use."));
            }
        }

        *old_swap_chain = res.unwrap();

        Ok(())
    }

    fn destroy_swap_chain(&mut self, force_destroy: bool) {
        if force_destroy {
            unsafe {
                self.ash_vk.vk_swap_chain_ash.destroy_swapchain();
            }
        }
    }

    #[must_use]
    fn get_swap_chain_image_handles(&mut self) -> bool {
        let res = unsafe { self.ash_vk.vk_swap_chain_ash.get_swapchain_images() };
        if res.is_err() {
            self.error
                .lock()
                .unwrap()
                .set_error(EGFXErrorType::Init, "Could not get swap chain images.");
            return false;
        }

        self.render.get_mut().native.swap_chain_images = res.unwrap();
        self.device.swap_chain_image_count =
            self.render.get().native.swap_chain_images.len() as u32;
        self.ash_vk
            .vk_device
            .memory_allocator
            .lock()
            .set_frame_count(self.render.get().native.swap_chain_images.len());
        self.device
            .mem_allocator
            .lock()
            .set_frame_count(self.render.get().native.swap_chain_images.len());
        self.command_pool
            .set_frame_count(self.render.get().native.swap_chain_images.len());
        for thread in self.render_thread_infos.iter() {
            thread.1.store(
                self.render.get().native.swap_chain_images.len() as u32,
                std::sync::atomic::Ordering::SeqCst,
            );
        }

        true
    }

    fn clear_swap_chain_image_handles(&mut self) {
        self.render.get_mut().native.swap_chain_images.clear();
    }

    fn get_device_queue(
        device: &ash::Device,
        graphics_queue_index: u32,
    ) -> anyhow::Result<(vk::Queue, vk::Queue)> {
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
            panic!("[vulkan debug] error: {}", unsafe {
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

        vk::FALSE
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

    fn destroy_debug_utils_messenger_ext(&self, debug_messenger: &vk::DebugUtilsMessengerEXT) {
        let dbg_utils = ash::extensions::ext::DebugUtils::new(
            &self.ash_vk.instance.vk_entry,
            &self.ash_vk.instance.vk_instance,
        );
        unsafe { dbg_utils.destroy_debug_utils_messenger(*debug_messenger, None) };
    }

    fn setup_debug_callback(
        entry: &ash::Entry,
        instance: &ash::Instance,
        logger: &SystemLogGroup,
    ) -> anyhow::Result<vk::DebugUtilsMessengerEXT> {
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
            return Err(anyhow!("Debug extension could not be loaded."));
        } else {
            logger
                .log(LogLevel::Info)
                .msg("enabled vulkan debug context.");
        }
        Ok(res_dbg)
    }

    fn unregister_debug_callback(&mut self) {
        if self.debug_messenger != vk::DebugUtilsMessengerEXT::null() {
            self.destroy_debug_utils_messenger_ext(&self.debug_messenger);
        }
    }

    #[must_use]
    fn create_image_views(&mut self) -> bool {
        let onscreen = &mut self.render.onscreen;
        let swap_chain_count = self.device.swap_chain_image_count;
        let img_format = onscreen.vk_surf_format.format;
        let image_views = &mut onscreen.native.swap_chain_image_view_list;
        let images = &mut onscreen.native.swap_chain_images;

        image_views.resize(swap_chain_count as usize, Default::default());

        for i in 0..swap_chain_count {
            let res = self.device.create_image_view_swap_chain(
                &ImageFakeForSwapchainImgs {
                    img: images[i as usize],
                },
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
        for imgage_view in &mut self.render.get_mut().native.swap_chain_image_view_list {
            unsafe {
                self.ash_vk
                    .vk_device
                    .device
                    .destroy_image_view(*imgage_view, None);
            }
        }

        self.render
            .get_mut()
            .native
            .swap_chain_image_view_list
            .clear();
    }

    #[must_use]
    fn create_swapchain_images_for_switching_framebuffer(&mut self, ty: RenderPassType) -> bool {
        let swap_chain_count = self.device.swap_chain_image_count as usize;

        let onscreen = &mut self.render.onscreen;

        let offscreen_framebuffer = match ty {
            RenderPassType::Switching1 => &mut onscreen.switching.passes[0].surface,
            RenderPassType::Switching2 => &mut onscreen.switching.passes[1].surface,
            _ => panic!("only for switching buffers"),
        };

        offscreen_framebuffer.image_list.reserve(swap_chain_count);

        let mut res = true;
        (0..swap_chain_count).for_each(|_| {
            let img_res = self.device.mem_allocator.lock().create_image_ex(
                self.vk_swap_img_and_viewport_extent
                    .swap_image_viewport
                    .width,
                self.vk_swap_img_and_viewport_extent
                    .swap_image_viewport
                    .height,
                1,
                1,
                onscreen.vk_surf_format.format,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | vk::ImageUsageFlags::INPUT_ATTACHMENT
                    | vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::SAMPLED,
                None,
                vk::ImageLayout::UNDEFINED,
            );
            if img_res.is_err() {
                res = false;
            }
            let (img, img_mem) = img_res.unwrap();

            let img_view = self
                .device
                .create_image_view(
                    &img,
                    onscreen.vk_surf_format.format,
                    vk::ImageViewType::TYPE_2D,
                    1,
                    1,
                    vk::ImageAspectFlags::COLOR,
                )
                .unwrap(); // TODO: error handling

            let samplers = [
                self.device
                    .get_texture_sampler(ESupportedSamplerTypes::Repeat),
                self.device
                    .get_texture_sampler(ESupportedSamplerTypes::ClampToEdge),
            ];

            let descr_res = self
                .device
                .create_new_textured_standard_descriptor_sets(img_view.image_view, &samplers);
            if descr_res.is_err() {
                self.error.lock().unwrap().set_error(
                    EGFXErrorType::Init,
                    "Could not create image descriptors for switching pass images.",
                );
                res = false;
            }
            let descr = descr_res.unwrap();

            offscreen_framebuffer.image_list.push(SwapChainImageFull {
                base: SwapChainImageBase {
                    image: img,
                    img_mem,
                    img_view,

                    layout_is_undefined: true,
                },
                samplers,
                texture_descr_sets: descr,
            });
        });
        res
    }

    #[must_use]
    fn create_images_for_switching_passes(&mut self) -> bool {
        self.create_swapchain_images_for_switching_framebuffer(RenderPassType::Switching1)
            && self.create_swapchain_images_for_switching_framebuffer(RenderPassType::Switching2)
    }

    fn destroy_images_for_switching_passes(&mut self) {
        self.render.get_mut().switching.passes[0]
            .surface
            .image_list
            .clear();
        self.render.get_mut().switching.passes[1]
            .surface
            .image_list
            .clear();
    }

    #[must_use]
    fn create_multi_sampler_image_attachments(&mut self) -> bool {
        let has_multi_sampling = self.has_multi_sampling();
        let multi_sampling_count = self
            .device
            .vk_gpu
            .config
            .read()
            .unwrap()
            .multi_sampling_count;
        let onscreen = &mut self.render.onscreen;
        let multi_sampling_images = &mut onscreen.native.swap_chain_multi_sampling_images;
        multi_sampling_images.reserve(self.device.swap_chain_image_count as usize);
        if has_multi_sampling {
            for _ in 0..self.device.swap_chain_image_count {
                let img_res = self.device.mem_allocator.lock().create_image_ex(
                    self.vk_swap_img_and_viewport_extent
                        .swap_image_viewport
                        .width,
                    self.vk_swap_img_and_viewport_extent
                        .swap_image_viewport
                        .height,
                    1,
                    1,
                    onscreen.vk_surf_format.format,
                    vk::ImageTiling::OPTIMAL,
                    vk::ImageUsageFlags::TRANSIENT_ATTACHMENT
                        | vk::ImageUsageFlags::COLOR_ATTACHMENT,
                    Some(multi_sampling_count),
                    vk::ImageLayout::UNDEFINED,
                );
                if img_res.is_err() {
                    return false;
                }
                let (img, img_mem) = img_res.unwrap();

                let img_view = self
                    .device
                    .create_image_view(
                        &img,
                        onscreen.vk_surf_format.format,
                        vk::ImageViewType::TYPE_2D,
                        1,
                        1,
                        vk::ImageAspectFlags::COLOR,
                    )
                    .unwrap(); // TODO: err handling

                multi_sampling_images.push(SwapChainImageBase {
                    image: img,
                    img_mem,
                    img_view,

                    layout_is_undefined: true,
                });
            }
        }

        true
    }

    fn destroy_multi_sampler_image_attachments(&mut self) {
        let multi_sampling_images = &mut self
            .render
            .get_mut()
            .native
            .swap_chain_multi_sampling_images;
        multi_sampling_images.clear();
    }

    #[must_use]
    fn create_stencil_attachments_for_pass_transition(&mut self) -> bool {
        let has_multi_sampling = self.has_multi_sampling();
        let multi_sampling_count = if has_multi_sampling {
            Some(
                self.device
                    .vk_gpu
                    .config
                    .read()
                    .unwrap()
                    .multi_sampling_count,
            )
        } else {
            None
        };
        let onscreen = &mut self.render.onscreen;
        let stencil_images = &mut onscreen.switching.stencil_list_for_pass_transition;
        stencil_images.reserve(self.device.swap_chain_image_count as usize);

        // determine stencil image format
        onscreen.stencil_format = [
            vk::Format::S8_UINT,
            vk::Format::D32_SFLOAT_S8_UINT,
            vk::Format::D24_UNORM_S8_UINT,
            vk::Format::D16_UNORM_S8_UINT,
        ]
        .into_iter()
        .find(|format| {
            let props = unsafe {
                self.ash_vk
                    .instance
                    .vk_instance
                    .get_physical_device_format_properties(self.vk_gpu.cur_device, *format)
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

        for _ in 0..self.device.swap_chain_image_count {
            let img_res = self.device.mem_allocator.lock().create_image_ex(
                self.vk_swap_img_and_viewport_extent
                    .swap_image_viewport
                    .width,
                self.vk_swap_img_and_viewport_extent
                    .swap_image_viewport
                    .height,
                1,
                1,
                onscreen.stencil_format,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                multi_sampling_count,
                vk::ImageLayout::UNDEFINED,
            );
            if img_res.is_err() {
                return false;
            }
            let (img, img_mem) = img_res.unwrap();
            let img_view = self
                .device
                .create_image_view(
                    &img,
                    onscreen.stencil_format,
                    vk::ImageViewType::TYPE_2D,
                    1,
                    1,
                    vk::ImageAspectFlags::STENCIL,
                )
                .unwrap(); // TODO: err handling

            stencil_images.push(SwapChainImageBase {
                image: img,
                img_mem,
                img_view,

                layout_is_undefined: true,
            });
        }
        true
    }

    fn destroy_stencil_attachments_for_pass_transition(&mut self) {
        let stencil_images = &mut self
            .render
            .get_mut()
            .switching
            .stencil_list_for_pass_transition;
        stencil_images.clear();
    }

    #[must_use]
    fn create_render_pass(&mut self) -> bool {
        let has_multi_sampling = self.has_multi_sampling();
        let onscreen = &mut self.render.onscreen;
        match onscreen.create_render_pass_impl(
            &self.vk_gpu,
            &self.ash_vk.vk_device,
            has_multi_sampling,
            onscreen.vk_surf_format.format,
        ) {
            Ok(render_pass) => {
                onscreen.native.render_pass.pass = render_pass;
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
    fn create_render_pass_switchting(&mut self) -> bool {
        let mut res = true;
        let has_multi_sampling = self.has_multi_sampling();
        let onscreen = &mut self.render.onscreen;
        for i in 0..2 {
            res &= match onscreen.create_render_pass_switching(
                &self.vk_gpu,
                &self.ash_vk.vk_device,
                has_multi_sampling,
                onscreen.vk_surf_format.format,
            ) {
                Ok(render_pass) => {
                    onscreen.switching.passes[i].render_pass.pass = render_pass;
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
        res
    }

    fn destroy_render_pass(&mut self) {
        unsafe {
            self.ash_vk
                .vk_device
                .device
                .destroy_render_pass(self.render.get().native.render_pass.pass, None);
        }
    }

    fn destroy_render_pass_switching_passes(&mut self) {
        for i in 0..2 {
            unsafe {
                self.ash_vk.vk_device.device.destroy_render_pass(
                    self.render.get().switching.passes[i].render_pass.pass,
                    None,
                );
            }
        }
    }

    #[must_use]
    fn create_framebuffers_impl(&mut self, ty: RenderPassType) -> bool {
        let has_multi_sampling_targets = self.has_multi_sampling();
        let onscreen = &mut self.render.onscreen;
        let framebuffer_list = match ty {
            RenderPassType::Single => &mut onscreen.native.framebuffer_list,
            RenderPassType::Switching1 => &mut onscreen.switching.passes[0].framebuffer_list,
            RenderPassType::Switching2 => &mut onscreen.switching.passes[1].framebuffer_list,
        };
        framebuffer_list.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );

        for i in 0..self.device.swap_chain_image_count as usize {
            let mut attachments: [vk::ImageView; 5] = Default::default();
            let mut attachment_count = 0;
            attachments[attachment_count] = onscreen.native.swap_chain_image_view_list[i];
            attachment_count += 1;
            if has_multi_sampling_targets {
                attachments[attachment_count] = onscreen.native.swap_chain_multi_sampling_images[i]
                    .img_view
                    .image_view;
                attachment_count += 1;
            }
            match ty {
                RenderPassType::Switching1 | RenderPassType::Switching2 => {
                    let this_index = if let RenderPassType::Switching1 = ty {
                        0
                    } else {
                        1
                    };

                    attachments[0] = onscreen.switching.passes[this_index].surface.image_list[i]
                        .base
                        .img_view
                        .image_view;
                    attachments[attachment_count] =
                        onscreen.switching.stencil_list_for_pass_transition[i]
                            .img_view
                            .image_view;
                    attachment_count += 1;
                }
                _ => {}
            }

            let mut framebuffer_info = vk::FramebufferCreateInfo::default();
            framebuffer_info.render_pass = match ty {
                RenderPassType::Single => onscreen.native.render_pass.pass,
                RenderPassType::Switching1 => onscreen.switching.passes[0].render_pass.pass,
                RenderPassType::Switching2 => onscreen.switching.passes[1].render_pass.pass,
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
                    .device
                    .create_framebuffer(&framebuffer_info, None)
            };
            if res.is_err() {
                self.error
                    .lock()
                    .unwrap()
                    .set_error(EGFXErrorType::Init, "Creating the framebuffers failed.");
                return false;
            }
            framebuffer_list[i] = res.unwrap();
        }

        true
    }

    fn create_framebuffers(&mut self) -> bool {
        self.create_framebuffers_impl(RenderPassType::Single)
    }

    fn create_framebuffers_switching_passes(&mut self) -> bool {
        self.create_framebuffers_impl(RenderPassType::Switching1)
            && self.create_framebuffers_impl(RenderPassType::Switching2)
    }

    fn destroy_framebuffers(&mut self) {
        let onscreen = &mut self.render.onscreen;
        for frame_buffer in &mut onscreen.native.framebuffer_list {
            unsafe {
                self.ash_vk
                    .vk_device
                    .device
                    .destroy_framebuffer(*frame_buffer, None);
            }
        }

        onscreen.native.framebuffer_list.clear();
    }

    fn destroy_framebuffers_switching_passes(&mut self) {
        let onscreen = &mut self.render.onscreen;
        for i in 0..2 {
            for frame_buffer in &mut onscreen.switching.passes[i].framebuffer_list {
                unsafe {
                    self.ash_vk
                        .vk_device
                        .device
                        .destroy_framebuffer(*frame_buffer, None);
                }
            }

            onscreen.switching.passes[i].framebuffer_list.clear();
        }
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
                .device
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

        true
    }

    fn load_shader(&mut self, file_name: &str) -> anyhow::Result<Vec<u8>> {
        let f = self
            .shader_files
            .get(file_name)
            .ok_or(anyhow!("Shader file was not loaded: "))?;

        Ok(f.binary.clone())
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

        shader_module.vk_device = self.ash_vk.vk_device.device.clone();

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
        true
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
        multisampling.rasterization_samples = Device::get_sample_count(
            self.device
                .vk_gpu
                .config
                .read()
                .unwrap()
                .multi_sampling_count,
            &self.device.vk_gpu.limits,
        );

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

        true
    }

    #[must_use]
    fn create_graphics_pipeline_ex<const FORCE_REQUIRE_DESCRIPTORS: bool>(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut PipelineContainer,
        stride: u32,
        input_attributes: &[vk::VertexInputAttributeDescription],
        set_layouts: &[vk::DescriptorSetLayout],
        push_constants: &[vk::PushConstantRange],
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        is_line_prim: bool,
        render_pass_type: RenderPassType,
        stencil_type: StencilOpType,
    ) -> bool {
        let mut shader_stages: [vk::PipelineShaderStageCreateInfo; 2] = Default::default();
        let mut module = SShaderModule::new(&self.ash_vk.vk_device.device);
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
        );

        let res = unsafe {
            self.ash_vk
                .vk_device
                .device
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
                stencil_state.stencil_test_enable = vk::FALSE;
                pipeline_info.p_depth_stencil_state = &stencil_state;
            }
        }
        pipeline_info.layout = *pipe_layout;
        pipeline_info.render_pass = match render_pass_type {
            RenderPassType::Single => self.render.get().native.render_pass.pass,
            RenderPassType::Switching1 => self.render.get().switching.passes[0].render_pass.pass,
            RenderPassType::Switching2 => self.render.get().switching.passes[1].render_pass.pass,
        };
        pipeline_info.subpass = match render_pass_type {
            RenderPassType::Single => 0,
            RenderPassType::Switching1 => 0,
            RenderPassType::Switching2 => 0,
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
            self.ash_vk.vk_device.device.create_graphics_pipelines(
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

        true
    }

    #[must_use]
    fn create_graphics_pipeline<const FORCE_REQUIRE_DESCRIPTORS: bool>(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut PipelineContainer,
        stride: u32,
        input_attributes: &[vk::VertexInputAttributeDescription],
        set_layouts: &[vk::DescriptorSetLayout],
        push_constants: &[vk::PushConstantRange],
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
        stencil_only: StencilOpType,
    ) -> bool {
        self.create_graphics_pipeline_ex::<{ FORCE_REQUIRE_DESCRIPTORS }>(
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
        )
    }

    #[must_use]
    fn create_standard_graphics_pipeline_impl(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut PipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        is_line_prim: bool,
        render_pass_type: RenderPassType,
        stencil_only: StencilOpType,
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

        let set_layouts = [self.device.standard_textured_descriptor_set_layout.layout];

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

        self.create_graphics_pipeline_ex::<false>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &attribute_descriptors,
            &set_layouts,
            push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            is_line_prim,
            render_pass_type,
            stencil_only,
        )
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

        for n in 0..RENDER_PASS_TYPE_COUNT {
            let render_pass_type = FromPrimitive::from_usize(n).unwrap();
            let mut pipe_container = if is_line_pipe {
                self.render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .standard_line_pipeline
                    .clone()
            } else {
                match stencil_only {
                    StencilOpType::AlwaysPass => self
                        .render
                        .get()
                        .sub_render_pass(render_pass_type)
                        .standard_stencil_only_pipeline
                        .clone(),
                    StencilOpType::OnlyWhenPassed => self
                        .render
                        .get()
                        .sub_render_pass(render_pass_type)
                        .standard_stencil_when_passed_pipeline
                        .clone(),
                    StencilOpType::OnlyWhenNotPassed => self
                        .render
                        .get()
                        .sub_render_pass(render_pass_type)
                        .standard_stencil_pipeline
                        .clone(),
                    StencilOpType::None => self
                        .render
                        .get()
                        .sub_render_pass(render_pass_type)
                        .standard_pipeline
                        .clone(),
                }
            };

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
                    );
                }
            }

            let cont = if is_line_pipe {
                &mut self
                    .render
                    .get_mut()
                    .sub_render_pass_mut(render_pass_type)
                    .standard_line_pipeline
            } else {
                match stencil_only {
                    StencilOpType::AlwaysPass => {
                        &mut self
                            .render
                            .get_mut()
                            .sub_render_pass_mut(render_pass_type)
                            .standard_stencil_only_pipeline
                    }
                    StencilOpType::OnlyWhenPassed => {
                        &mut self
                            .render
                            .get_mut()
                            .sub_render_pass_mut(render_pass_type)
                            .standard_stencil_when_passed_pipeline
                    }
                    StencilOpType::OnlyWhenNotPassed => {
                        &mut self
                            .render
                            .get_mut()
                            .sub_render_pass_mut(render_pass_type)
                            .standard_stencil_pipeline
                    }
                    StencilOpType::None => {
                        &mut self
                            .render
                            .get_mut()
                            .sub_render_pass_mut(render_pass_type)
                            .standard_pipeline
                    }
                }
            };
            *cont = pipe_container;
        }

        ret
    }

    #[must_use]
    fn create_standard_3d_graphics_pipeline_impl(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut PipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
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

        let set_layouts = [self
            .device
            .standard_3d_textured_descriptor_set_layout
            .layout];

        let push_constants = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: std::mem::size_of::<SUniformGPos>() as u32,
        }];

        self.create_graphics_pipeline::<false>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * 2
                + std::mem::size_of::<u8>() * 4
                + std::mem::size_of::<f32>() * 3) as u32,
            &attribute_descriptors,
            &set_layouts,
            &push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
        )
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

        for n in 0..RENDER_PASS_TYPE_COUNT {
            let render_pass_type = FromPrimitive::from_usize(n).unwrap();
            let mut pipe_container = self
                .render
                .get()
                .sub_render_pass(render_pass_type)
                .standard_3d_pipeline
                .clone();

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
                    );
                }
            }

            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .standard_3d_pipeline = pipe_container;
        }

        ret
    }

    #[must_use]
    fn create_blur_graphics_pipeline_impl(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut PipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
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

        let set_layouts = [self.device.standard_textured_descriptor_set_layout.layout];

        let push_constants = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: std::mem::size_of::<SUniformGBlur>() as u32,
        }];

        self.create_graphics_pipeline_ex::<false>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &attribute_descriptors,
            &set_layouts,
            &push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            false,
            render_pass_type,
            StencilOpType::OnlyWhenPassed,
        )
    }

    #[must_use]
    fn create_blur_graphics_pipeline(&mut self, vert_name: &str, frag_name: &str) -> bool {
        let mut ret: bool = true;

        let tex_mode = EVulkanBackendTextureModes::Textured;

        for n in 0..RENDER_PASS_TYPE_COUNT {
            let render_pass_type = FromPrimitive::from_usize(n).unwrap();

            let mut pipe_container = self
                .render
                .get()
                .sub_render_pass(render_pass_type)
                .blur_pipeline
                .clone();

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
                    );
                }
            }

            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .blur_pipeline = pipe_container;
        }

        ret
    }

    #[must_use]
    fn create_tile_graphics_pipeline_impl<const HAS_SAMPLER: bool>(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_type: i32, // TODO: use a type instead
        pipe_container: &mut PipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
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

        let set_layouts = [self
            .device
            .standard_3d_textured_descriptor_set_layout
            .layout];

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
            &attribute_descriptors
                .split_at_mut(if HAS_SAMPLER { 2 } else { 1 })
                .0,
            &set_layouts,
            &push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
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

        for n in 0..RENDER_PASS_TYPE_COUNT {
            let render_pass_type = FromPrimitive::from_usize(n).unwrap();

            let mut pipe_container = if pipe_type == 0 {
                self.render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .tile_pipeline
                    .clone()
            } else {
                if pipe_type == 1 {
                    self.render
                        .get()
                        .sub_render_pass(render_pass_type)
                        .tile_border_pipeline
                        .clone()
                } else {
                    self.render
                        .get()
                        .sub_render_pass(render_pass_type)
                        .tile_border_line_pipeline
                        .clone()
                }
            };

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
                    );
                }
            }

            let cont = if pipe_type == 0 {
                &mut self
                    .render
                    .get_mut()
                    .sub_render_pass_mut(render_pass_type)
                    .tile_pipeline
            } else {
                if pipe_type == 1 {
                    &mut self
                        .render
                        .get_mut()
                        .sub_render_pass_mut(render_pass_type)
                        .tile_border_pipeline
                } else {
                    &mut self
                        .render
                        .get_mut()
                        .sub_render_pass_mut(render_pass_type)
                        .tile_border_line_pipeline
                }
            };
            *cont = pipe_container;
        }

        ret
    }

    #[must_use]
    fn create_prim_ex_graphics_pipeline_impl(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        rotationless: bool,
        pipe_container: &mut PipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
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

        let set_layouts = [self.device.standard_textured_descriptor_set_layout.layout];
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

        self.create_graphics_pipeline::<false>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &attribute_descriptors,
            &set_layouts,
            &push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
        )
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

        for n in 0..RENDER_PASS_TYPE_COUNT {
            let render_pass_type = FromPrimitive::from_usize(n).unwrap();

            let mut pipe_container = if rotationless {
                self.render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .prim_ex_rotationless_pipeline
                    .clone()
            } else {
                self.render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .prim_ex_pipeline
                    .clone()
            };

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
                    );
                }
            }

            let cont = if rotationless {
                &mut self
                    .render
                    .get_mut()
                    .sub_render_pass_mut(render_pass_type)
                    .prim_ex_rotationless_pipeline
            } else {
                &mut self
                    .render
                    .get_mut()
                    .sub_render_pass_mut(render_pass_type)
                    .prim_ex_pipeline
            };
            *cont = pipe_container;
        }

        ret
    }

    #[must_use]
    fn create_sprite_multi_graphics_pipeline_impl(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut PipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
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

        let set_layouts = [
            self.device.standard_textured_descriptor_set_layout.layout,
            self.device
                .sprite_multi_uniform_descriptor_set_layout
                .layout,
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

        self.create_graphics_pipeline::<false>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &attribute_descriptors,
            &set_layouts,
            &push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
        )
    }

    #[must_use]
    fn create_sprite_multi_graphics_pipeline(&mut self, vert_name: &str, frag_name: &str) -> bool {
        let mut ret: bool = true;

        let tex_mode = EVulkanBackendTextureModes::Textured;

        for n in 0..RENDER_PASS_TYPE_COUNT {
            let render_pass_type = FromPrimitive::from_usize(n).unwrap();

            let mut pipe_container = self
                .render
                .get()
                .sub_render_pass(render_pass_type)
                .sprite_multi_pipeline
                .clone();

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
                    );
                }
            }

            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .sprite_multi_pipeline = pipe_container;
        }

        ret
    }

    #[must_use]
    fn create_sprite_multi_push_graphics_pipeline_impl(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut PipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
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

        let set_layouts = [self.device.standard_textured_descriptor_set_layout.layout];

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

        self.create_graphics_pipeline::<false>(
            vert_name,
            frag_name,
            pipe_container,
            (std::mem::size_of::<f32>() * (2 + 2) + std::mem::size_of::<u8>() * 4) as u32,
            &attribute_descriptors,
            &set_layouts,
            &push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
        )
    }

    #[must_use]
    fn create_sprite_multi_push_graphics_pipeline(
        &mut self,
        vert_name: &str,
        frag_name: &str,
    ) -> bool {
        let mut ret: bool = true;

        let tex_mode = EVulkanBackendTextureModes::Textured;

        for n in 0..RENDER_PASS_TYPE_COUNT {
            let render_pass_type = FromPrimitive::from_usize(n).unwrap();

            let mut pipe_container = self
                .render
                .get()
                .sub_render_pass(render_pass_type)
                .sprite_multi_push_pipeline
                .clone();

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
                    );
                }
            }

            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .sprite_multi_push_pipeline = pipe_container;
        }

        ret
    }

    #[must_use]
    fn create_quad_graphics_pipeline_impl<const IS_TEXTURED: bool>(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut PipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
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
            set_layouts[0] = self.device.standard_textured_descriptor_set_layout.layout;
            set_layouts[1] = self.device.quad_uniform_descriptor_set_layout.layout;
        } else {
            set_layouts[0] = self.device.quad_uniform_descriptor_set_layout.layout;
        }

        let push_constant_size = std::mem::size_of::<SUniformQuadGPos>();

        let push_constants = [vk::PushConstantRange {
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
            &attribute_descriptors
                .split_at_mut(if IS_TEXTURED { 3 } else { 2 })
                .0,
            &set_layouts.split_at_mut(if IS_TEXTURED { 2 } else { 1 }).0,
            &push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
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

        for n in 0..RENDER_PASS_TYPE_COUNT {
            let render_pass_type = FromPrimitive::from_usize(n).unwrap();

            let mut pipe_container = self
                .render
                .get()
                .sub_render_pass(render_pass_type)
                .quad_pipeline
                .clone();

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
                    );
                }
            }

            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .quad_pipeline = pipe_container;
        }

        ret
    }

    #[must_use]
    fn create_quad_push_graphics_pipeline_impl<const IS_TEXTURED: bool>(
        &mut self,
        vert_name: &str,
        frag_name: &str,
        pipe_container: &mut PipelineContainer,
        tex_mode: EVulkanBackendTextureModes,
        blend_mode: EVulkanBackendBlendModes,
        dynamic_mode: EVulkanBackendClipModes,
        render_pass_type: RenderPassType,
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

        let set_layouts = [self.device.standard_textured_descriptor_set_layout.layout];

        let push_constant_size = std::mem::size_of::<SUniformQuadPushGPos>();

        let push_constants = [vk::PushConstantRange {
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
            &attribute_descriptors
                .split_at_mut(if IS_TEXTURED { 3 } else { 2 })
                .0,
            &set_layouts,
            &push_constants,
            tex_mode,
            blend_mode,
            dynamic_mode,
            render_pass_type,
            StencilOpType::None,
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

        for n in 0..RENDER_PASS_TYPE_COUNT {
            let render_pass_type = FromPrimitive::from_usize(n).unwrap();

            let mut pipe_container = self
                .render
                .get()
                .sub_render_pass(render_pass_type)
                .quad_push_pipeline
                .clone();

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
                    );
                }
            }

            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .quad_push_pipeline = pipe_container;
        }

        ret
    }

    fn create_command_pools(
        device: Arc<LogicalDevice>,
        queue_family_index: u32,
        count: usize,
        default_primary_count: usize,
        default_secondary_count: usize,
    ) -> anyhow::Result<Vec<Rc<CommandPool>>> {
        let mut command_pools = Vec::new();
        for _ in 0..count {
            command_pools.push(CommandPool::new(
                device.clone(),
                queue_family_index,
                default_primary_count,
                default_secondary_count,
            )?);
        }
        Ok(command_pools)
    }

    fn create_command_buffers(&mut self) -> anyhow::Result<()> {
        self.device
            .used_memory_command_buffer
            .resize(self.device.swap_chain_image_count as usize, false);

        self.device.memory_command_buffers = Some(CommandBuffers::new(
            self.command_pool.clone(),
            vk::CommandBufferLevel::PRIMARY,
            self.device.swap_chain_image_count as usize,
        )?);

        Ok(())
    }

    fn destroy_command_buffer(&mut self) {
        self.device.memory_command_buffers = None;
        self.device.used_memory_command_buffer.clear();
    }

    fn create_sync_objects(&mut self) -> anyhow::Result<()> {
        for _ in 0..self.device.swap_chain_image_count {
            self.wait_semaphores.push(Semaphore::new(
                self.ash_vk.vk_device.clone(),
                self.device.is_headless,
            )?)
        }
        for _ in 0..self.device.swap_chain_image_count {
            self.sig_semaphores.push(Semaphore::new(
                self.ash_vk.vk_device.clone(),
                self.device.is_headless,
            )?)
        }

        for _ in 0..self.device.swap_chain_image_count {
            self.memory_sempahores.push(Semaphore::new(
                self.ash_vk.vk_device.clone(),
                self.device.is_headless,
            )?)
        }

        for _ in 0..self.device.swap_chain_image_count {
            self.frame_fences
                .push(Fence::new(self.ash_vk.vk_device.clone())?);
        }
        self.image_fences.resize(
            self.device.swap_chain_image_count as usize,
            Default::default(),
        );

        Ok(())
    }

    fn destroy_sync_objects(&mut self) {
        self.wait_semaphores.clear();
        self.sig_semaphores.clear();

        self.memory_sempahores.clear();

        self.frame_fences.clear();
        self.image_fences.clear();
    }

    /*************
     * SWAP CHAIN
     **************/
    fn cleanup_vulkan_swap_chain(&mut self, force_swap_chain_destruct: bool) {
        for n in 0..RENDER_PASS_TYPE_COUNT {
            let render_pass_type = FromPrimitive::from_usize(n).unwrap();

            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .standard_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .standard_line_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .standard_stencil_only_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .standard_stencil_when_passed_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .standard_stencil_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .standard_3d_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .blur_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .tile_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .tile_border_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .tile_border_line_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .prim_ex_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .prim_ex_rotationless_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .sprite_multi_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .sprite_multi_push_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .quad_pipeline
                .destroy(&self.ash_vk.vk_device.device);
            self.render
                .get_mut()
                .sub_render_pass_mut(render_pass_type)
                .quad_push_pipeline
                .destroy(&self.ash_vk.vk_device.device);
        }

        self.destroy_framebuffers_switching_passes();
        self.destroy_framebuffers();

        self.destroy_render_pass();
        self.destroy_render_pass_switching_passes();

        self.destroy_stencil_attachments_for_pass_transition();

        self.destroy_multi_sampler_image_attachments();

        self.destroy_images_for_switching_passes();

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
            self.device.textures.clear();
            self.device.buffer_objects.clear();
        }

        self.image_last_frame_check.clear();

        self.device.streamed_vertex_buffer.destroy(&mut |_, _| {});
        for i in 0..self.thread_count {
            self.device.streamed_uniform.lock().buffers[i].destroy(&mut |_, _| {});
        }
        self.device.streamed_vertex_buffer = Default::default();
        self.device.streamed_uniform.lock().buffers.clear();

        for i in 0..self.device.swap_chain_image_count {
            self.clear_frame_data(i as usize);
        }

        self.device.mem_allocator.lock().destroy_frame_data();

        if IS_LAST_CLEANUP {
            self.device.mem_allocator.lock().destroy_caches();

            self.destroy_texture_samplers();
            self.destroy_descriptor_pools();

            self.delete_presented_image_data_image();
        }

        self.destroy_sync_objects();
        self.destroy_command_buffer();
    }

    fn cleanup_vulkan_sdl(&mut self) {
        self.destroy_surface();

        let dbg_val = self.dbg.load(std::sync::atomic::Ordering::Relaxed);
        if dbg_val == EDebugGFXModes::Minimum as u8 || dbg_val == EDebugGFXModes::All as u8 {
            self.unregister_debug_callback();
        }
    }

    fn recreate_swap_chain(&mut self) -> bool {
        unsafe { self.ash_vk.vk_device.device.device_wait_idle().unwrap() };

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
            self.device
                .vk_gpu
                .config
                .write()
                .unwrap()
                .multi_sampling_count = self.next_multi_sampling_count;
            self.next_multi_sampling_count = u32::MAX;
        }

        let mut ret = Ok(());
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

        if let Err(ref err) = ret {
            self.logger
                .log(LogLevel::Error)
                .msg("recreating swap chain failed: ")
                .msg_var(&err.to_string());
        }

        ret.is_ok()
    }

    fn init_vulkan_sdl(
        window: &BackendWindow,
        _canvas_width: f64,
        _canvas_height: f64,
        dbg_mode: EDebugGFXModes,
        dbg: Arc<AtomicU8>,
        error: &Arc<Mutex<Error>>,
        logger: &SystemLogGroup,
        sys: &System,
        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,
        thread_count: usize,
        options: &Options,
    ) -> anyhow::Result<(
        Arc<Instance>,
        Arc<LogicalDevice>,
        Arc<PhyDevice>,
        Arc<spin::Mutex<Queue>>,
        BackendSurface,
        Device,
        vk::DebugUtilsMessengerEXT,
        Vec<Rc<CommandPool>>,
    )> {
        let benchmark = Benchmark::new(options.dbg.bench);
        let instance = Instance::new(window, dbg_mode, error)?;
        benchmark.bench("\t\tcreating vk instance");

        let mut dbg_callback = vk::DebugUtilsMessengerEXT::null();
        if dbg_mode == EDebugGFXModes::Minimum || dbg_mode == EDebugGFXModes::All {
            let dbg_res =
                Self::setup_debug_callback(&instance.vk_entry, &instance.vk_instance, logger);
            if let Ok(dbg) = dbg_res {
                dbg_callback = dbg;
            }

            for vk_layer in &instance.layers {
                logger
                    .log(LogLevel::Info)
                    .msg("Validation layer: ")
                    .msg(vk_layer.as_str());
            }
        }

        let physical_gpu = PhyDevice::new(instance.clone(), options, logger, window.is_headless())?;
        benchmark.bench("\t\tselecting vk physical device");

        let device = LogicalDevice::new(
            physical_gpu.clone(),
            physical_gpu.queue_node_index,
            &instance.vk_instance,
            &instance.layers,
            window.is_headless(),
            dbg.clone(),
            texture_memory_usage.clone(),
            buffer_memory_usage.clone(),
            stream_memory_usage.clone(),
            staging_memory_usage.clone(),
        )?;
        benchmark.bench("\t\tcreating vk logical device");

        let (graphics_queue, presentation_queue) =
            Self::get_device_queue(&device.device, physical_gpu.queue_node_index)?;

        let queue = Queue::new(graphics_queue, presentation_queue);

        let mut surface = window.create_surface(&instance.vk_entry, &instance.vk_instance)?;

        let mut device_instance = Device::new(
            dbg,
            instance.clone(),
            device.clone(),
            error.clone(),
            physical_gpu.clone(),
            queue.clone(),
            texture_memory_usage,
            buffer_memory_usage,
            stream_memory_usage,
            staging_memory_usage,
            &sys.log,
            window.is_headless(),
            options,
            thread_count,
        )?;

        Self::create_surface(
            &instance.vk_entry,
            window,
            &mut surface,
            &instance.vk_instance,
            &physical_gpu.cur_device,
            physical_gpu.queue_node_index,
            &device_instance,
        )?;
        benchmark.bench("\t\tcreating vk surface");

        let command_pools =
            Self::create_command_pools(device.clone(), physical_gpu.queue_node_index, 1, 5, 0)?;

        benchmark.bench("\t\tcreating vk command buffers");

        device_instance.standard_texture_descr_pool = Self::create_descriptor_pools(&device)?;

        let (repeat, clamp_to_edge, texture_2d_array) = Self::create_texture_samplers(
            &device,
            &physical_gpu.limits,
            options.gl.global_texture_lod_bias,
        )?;
        device_instance.samplers[ESupportedSamplerTypes::Repeat as usize] = repeat;
        device_instance.samplers[ESupportedSamplerTypes::ClampToEdge as usize] = clamp_to_edge;
        device_instance.samplers[ESupportedSamplerTypes::Texture2DArray as usize] =
            texture_2d_array;

        benchmark.bench("\t\tcreating vk descriptor layouts & pools");

        Ok((
            instance,
            device,
            physical_gpu,
            queue,
            surface,
            device_instance,
            dbg_callback,
            command_pools,
        ))
    }

    #[must_use]
    fn has_multi_sampling(&mut self) -> bool {
        Device::get_sample_count(
            self.device
                .vk_gpu
                .config
                .read()
                .unwrap()
                .multi_sampling_count,
            &self.device.vk_gpu.limits,
        ) != vk::SampleCountFlags::TYPE_1
    }

    fn init_vulkan_swap_chain(
        &mut self,
        old_swap_chain: &mut vk::SwapchainKHR,
    ) -> anyhow::Result<()> {
        *old_swap_chain = vk::SwapchainKHR::null();

        self.create_swap_chain(old_swap_chain)?;

        if !self.get_swap_chain_image_handles() {
            return Err(anyhow!("Get swapchain image handles failed."));
        }

        if !self.create_image_views() {
            return Err(anyhow!("create image failed."));
        }

        if !self.create_multi_sampler_image_attachments() {
            return Err(anyhow!("Create multi sampling image attachments failed."));
        }

        if !self.create_images_for_switching_passes() {
            return Err(anyhow!("Create images for switching pass."));
        }

        if !self.create_stencil_attachments_for_pass_transition() {
            return Err(anyhow!("Create stencil attachments for pass transition."));
        }

        self.last_presented_swap_chain_image_index = u32::MAX;

        if !self.create_render_pass() {
            return Err(anyhow!("Create render pass."));
        }

        if !self.create_render_pass_switchting() {
            return Err(anyhow!("Create render switching pass."));
        }

        if !self.create_framebuffers() {
            return Err(anyhow!("Create framebuffers."));
        }

        if !self.create_framebuffers_switching_passes() {
            return Err(anyhow!("Create framebuffers switching passes."));
        }

        if !self.create_standard_graphics_pipeline(
            "shader/vulkan/prim.vert.spv",
            "shader/vulkan/prim.frag.spv",
            false,
            false,
            StencilOpType::None,
        ) {
            return Err(anyhow!("Create standard graphics pipeline."));
        }

        if !self.create_standard_graphics_pipeline(
            "shader/vulkan/prim_textured.vert.spv",
            "shader/vulkan/prim_textured.frag.spv",
            true,
            false,
            StencilOpType::None,
        ) {
            return Err(anyhow!("Create standard graphics pipeline (sampler)."));
        }

        if !self.create_standard_graphics_pipeline(
            "shader/vulkan/prim.vert.spv",
            "shader/vulkan/prim.frag.spv",
            false,
            true,
            StencilOpType::None,
        ) {
            return Err(anyhow!("Create standard line graphics pipeline."));
        }

        // stencil only pipeline, does not write to any color attachments
        if !self.create_standard_graphics_pipeline(
            "shader/vulkan/prim.vert.spv",
            "shader/vulkan/prim.frag.spv",
            false,
            false,
            StencilOpType::AlwaysPass,
        ) {
            return Err(anyhow!(
                "Create standard stencil always pass graphics pipeline."
            ));
        }

        if !self.create_standard_graphics_pipeline(
            "shader/vulkan/full.vert.spv",
            "shader/vulkan/prim_textured.frag.spv",
            true,
            false,
            StencilOpType::OnlyWhenPassed,
        ) {
            return Err(anyhow!(
                "Create standard stencil only when passed graphics pipeline."
            ));
        }

        if !self.create_standard_graphics_pipeline(
            "shader/vulkan/full.vert.spv",
            "shader/vulkan/prim_no_alpha.frag.spv",
            true,
            false,
            StencilOpType::OnlyWhenNotPassed,
        ) {
            return Err(anyhow!(
                "Create standard only when not passed graphics pipeline."
            ));
        }

        if !self.create_blur_graphics_pipeline(
            "shader/vulkan/full.vert.spv",
            "shader/vulkan/blur.frag.spv",
        ) {
            return Err(anyhow!("Create blur graphics pipeline."));
        }

        if !self.create_standard_3d_graphics_pipeline(
            "shader/vulkan/prim3d.vert.spv",
            "shader/vulkan/prim3d.frag.spv",
            false,
        ) {
            return Err(anyhow!("Create standard 3d graphics pipeline."));
        }

        if !self.create_standard_3d_graphics_pipeline(
            "shader/vulkan/prim3d_textured.vert.spv",
            "shader/vulkan/prim3d_textured.frag.spv",
            true,
        ) {
            return Err(anyhow!("Create standard 3d textured graphics pipeline."));
        }

        if !self.create_tile_graphics_pipeline::<false>(
            "shader/vulkan/tile.vert.spv",
            "shader/vulkan/tile.frag.spv",
            0,
        ) {
            return Err(anyhow!("Create tile graphics pipeline."));
        }

        if !self.create_tile_graphics_pipeline::<true>(
            "shader/vulkan/tile_textured.vert.spv",
            "shader/vulkan/tile_textured.frag.spv",
            0,
        ) {
            return Err(anyhow!("Create tile textured graphics pipeline."));
        }

        if !self.create_tile_graphics_pipeline::<false>(
            "shader/vulkan/tile_border.vert.spv",
            "shader/vulkan/tile_border.frag.spv",
            1,
        ) {
            return Err(anyhow!("Create tile border graphics pipeline."));
        }

        if !self.create_tile_graphics_pipeline::<true>(
            "shader/vulkan/tile_border_textured.vert.spv",
            "shader/vulkan/tile_border_textured.frag.spv",
            1,
        ) {
            return Err(anyhow!("Create tile border textured graphics pipeline."));
        }

        if !self.create_tile_graphics_pipeline::<false>(
            "shader/vulkan/tile_border_line.vert.spv",
            "shader/vulkan/tile_border_line.frag.spv",
            2,
        ) {
            return Err(anyhow!("Create tile border line graphics pipeline."));
        }

        if !self.create_tile_graphics_pipeline::<true>(
            "shader/vulkan/tile_border_line_textured.vert.spv",
            "shader/vulkan/tile_border_line_textured.frag.spv",
            2,
        ) {
            return Err(anyhow!(
                "Create tile border line textured graphics pipeline."
            ));
        }

        if !self.create_prim_ex_graphics_pipeline(
            "shader/vulkan/primex_rotationless.vert.spv",
            "shader/vulkan/primex_rotationless.frag.spv",
            false,
            true,
        ) {
            return Err(anyhow!("Create prim ex graphics pipeline."));
        }

        if !self.create_prim_ex_graphics_pipeline(
            "shader/vulkan/primex_tex_rotationless.vert.spv",
            "shader/vulkan/primex_tex_rotationless.frag.spv",
            true,
            true,
        ) {
            return Err(anyhow!("Create prim ex textured graphics pipeline."));
        }

        if !self.create_prim_ex_graphics_pipeline(
            "shader/vulkan/primex.vert.spv",
            "shader/vulkan/primex.frag.spv",
            false,
            false,
        ) {
            return Err(anyhow!("Create prim ex rotationless graphics pipeline."));
        }

        if !self.create_prim_ex_graphics_pipeline(
            "shader/vulkan/primex_tex.vert.spv",
            "shader/vulkan/primex_tex.frag.spv",
            true,
            false,
        ) {
            return Err(anyhow!(
                "Create prim ex rotationless textured graphics pipeline."
            ));
        }

        if !self.create_sprite_multi_graphics_pipeline(
            "shader/vulkan/spritemulti.vert.spv",
            "shader/vulkan/spritemulti.frag.spv",
        ) {
            return Err(anyhow!("Create sprite multi graphics pipeline."));
        }

        if !self.create_sprite_multi_push_graphics_pipeline(
            "shader/vulkan/spritemulti_push.vert.spv",
            "shader/vulkan/spritemulti_push.frag.spv",
        ) {
            return Err(anyhow!("Create sprite multi textured graphics pipeline."));
        }

        if !self.create_quad_graphics_pipeline::<false>(
            "shader/vulkan/quad.vert.spv",
            "shader/vulkan/quad.frag.spv",
        ) {
            return Err(anyhow!("Create quad graphics pipeline."));
        }

        if !self.create_quad_graphics_pipeline::<true>(
            "shader/vulkan/quad_textured.vert.spv",
            "shader/vulkan/quad_textured.frag.spv",
        ) {
            return Err(anyhow!("Create quad textured graphics pipeline."));
        }

        if !self.create_quad_push_graphics_pipeline::<false>(
            "shader/vulkan/quad_push.vert.spv",
            "shader/vulkan/quad_push.frag.spv",
        ) {
            return Err(anyhow!("Create quad pushed graphics pipeline."));
        }

        if !self.create_quad_push_graphics_pipeline::<true>(
            "shader/vulkan/quad_push_textured.vert.spv",
            "shader/vulkan/quad_push_textured.frag.spv",
        ) {
            return Err(anyhow!("Create quad pushed textured graphics pipeline."));
        }

        self.swap_chain_created = true;
        Ok(())
    }

    fn init_vulkan_with_io<const IS_FIRST_INITIALIZATION: bool>(&mut self) -> i32 {
        if IS_FIRST_INITIALIZATION {
            let mut old_swap_chain = vk::SwapchainKHR::null();
            if self.init_vulkan_swap_chain(&mut old_swap_chain).is_err() {
                return -1;
            }
        }

        if let Err(_) = self.create_command_buffers() {
            self.error
                .lock()
                .unwrap()
                .set_error(EGFXErrorType::Init, "Creating the command buffers failed.");
            return -1;
        }

        if let Err(err) = self.create_sync_objects() {
            self.error
                .lock()
                .unwrap()
                .set_error(EGFXErrorType::Init, &err.to_string());
            return -1;
        }

        self.device.streamed_vertex_buffer = Default::default();
        self.device
            .streamed_vertex_buffer
            .init(self.device.swap_chain_image_count as usize);
        self.device
            .streamed_uniform
            .lock()
            .buffers
            .resize(self.thread_count, Default::default());
        for i in 0..self.thread_count {
            self.device.streamed_uniform.lock().buffers[i]
                .init(self.device.swap_chain_image_count as usize);
        }

        self.device.mem_allocator.lock().init_caches();

        self.image_last_frame_check
            .resize(self.device.swap_chain_image_count as usize, 0);

        if IS_FIRST_INITIALIZATION {
            // check if image format supports linear blitting
            let mut format_properties = unsafe {
                self.ash_vk
                    .instance
                    .vk_instance
                    .get_physical_device_format_properties(
                        self.vk_gpu.cur_device,
                        vk::Format::R8G8B8A8_UNORM,
                    )
            };
            if !(format_properties.optimal_tiling_features
                & vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR)
                .is_empty()
            {
                self.ash_vk
                    .vk_device
                    .phy_device
                    .config
                    .write()
                    .unwrap()
                    .allows_linear_blitting = true;
            }
            if !(format_properties.optimal_tiling_features & vk::FormatFeatureFlags::BLIT_SRC)
                .is_empty()
                && !(format_properties.optimal_tiling_features & vk::FormatFeatureFlags::BLIT_DST)
                    .is_empty()
            {
                self.ash_vk
                    .vk_device
                    .phy_device
                    .config
                    .write()
                    .unwrap()
                    .optimal_rgba_image_blitting = true;
            }
            // check if image format supports blitting to linear tiled images
            if !(format_properties.linear_tiling_features & vk::FormatFeatureFlags::BLIT_DST)
                .is_empty()
            {
                self.ash_vk
                    .vk_device
                    .phy_device
                    .config
                    .write()
                    .unwrap()
                    .linear_rgba_image_blitting = true;
            }

            format_properties = unsafe {
                self.ash_vk
                    .instance
                    .vk_instance
                    .get_physical_device_format_properties(
                        self.vk_gpu.cur_device,
                        self.render.get().vk_surf_format.format,
                    )
            };
            if !(format_properties.optimal_tiling_features & vk::FormatFeatureFlags::BLIT_SRC)
                .is_empty()
            {
                self.ash_vk
                    .vk_device
                    .phy_device
                    .config
                    .write()
                    .unwrap()
                    .optimal_swap_chain_image_blitting = true;
            }
        }

        0
    }

    fn init_vulkan<const IS_FIRST_INITIALIZATION: bool>(&mut self) -> i32 {
        let res = self.init_vulkan_with_io::<{ IS_FIRST_INITIALIZATION }>();
        if res != 0 {
            return res;
        }

        0
    }

    /************************
     * COMMAND IMPLEMENTATION
     ************************/
    fn cmd_texture_update(&mut self, cmd: &CommandTextureUpdate) -> anyhow::Result<()> {
        let index_tex = cmd.texture_index;

        self.update_texture(
            index_tex,
            vk::Format::R8G8B8A8_UNORM,
            &cmd.data,
            cmd.x as i64,
            cmd.y as i64,
            cmd.width as usize,
            cmd.height as usize,
            tex_format_to_image_color_channel_count(cmd.format),
        )
    }

    fn cmd_texture_destroy(&mut self, cmd: &CommandTextureDestroy) -> anyhow::Result<()> {
        let image_index = cmd.texture_index;
        self.device
            .textures
            .remove(&image_index)
            .ok_or(anyhow!("texture not found in vk backend"))?;

        Ok(())
    }

    fn cmd_texture_create(&mut self, cmd: CommandTextureCreate) -> anyhow::Result<()> {
        let texture_index = cmd.texture_index;
        let width = cmd.width;
        let height = cmd.height;
        let depth = cmd.depth;
        let pixel_size = cmd.pixel_size;
        let format = cmd.format;
        let store_format = cmd.store_format;
        let flags = cmd.flags;
        let is_3d_tex = cmd.is_3d_tex;

        let data_mem = cmd.data;
        let usage = GraphicsMemoryAllocationType::Texture {
            width,
            height,
            depth,
            is_3d_tex,
            flags,
        };
        let mut data_mem = self
            .device
            .mem_allocator
            .lock()
            .memory_to_internal_memory(data_mem, usage);
        if let Err((mem, _)) = data_mem {
            self.skip_frames_until_current_frame_is_used_again()?;
            data_mem = self
                .device
                .mem_allocator
                .lock()
                .memory_to_internal_memory(mem, usage);
        }
        let data_mem = data_mem.map_err(|(_, err)| err)?;

        self.create_texture_cmd(
            texture_index,
            pixel_size,
            texture_format_to_vulkan_format(format),
            texture_format_to_vulkan_format(store_format),
            data_mem,
        )?;

        Ok(())
    }

    fn cmd_clear_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut RenderCommandExecuteBuffer,
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

    fn cmd_clear(
        device: &LogicalDevice,
        exec_buffer: &RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
        cmd: &CommandClear,
    ) -> anyhow::Result<()> {
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
                    extent: exec_buffer.viewport_size,
                },
                base_array_layer: 0,
                layer_count: 1,
            }];

            unsafe {
                device.device.cmd_clear_attachments(
                    command_buffer.command_buffer,
                    &clear_attachments,
                    &clear_rects,
                );
            }
        }

        Ok(())
    }

    fn cmd_render_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut RenderCommandExecuteBuffer,
        cmd: &CommandRender,
    ) {
        let is_textured: bool = Self::get_is_textured(&cmd.state);
        if is_textured {
            let address_mode_index: usize = Self::get_address_mode_index(&cmd.state);
            exec_buffer.descriptors[0] = Some(
                self.device
                    .textures
                    .get(&cmd.state.texture_index.unwrap())
                    .unwrap()
                    .data
                    .unwrap_2d_descr(address_mode_index)
                    .clone(),
            );
        }

        exec_buffer.index_buffer = self.index_buffer.as_ref().unwrap().buffer;

        exec_buffer.estimated_render_call_count = 1;

        self.exec_buffer_fill_dynamic_states(&cmd.state, exec_buffer);

        let cur_stream_buffer = self
            .device
            .streamed_vertex_buffer
            .get_current_buffer(self.cur_image_index as usize);
        exec_buffer.buffer = cur_stream_buffer.buffer.buffer;
        exec_buffer.buffer_off = cur_stream_buffer.offset_in_buffer
            + self.cur_stream_vertex_byte_offset
            + cmd.vertices_offset * std::mem::size_of::<GlVertex>();
    }

    #[must_use]
    fn cmd_render(
        device: &LogicalDevice,
        render: &RenderSetupGroup,
        render_pass_type: RenderPassType,
        cmd: &CommandRender,
        exec_buffer: &RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
    ) -> bool {
        Self::render_standard::<GlVertex, false>(
            device,
            render,
            render_pass_type,
            exec_buffer,
            command_buffer,
            &cmd.state,
            cmd.prim_type,
            cmd.prim_count,
            StencilOpType::None,
            true,
        )
    }

    fn cmd_render_blurred_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut RenderCommandExecuteBuffer,
        cmd: &CommandRender,
    ) {
        self.cmd_render_fill_execute_buffer(exec_buffer, cmd);
    }

    fn cmd_render_for_stencil_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut RenderCommandExecuteBuffer,
        cmd: &CommandRender,
    ) {
        self.cmd_render_fill_execute_buffer(exec_buffer, cmd);
    }

    fn cmd_render_where_stencil_did_not_pass_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut RenderCommandExecuteBuffer,
        cmd: &CommandRender,
    ) {
        self.cmd_render_fill_execute_buffer(exec_buffer, cmd);
    }

    fn clear_stencil(
        device: &LogicalDevice,
        exec_buffer: &RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
    ) {
        let clear_attachments = [vk::ClearAttachment {
            aspect_mask: vk::ImageAspectFlags::STENCIL,
            color_attachment: 1, // TODO: this is not 1 if multi sampling is used
            clear_value: vk::ClearValue {
                color: vk::ClearColorValue {
                    int32: [0, 0, 0, 0],
                },
            },
        }];
        let clear_rects = [vk::ClearRect {
            rect: vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: exec_buffer.viewport_size,
            },
            base_array_layer: 0,
            layer_count: 1,
        }];

        unsafe {
            device.device.cmd_clear_attachments(
                command_buffer.command_buffer,
                &clear_attachments,
                &clear_rects,
            );
        }
    }

    #[must_use]
    fn cmd_render_for_stencil(
        device: &LogicalDevice,
        render: &RenderSetupGroup,
        render_pass_type: RenderPassType,
        cmd: &CommandRender,
        exec_buffer: &RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
    ) -> bool {
        Self::clear_stencil(device, exec_buffer, command_buffer);

        let mut state_real = cmd.state.clone();
        state_real.clear_texture();
        // draw the vertices and fill stencil buffer
        Self::render_standard::<GlVertex, false>(
            device,
            render,
            render_pass_type,
            exec_buffer,
            command_buffer,
            &state_real,
            cmd.prim_type,
            cmd.prim_count,
            StencilOpType::AlwaysPass,
            true,
        )
    }

    #[must_use]
    fn cmd_render_blurred(
        device: &LogicalDevice,
        render: &RenderSetupGroup,
        render_pass_type: RenderPassType,
        frame_index: u32,
        cmd: &CommandRender,
        mut exec_buffer: RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
        blur_radius: f32,
        blur_horizontal: bool,
        blur_color: vec4,
    ) -> bool {
        let img = if let RenderPassType::Switching1 = render_pass_type {
            &render.get().switching.passes[1].surface.image_list[frame_index as usize]
        } else {
            &render.get().switching.passes[0].surface.image_list[frame_index as usize]
        };

        let mut state_real = cmd.state.clone();
        struct FakeTexture {}
        impl SharedIndexGetIndexUnsafe for FakeTexture {
            fn get_index_unsafe(&self) -> u128 {
                0
            }
        }
        state_real.set_texture(&FakeTexture {});
        state_real.wrap_clamp();
        exec_buffer.descriptors = [
            Some(img.texture_descr_sets[WrapType::Clamp as usize].clone()),
            None,
        ];
        // draw where the stencil buffer triggered
        if !Self::render_blur::<GlVertex>(
            device,
            render,
            render_pass_type,
            &exec_buffer,
            command_buffer,
            &state_real,
            PrimType::Triangles,
            1,
            blur_radius,
            blur_horizontal,
            blur_color,
        ) {
            return false;
        }

        true
    }

    #[must_use]
    fn cmd_render_where_stencil_did_not_pass(
        device: &LogicalDevice,
        render: &RenderSetupGroup,
        render_pass_type: RenderPassType,
        frame_index: u32,
        cmd: &CommandRender,
        mut exec_buffer: RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
        clear_stencil: bool,
    ) -> bool {
        if clear_stencil {
            Self::clear_stencil(device, &exec_buffer, command_buffer);
        }

        let img = if let RenderPassType::Switching1 = render_pass_type {
            &render.get().switching.passes[1].surface.image_list[frame_index as usize]
        } else {
            &render.get().switching.passes[0].surface.image_list[frame_index as usize]
        };

        let mut state_real = cmd.state.clone();
        struct FakeTexture {}
        impl SharedIndexGetIndexUnsafe for FakeTexture {
            fn get_index_unsafe(&self) -> u128 {
                0
            }
        }
        state_real.set_texture(&FakeTexture {});
        state_real.wrap_clamp();
        exec_buffer.descriptors = [
            Some(img.texture_descr_sets[WrapType::Clamp as usize].clone()),
            None,
        ];
        // then draw the rest of the first pass
        // where the stencil buffer didn't trigger
        if !Self::render_standard::<GlVertex, false>(
            device,
            render,
            render_pass_type,
            &exec_buffer,
            command_buffer,
            &state_real,
            PrimType::Triangles,
            1,
            StencilOpType::OnlyWhenNotPassed,
            false,
        ) {
            return false;
        }

        true
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

    fn cmd_update_viewport(&mut self, cmd: &CommandUpdateViewport) -> anyhow::Result<()> {
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

        Ok(())
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
    */

    fn cmd_swap(&mut self) -> anyhow::Result<()> {
        self.next_frame()
    }

    fn cmd_create_buffer_object(&mut self, cmd: CommandCreateBufferObject) -> anyhow::Result<()> {
        let upload_data_size = cmd.upload_data.len();

        let data_mem = cmd.upload_data;
        let usage = GraphicsMemoryAllocationType::Buffer {
            required_size: upload_data_size,
        };
        let mut data_mem = self
            .device
            .mem_allocator
            .lock()
            .memory_to_internal_memory(data_mem, usage);
        if let Err((mem, _)) = data_mem {
            data_mem = self
                .device
                .mem_allocator
                .lock()
                .memory_to_internal_memory(mem, usage);
        }
        let data_mem = data_mem.map_err(|(_, err)| err)?;

        Ok(self.device.create_buffer_object(
            cmd.buffer_index,
            data_mem,
            upload_data_size as vk::DeviceSize,
            self.cur_image_index,
        )?)
    }

    fn cmd_recreate_buffer_object(
        &mut self,
        cmd: CommandRecreateBufferObject,
    ) -> anyhow::Result<()> {
        self.device.delete_buffer_object(cmd.buffer_index);

        let upload_data_size = cmd.upload_data.len();

        let data_mem = cmd.upload_data;
        let usage = GraphicsMemoryAllocationType::Buffer {
            required_size: upload_data_size,
        };
        let mut data_mem = self
            .device
            .mem_allocator
            .lock()
            .memory_to_internal_memory(data_mem, usage);
        if let Err((mem, _)) = data_mem {
            data_mem = self
                .device
                .mem_allocator
                .lock()
                .memory_to_internal_memory(mem, usage);
        }
        let data_mem = data_mem.map_err(|(_, err)| err)?;

        Ok(self.device.create_buffer_object(
            cmd.buffer_index,
            data_mem,
            upload_data_size as vk::DeviceSize,
            self.cur_image_index,
        )?)
    }

    fn cmd_delete_buffer_object(&mut self, cmd: &CommandDeleteBufferObject) -> anyhow::Result<()> {
        let buffer_index = cmd.buffer_index;
        self.device.delete_buffer_object(buffer_index);

        Ok(())
    }

    fn cmd_indices_required_num_notify(
        &mut self,
        cmd: &CommandIndicesRequiredNumNotify,
    ) -> anyhow::Result<()> {
        let indices_count: usize = cmd.required_indices_num;
        if self.cur_render_index_primitive_count < indices_count / 6 {
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
            (self.render_index_buffer, self.render_index_buffer_memory) = self
                .device
                .create_index_buffer(
                    upload_indices.as_ptr() as *const c_void,
                    upload_indices.len() * std::mem::size_of::<u32>(),
                    self.cur_image_index,
                )
                .map(|(i, m)| (Some(i), Some(m)))?;
            self.cur_render_index_primitive_count = indices_count / 6;
        }

        Ok(())
    }

    fn cmd_render_tile_layer_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut RenderCommandExecuteBuffer,
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
        device: &LogicalDevice,
        render: &RenderSetupGroup,
        render_pass_type: RenderPassType,
        cmd: &CommandRenderTileLayer,
        exec_buffer: &RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
    ) -> bool {
        let layer_type: i32 = 0;
        let dir = vec2::default();
        let off = vec2::default();
        let jump_index: i32 = 0;
        Self::render_tile_layer(
            device,
            render,
            render_pass_type,
            exec_buffer,
            command_buffer,
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
        )
    }

    fn cmd_render_border_tile_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut RenderCommandExecuteBuffer,
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
        device: &LogicalDevice,
        render: &RenderSetupGroup,
        render_pass_type: RenderPassType,
        cmd: &CommandRenderBorderTile,
        exec_buffer: &RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
    ) -> bool {
        let layer_type: i32 = 1; // TODO: use type
        let dir = cmd.dir;
        let off = cmd.offset;
        let draw_num = 6;
        Self::render_tile_layer(
            device,
            render,
            render_pass_type,
            exec_buffer,
            command_buffer,
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
        )
    }

    fn cmd_render_border_tile_line_fill_execute_buffer(
        &mut self,
        exec_buffer: &mut RenderCommandExecuteBuffer,
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
        device: &LogicalDevice,
        render: &RenderSetupGroup,
        render_pass_type: RenderPassType,
        cmd: &CommandRenderBorderTileLine,
        exec_buffer: &RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
    ) -> bool {
        let layer_type: i32 = 2; // TODO: use type
        let dir = cmd.dir;
        let off = cmd.offset;
        Self::render_tile_layer(
            device,
            render,
            render_pass_type,
            exec_buffer,
            command_buffer,
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
        )
    }

    fn cmd_render_quad_layer_fill_execute_buffer(
        &self,
        exec_buffer: &mut RenderCommandExecuteBuffer,
        cmd: &CommandRenderQuadLayer,
    ) {
        let buffer_object = self
            .device
            .buffer_objects
            .get(&cmd.buffer_object_index)
            .unwrap();

        exec_buffer.buffer = buffer_object.cur_buffer.buffer;
        exec_buffer.buffer_off = buffer_object.cur_buffer_offset;

        let is_textured: bool = Self::get_is_textured(&cmd.state);
        if is_textured {
            let address_mode_index: usize = Self::get_address_mode_index(&cmd.state);
            exec_buffer.descriptors[0] = Some(
                self.device
                    .textures
                    .get(&cmd.state.texture_index.unwrap())
                    .unwrap()
                    .data
                    .unwrap_2d_descr(address_mode_index)
                    .clone(),
            );
        }

        exec_buffer.index_buffer = self.render_index_buffer.as_ref().unwrap().buffer;

        exec_buffer.estimated_render_call_count =
            ((cmd.quad_num - 1) / GRAPHICS_MAX_QUADS_RENDER_COUNT) + 1;

        self.exec_buffer_fill_dynamic_states(&cmd.state, exec_buffer);
    }

    #[must_use]
    fn cmd_render_quad_layer(
        device: &LogicalDevice,
        render: &RenderSetupGroup,
        render_pass_type: RenderPassType,
        thread_index: usize,
        streamed_uniform: &Arc<spin::Mutex<StreamedUniform>>,
        frame_index: u32,
        cmd: &CommandRenderQuadLayer,
        exec_buffer: &RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::get_state_matrix(&cmd.state, &mut m);

        let can_be_pushed: bool = cmd.quad_num == 1;

        let mut is_textured: bool = Default::default();
        let mut blend_mode_index: usize = Default::default();
        let mut dynamic_index: usize = Default::default();
        let mut address_mode_index: usize = Default::default();
        Self::get_state_indices(
            exec_buffer,
            &cmd.state,
            &mut is_textured,
            &mut blend_mode_index,
            &mut dynamic_index,
            &mut address_mode_index,
        );

        let (pipeline, pipe_layout) = Self::get_pipeline_and_layout(
            if can_be_pushed {
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .quad_push_pipeline
            } else {
                &render.get().sub_render_pass(render_pass_type).quad_pipeline
            },
            is_textured,
            blend_mode_index,
            dynamic_index,
        );
        let (pipeline, pipe_layout) = (*pipeline, *pipe_layout);

        Self::bind_pipeline(
            &device.device,
            command_buffer.command_buffer,
            exec_buffer,
            pipeline,
            &cmd.state,
        );

        let vertex_buffers = [exec_buffer.buffer];
        let buffer_offsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            device.device.cmd_bind_vertex_buffers(
                command_buffer.command_buffer,
                0,
                &vertex_buffers,
                &buffer_offsets,
            );
        }

        unsafe {
            device.device.cmd_bind_index_buffer(
                command_buffer.command_buffer,
                exec_buffer.index_buffer,
                0,
                vk::IndexType::UINT32,
            );
        }

        if is_textured {
            unsafe {
                device.device.cmd_bind_descriptor_sets(
                    command_buffer.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipe_layout,
                    0,
                    &[exec_buffer.descriptors[0].as_ref().unwrap().set()],
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
                device.device.cmd_push_constants(
                    command_buffer.command_buffer,
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
                device.device.cmd_push_constants(
                    command_buffer.command_buffer,
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
                let res = Self::get_uniform_buffer_object(
                    streamed_uniform,
                    thread_index,
                    true,
                    real_draw_count,
                    &cmd.quad_info[render_offset] as *const SQuadRenderInfo as *const c_void,
                    real_draw_count * std::mem::size_of::<SQuadRenderInfo>(),
                    frame_index,
                );
                if res.is_err() {
                    return false;
                }
                let uni_descr_set = res.unwrap();

                unsafe {
                    device.device.cmd_bind_descriptor_sets(
                        command_buffer.command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipe_layout,
                        if is_textured { 1 } else { 0 },
                        &[uni_descr_set.set()],
                        &[],
                    );
                }
                if render_offset > 0 {
                    let quad_offset: i32 = (cmd.quad_offset + render_offset) as i32;
                    unsafe {
                        device.device.cmd_push_constants(
                            command_buffer.command_buffer,
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
                device.device.cmd_draw_indexed(
                    command_buffer.command_buffer,
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

        true
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
        exec_buffer: &mut RenderCommandExecuteBuffer,
        state: &State,
        buffer_object_index: u128,
        draw_calls: usize,
    ) {
        let buffer_object = self
            .device
            .buffer_objects
            .get(&buffer_object_index)
            .unwrap();

        exec_buffer.buffer = buffer_object.cur_buffer.buffer;
        exec_buffer.buffer_off = buffer_object.cur_buffer_offset;

        let is_textured: bool = Self::get_is_textured(state);
        if is_textured {
            let address_mode_index: usize = Self::get_address_mode_index(state);
            exec_buffer.descriptors[0] = Some(
                self.device
                    .textures
                    .get(&state.texture_index.unwrap())
                    .unwrap()
                    .data
                    .unwrap_2d_descr(address_mode_index)
                    .clone(),
            );
        }

        exec_buffer.index_buffer = self.render_index_buffer.as_ref().unwrap().buffer;

        exec_buffer.estimated_render_call_count = draw_calls;

        self.exec_buffer_fill_dynamic_states(&state, exec_buffer);
    }

    fn cmd_render_quad_container_ex_fill_execute_buffer(
        &self,
        exec_buffer: &mut RenderCommandExecuteBuffer,
        cmd: &CommandRenderQuadContainer,
    ) {
        self.buffer_object_fill_execute_buffer(exec_buffer, &cmd.state, cmd.buffer_object_index, 1);
    }

    #[must_use]
    fn cmd_render_quad_container_ex(
        device: &LogicalDevice,
        render: &RenderSetupGroup,
        render_pass_type: RenderPassType,
        cmd: &CommandRenderQuadContainer,
        exec_buffer: &RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::get_state_matrix(&cmd.state, &mut m);

        let is_rotationless: bool = !(cmd.rotation != 0.0);
        let mut is_textured: bool = Default::default();
        let mut blend_mode_index: usize = Default::default();
        let mut dynamic_index: usize = Default::default();
        let mut address_mode_index: usize = Default::default();
        Self::get_state_indices(
            exec_buffer,
            &cmd.state,
            &mut is_textured,
            &mut blend_mode_index,
            &mut dynamic_index,
            &mut address_mode_index,
        );

        let (pipeline, pipe_layout) = Self::get_pipeline_and_layout(
            if is_rotationless {
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .prim_ex_rotationless_pipeline
            } else {
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .prim_ex_pipeline
            },
            is_textured,
            blend_mode_index,
            dynamic_index,
        );
        let (pipeline, pipe_layout) = (*pipeline, *pipe_layout);

        Self::bind_pipeline(
            &device.device,
            command_buffer.command_buffer,
            exec_buffer,
            pipeline,
            &cmd.state,
        );

        let vertex_buffers = [exec_buffer.buffer];
        let buffer_offsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            device.device.cmd_bind_vertex_buffers(
                command_buffer.command_buffer,
                0,
                &vertex_buffers,
                &buffer_offsets,
            );
        }

        let index_offset = cmd.offset as vk::DeviceSize;

        unsafe {
            device.device.cmd_bind_index_buffer(
                command_buffer.command_buffer,
                exec_buffer.index_buffer,
                index_offset,
                vk::IndexType::UINT32,
            );
        }

        if is_textured {
            unsafe {
                device.device.cmd_bind_descriptor_sets(
                    command_buffer.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipe_layout,
                    0,
                    &[exec_buffer.descriptors[0].as_ref().unwrap().set()],
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
            device.device.cmd_push_constants(
                command_buffer.command_buffer,
                pipe_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                std::slice::from_raw_parts(
                    &push_constant_vertex as *const SUniformPrimExGPos as *const u8,
                    vertex_push_constant_size,
                ),
            );
        }
        unsafe {
            device.device.cmd_push_constants(
                command_buffer.command_buffer,
                pipe_layout,
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
            device.device.cmd_draw_indexed(
                command_buffer.command_buffer,
                (cmd.draw_num) as u32,
                1,
                0,
                0,
                0,
            );
        }

        true
    }

    fn cmd_render_quad_container_as_sprite_multiple_fill_execute_buffer(
        &self,
        exec_buffer: &mut RenderCommandExecuteBuffer,
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
        device: &LogicalDevice,
        render: &RenderSetupGroup,
        render_pass_type: RenderPassType,
        thread_index: usize,
        streamed_uniform: &Arc<spin::Mutex<StreamedUniform>>,
        frame_index: u32,
        cmd: &CommandRenderQuadContainerAsSpriteMultiple,
        exec_buffer: &RenderCommandExecuteBuffer,
        command_buffer: &AutoCommandBuffer,
    ) -> bool {
        let mut m: [f32; 4 * 2] = Default::default();
        Self::get_state_matrix(&cmd.state, &mut m);

        let can_be_pushed: bool = cmd.draw_count <= 1;

        let mut is_textured: bool = Default::default();
        let mut blend_mode_index: usize = Default::default();
        let mut dynamic_index: usize = Default::default();
        let mut address_mode_index: usize = Default::default();
        Self::get_state_indices(
            exec_buffer,
            &cmd.state,
            &mut is_textured,
            &mut blend_mode_index,
            &mut dynamic_index,
            &mut address_mode_index,
        );

        let (pipeline, pipe_layout) = Self::get_pipeline_and_layout(
            if can_be_pushed {
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .sprite_multi_push_pipeline
            } else {
                &render
                    .get()
                    .sub_render_pass(render_pass_type)
                    .sprite_multi_pipeline
            },
            is_textured,
            blend_mode_index,
            dynamic_index,
        );
        let (pipeline, pipe_layout) = (*pipeline, *pipe_layout);

        Self::bind_pipeline(
            &device.device,
            command_buffer.command_buffer,
            exec_buffer,
            pipeline,
            &cmd.state,
        );

        let vertex_buffers = [exec_buffer.buffer];
        let buffer_offsets = [exec_buffer.buffer_off as vk::DeviceSize];
        unsafe {
            device.device.cmd_bind_vertex_buffers(
                command_buffer.command_buffer,
                0,
                &vertex_buffers,
                &buffer_offsets,
            );
        }

        let index_offset = cmd.offset as vk::DeviceSize;
        unsafe {
            device.device.cmd_bind_index_buffer(
                command_buffer.command_buffer,
                exec_buffer.index_buffer,
                index_offset,
                vk::IndexType::UINT32,
            );
        }

        unsafe {
            device.device.cmd_bind_descriptor_sets(
                command_buffer.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipe_layout,
                0,
                &[exec_buffer.descriptors[0].as_ref().unwrap().set()],
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
                device.device.cmd_push_constants(
                    command_buffer.command_buffer,
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
                device.device.cmd_push_constants(
                    command_buffer.command_buffer,
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
                device.device.cmd_push_constants(
                    command_buffer.command_buffer,
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
                device.device.cmd_push_constants(
                    command_buffer.command_buffer,
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
                let res = Self::get_uniform_buffer_object(
                    streamed_uniform,
                    thread_index,
                    false,
                    uniform_count,
                    &cmd.render_info[render_offset] as *const SRenderSpriteInfo as *const c_void,
                    uniform_count * std::mem::size_of::<SRenderSpriteInfo>(),
                    frame_index,
                );
                if res.is_err() {
                    return false;
                }
                let uni_descr_set = res.unwrap();

                unsafe {
                    device.device.cmd_bind_descriptor_sets(
                        command_buffer.command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipe_layout,
                        1,
                        &[uni_descr_set.set()],
                        &[],
                    );
                }
            }

            unsafe {
                device.device.cmd_draw_indexed(
                    command_buffer.command_buffer,
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

        true
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
    pub fn init_instance_while_io(
        window: &BackendWindow,
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
    ) -> anyhow::Result<Pin<Box<Self>>> {
        let dbg_mode = options.dbg.gfx; // TODO config / options
        let dbg = Arc::new(AtomicU8::new(dbg_mode as u8));
        let error = Arc::new(Mutex::new(Error::default()));
        let logger = sys.log.logger("vulkan");

        // thread count

        let thread_count = (options.gl.thread_count as usize).clamp(
            1,
            std::thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(1).unwrap())
                .get(),
        );

        let (
            instance,
            device,
            phy_gpu,
            queue,
            ash_surface,
            device_instance,
            dbg_utils_messenger,
            mut command_pools,
        ) = Self::init_vulkan_sdl(
            window,
            canvas_width,
            canvas_height,
            dbg_mode,
            dbg.clone(),
            &error,
            &logger,
            sys,
            texture_memory_usage.clone(),
            buffer_memory_usage.clone(),
            stream_memory_usage.clone(),
            staging_memory_usage.clone(),
            thread_count,
            options,
        )?;

        let benchmark = Benchmark::new(options.dbg.bench);

        let render_threads: Vec<Arc<SRenderThread>> = Default::default();

        let swap_chain = ash_surface
            .create_swapchain(
                &instance.vk_instance, /* TODO: use the wrapper func */
                &device.device,
            )
            .unwrap();
        benchmark.bench("\t\tcreating vk swap chain");

        let command_pool = command_pools.remove(0);

        let mut res = Box::pin(Self {
            dbg: dbg.clone(),
            gfx_vsync: Default::default(),
            shader_files: Default::default(),
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
            fetch_frame_buffer: Default::default(),
            thread_count,
            cur_stream_vertex_byte_offset: Default::default(),
            cur_render_call_count_in_pipe: Default::default(),
            commands_in_pipe: Default::default(),
            render_calls_in_pipe: Default::default(),

            last_render_thread_index: Default::default(),

            render_threads,
            render_thread_infos: Default::default(),

            main_render_command_buffer: Default::default(),
            wait_semaphores: Default::default(),
            sig_semaphores: Default::default(),
            memory_sempahores: Default::default(),
            frame_fences: Default::default(),
            image_fences: Default::default(),
            cur_frame: Default::default(),
            order_id_gen: Default::default(),
            image_last_frame_check: Default::default(),
            last_presented_swap_chain_image_index: Default::default(),

            frame: Arc::new(spin::Mutex::new(Frame::new())),

            ash_vk: VulkanBackendAsh {
                instance: instance.clone(),
                surface: ash_surface,
                vk_device: device.clone(),
                vk_swap_chain_ash: swap_chain,
            },

            vk_gpu: phy_gpu,
            device: device_instance,
            queue,
            vk_swap_img_and_viewport_extent: Default::default(),
            debug_messenger: dbg_utils_messenger,

            command_pool,

            render: RenderSetupGroup {
                onscreen: RenderSetup::new(),
                offscreen: RenderSetup::new(),
            },
            cur_frames: Default::default(),
            cur_image_index: Default::default(),
            canvas_width,
            canvas_height,
            clear_color: Default::default(),

            command_groups: Default::default(),
            current_command_group: Default::default(),

            error: error,
            check_res: Default::default(),

            logger,

            _runtime_threadpool: runtime_threadpool.clone(),
        });

        // start threads
        assert!(
            thread_count >= 1,
            "At least one rendering thread must exist."
        );

        for _ in 0..thread_count {
            let next_frame_index = Arc::new(AtomicU32::new(u32::MAX));
            let next_frame_count = Arc::new(AtomicU32::new(u32::MAX));
            res.render_thread_infos
                .push((next_frame_index.clone(), next_frame_count.clone()));

            let render_thread = Arc::new(SRenderThread {
                inner: Mutex::new(SRenderThreadInner {
                    is_rendering: false,
                    thread: None,
                    finished: false,
                    started: false,
                    next_frame_index,
                    next_frame_count,
                    command_groups: Default::default(),
                }),
                cond: Condvar::new(),
            });
            res.render_threads.push(render_thread);
        }
        for i in 0..thread_count {
            let unsafe_vk_backend = ThreadVkBackendWrapper {
                render: &res.render,
            };
            let render_thread = &res.render_threads[i];

            let render_thread_param = render_thread.clone();
            let frame = res.frame.clone();
            let device = res.ash_vk.vk_device.clone();
            let streamed_uniform = res.device.streamed_uniform.clone();
            let queue_index = res.ash_vk.vk_device.phy_device.queue_node_index;

            let mut g = render_thread.inner.lock().unwrap();

            g.thread = Some(std::thread::spawn(move || {
                Self::run_thread(
                    unsafe_vk_backend,
                    render_thread_param,
                    frame,
                    device,
                    queue_index,
                    streamed_uniform,
                    i,
                )
            }));
            // wait until thread started
            let _g = render_thread
                .cond
                .wait_while(g, |render_thread| !render_thread.started)
                .unwrap();
        }

        benchmark.bench("\t\tcreating vk render threads");

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

    fn run_thread(
        selfi_ptr: ThreadVkBackendWrapper,
        thread: Arc<SRenderThread>,
        frame: Arc<spin::Mutex<Frame>>,
        device: Arc<LogicalDevice>,
        queue_family_index: u32,
        streamed_uniform: Arc<spin::Mutex<StreamedUniform>>,
        thread_index: usize,
    ) {
        let command_pool = Self::create_command_pools(device.clone(), queue_family_index, 1, 0, 5)
            .unwrap()
            .remove(0);

        let render = unsafe { &*selfi_ptr.render };
        let mut guard = thread.inner.lock().unwrap();
        guard.started = true;
        thread.cond.notify_one();

        while !guard.finished {
            guard = thread
                .cond
                .wait_while(guard, |thread| -> bool {
                    !thread.is_rendering && !thread.finished
                })
                .unwrap();
            thread.cond.notify_one();

            // set this to true, if you want to benchmark the render thread times
            let benchmark = Benchmark::new(false);

            if !guard.finished {
                // make the pool ready
                let frame_count = guard
                    .next_frame_count
                    .swap(u32::MAX, std::sync::atomic::Ordering::SeqCst);
                if frame_count != u32::MAX {
                    command_pool.set_frame_count(frame_count as usize);
                }
                let frame_index = guard
                    .next_frame_index
                    .swap(u32::MAX, std::sync::atomic::Ordering::SeqCst);
                if frame_index != u32::MAX {
                    command_pool.set_frame_index(frame_index as usize);
                }

                let mut has_error_from_cmd: bool = false;
                while let Some(mut cmd_group) = guard.command_groups.pop() {
                    let command_buffer = command_pool
                        .get_render_buffer(
                            AutoCommandBufferType::Secondary {
                                render: render,
                                cur_image_index: cmd_group.cur_frame_index,
                                render_pass_type: cmd_group.render_pass,
                                render_pass_frame_index: cmd_group.render_pass_index,
                                buffer_in_order_id: cmd_group.in_order_id,
                            },
                            &frame,
                        )
                        .unwrap();
                    for mut next_cmd in cmd_group.cmds.drain(..) {
                        let cmd = next_cmd.raw_render_command.take().unwrap();
                        if !Self::command_cb_render(
                            &device,
                            render,
                            &streamed_uniform,
                            cmd_group.cur_frame_index,
                            thread_index,
                            cmd_group.render_pass,
                            &cmd,
                            next_cmd,
                            &command_buffer,
                        ) {
                            // an error occured, the thread will not continue execution
                            has_error_from_cmd = true;
                            break;
                        }
                    }
                }
                if has_error_from_cmd {
                    panic!("TODO:")
                }
            }

            benchmark.bench("vulkan render thread");

            guard.is_rendering = false;
        }
    }

    pub fn create_mt_backend(&self) -> VulkanBackendMt {
        VulkanBackendMt {
            mem_allocator: self.device.mem_allocator.clone(),
            flush_lock: Default::default(),
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

    fn init_while_io(&mut self, capabilities: &mut SBackendCapabilites) -> anyhow::Result<()> {
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

        Ok(())
    }

    fn init(&mut self) -> anyhow::Result<()> {
        if self.init_vulkan_with_io::<true>() != 0 {
            return Err(anyhow!("Failed to initialize vulkan."));
        }

        let mut indices_upload: Vec<u32> = Vec::new();
        indices_upload.reserve(StreamDataMax::MaxVertices as usize / 4 * 6);
        let mut primitive_count: u32 = 0;
        for _ in (0..(StreamDataMax::MaxVertices as usize / 4 * 6)).step_by(6) {
            indices_upload.push(primitive_count);
            indices_upload.push(primitive_count + 1);
            indices_upload.push(primitive_count + 2);
            indices_upload.push(primitive_count);
            indices_upload.push(primitive_count + 2);
            indices_upload.push(primitive_count + 3);
            primitive_count += 4;
        }

        self.prepare_frame()?;

        // TODO: ??? looks completely stupid.. better handle all errors instead
        if self.error.lock().unwrap().has_error {
            return Err(anyhow!("This is a stupid call."));
        }

        let img_res = self.device.create_index_buffer(
            indices_upload.as_mut_ptr() as *mut c_void,
            std::mem::size_of::<u32>() * indices_upload.len(),
            0,
        );
        if img_res.is_err() {
            return Err(anyhow!("Failed to create index buffer."));
        }
        (self.index_buffer, self.index_buffer_memory) =
            img_res.map(|(i, m)| (Some(i), Some(m))).unwrap();

        let img_res = self.device.create_index_buffer(
            indices_upload.as_mut_ptr() as *mut c_void,
            std::mem::size_of::<u32>() * indices_upload.len(),
            0,
        );
        if img_res.is_err() {
            return Err(anyhow!("Failed to create index buffer."));
        }
        (self.render_index_buffer, self.render_index_buffer_memory) =
            img_res.map(|(i, m)| (Some(i), Some(m))).unwrap();
        self.cur_render_index_primitive_count = StreamDataMax::MaxVertices as usize / 4;

        self.error.lock().unwrap().can_assert = true;

        Ok(())
    }

    fn destroy(mut self) {
        unsafe { self.ash_vk.vk_device.device.device_wait_idle().unwrap() };

        self.cleanup_vulkan::<true>();
        self.cleanup_vulkan_sdl();
    }

    fn get_presented_image_data(
        &mut self,
        width: &mut u32,
        height: &mut u32,
        dest_data_buffer: &mut Vec<u8>,
        ignore_alpha: bool,
    ) -> anyhow::Result<ImageFormat> {
        self.get_presented_image_data_impl(width, height, dest_data_buffer, false, ignore_alpha)
    }

    fn run_command(&mut self, cmd: AllCommands) -> anyhow::Result<()> {
        /* TODO! no locking pls if(self.m_HasError)
        {
            // ignore all further commands
            return ERunCommandReturnTypes::RUN_COMMAND_COMMAND_ERROR;
        }*/

        let mut buffer = RenderCommandExecuteBuffer::default();
        buffer.viewport_size = self.vk_swap_img_and_viewport_extent.swap_image_viewport;

        let mut can_start_thread: bool = false;
        if let AllCommands::Render(render_cmd) = &cmd {
            let thread_index = ((self.cur_render_call_count_in_pipe * self.thread_count)
                / self.render_calls_in_pipe.max(1))
                % self.thread_count;

            if thread_index > self.last_render_thread_index {
                can_start_thread = true;
            }
            self.fill_execute_buffer(&render_cmd, &mut buffer);
            self.cur_render_call_count_in_pipe += buffer.estimated_render_call_count;
        }
        let mut is_misc_cmd = false;
        if let AllCommands::Misc(_) = cmd {
            is_misc_cmd = true;
        }
        if is_misc_cmd {
            match cmd {
                AllCommands::Misc(cmd) => {
                    self.command_cb_misc(cmd)?;
                }
                _ => {}
            }
        } else if !self.rendering_paused {
            match cmd {
                AllCommands::Render(render_cmd) => buffer.raw_render_command = Some(render_cmd),
                _ => {}
            }
            self.current_command_group.cmds.push(buffer);

            if can_start_thread {
                self.new_command_group(
                    self.current_command_group.render_pass_index,
                    self.current_command_group.render_pass,
                );
            }
        }

        Ok(())
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
        self.cur_render_call_count_in_pipe = 0;
        self.device.update_stream_vertex_buffer(
            STREAM_DATA_MEMORY_BLOCK_SIZE,
            stream_data.borrow().vertices_count() * std::mem::size_of::<GlVertex>(),
            self.cur_image_index,
        );
    }

    fn end_commands(&mut self) -> Result<&'static mut [GlVertex], ()> {
        self.commands_in_pipe = 0;
        self.render_calls_in_pipe = 0;
        self.last_render_thread_index = 0;

        let res = self
            .device
            .create_stream_vertex_buffer(STREAM_DATA_MEMORY_BLOCK_SIZE, self.cur_image_index);
        if res.is_err() {
            return Err(());
        }
        let (_, _, _, buffer_off, memory_ptr) = res.unwrap();

        self.cur_stream_vertex_byte_offset = buffer_off;
        Ok(unsafe {
            std::slice::from_raw_parts_mut(
                memory_ptr
                    .1
                    .get_mem()
                    .offset(memory_ptr.0)
                    .offset(buffer_off as isize) as *mut GlVertex,
                StreamDataMax::MaxVertices as usize,
            )
        })
    }
}

#[derive(Debug)]
pub struct VulkanBackendMt {
    pub mem_allocator: Arc<spin::Mutex<VulkanAllocator>>,
    pub flush_lock: spin::Mutex<()>,
}

#[derive(Debug)]
pub struct VulkanBackendDellocator {
    pub mem_allocator: Arc<spin::Mutex<VulkanAllocator>>,
}

impl GraphicsBackendMemoryStaticCleaner for VulkanBackendDellocator {
    fn destroy(&self, mem: &'static mut [u8]) {
        self.mem_allocator.lock().free_mem_raw(mem.as_mut_ptr());
    }
}

impl GraphicsBackendMtInterface for VulkanBackendMt {
    fn mem_alloc(&self, alloc_type: GraphicsMemoryAllocationType) -> GraphicsBackendMemory {
        let buffer_data: *const c_void = std::ptr::null();
        let allocator_clone = self.mem_allocator.clone();
        let mut allocator = self.mem_allocator.lock();
        match alloc_type {
            GraphicsMemoryAllocationType::Buffer { required_size } => {
                let res = allocator
                    .get_staging_buffer_for_mem_alloc(buffer_data, required_size as vk::DeviceSize);
                match res {
                    Ok(res) => GraphicsBackendMemory::Static(GraphicsBackendMemoryStatic {
                        mem: Some(res),
                        deallocator: Some(Box::new(VulkanBackendDellocator {
                            mem_allocator: allocator_clone,
                        })),
                    }),
                    Err(_) => {
                        // go to slow memory as backup
                        let mut res = Vec::new();
                        res.resize(required_size, Default::default());
                        GraphicsBackendMemory::Vector(res)
                    }
                }
            }
            GraphicsMemoryAllocationType::Texture {
                width,
                height,
                depth,
                is_3d_tex,
                flags,
            } => {
                let res = allocator.get_staging_buffer_image_for_mem_alloc(
                    buffer_data,
                    width,
                    height,
                    depth,
                    is_3d_tex,
                    flags,
                );
                match res {
                    Ok(res) => GraphicsBackendMemory::Static(GraphicsBackendMemoryStatic {
                        mem: Some(res),
                        deallocator: Some(Box::new(VulkanBackendDellocator {
                            mem_allocator: allocator_clone,
                        })),
                    }),
                    Err(_) => {
                        // go to slow memory as backup
                        let mut res = Vec::new();
                        res.resize(width * height * depth * 4, Default::default());
                        GraphicsBackendMemory::Vector(res)
                    }
                }
            }
        }
    }

    fn try_flush_mem(
        &self,
        mem: &mut GraphicsBackendMemory,
        do_expensive_flushing: bool,
    ) -> anyhow::Result<()> {
        // make sure only one flush at a time happens
        let _lock = self.flush_lock.lock();
        let res = self
            .mem_allocator
            .lock()
            .try_flush_mem(mem, do_expensive_flushing)?;
        if let Some((fence, command_buffer, device)) = res {
            unsafe {
                device.wait_for_fences(&[fence], true, u64::MAX)?;
                device.reset_command_buffer(
                    command_buffer,
                    vk::CommandBufferResetFlags::RELEASE_RESOURCES,
                )
            }?;
        }
        Ok(())
    }
}
