use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::CStr,
    num::NonZeroUsize,
    os::raw::c_void,
    rc::Rc,
    sync::{atomic::AtomicU64, Arc},
};

use base_io::{io::IOFileSys, io_batcher::IOBatcherTask};
use base_log::log::{LogLevel, SystemLogGroup, SystemLogInterface};
use graphics_backend_traits::{
    frame_fetcher_plugin::{BackendFrameFetcher, BackendPresentedImageData, FetchCanvasIndex},
    plugin::{BackendCustomPipeline, BackendRenderExecuteInterface},
    traits::{DriverBackendInterface, GraphicsBackendMtInterface},
    types::BackendCommands,
};
use graphics_base_traits::traits::{
    GraphicsStreamDataInterface, GraphicsStreamedData, GraphicsStreamedUniformData,
    GraphicsStreamedUniformDataType,
};

use anyhow::anyhow;
use graphics_types::{
    commands::{
        AllCommands, CommandClear, CommandCreateBufferObject, CommandDeleteBufferObject,
        CommandIndicesRequiredNumNotify, CommandRecreateBufferObject, CommandRender,
        CommandRenderQuadContainer, CommandRenderQuadContainerAsSpriteMultiple,
        CommandSwitchCanvasMode, CommandSwitchCanvasModeType, CommandTextureCreate,
        CommandTextureDestroy, CommandTextureUpdate, CommandUpdateViewport, Commands,
        CommandsRender, CommandsRenderQuadContainer, CommandsRenderStream, GlVertexTex3DStream,
        RenderSpriteInfo, StreamDataMax, GRAPHICS_DEFAULT_UNIFORM_SIZE,
        GRAPHICS_MAX_UNIFORM_RENDER_COUNT,
    },
    rendering::{GlVertex, State, StateTexture},
    types::{
        GraphicsBackendMemory, GraphicsBackendMemoryStatic, GraphicsBackendMemoryStaticCleaner,
        GraphicsMemoryAllocationType, ImageFormat,
    },
};

use ash::vk::{self};
use hashlink::LinkedHashMap;
use hiarc::{Hi, HiArc, HiRc};
use pool::{datatypes::PoolVec, pool::Pool, rc::PoolRc};
use pool::{mt_datatypes::PoolVec as MtPoolVec, mt_pool::Pool as MtPool};

use crate::{
    backends::vulkan::{
        barriers::image_barrier, image::ImageLayout, pipeline_cache::PipelineCache,
        utils::blit_color_attachment_to_color_attachment_auto_transition,
        vulkan_types::RenderThreadInner,
    },
    window::{BackendDisplayRequirements, BackendSurface, BackendSwapchain, BackendWindow},
};

use base::{benchmark::Benchmark, system::System};
use config::config::{AtomicEDebugGFXModes, EDebugGFXModes};

use super::{
    buffer::Buffer,
    command_pool::{AutoCommandBuffer, AutoCommandBufferType, CommandPool},
    common::{
        tex_format_to_image_color_channel_count, texture_format_to_vulkan_format,
        TTWGraphicsGPUList,
    },
    compiler::compiler::{ShaderCompiler, ShaderCompilerType},
    dbg_utils_messenger::DebugUtilsMessengerEXT,
    descriptor_set::{split_descriptor_sets, DescriptorSet},
    fence::Fence,
    frame::{Frame, FrameCanvasIndex, FrameRenderCanvas},
    frame_resources::{
        FrameResources, FrameResourcesPool, RenderThreadFrameResources,
        RenderThreadFrameResourcesPool,
    },
    image::Image,
    instance::Instance,
    logical_device::LogicalDevice,
    mapped_memory::MappedMemory,
    memory_block::DeviceMemoryBlock,
    phy_device::PhyDevice,
    queue::Queue,
    render_cmds::{command_cb_render, get_address_mode_index},
    render_fill_manager::{RenderCommandExecuteBuffer, RenderCommandExecuteManager},
    render_group::{CanvasMode, RenderSetup, RenderSetupOptions},
    render_pass::CanvasSetup,
    semaphore::Semaphore,
    stream_memory_pool::{StreamMemoryBlock, StreamMemoryPool},
    swapchain::Swapchain,
    utils::copy_color_attachment_to_present_src,
    vulkan_allocator::{
        VulkanAllocator, VulkanAllocatorImageCacheEntryData, VulkanDeviceInternalMemory,
    },
    vulkan_dbg::is_verbose,
    vulkan_device::Device,
    vulkan_types::{
        CTexture, DescriptorPoolType, DeviceDescriptorPools, EMemoryBlockUsage, RenderPassType,
        RenderThread, RenderThreadEvent, StreamedUniformBuffer, TextureData, ThreadCommandGroup,
    },
    Options,
};

#[derive(Debug)]
pub struct VulkanBackendLoadingIO {
    shader_compiler: IOBatcherTask<ShaderCompiler>,
    pipeline_cache: IOBatcherTask<Option<Vec<u8>>>,

    io: IOFileSys,
}

impl VulkanBackendLoadingIO {
    pub fn new(io: &IOFileSys) -> Self {
        let fs = io.fs.clone();
        let backend_files = io.io_batcher.spawn(async move {
            let mut shader_compiler = ShaderCompiler::new(ShaderCompilerType::WgslInSpvOut, fs);

            shader_compiler
                .compile("shader/wgsl", "compile.json")
                .await?;

            Ok(shader_compiler)
        });

        let pipeline_cache = PipelineCache::load_previous_cache(io);

        Self {
            shader_compiler: backend_files,
            pipeline_cache,
            io: io.clone(),
        }
    }
}

pub struct VulkanBackendAsh {
    vk_device: HiArc<LogicalDevice>,
}

impl std::fmt::Debug for VulkanBackendAsh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanBackendAsh").finish()
    }
}

pub struct VulkanBackendSurfaceAsh {
    vk_swap_chain_ash: BackendSwapchain,
    surface: BackendSurface,
}

impl std::fmt::Debug for VulkanBackendSurfaceAsh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanBackendSurfaceAsh").finish()
    }
}

#[derive(Debug)]
pub struct VulkanFetchFramebuffer {
    get_presented_img_data_helper_mem: HiArc<DeviceMemoryBlock>,
    get_presented_img_data_helper_image: HiArc<Image>,
    get_presented_img_data_helper_mapped_memory: HiArc<MappedMemory>,
    get_presented_img_data_helper_mapped_layout_offset: vk::DeviceSize,
    get_presented_img_data_helper_mapped_layout_pitch: vk::DeviceSize,
    get_presented_img_data_helper_width: u32,
    get_presented_img_data_helper_height: u32,
    get_presented_img_data_helper_fence: HiArc<Fence>,
}

#[derive(Debug)]
pub(crate) struct VulkanCustomPipes {
    pub(crate) pipes: Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>,

    pub(crate) pipe_indices: HashMap<String, usize>,
}

impl VulkanCustomPipes {
    pub fn new(pipes: Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>) -> Arc<Self> {
        let mut pipe_indices: HashMap<String, usize> = Default::default();
        let pipes_guard = pipes.read();
        for (index, pipe) in pipes_guard.iter().enumerate() {
            pipe_indices.insert(pipe.pipe_name(), index);
        }
        drop(pipes_guard);
        Arc::new(Self {
            pipes,
            pipe_indices,
        })
    }
}

#[derive(Debug)]
pub(crate) struct VulkanBackendProps {
    /************************
     * MEMBER VARIABLES
     ************************/
    dbg: Arc<AtomicEDebugGFXModes>,
    gfx_vsync: bool,

    next_multi_sampling_count: u32,

    thread_count: usize,

    graphics_uniform_buffers: MtPool<Vec<GraphicsStreamedUniformData>>,

    ash_vk: VulkanBackendAsh,

    vk_gpu: HiArc<PhyDevice>,
    pub(crate) device: Device,
    queue: HiArc<Queue>,

    // never read from, but automatic cleanup
    _debug_messenger: Option<HiArc<DebugUtilsMessengerEXT>>,

    command_pool: HiRc<CommandPool>,

    uniform_buffer_descr_pools: HiArc<parking_lot::Mutex<DeviceDescriptorPools>>,

    /************************
     * ERROR MANAGEMENT
     ************************/
    logger: SystemLogGroup,

    custom_pipes: Arc<VulkanCustomPipes>,
}

fn create_command_pools(
    device: HiArc<LogicalDevice>,
    queue_family_index: u32,
    count: usize,
    default_primary_count: usize,
    default_secondary_count: usize,
) -> anyhow::Result<Vec<HiRc<CommandPool>>> {
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

#[derive(Debug)]
pub struct VulkanBackendLoading {
    props: VulkanBackendProps,
}

// TODO
unsafe impl Send for VulkanBackendLoading {}

impl VulkanBackendLoading {
    unsafe extern "system" fn vk_debug_callback(
        message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
        _message_type: vk::DebugUtilsMessageTypeFlagsEXT,
        ptr_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
        _ptr_raw_user: *mut c_void,
    ) -> vk::Bool32 {
        if !(message_severity & vk::DebugUtilsMessageSeverityFlagsEXT::ERROR).is_empty() {
            let msg = unsafe {
                CStr::from_ptr((*ptr_callback_data).p_message)
                    .to_str()
                    .unwrap()
            };
            println!("{msg}");
            panic!("[vulkan debug] error: {msg}");
        } else {
            /*println!("[vulkan debug] {}", unsafe {
                CStr::from_ptr((*ptr_callback_data).p_message)
                    .to_str()
                    .unwrap()
            });*/
        }

        vk::FALSE
    }

    fn setup_debug_callback(
        entry: &ash::Entry,
        instance: &ash::Instance,
        logger: &SystemLogGroup,
    ) -> anyhow::Result<HiArc<DebugUtilsMessengerEXT>> {
        let mut create_info = vk::DebugUtilsMessengerCreateInfoEXT::default();
        create_info.message_severity = vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
            | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
            | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR;
        create_info.message_type = vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE; // | vk::DEBUG_UTILS_MESSAGE_TYPE_GENERAL_BIT_EXT <- too annoying
        create_info.pfn_user_callback = Some(Self::vk_debug_callback);

        let res_dbg = DebugUtilsMessengerEXT::new(entry, instance, &create_info)
            .map_err(|err| anyhow!("Debug extension could not be loaded: {err}"))?;

        logger
            .log(LogLevel::Info)
            .msg("enabled vulkan debug context.");
        Ok(res_dbg)
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

    fn init_vulkan_sdl(
        display_requirements: &BackendDisplayRequirements,
        dbg_mode: EDebugGFXModes,
        dbg: Arc<AtomicEDebugGFXModes>,
        logger: &SystemLogGroup,
        sys: &System,
        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,
        options: &Options,
    ) -> anyhow::Result<(
        HiArc<LogicalDevice>,
        HiArc<PhyDevice>,
        HiArc<Queue>,
        Device,
        Option<HiArc<DebugUtilsMessengerEXT>>,
        Vec<HiRc<CommandPool>>,
    )> {
        let benchmark = Benchmark::new(options.dbg.bench);
        let instance = Instance::new(display_requirements, dbg_mode)?;
        benchmark.bench("creating vk instance");

        let mut dbg_callback = None;
        if dbg_mode == EDebugGFXModes::Minimum || dbg_mode == EDebugGFXModes::All {
            let dbg_res =
                Self::setup_debug_callback(&instance.vk_entry, &instance.vk_instance, logger);
            if let Ok(dbg) = dbg_res {
                dbg_callback = Some(dbg);
            }

            for vk_layer in &instance.layers {
                logger
                    .log(LogLevel::Info)
                    .msg("Validation layer: ")
                    .msg(vk_layer.as_str());
            }
        }

        let physical_gpu = PhyDevice::new(
            instance.clone(),
            options,
            logger,
            display_requirements.is_headless,
        )?;
        benchmark.bench("selecting vk physical device");

        let device = LogicalDevice::new(
            physical_gpu.clone(),
            physical_gpu.queue_node_index,
            &instance.vk_instance,
            &instance.layers,
            display_requirements.is_headless,
            dbg.clone(),
            texture_memory_usage.clone(),
            buffer_memory_usage.clone(),
            stream_memory_usage.clone(),
            staging_memory_usage.clone(),
        )?;
        benchmark.bench("creating vk logical device");

        let (graphics_queue, presentation_queue) =
            Self::get_device_queue(&device.device, physical_gpu.queue_node_index)?;

        let queue = Queue::new(graphics_queue, presentation_queue);

        benchmark.bench("creating vk queue");

        let command_pools =
            create_command_pools(device.clone(), physical_gpu.queue_node_index, 1, 5, 0)?;

        let device_instance = Device::new(
            dbg,
            instance.clone(),
            device.clone(),
            physical_gpu.clone(),
            queue.clone(),
            texture_memory_usage,
            buffer_memory_usage,
            stream_memory_usage,
            staging_memory_usage,
            &sys.log,
            display_requirements.is_headless,
            options,
            command_pools[0].clone(),
        )?;
        benchmark.bench("creating vk command pools & layouts etc.");

        Ok((
            device,
            physical_gpu,
            queue,
            device_instance,
            dbg_callback,
            command_pools,
        ))
    }

    pub fn new(
        display_requirements: BackendDisplayRequirements,
        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,

        sys: &System,

        options: &Options,

        custom_pipes: Option<Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>>,
    ) -> anyhow::Result<(Self, TTWGraphicsGPUList)> {
        let dbg_mode = options.dbg.gfx; // TODO config / options
        let dbg = Arc::new(AtomicEDebugGFXModes::new(dbg_mode));
        let logger = sys.log.logger("vulkan");

        // thread count
        let thread_count = (options.gl.thread_count as usize).clamp(
            1,
            std::thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(1).unwrap())
                .get(),
        );

        let (device, phy_gpu, queue, device_instance, dbg_utils_messenger, mut command_pools) =
            Self::init_vulkan_sdl(
                &display_requirements,
                dbg_mode,
                dbg.clone(),
                &logger,
                sys,
                texture_memory_usage.clone(),
                buffer_memory_usage.clone(),
                stream_memory_usage.clone(),
                staging_memory_usage.clone(),
                options,
            )?;

        let benchmark = Benchmark::new(options.dbg.bench);

        let command_pool = command_pools.remove(0);

        let res = Self {
            props: VulkanBackendProps {
                dbg: dbg.clone(),
                gfx_vsync: options.gl.vsync,
                thread_count,

                graphics_uniform_buffers: MtPool::with_capacity(128 * 2),

                ash_vk: VulkanBackendAsh {
                    vk_device: device.clone(),
                },

                vk_gpu: phy_gpu.clone(),

                device: device_instance,

                queue,
                _debug_messenger: dbg_utils_messenger,

                command_pool,

                uniform_buffer_descr_pools: DeviceDescriptorPools::new(
                    &device,
                    512,
                    DescriptorPoolType::Uniform,
                )?,

                logger,

                next_multi_sampling_count: Default::default(),

                custom_pipes: VulkanCustomPipes::new(custom_pipes.unwrap_or_default()),
            },
        };
        benchmark.bench("creating initial vk props");

        let gpu_list = res.props.ash_vk.vk_device.phy_device.gpu_list.clone();
        Ok((res, gpu_list))
    }
}

#[derive(Debug)]
pub struct VulkanBackend {
    pub(crate) props: VulkanBackendProps,
    ash_surf: VulkanBackendSurfaceAsh,
    runtime_threadpool: Arc<rayon::ThreadPool>,

    pub(crate) cur_stream_vertex_buffer: PoolRc<StreamMemoryBlock<()>>,
    pub(crate) cur_stream_uniform_buffers: PoolRc<StreamMemoryBlock<StreamedUniformBuffer>>,

    streamed_vertex_buffers_pool: StreamMemoryPool<()>,
    streamed_uniform_buffers_pool: StreamMemoryPool<StreamedUniformBuffer>,

    pub(crate) render_index_buffer: HiArc<Buffer>,
    render_index_buffer_memory: HiArc<DeviceMemoryBlock>,
    cur_render_index_primitive_count: u64,

    last_render_thread_index: usize,
    recreate_swap_chain: bool,
    rendering_paused: bool,
    pub(crate) has_dynamic_viewport: bool,
    pub(crate) dynamic_viewport_offset: vk::Offset2D,
    pub(crate) dynamic_viewport_size: vk::Extent2D,
    cur_render_call_count_in_pipe: usize,

    commands_in_pipe: usize,
    render_calls_in_pipe: usize,

    main_render_command_buffer: Option<AutoCommandBuffer>,
    frame: HiArc<parking_lot::Mutex<Frame>>,

    // swapped by use case
    wait_semaphores: Vec<HiArc<Semaphore>>,
    sig_semaphores: Vec<HiArc<Semaphore>>,

    memory_sempahores: Vec<HiArc<Semaphore>>,

    frame_fences: Vec<HiArc<Fence>>,
    image_fences: Vec<Option<HiArc<Fence>>>,

    order_id_gen: usize,
    cur_frame: u64,
    image_last_frame_check: Vec<u64>,

    fetch_frame_buffer: Option<VulkanFetchFramebuffer>,
    last_presented_swap_chain_image_index: u32,
    frame_fetchers: LinkedHashMap<String, Arc<dyn BackendFrameFetcher>>,
    frame_data_pool: MtPool<Vec<u8>>,

    render_threads: Vec<Arc<RenderThread>>,
    pub(crate) render: RenderSetup,

    cur_frames: u32,
    pub(crate) cur_image_index: u32,

    canvas_width: f64,
    canvas_height: f64,

    pub(crate) clear_color: [f32; 4],

    pub(crate) current_command_group: ThreadCommandGroup,
    command_groups: Vec<ThreadCommandGroup>,
    pub(crate) current_frame_resources: FrameResources,
    frame_resources: HashMap<u32, FrameResources>,

    frame_resources_pool: FrameResourcesPool,

    pipeline_cache: Option<Hi<PipelineCache>>,
}

impl VulkanBackend {
    /************************
     * ERROR MANAGEMENT HELPER
     ************************/

    fn skip_frames_until_current_frame_is_used_again(&mut self) -> anyhow::Result<()> {
        // aggressivly try to get more memory
        unsafe {
            let _g = self.props.queue.queues.lock();
            self.props
                .ash_vk
                .vk_device
                .device
                .device_wait_idle()
                .unwrap()
        };
        for _ in 0..self.render.onscreen.swap_chain_image_count() + 1 {
            self.next_frame()?;
        }

        Ok(())
    }

    fn uniform_stream_alloc_func(&mut self, count: usize) -> anyhow::Result<()> {
        let device = &self.props.ash_vk.vk_device;
        let pools = &mut self.props.uniform_buffer_descr_pools;
        let sprite_descr_layout = &self
            .props
            .device
            .layouts
            .vertex_uniform_descriptor_set_layout;
        let quad_descr_layout = &self
            .props
            .device
            .layouts
            .vertex_fragment_uniform_descriptor_set_layout;

        let alloc_func = |buffer: &HiArc<Buffer>,
                          mem_offset: vk::DeviceSize,
                          set_count: usize|
         -> anyhow::Result<Vec<StreamedUniformBuffer>> {
            let mut res: Vec<StreamedUniformBuffer> = Vec::with_capacity(set_count);
            let descr1: Vec<HiArc<DescriptorSet>> =
                VulkanAllocator::create_uniform_descriptor_sets(
                    &device,
                    pools,
                    sprite_descr_layout,
                    set_count,
                    buffer,
                    GRAPHICS_MAX_UNIFORM_RENDER_COUNT * GRAPHICS_DEFAULT_UNIFORM_SIZE,
                    mem_offset,
                )?
                .into_iter()
                .map(|sets| split_descriptor_sets(&sets))
                .flatten()
                .collect();
            let descr2: Vec<HiArc<DescriptorSet>> =
                VulkanAllocator::create_uniform_descriptor_sets(
                    &device,
                    pools,
                    quad_descr_layout,
                    set_count,
                    buffer,
                    GRAPHICS_MAX_UNIFORM_RENDER_COUNT * GRAPHICS_DEFAULT_UNIFORM_SIZE,
                    mem_offset,
                )?
                .into_iter()
                .map(|sets| split_descriptor_sets(&sets))
                .flatten()
                .collect();

            for (descr1, descr2) in descr1.into_iter().zip(descr2.into_iter()) {
                res.push(StreamedUniformBuffer {
                    uniform_sets: [descr1, descr2],
                });
            }

            Ok(res)
        };

        self.streamed_uniform_buffers_pool
            .try_alloc(alloc_func, count)?;

        Ok(())
    }

    /************************
     * COMMAND CALLBACKS
     ************************/
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
            Commands::SwitchCanvas(cmd) => self.cmd_switch_canvas_mode(cmd),
            Commands::UpdateViewport(cmd) => self.cmd_update_viewport(&cmd),
            Commands::Multisampling => todo!(),
            Commands::VSync => todo!(),
            Commands::WindowCreateNtf => todo!(),
            Commands::WindowDestroyNtf => todo!(),
        }
    }

    fn fill_execute_buffer(
        &mut self,
        cmd: &CommandsRender,
        exec_buffer: &mut RenderCommandExecuteBuffer,
    ) {
        let mut render_execute_manager = RenderCommandExecuteManager::new(exec_buffer, self);
        match &cmd {
            CommandsRender::Clear(cmd) => {
                Self::cmd_clear_fill_execute_buffer(&mut render_execute_manager, cmd)
            }
            CommandsRender::Stream(cmd) => match cmd {
                CommandsRenderStream::Render(cmd) => {
                    Self::cmd_render_fill_execute_buffer(&mut render_execute_manager, cmd)
                }
                CommandsRenderStream::RenderTex3D(_) => {}
                CommandsRenderStream::RenderBlurred { cmd, .. } => {
                    Self::cmd_render_blurred_fill_execute_buffer(&mut render_execute_manager, cmd)
                }
            },
            CommandsRender::QuadContainer(cmd) => match cmd {
                CommandsRenderQuadContainer::Render(cmd) => {
                    Self::cmd_render_quad_container_ex_fill_execute_buffer(
                        &mut render_execute_manager,
                        cmd,
                    )
                }
                CommandsRenderQuadContainer::RenderAsSpriteMultiple(cmd) => {
                    Self::cmd_render_quad_container_as_sprite_multiple_fill_execute_buffer(
                        &mut render_execute_manager,
                        cmd,
                    )
                }
            },
            CommandsRender::Mod { mod_name, cmd } => {
                if let Some(mod_index) = render_execute_manager
                    .backend
                    .props
                    .custom_pipes
                    .pipe_indices
                    .get(mod_name.as_str())
                {
                    let pipes = render_execute_manager
                        .backend
                        .props
                        .custom_pipes
                        .pipes
                        .clone();
                    pipes.read()[*mod_index].fill_exec_buffer(cmd, &mut render_execute_manager);
                }
            }
        }
    }

    /*****************************
     * VIDEO AND SCREENSHOT HELPER
     ******************************/
    fn prepare_presented_image_data_image(
        &mut self,
        res_image_data: &mut &mut [u8],
        width: u32,
        height: u32,
    ) -> anyhow::Result<()> {
        let needs_new_img: bool = self.fetch_frame_buffer.is_none()
            || width
                != self
                    .fetch_frame_buffer
                    .as_ref()
                    .unwrap()
                    .get_presented_img_data_helper_width
            || height
                != self
                    .fetch_frame_buffer
                    .as_ref()
                    .unwrap()
                    .get_presented_img_data_helper_height;
        if needs_new_img {
            if self.fetch_frame_buffer.is_some() {
                self.delete_presented_image_data_image();
            }

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

            let presented_img_data_helper_image =
                Image::new(self.props.ash_vk.vk_device.clone(), image_info)?;
            // Create memory to back up the image
            let mem_requirements = unsafe {
                self.props
                    .ash_vk
                    .vk_device
                    .device
                    .get_image_memory_requirements(
                        presented_img_data_helper_image
                            .inner_arc()
                            .img(&mut FrameResources::new(None)),
                    )
            };

            let mut mem_alloc_info = vk::MemoryAllocateInfo::default();
            mem_alloc_info.allocation_size = mem_requirements.size;
            mem_alloc_info.memory_type_index = self.props.device.mem.find_memory_type(
                self.props.vk_gpu.cur_device,
                mem_requirements.memory_type_bits,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
            );

            let presented_img_data_helper_mem = DeviceMemoryBlock::new(
                self.props.ash_vk.vk_device.clone(),
                mem_alloc_info,
                EMemoryBlockUsage::Texture,
            )?;
            presented_img_data_helper_image.bind(presented_img_data_helper_mem.clone(), 0)?;

            self.props.device.image_barrier(
                &mut self.current_frame_resources,
                &presented_img_data_helper_image,
                0,
                1,
                0,
                1,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::GENERAL,
            )?;

            let sub_resource = vk::ImageSubresource::builder()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .mip_level(0)
                .array_layer(0)
                .build();
            let sub_resource_layout = unsafe {
                self.props
                    .ash_vk
                    .vk_device
                    .device
                    .get_image_subresource_layout(
                        presented_img_data_helper_image
                            .inner_arc()
                            .img(&mut FrameResources::new(None)),
                        sub_resource,
                    )
            };

            self.fetch_frame_buffer = Some(VulkanFetchFramebuffer {
                get_presented_img_data_helper_mapped_memory: MappedMemory::new(
                    self.props.ash_vk.vk_device.clone(),
                    presented_img_data_helper_mem.clone(),
                    sub_resource_layout.offset,
                )?,
                get_presented_img_data_helper_mapped_layout_offset: sub_resource_layout.offset,
                get_presented_img_data_helper_mapped_layout_pitch: sub_resource_layout.row_pitch,
                get_presented_img_data_helper_fence: Fence::new(
                    self.props.ash_vk.vk_device.clone(),
                )?,
                get_presented_img_data_helper_width: width,
                get_presented_img_data_helper_height: height,
                get_presented_img_data_helper_image: presented_img_data_helper_image,
                get_presented_img_data_helper_mem: presented_img_data_helper_mem,
            });
        }
        *res_image_data = unsafe {
            std::slice::from_raw_parts_mut(
                self.fetch_frame_buffer
                    .as_ref()
                    .ok_or_else(|| anyhow!("copy image mapped mem was empty"))?
                    .get_presented_img_data_helper_mapped_memory
                    .get_mem(),
                self.fetch_frame_buffer
                    .as_ref()
                    .ok_or_else(|| anyhow!("copy image mem was empty"))?
                    .get_presented_img_data_helper_mem
                    .as_ref()
                    .size() as usize
                    - self
                        .fetch_frame_buffer
                        .as_ref()
                        .ok_or_else(|| anyhow!("copy image offset was empty"))?
                        .get_presented_img_data_helper_mapped_layout_offset
                        as usize,
            )
        };
        Ok(())
    }

    fn delete_presented_image_data_image(&mut self) {
        self.fetch_frame_buffer = None;
    }

    fn get_presented_image_data_impl(
        &mut self,
        flip_img_data: bool,
        reset_alpha: bool,
        fetch_index: FetchCanvasIndex,
    ) -> anyhow::Result<BackendPresentedImageData> {
        let width: u32;
        let height: u32;
        let mut dest_data_buff = self.frame_data_pool.new();
        let render = match fetch_index {
            FetchCanvasIndex::Onscreen => &self.render.onscreen,
            FetchCanvasIndex::Offscreen(id) => &self.render.offscreens.get(&id).unwrap(),
        };
        let mut is_b8_g8_r8_a8: bool = render.surf_format.format == vk::Format::B8G8R8A8_UNORM;
        let uses_rgba_like_format: bool =
            render.surf_format.format == vk::Format::R8G8B8A8_UNORM || is_b8_g8_r8_a8;
        if uses_rgba_like_format && self.last_presented_swap_chain_image_index != u32::MAX {
            let viewport = render.native.swap_img_and_viewport_extent;
            width = viewport.width;
            height = viewport.height;
            let format = ImageFormat::Rgba;

            let image_total_size: usize = width as usize * height as usize * 4;

            let mut res_image_data: &mut [u8] = &mut [];
            self.prepare_presented_image_data_image(&mut res_image_data, width, height)
                .map_err(|err| anyhow!("Could not prepare presented image data: {err}"))?;

            let render = match fetch_index {
                FetchCanvasIndex::Onscreen => &self.render.onscreen,
                FetchCanvasIndex::Offscreen(id) => self.render.offscreens.get(&id).unwrap(),
            };

            let fetch_frame_buffer = self
                .fetch_frame_buffer
                .as_ref()
                .ok_or_else(|| anyhow!("fetch resources were none"))?;

            let command_buffer = self
                .props
                .device
                .get_memory_command_buffer(&mut FrameResources::new(None))
                .map_err(|err| anyhow!("Could not get memory command buffer: {err}"))?
                .command_buffer;

            let final_layout = self.props.ash_vk.vk_device.final_layout();
            let swap_img = &render.native.swap_chain_images
                [self.last_presented_swap_chain_image_index as usize];

            self.props
                .device
                .image_barrier(
                    &mut self.current_frame_resources,
                    &fetch_frame_buffer.get_presented_img_data_helper_image,
                    0,
                    1,
                    0,
                    1,
                    vk::ImageLayout::GENERAL,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                )
                .map_err(|err| anyhow!("Image barrier failed for the helper image: {err}"))?;
            self.props
                .device
                .image_barrier(
                    &mut self.current_frame_resources,
                    swap_img,
                    0,
                    1,
                    0,
                    1,
                    final_layout,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                )
                .map_err(|err| anyhow!("Image barrier failed for the swapchain image: {err}"))?;

            // If source and destination support blit we'll blit as this also does
            // automatic format conversion (e.g. from BGR to RGB)
            if self
                .props
                .ash_vk
                .vk_device
                .phy_device
                .config
                .read()
                .unwrap()
                .optimal_swap_chain_image_blitting
                && self
                    .props
                    .ash_vk
                    .vk_device
                    .phy_device
                    .config
                    .read()
                    .unwrap()
                    .linear_rgba_image_blitting
            {
                let mut blit_size = vk::Offset3D::default();
                blit_size.x = width as i32;
                blit_size.y = height as i32;
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
                    self.props.ash_vk.vk_device.device.cmd_blit_image(
                        command_buffer,
                        swap_img.inner_arc().img(&mut self.current_frame_resources),
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        fetch_frame_buffer
                            .get_presented_img_data_helper_image
                            .inner_arc()
                            .img(&mut FrameResources::new(None)),
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
                image_copy_region.extent.width = width;
                image_copy_region.extent.height = height;
                image_copy_region.extent.depth = 1;

                // Issue the copy command
                unsafe {
                    self.props.ash_vk.vk_device.device.cmd_copy_image(
                        command_buffer,
                        swap_img.inner_arc().img(&mut self.current_frame_resources),
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        fetch_frame_buffer
                            .get_presented_img_data_helper_image
                            .inner_arc()
                            .img(&mut FrameResources::new(None)),
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[image_copy_region],
                    );
                }
            }

            self.props
                .device
                .image_barrier(
                    &mut self.current_frame_resources,
                    &fetch_frame_buffer.get_presented_img_data_helper_image,
                    0,
                    1,
                    0,
                    1,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::ImageLayout::GENERAL,
                )
                .map_err(|err| anyhow!("Image barrier failed for the helper image: {err}"))?;
            self.props
                .device
                .image_barrier(
                    &mut self.current_frame_resources,
                    swap_img,
                    0,
                    1,
                    0,
                    1,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    self.props.ash_vk.vk_device.final_layout(),
                )
                .map_err(|err| anyhow!("Image barrier failed for the swap chain image: {err}"))?;

            self.props.device.memory_command_buffer = None;

            let mut submit_info = vk::SubmitInfo::default();

            submit_info.command_buffer_count = 1;
            submit_info.p_command_buffers = &command_buffer;

            unsafe {
                self.props
                    .ash_vk
                    .vk_device
                    .device
                    .reset_fences(&[fetch_frame_buffer.get_presented_img_data_helper_fence.fence])
            }
            .map_err(|err| anyhow!("Could not reset fences: {err}"))?;
            unsafe {
                let queue = &self.props.queue.queues.lock();
                self.props.ash_vk.vk_device.device.queue_submit(
                    queue.graphics_queue,
                    &[submit_info],
                    fetch_frame_buffer.get_presented_img_data_helper_fence.fence,
                )
            }
            .map_err(|err| anyhow!("Queue submit failed: {err}"))?;
            unsafe {
                self.props.ash_vk.vk_device.device.wait_for_fences(
                    &[fetch_frame_buffer.get_presented_img_data_helper_fence.fence],
                    true,
                    u64::MAX,
                )
            }
            .map_err(|err| anyhow!("Could not wait for fences: {err}"))?;

            let mut mem_range = vk::MappedMemoryRange::default();
            mem_range.memory = fetch_frame_buffer
                .get_presented_img_data_helper_mem
                .inner_arc()
                .mem(&mut FrameResources::new(None));
            mem_range.offset =
                fetch_frame_buffer.get_presented_img_data_helper_mapped_layout_offset;
            mem_range.size = vk::WHOLE_SIZE;
            unsafe {
                self.props
                    .ash_vk
                    .vk_device
                    .device
                    .invalidate_mapped_memory_ranges(&[mem_range])
            }
            .map_err(|err| anyhow!("Could not invalidate mapped memory ranges: {err}"))?;

            let real_full_image_size: usize = image_total_size.max(
                height as usize
                    * fetch_frame_buffer.get_presented_img_data_helper_mapped_layout_pitch as usize,
            );
            if dest_data_buff.len() < real_full_image_size + (width * 4) as usize {
                dest_data_buff.resize(
                    real_full_image_size + (width * 4) as usize,
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
            if width as u64 * 4
                < fetch_frame_buffer.get_presented_img_data_helper_mapped_layout_pitch
            {
                for y in 0..height as usize {
                    let offset_image_packed: usize = y * width as usize * 4;
                    let offset_image_unpacked: usize = y * fetch_frame_buffer
                        .get_presented_img_data_helper_mapped_layout_pitch
                        as usize;

                    let (img_part, help_part) = dest_data_buff
                        .as_mut_slice()
                        .split_at_mut(real_full_image_size);

                    let unpacked_part = img_part.split_at(offset_image_unpacked).1;
                    help_part.copy_from_slice(unpacked_part.split_at(width as usize * 4).0);

                    let packed_part = img_part.split_at_mut(offset_image_packed).1;
                    packed_part
                        .split_at_mut(width as usize * 4)
                        .0
                        .copy_from_slice(help_part);
                }
            }

            if is_b8_g8_r8_a8 || reset_alpha {
                // swizzle
                for y in 0..height as usize {
                    for x in 0..width as usize {
                        let img_off: usize = (y * width as usize * 4) + (x * 4);
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
                    .split_at_mut(width as usize * height as usize * 4);
                for y in 0..height as usize / 2 {
                    temp_dest_copy_row.copy_from_slice(
                        data_dest_real
                            .split_at(y * width as usize * 4)
                            .1
                            .split_at(width as usize * 4)
                            .0,
                    );
                    let write_dest = data_dest_real.split_at_mut(y * width as usize * 4).1;
                    let (write_dest, read_dest) = write_dest.split_at_mut(
                        (((height as usize - y) - 1) * width as usize * 4)
                            - (y * width as usize * 4),
                    );
                    write_dest.copy_from_slice(read_dest.split_at(width as usize * 4).0);
                    data_dest_real
                        .split_at_mut(((height as usize - y) - 1) * width as usize * 4)
                        .1
                        .copy_from_slice(temp_dest_copy_row.split_at(width as usize * 4).0);
                }
            }

            dest_data_buff.resize(width as usize * height as usize * 4, Default::default());

            Ok(BackendPresentedImageData {
                img_format: format,
                width,
                height,
                dest_data_buffer: dest_data_buff,
            })
        } else {
            if !uses_rgba_like_format {
                Err(anyhow!(
                    "Swap chain image was not ready to be copied, because it was not in a RGBA like format."
                ))
            } else {
                Err(anyhow!("Swap chain image was not ready to be copied."))
            }
        }
    }

    /************************
     * SWAPPING MECHANISM
     ************************/
    fn start_render_thread(&mut self, thread_index: usize) {
        if !self.command_groups.is_empty() {
            let thread = &mut self.render_threads[thread_index];
            let mut guard = thread.inner.lock();
            for command_group in self.command_groups.drain(..) {
                guard
                    .render_calls
                    .push((command_group, self.render.get().clone()));
            }
            thread.cond.notify_one();
        }
    }

    fn finish_render_threads(&mut self) {
        // execute threads
        let mut thread_index = self.last_render_thread_index;
        while !self.command_groups.is_empty() {
            self.start_render_thread(thread_index % self.props.thread_count);
            thread_index += 1;
        }

        for thread_index in 0..self.props.thread_count {
            let render_thread = &mut self.render_threads[thread_index];
            let mut guard = render_thread.inner.lock();
            render_thread
                .cond
                .wait_while(&mut guard, |p| !p.render_calls.is_empty());
        }
    }

    fn execute_memory_command_buffer(&mut self) {
        if let Some(memory_command_buffer) = self.props.device.memory_command_buffer.take() {
            let mut submit_info = vk::SubmitInfo::default();

            let command_buffer = memory_command_buffer.command_buffer;
            drop(memory_command_buffer);

            submit_info.command_buffer_count = 1;
            submit_info.p_command_buffers = &command_buffer;
            unsafe {
                let queue = &self.props.queue.queues.lock();
                self.props
                    .ash_vk
                    .vk_device
                    .device
                    .queue_submit(queue.graphics_queue, &[submit_info], vk::Fence::null())
                    .unwrap();
            }
            unsafe {
                let queue = &self.props.queue.queues.lock();
                self.props
                    .ash_vk
                    .vk_device
                    .device
                    .queue_wait_idle(queue.graphics_queue)
                    .unwrap();
            }
        }
    }

    fn flush_memory_ranges(&mut self) {
        if !self.props.device.non_flushed_memory_ranges.is_empty() {
            unsafe {
                self.props
                    .ash_vk
                    .vk_device
                    .device
                    .flush_mapped_memory_ranges(
                        self.props.device.non_flushed_memory_ranges.as_slice(),
                    )
                    .unwrap();
            }

            self.props.device.non_flushed_memory_ranges.clear();
        }
    }

    fn upload_non_flushed_buffers(&mut self) {
        self.flush_memory_ranges();
    }

    fn clear_frame_data(&mut self, frame_index: u32) {
        self.flush_memory_ranges();
        self.frame_resources.remove(&frame_index);
    }

    fn clear_frame_memory_usage(&mut self) {
        self.clear_frame_data(self.cur_image_index);
    }

    fn command_buffer_start_render_pass(
        device: &HiArc<LogicalDevice>,
        render: &CanvasSetup,
        swap_chain_extent_info: &vk::Extent2D,
        clear_color: &[f32; 4],
        cur_image_index: u32,
        render_pass_type: RenderPassType,
        command_buffer: vk::CommandBuffer,
    ) -> anyhow::Result<()> {
        let mut render_pass_info = vk::RenderPassBeginInfo::default();
        render_pass_info.render_pass = match render_pass_type {
            RenderPassType::Single => render.native.render_pass.pass.pass,
            RenderPassType::Switching1 => render.switching.passes[0].render_pass.pass.pass,
            RenderPassType::Switching2 => render.switching.passes[1].render_pass.pass.pass,
        };
        render_pass_info.framebuffer = match render_pass_type {
            RenderPassType::Single => {
                render.native.framebuffer_list[cur_image_index as usize].buffer
            }
            RenderPassType::Switching1 => {
                render.switching.passes[0].framebuffer_list[cur_image_index as usize].buffer
            }
            RenderPassType::Switching2 => {
                render.switching.passes[1].framebuffer_list[cur_image_index as usize].buffer
            }
        };
        render_pass_info.render_area.offset = vk::Offset2D::default();
        render_pass_info.render_area.extent = *swap_chain_extent_info;

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
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 0.0,
                    stencil: 0,
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
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 0.0,
                    stencil: 0,
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

    fn command_buffer_end_render_pass(
        device: &HiArc<LogicalDevice>,
        render: &CanvasSetup,
        command_buffer: vk::CommandBuffer,
        render_pass_type: RenderPassType,
        cur_image_index: u32,
    ) -> anyhow::Result<()> {
        unsafe { device.device.cmd_end_render_pass(command_buffer) };

        let framebuffer = match render_pass_type {
            RenderPassType::Single => &render.native.framebuffer_list[cur_image_index as usize],
            RenderPassType::Switching1 => {
                &render.switching.passes[0].framebuffer_list[cur_image_index as usize]
            }
            RenderPassType::Switching2 => {
                &render.switching.passes[1].framebuffer_list[cur_image_index as usize]
            }
        };

        framebuffer.transition_images()?;

        Ok(())
    }

    fn start_new_render_pass(&mut self, render_pass_type: RenderPassType) -> anyhow::Result<()> {
        self.new_command_group(
            self.current_command_group.canvas_index,
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

    fn cmd_switch_canvas_mode(&mut self, cmd: CommandSwitchCanvasMode) -> anyhow::Result<()> {
        let canvas_index = match &cmd.mode {
            CommandSwitchCanvasModeType::Onscreen => FrameCanvasIndex::Onscreen,
            CommandSwitchCanvasModeType::Offscreen { id, .. } => FrameCanvasIndex::Offscreen(*id),
        };
        self.new_command_group(canvas_index, 0, RenderPassType::Single);
        let mut frame_g = self.frame.lock();
        let frame = &mut *frame_g;
        match canvas_index {
            FrameCanvasIndex::Onscreen => {}
            FrameCanvasIndex::Offscreen(index) => frame.new_offscreen(index),
        }
        drop(frame_g);
        match &cmd.mode {
            CommandSwitchCanvasModeType::Offscreen { id, width, height } => {
                self.render.switch_canvas(CanvasMode::Offscreen {
                    id: *id,
                    device: &self.props.device.ash_vk.device,
                    layouts: &self.props.device.layouts,
                    custom_pipes: &self.props.custom_pipes.pipes,
                    pipeline_cache: &self
                        .pipeline_cache
                        .as_ref()
                        .map(|cache| cache.inner.clone()),
                    standard_texture_descr_pool: &self.props.device.standard_texture_descr_pool,
                    mem_allocator: &self.props.device.mem_allocator,
                    runtime_threadpool: &self.runtime_threadpool,
                    options: &RenderSetupOptions {
                        offscreen_extent: vk::Extent2D {
                            width: *width,
                            height: *height,
                        },
                    },
                    frame_resources: &mut self.current_frame_resources,
                })?
            }
            CommandSwitchCanvasModeType::Onscreen => {
                self.render.switch_canvas(CanvasMode::Onscreen)?
            }
        }
        Ok(())
    }

    fn advance_to_render_pass_type(
        current_frame_resources: &mut FrameResources,
        render: &CanvasSetup,
        props: &VulkanBackendProps,
        cur_image_index: u32,
        main_command_buffer: vk::CommandBuffer,
        new_render_pass_type: RenderPassType,
        cur_render_pass_type: RenderPassType,
    ) -> anyhow::Result<()> {
        if matches!(
            new_render_pass_type,
            RenderPassType::Switching1 | RenderPassType::Switching2
        ) {
            let img = if let RenderPassType::Switching1 = new_render_pass_type {
                &render.switching.passes[1].surface.image_list[cur_image_index as usize]
            } else {
                &render.switching.passes[0].surface.image_list[cur_image_index as usize]
            };

            // transition the current frame image to shader_read
            let img_layout = img
                .base
                .image
                .layout
                .load(std::sync::atomic::Ordering::SeqCst);
            assert!(
                img_layout == ImageLayout::Undefined || img_layout == ImageLayout::ColorAttachment,
                "{:?}",
                img_layout
            );
            image_barrier(
                current_frame_resources,
                &props.ash_vk.vk_device,
                main_command_buffer,
                &img.base.image,
                0,
                1,
                0,
                1,
                if img_layout == ImageLayout::Undefined {
                    vk::ImageLayout::UNDEFINED
                } else {
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
                },
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            )
            .map_err(|err| anyhow!("could not transition image for swapping framebuffer: {err}"))?;

            // if the previous pass type was normal, then copy the image data of it
            // to the unused switching color attachment
            if let RenderPassType::Single = cur_render_pass_type {
                blit_color_attachment_to_color_attachment_auto_transition(
                    current_frame_resources,
                    &props.ash_vk.vk_device,
                    main_command_buffer,
                    &render.native.swap_chain_images[cur_image_index as usize],
                    &img.base.image,
                    render.native.swap_img_and_viewport_extent.width,
                    render.native.swap_img_and_viewport_extent.height,
                    render.native.swap_img_and_viewport_extent.width,
                    render.native.swap_img_and_viewport_extent.height,
                )?;
            }

            // transition the stencil buffer if needed
            let stencil =
                &render.switching.stencil_list_for_pass_transition[cur_image_index as usize];

            if stencil
                .image
                .layout
                .load(std::sync::atomic::Ordering::SeqCst)
                == ImageLayout::Undefined
            {
                image_barrier(
                    current_frame_resources,
                    &props.ash_vk.vk_device,
                    main_command_buffer,
                    &stencil.image,
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
            }
        }
        Ok(())
    }

    fn render_render_pass_type_ended(
        current_frame_resources: &mut FrameResources,
        render: &CanvasSetup,
        props: &VulkanBackendProps,
        cur_image_index: u32,
        main_command_buffer: vk::CommandBuffer,
        new_render_pass_type: RenderPassType,
    ) -> anyhow::Result<()> {
        if matches!(
            new_render_pass_type,
            RenderPassType::Switching1 | RenderPassType::Switching2
        ) {
            let img = if let RenderPassType::Switching1 = new_render_pass_type {
                &render.switching.passes[1].surface.image_list[cur_image_index as usize]
            } else {
                &render.switching.passes[0].surface.image_list[cur_image_index as usize]
            };
            // transition the current frame image to shader_read
            image_barrier(
                current_frame_resources,
                &props.ash_vk.vk_device,
                main_command_buffer,
                &img.base.image,
                0,
                1,
                0,
                1,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            )
            .map_err(|err| anyhow!("could not transition image for swapping framebuffer: {err}"))?;
        }
        Ok(())
    }

    fn finish_render_mode_frame_collecting(
        render_pass_type: RenderPassType,
        current_frame_resources: &mut FrameResources,
        render: &CanvasSetup,
        props: &VulkanBackendProps,
        cur_image_index: u32,
        main_command_buffer: vk::CommandBuffer,
    ) -> anyhow::Result<()> {
        // if the frame finished with switching passes, make sure to copy their content
        if let RenderPassType::Switching1 | RenderPassType::Switching2 = render_pass_type {
            // copy to presentation render pass
            let img = if let RenderPassType::Switching1 = render_pass_type {
                &render.switching.passes[0].surface.image_list[cur_image_index as usize]
            } else {
                &render.switching.passes[1].surface.image_list[cur_image_index as usize]
            };

            copy_color_attachment_to_present_src(
                current_frame_resources,
                &props.ash_vk.vk_device,
                main_command_buffer,
                &img.base.image,
                &render.native.swap_chain_images[cur_image_index as usize],
                render.native.swap_img_and_viewport_extent.width,
                render.native.swap_img_and_viewport_extent.height,
            )?;
        }

        Ok(())
    }

    fn collect_frame_of_canvas(
        frame: &Frame,
        props: &VulkanBackendProps,
        frame_resources: &mut FrameResources,
        render_setup: &HiArc<CanvasSetup>,
        render_canvas: &FrameRenderCanvas,
        main_command_buffer: vk::CommandBuffer,

        cur_image_index: u32,
        clear_color: &[f32; 4],
    ) -> anyhow::Result<()> {
        let mut did_at_least_one_render_pass = false;
        let mut cur_render_pass_type = RenderPassType::Single;
        for render_pass in render_canvas.passes.iter() {
            Self::advance_to_render_pass_type(
                frame_resources,
                render_setup,
                props,
                cur_image_index,
                main_command_buffer,
                render_pass.render_pass_type,
                cur_render_pass_type,
            )?;

            // start the render pass
            Self::command_buffer_start_render_pass(
                &props.ash_vk.vk_device,
                render_setup,
                &render_setup.native.swap_img_and_viewport_extent,
                clear_color,
                cur_image_index,
                render_pass.render_pass_type,
                main_command_buffer,
            )?;
            did_at_least_one_render_pass = true;

            // collect commands
            for (index, subpass) in render_pass.subpasses.iter().enumerate() {
                if index != 0 {
                    unsafe {
                        props.ash_vk.vk_device.device.cmd_next_subpass(
                            main_command_buffer,
                            vk::SubpassContents::SECONDARY_COMMAND_BUFFERS,
                        )
                    };
                }
                // collect in order
                let mut buffers: MtPoolVec<vk::CommandBuffer> =
                    frame.command_buffer_exec_pool.new();
                buffers.extend(subpass.command_buffers.values().map(|buffer| *buffer));
                unsafe {
                    props
                        .ash_vk
                        .vk_device
                        .device
                        .cmd_execute_commands(main_command_buffer, &buffers);
                }
            }

            // end render pass
            Self::command_buffer_end_render_pass(
                &props.ash_vk.vk_device,
                render_setup,
                main_command_buffer,
                render_pass.render_pass_type,
                cur_image_index,
            )?;

            Self::render_render_pass_type_ended(
                frame_resources,
                render_setup,
                props,
                cur_image_index,
                main_command_buffer,
                render_pass.render_pass_type,
            )?;

            cur_render_pass_type = render_pass.render_pass_type;
        }

        if !did_at_least_one_render_pass {
            // fake (empty) render pass
            Self::command_buffer_start_render_pass(
                &props.ash_vk.vk_device,
                render_setup,
                &render_setup.native.swap_img_and_viewport_extent,
                clear_color,
                cur_image_index,
                RenderPassType::Single,
                main_command_buffer,
            )?;
            Self::command_buffer_end_render_pass(
                &props.ash_vk.vk_device,
                render_setup,
                main_command_buffer,
                RenderPassType::Single,
                cur_image_index,
            )?;
        }

        Self::finish_render_mode_frame_collecting(
            cur_render_pass_type,
            frame_resources,
            render_setup,
            props,
            cur_image_index,
            main_command_buffer,
        )?;

        Ok(())
    }

    /// returns if any render pass at all was started
    fn collect_frame(&mut self) -> anyhow::Result<()> {
        let frame = self.frame.lock();
        let main_command_buffer = frame.render.main_command_buffer;

        for (id, render_canvas) in frame.render.offscreen_canvases.iter() {
            let render_setup = &self.render.offscreens.get(&id).unwrap();
            Self::collect_frame_of_canvas(
                &frame,
                &self.props,
                &mut self.current_frame_resources,
                render_setup,
                render_canvas,
                main_command_buffer,
                self.cur_image_index,
                &self.clear_color,
            )?;
        }
        // onscreen canvas always after the offscreen canvases
        Self::collect_frame_of_canvas(
            &frame,
            &self.props,
            &mut self.current_frame_resources,
            &self.render.onscreen,
            &frame.render.onscreen_canvas,
            main_command_buffer,
            self.cur_image_index,
            &self.clear_color,
        )?;

        Ok(())
    }

    fn new_command_group(
        &mut self,
        canvas_index: FrameCanvasIndex,
        render_pass_index: usize,
        render_pass_type: RenderPassType,
    ) {
        if !self.current_command_group.cmds.is_empty() {
            self.command_groups
                .push(std::mem::take(&mut self.current_command_group));
        }

        self.start_render_thread(self.last_render_thread_index);
        self.last_render_thread_index =
            (self.last_render_thread_index + 1) % self.props.thread_count;

        self.order_id_gen += 1;
        self.current_command_group.render_pass_index = render_pass_index;
        self.current_command_group.in_order_id = self.order_id_gen;
        self.current_command_group.render_pass = render_pass_type;
        self.current_command_group.cur_frame_index = self.cur_image_index;
        self.current_command_group.canvas_index = canvas_index;
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
        self.upload_non_flushed_buffers();

        self.collect_frame()?;

        // add frame resources
        self.frame_resources.insert(
            self.cur_image_index,
            self.current_frame_resources
                .take(Some(&self.frame_resources_pool)),
        );

        self.main_render_command_buffer = None;

        let wait_semaphore = self.wait_semaphores[self.cur_frames as usize].semaphore;

        let mut submit_info = vk::SubmitInfo::default();

        let mut command_buffers: [vk::CommandBuffer; 2] = Default::default();
        command_buffers[0] = command_buffer;

        submit_info.command_buffer_count = 1;
        submit_info.p_command_buffers = command_buffers.as_ptr();

        if let Some(memory_command_buffer) = self.props.device.memory_command_buffer.take() {
            let memory_command_buffer = memory_command_buffer.command_buffer;

            command_buffers[0] = memory_command_buffer;
            command_buffers[1] = command_buffer;
            submit_info.command_buffer_count = 2;
            submit_info.p_command_buffers = command_buffers.as_ptr();
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

        if self.props.device.is_headless {
            let wait_counter = unsafe {
                self.props
                    .ash_vk
                    .vk_device
                    .device
                    .get_semaphore_counter_value(wait_semaphore)
                    .unwrap()
            };
            let signal_counter = unsafe {
                self.props
                    .ash_vk
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
            self.props
                .ash_vk
                .vk_device
                .device
                .reset_fences(&[self.frame_fences[self.cur_frames as usize].fence])
                .map_err(|err| anyhow!("could not reset fences {err}"))
        }?;

        unsafe {
            let queue = &self.props.queue.queues.lock();
            self.props.ash_vk.vk_device.device.queue_submit(
                queue.graphics_queue,
                &[submit_info],
                self.frame_fences[self.cur_frames as usize].fence,
            )
        }
        .map_err(|err| anyhow!("Submitting to graphics queue failed: {err}"))?;

        std::mem::swap(
            &mut self.wait_semaphores[self.cur_frames as usize],
            &mut self.sig_semaphores[self.cur_frames as usize],
        );

        let mut present_info = vk::PresentInfoKHR::default();

        present_info.wait_semaphore_count = signal_semaphores.len() as u32;
        present_info.p_wait_semaphores = signal_semaphores.as_ptr();

        present_info.p_image_indices = &self.cur_image_index;

        self.last_presented_swap_chain_image_index = self.cur_image_index;

        let is_suboptimal = unsafe {
            let queue = &self.props.queue.queues.lock();
            self.ash_surf
                .vk_swap_chain_ash
                .queue_present(queue.present_queue, present_info)
        }
        .map_err(|err| anyhow!("Presenting graphics queue failed: {err}"))?;

        // TODO: is this assignment good here?
        if is_suboptimal {
            self.recreate_swap_chain = true;
        }
        // TODO: handle out of date err here directly
        // TODO: handle surface lost

        self.cur_frames =
            (self.cur_frames + 1) % self.render.onscreen.swap_chain_image_count() as u32;

        if !self.frame_fetchers.is_empty() {
            // TODO: removed cloning
            let keys: Vec<String> = self.frame_fetchers.keys().map(|k| k.clone()).collect();
            for i in keys.iter() {
                // get current frame and fill the frame fetcher with it
                let fetch_index = self.frame_fetchers.get(i).unwrap().current_fetch_index();
                let img_data = self.get_presented_image_data_impl(false, false, fetch_index)?;
                let frame_fetcher = self.frame_fetchers.get(i).unwrap();
                frame_fetcher.next_frame(img_data);
            }
        }

        Ok(())
    }

    fn prepare_frame(&mut self) -> anyhow::Result<()> {
        if self.recreate_swap_chain {
            self.recreate_swap_chain = false;
            if is_verbose(&self.props.dbg) {
                self.props
                    .logger
                    .log(LogLevel::Debug)
                    .msg("recreating swap chain requested by user (prepare frame).");
            }
            self.recreate_swap_chain()?;
        }

        let (next_image_index, is_suboptimal) = unsafe {
            self.ash_surf.vk_swap_chain_ash.acquire_next_image(
                u64::MAX,
                self.sig_semaphores[self.cur_frames as usize].semaphore,
                vk::Fence::null(),
            )
        }
        .map_err(|err| anyhow!("Acquiring next image failed: {err}"))?;
        if is_suboptimal {
            self.recreate_swap_chain = false;
            if is_verbose(&*self.props.dbg) {
                self.props
                    .logger
                    .log(LogLevel::Debug)
                    .msg("recreating swap chain requested by acquire next image (prepare frame).");
            }
            self.recreate_swap_chain()?;
            return self.prepare_frame();
        }
        /* TODO!
         else {
            if err == vk::Result::ERROR_OUT_OF_DATE_KHR
            {
            }
            if err == vk::Result::ERROR_SURFACE_LOST_KHR {
                self.rendering_paused = true;
            }
        }
        */
        self.cur_image_index = next_image_index;
        std::mem::swap(
            &mut self.wait_semaphores[self.cur_frames as usize],
            &mut self.sig_semaphores[self.cur_frames as usize],
        );

        if let Some(img_fence) = &self.image_fences[self.cur_image_index as usize] {
            unsafe {
                self.props.ash_vk.vk_device.device.wait_for_fences(
                    &[img_fence.fence],
                    true,
                    u64::MAX,
                )
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
        self.current_command_group.canvas_index = Default::default();
        self.render.new_frame();

        // check if older frames weren't used in a long time
        for frame_image_index in 0..self.image_last_frame_check.len() {
            let last_frame = self.image_last_frame_check[frame_image_index];
            if self.cur_frame - last_frame > self.render.onscreen.swap_chain_image_count() as u64 {
                if let Some(img_fence) = &self.image_fences[frame_image_index] {
                    unsafe {
                        self.props.ash_vk.vk_device.device.wait_for_fences(
                            &[img_fence.fence],
                            true,
                            u64::MAX,
                        )
                    }?;
                    self.clear_frame_data(frame_image_index as u32);
                    self.image_fences[frame_image_index] = None;
                }
                self.image_last_frame_check[frame_image_index] = self.cur_frame;
            }
        }

        // clear frame's memory data
        self.clear_frame_memory_usage();

        for thread in &self.render_threads {
            thread
                .inner
                .lock()
                .events
                .push(RenderThreadEvent::ClearFrame(self.cur_image_index));
        }

        // prepare new frame_collection frame
        self.main_render_command_buffer = Some(CommandPool::get_render_buffer(
            &self.props.command_pool,
            AutoCommandBufferType::Primary,
            &mut self.current_frame_resources.render,
        )?);
        self.frame.lock().new_frame(
            self.main_render_command_buffer
                .as_ref()
                .unwrap()
                .command_buffer,
        );
        Ok(())
    }

    fn pure_memory_frame(&mut self) -> anyhow::Result<()> {
        self.execute_memory_command_buffer();

        // reset streamed data
        self.upload_non_flushed_buffers();

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
        let mut staging_allocation = self
            .props
            .device
            .mem_allocator
            .lock()
            .get_staging_buffer_image(
                &self.props.device.mem,
                &self.props.device.vk_gpu.limits,
                data,
                image_size as u64,
            );
        if let Err(_) = staging_allocation {
            self.skip_frames_until_current_frame_is_used_again()?;
            staging_allocation = self
                .props
                .device
                .mem_allocator
                .lock()
                .get_staging_buffer_image(
                    &self.props.device.mem,
                    &self.props.device.vk_gpu.limits,
                    data,
                    image_size as u64,
                );
        }
        let staging_buffer = staging_allocation?;

        let tex = self.props.device.textures.get(&texture_slot).unwrap();
        match &tex.data {
            TextureData::Tex2D { img, .. } => {
                let img = img.clone();
                let mip_map_count = tex.mip_map_count;
                self.props
                    .device
                    .image_barrier(
                        &mut self.current_frame_resources,
                        &img,
                        0,
                        tex.mip_map_count as usize,
                        0,
                        1,
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    )
                    .map_err(|err| {
                        anyhow!("updating texture failed when transitioning to transfer dst: {err}")
                    })?;
                let buffer = staging_buffer
                    .inner_arc()
                    .buffer(&mut self.current_frame_resources)
                    .as_ref()
                    .unwrap();
                self.props
                    .device
                    .copy_buffer_to_image(
                        &mut self.current_frame_resources,
                        buffer,
                        staging_buffer.heap_data.offset_to_align as u64,
                        &img,
                        x_off as i32,
                        y_off as i32,
                        width as u32,
                        height as u32,
                        1,
                    )
                    .map_err(|err| {
                        anyhow!("texture updating failed while copying buffer to image: {err}")
                    })?;

                if mip_map_count > 1 {
                    self.props
                        .device
                        .build_mipmaps(
                            &mut self.current_frame_resources,
                            &img,
                            format,
                            width,
                            height,
                            1,
                            mip_map_count as usize,
                        )
                        .map_err(|err| {
                            anyhow!("updating texture failed when building mipmaps: {err}")
                        })?;
                } else {
                    self.props.device.image_barrier(&mut self.current_frame_resources,
                        &img,
                        0,
                        1,
                        0,
                        1,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    ).map_err(|err| anyhow!("updating texture failed when transitioning back from transfer dst: {err}"))?;
                }
            }
            TextureData::Tex3D { .. } => panic!("not implemented for 3d textures"),
        }

        self.props.device.upload_and_free_staging_image_mem_block(
            &mut self.current_frame_resources,
            staging_buffer,
        );

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
            .props
            .device
            .mem_allocator
            .lock()
            .mem_image_cache_entry(upload_data.mem.as_mut_ptr());

        let texture_data = if !is_3d_tex {
            match self.props.device.create_texture_image(
                &mut self.current_frame_resources,
                image_index,
                upload_data,
                tex_format,
                width,
                height,
                depth,
                pixel_size,
                mip_map_count,
            ) {
                Ok((img, img_mem)) => {
                    let img_format = tex_format;
                    let img_view = self.props.device.create_texture_image_view(
                        &mut self.current_frame_resources,
                        &img,
                        img_format,
                        vk::ImageViewType::TYPE_2D,
                        1,
                        mip_map_count,
                    );
                    let img_view = img_view.unwrap(); // TODO: err handling

                    let descriptor = Device::create_new_textured_standard_descriptor_sets(
                        &self.props.device.ash_vk.device,
                        &self.props.device.layouts,
                        &self.props.device.standard_texture_descr_pool,
                        &img_view,
                    )?;
                    TextureData::Tex2D {
                        img,
                        img_mem,
                        img_view,
                        vk_standard_textured_descr_set: descriptor,
                    }
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        } else {
            let image_3d_width = width;
            let image_3d_height = height;

            let (img_3d, img_mem_3d) = self.props.device.create_texture_image(
                &mut self.current_frame_resources,
                image_index,
                upload_data,
                tex_format,
                image_3d_width,
                image_3d_height,
                depth,
                pixel_size,
                mip_map_count,
            )?;
            let img_format = tex_format;
            let img_view = self.props.device.create_texture_image_view(
                &mut self.current_frame_resources,
                &img_3d,
                img_format,
                vk::ImageViewType::TYPE_2D_ARRAY,
                depth,
                mip_map_count,
            );
            let img_3d_view = img_view.unwrap(); // TODO: err handling;

            let descr = self
                .props
                .device
                .create_new_3d_textured_standard_descriptor_sets(&img_3d_view)?;

            TextureData::Tex3D {
                img_3d,
                img_3d_mem: img_mem_3d,
                img_3d_view,
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

        self.props.device.textures.insert(image_index, texture); // TODO better fix
        Ok(())
    }

    /************************
     * VULKAN SETUP CODE
     ************************/
    fn destroy_command_buffer(&mut self) {
        self.props.device.memory_command_buffer = None;
    }

    fn create_sync_objects(&mut self) -> anyhow::Result<()> {
        for _ in 0..self.render.onscreen.swap_chain_image_count() {
            self.wait_semaphores.push(Semaphore::new(
                self.props.ash_vk.vk_device.clone(),
                self.props.device.is_headless,
            )?)
        }
        for _ in 0..self.render.onscreen.swap_chain_image_count() {
            self.sig_semaphores.push(Semaphore::new(
                self.props.ash_vk.vk_device.clone(),
                self.props.device.is_headless,
            )?)
        }

        for _ in 0..self.render.onscreen.swap_chain_image_count() {
            self.memory_sempahores.push(Semaphore::new(
                self.props.ash_vk.vk_device.clone(),
                self.props.device.is_headless,
            )?)
        }

        for _ in 0..self.render.onscreen.swap_chain_image_count() {
            self.frame_fences
                .push(Fence::new(self.props.ash_vk.vk_device.clone())?);
        }
        self.image_fences.resize(
            self.render.onscreen.swap_chain_image_count(),
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
    fn cleanup_vulkan<const IS_LAST_CLEANUP: bool>(&mut self) {
        self.image_last_frame_check.clear();

        self.frame_resources.clear();

        if IS_LAST_CLEANUP {
            self.props.device.mem_allocator.lock().destroy_caches();

            self.delete_presented_image_data_image();
        }

        self.destroy_sync_objects();
        self.destroy_command_buffer();
    }

    fn recreate_swap_chain(&mut self) -> anyhow::Result<()> {
        unsafe {
            let _g = self.props.queue.queues.lock();
            self.props
                .ash_vk
                .vk_device
                .device
                .device_wait_idle()
                .map_err(|err| anyhow!("wait idle wait while recreating swapchain {err}"))?
        };

        if is_verbose(&*self.props.dbg) {
            self.props
                .logger
                .log(LogLevel::Info)
                .msg("recreating swap chain.");
        }

        let old_swap_chain_image_count = self.render.onscreen.swap_chain_image_count();

        // set new multi sampling if it was requested
        if self.props.next_multi_sampling_count != u32::MAX {
            self.props
                .device
                .vk_gpu
                .config
                .write()
                .unwrap()
                .multi_sampling_count = self.props.next_multi_sampling_count;
            self.props.next_multi_sampling_count = u32::MAX;
        }

        self.reinit_vulkan_swap_chain()?;

        if old_swap_chain_image_count != self.render.onscreen.swap_chain_image_count() {
            self.cleanup_vulkan::<false>();
            self.init_vulkan()?;
        }

        Ok(())
    }

    fn reinit_vulkan_swap_chain(&mut self) -> anyhow::Result<()> {
        let shader_files = self.render.shader_compiler.shader_files.clone();
        let ty = self.render.shader_compiler.ty;
        let fs = self.render.shader_compiler.fs.clone();

        self.render = RenderSetup::new(
            &self.props.device.ash_vk.device,
            &self.props.device.layouts,
            &self.props.custom_pipes.pipes,
            &self
                .pipeline_cache
                .as_ref()
                .map(|cache| cache.inner.clone()),
            &self.props.device.standard_texture_descr_pool,
            &self.props.device.mem_allocator,
            &self.runtime_threadpool,
            Swapchain::new(
                &self.props.vk_gpu,
                &self.ash_surf.surface,
                &mut self.ash_surf.vk_swap_chain_ash,
                &super::swapchain::SwapchainCreateOptions {
                    vsync: self.props.gfx_vsync,
                },
                &self.props.logger,
                &self.props.dbg,
                (self.canvas_width as u32, self.canvas_height as u32),
            )?,
            &self.ash_surf.vk_swap_chain_ash,
            ShaderCompiler::new_with_files(ty, fs, shader_files),
            false,
        )?;

        self.last_presented_swap_chain_image_index = u32::MAX;

        for thread in &self.render_threads {
            thread
                .inner
                .lock()
                .events
                .push(RenderThreadEvent::ClearFrames);
        }

        Ok(())
    }

    fn init_vulkan_with_io(&mut self) -> anyhow::Result<()> {
        self.create_sync_objects()?;

        self.image_last_frame_check
            .resize(self.render.onscreen.swap_chain_image_count(), 0);

        let onscreen = &self.render.onscreen;
        self.props
            .ash_vk
            .vk_device
            .phy_device
            .update_surface_texture_capabilities(onscreen.surf_format.format);

        Ok(())
    }

    fn init_vulkan(&mut self) -> anyhow::Result<()> {
        self.init_vulkan_with_io()
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
        self.props
            .device
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
            .props
            .device
            .mem_allocator
            .lock()
            .memory_to_internal_memory(data_mem, usage);
        if let Err((mem, _)) = data_mem {
            self.skip_frames_until_current_frame_is_used_again()?;
            data_mem = self
                .props
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
        render_execute_manager: &mut RenderCommandExecuteManager,
        cmd: &CommandClear,
    ) {
        render_execute_manager.clear_color_in_render_thread(cmd.force_clear, cmd.color);
        render_execute_manager.estimated_render_calls(0);
    }

    fn cmd_render_fill_execute_buffer(
        render_execute_manager: &mut RenderCommandExecuteManager,
        cmd: &CommandRender,
    ) {
        let address_mode_index: usize = get_address_mode_index(&cmd.state);
        match cmd.state.texture_index {
            StateTexture::Texture(texture_index) => {
                render_execute_manager.set_texture(0, texture_index, address_mode_index as u64);
            }
            StateTexture::ColorAttachmentOfPreviousPass => {
                render_execute_manager
                    .set_color_attachment_as_texture(0, address_mode_index as u64);
            }
            StateTexture::None => {
                // nothing to do
            }
        }

        render_execute_manager.uses_index_buffer();

        render_execute_manager.estimated_render_calls(1);

        render_execute_manager.exec_buffer_fill_dynamic_states(&cmd.state);

        render_execute_manager.uses_stream_vertex_buffer(
            (cmd.vertices_offset * std::mem::size_of::<GlVertex>()) as u64,
        );
    }

    fn cmd_render_blurred_fill_execute_buffer(
        render_execute_manager: &mut RenderCommandExecuteManager,
        cmd: &CommandRender,
    ) {
        Self::cmd_render_fill_execute_buffer(render_execute_manager, cmd);
    }

    /*
        void Cmd_RenderTex3D_FillExecuteBuffer(exec_buffer: &mut SRenderCommandExecuteBuffer, cmd: &CommandRenderTex3D)
        {
            let IsTextured: bool = Self::GetIsTextured(cmd.state);
            if(IsTextured)
            {
                exec_buffer.m_aDescriptors[0] = self.props.device.m_vTextures[cmd.state.texture_index.unwrap()].m_VKStandard3DTexturedDescrSet;
            }

            exec_buffer.m_IndexBuffer = self.m_IndexBuffer;

            exec_buffer.m_EstimatedRenderCallCount = 1;

            ExecBufferFillDynamicStates(cmd.state, exec_buffer);
        }

        #[must_use] fn Cmd_RenderTex3D(cmd: &CommandRenderTex3D, exec_buffer: &SRenderCommandExecuteBuffer ) { return RenderStandard<CCommandBuffer::SVertexTex3DStream, true>(&mut self,exec_buffer, cmd.state, cmd.m_PrimType, cmd.m_pVertices, cmd.m_PrimCount); } -> bool
    */

    fn cmd_update_viewport(&mut self, cmd: &CommandUpdateViewport) -> anyhow::Result<()> {
        if cmd.by_resize {
            if is_verbose(&*self.props.dbg) {
                self.props
                    .logger
                    .log(LogLevel::Debug)
                    .msg("queueing swap chain recreation because the viewport changed");
            }

            // TODO: rethink if this is a good idea (checking if width changed. maybe some weird edge cases)
            if self
                .render
                .onscreen
                .native
                .swap_img_and_viewport_extent
                .width
                != cmd.width
                || self
                    .render
                    .onscreen
                    .native
                    .swap_img_and_viewport_extent
                    .height
                    != cmd.height
            {
                self.canvas_width = cmd.width as f64;
                self.canvas_height = cmd.height as f64;
                self.recreate_swap_chain = true;
            }
        } else {
            let viewport = self.render.get().native.swap_img_and_viewport_extent;
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
                    if(IsVerbose(&*self.props.dbg))
                    {
                        dbg_msg("vulkan", "queueing swap chain recreation because vsync was changed");
                    }
                    self.m_RecreateSwapChain = true;
                    *cmd.m_pRetOk = true;

                    return true;
                }

                #[must_use] fn Cmd_MultiSampling(&mut self,cmd: &CommandMultiSampling) -> bool
                {
                    if(IsVerbose(&*self.props.dbg))
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
            .props
            .device
            .mem_allocator
            .lock()
            .memory_to_internal_memory(data_mem, usage);
        if let Err((mem, _)) = data_mem {
            data_mem = self
                .props
                .device
                .mem_allocator
                .lock()
                .memory_to_internal_memory(mem, usage);
        }
        let data_mem = data_mem.map_err(|(_, err)| err)?;

        Ok(self.props.device.create_buffer_object(
            &mut self.current_frame_resources,
            cmd.buffer_index,
            data_mem,
            upload_data_size as vk::DeviceSize,
        )?)
    }

    fn cmd_recreate_buffer_object(
        &mut self,
        cmd: CommandRecreateBufferObject,
    ) -> anyhow::Result<()> {
        self.props.device.delete_buffer_object(cmd.buffer_index);

        let upload_data_size = cmd.upload_data.len();

        let data_mem = cmd.upload_data;
        let usage = GraphicsMemoryAllocationType::Buffer {
            required_size: upload_data_size,
        };
        let mut data_mem = self
            .props
            .device
            .mem_allocator
            .lock()
            .memory_to_internal_memory(data_mem, usage);
        if let Err((mem, _)) = data_mem {
            data_mem = self
                .props
                .device
                .mem_allocator
                .lock()
                .memory_to_internal_memory(mem, usage);
        }
        let data_mem = data_mem.map_err(|(_, err)| err)?;

        Ok(self.props.device.create_buffer_object(
            &mut self.current_frame_resources,
            cmd.buffer_index,
            data_mem,
            upload_data_size as vk::DeviceSize,
        )?)
    }

    fn cmd_delete_buffer_object(&mut self, cmd: &CommandDeleteBufferObject) -> anyhow::Result<()> {
        let buffer_index = cmd.buffer_index;
        self.props.device.delete_buffer_object(buffer_index);

        Ok(())
    }

    fn cmd_indices_required_num_notify(
        &mut self,
        cmd: &CommandIndicesRequiredNumNotify,
    ) -> anyhow::Result<()> {
        let indices_count = cmd.required_indices_num;
        if self.cur_render_index_primitive_count < indices_count / 6 {
            let mut upload_indices = Vec::<u32>::new();
            upload_indices.resize(indices_count as usize, Default::default());
            let mut primitive_count: u32 = 0;
            for i in (0..indices_count as usize).step_by(6) {
                upload_indices[i] = primitive_count;
                upload_indices[i + 1] = primitive_count + 1;
                upload_indices[i + 2] = primitive_count + 2;
                upload_indices[i + 3] = primitive_count;
                upload_indices[i + 4] = primitive_count + 2;
                upload_indices[i + 5] = primitive_count + 3;
                primitive_count += 4;
            }
            (self.render_index_buffer, self.render_index_buffer_memory) =
                self.props.device.create_index_buffer(
                    &mut self.current_frame_resources,
                    upload_indices.as_ptr() as *const c_void,
                    upload_indices.len() * std::mem::size_of::<u32>(),
                )?;
            self.cur_render_index_primitive_count = indices_count / 6;
        }

        Ok(())
    }

    fn buffer_object_fill_execute_buffer(
        render_execute_manager: &mut RenderCommandExecuteManager,
        state: &State,
        buffer_object_index: u128,
        draw_calls: usize,
    ) {
        render_execute_manager.set_vertex_buffer(buffer_object_index);

        let address_mode_index: usize = get_address_mode_index(state);
        match state.texture_index {
            StateTexture::Texture(texture_index) => {
                render_execute_manager.set_texture(0, texture_index, address_mode_index as u64);
            }
            StateTexture::ColorAttachmentOfPreviousPass => {
                render_execute_manager
                    .set_color_attachment_as_texture(0, address_mode_index as u64);
            }
            StateTexture::None => {
                // nothing to do
            }
        }

        render_execute_manager.uses_index_buffer();

        render_execute_manager.estimated_render_calls(draw_calls as u64);

        render_execute_manager.exec_buffer_fill_dynamic_states(&state);
    }

    fn cmd_render_quad_container_ex_fill_execute_buffer(
        render_execute_manager: &mut RenderCommandExecuteManager,
        cmd: &CommandRenderQuadContainer,
    ) {
        Self::buffer_object_fill_execute_buffer(
            render_execute_manager,
            &cmd.state,
            cmd.buffer_object_index,
            1,
        );
    }

    fn cmd_render_quad_container_as_sprite_multiple_fill_execute_buffer(
        render_execute_manager: &mut RenderCommandExecuteManager,
        cmd: &CommandRenderQuadContainerAsSpriteMultiple,
    ) {
        render_execute_manager.uses_stream_uniform_buffer(
            0,
            cmd.render_info_uniform_instance as u64,
            0,
        );

        Self::buffer_object_fill_execute_buffer(
            render_execute_manager,
            &cmd.state,
            cmd.buffer_object_index,
            ((cmd.draw_count - 1) / GRAPHICS_MAX_UNIFORM_RENDER_COUNT) + 1,
        );
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

    fn init(&mut self) -> anyhow::Result<()> {
        self.init_vulkan_with_io()?;

        self.prepare_frame()?;

        Ok(())
    }

    fn create_initial_index_buffers(
        device: &mut Device,
        frame_resources: &mut FrameResources,
    ) -> anyhow::Result<((HiArc<Buffer>, HiArc<DeviceMemoryBlock>), usize)> {
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

        let (render_index_buffer, render_index_buffer_memory) = device
            .create_index_buffer(
                frame_resources,
                indices_upload.as_mut_ptr() as *mut c_void,
                std::mem::size_of::<u32>() * indices_upload.len(),
            )
            .map_err(|err| anyhow!("Failed to create index buffer: {err}"))?;

        let cur_render_index_primitive_count = StreamDataMax::MaxVertices as usize / 4;

        Ok((
            (render_index_buffer, render_index_buffer_memory),
            cur_render_index_primitive_count,
        ))
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

    pub fn new(
        mut loading: VulkanBackendLoading,
        loading_io: VulkanBackendLoadingIO,
        runtime_threadpool: &Arc<rayon::ThreadPool>,

        window: &BackendWindow,
        canvas_width: f64,
        canvas_height: f64,
        options: &Options,
    ) -> anyhow::Result<Box<Self>> {
        let benchmark = Benchmark::new(options.dbg.bench);
        // thread count
        let thread_count = loading.props.thread_count;

        let shader_compiler = loading_io.shader_compiler.get_storage()?;
        benchmark.bench("getting compiled shaders");

        let pipeline_cache = PipelineCache::new(
            loading.props.device.ash_vk.device.clone(),
            loading_io.pipeline_cache.get_storage()?.as_ref(),
            loading_io.io,
        )
        .ok();
        benchmark.bench("creating the pipeline cache");

        let phy_gpu = &loading.props.ash_vk.vk_device.phy_device;
        let instance = &loading.props.ash_vk.vk_device.phy_device.instance;
        let mut surface = window.create_surface(&instance.vk_entry, &instance.vk_instance)?;
        Self::create_surface(
            &instance.vk_entry,
            window,
            &mut surface,
            &instance.vk_instance,
            &phy_gpu.cur_device,
            phy_gpu.queue_node_index,
            &loading.props.device,
        )?;
        benchmark.bench("creating vk surface");

        let mut swap_chain = surface.create_swapchain(
            &instance.vk_instance, /* TODO: use the wrapper func */
            &loading.props.ash_vk.vk_device.device,
            &loading.props.queue,
        )?;
        benchmark.bench("creating vk swap chain");

        let render = RenderSetup::new(
            &loading.props.device.ash_vk.device,
            &loading.props.device.layouts,
            &loading.props.custom_pipes.pipes,
            &pipeline_cache.as_ref().map(|cache| cache.inner.clone()),
            &loading.props.device.standard_texture_descr_pool,
            &loading.props.device.mem_allocator,
            runtime_threadpool,
            Swapchain::new(
                &loading.props.vk_gpu,
                &surface,
                &mut swap_chain,
                &super::swapchain::SwapchainCreateOptions {
                    vsync: loading.props.gfx_vsync,
                },
                &loading.props.logger,
                &loading.props.dbg,
                (canvas_width as u32, canvas_height as u32),
            )?,
            &swap_chain,
            shader_compiler,
            true,
        )?;

        benchmark.bench("creating the vk render setup");

        let frame_resources_pool = FrameResourcesPool::new();
        let mut frame_resouces = FrameResources::new(Some(&frame_resources_pool));

        let ((render_index_buffer, render_index_buffer_memory), index_prim_count) =
            Self::create_initial_index_buffers(&mut loading.props.device, &mut frame_resouces)?;

        benchmark.bench("creating the vk render index buffer");

        let streamed_vertex_buffers_pool = StreamMemoryPool::new(
            loading.props.dbg.clone(),
            instance.clone(),
            loading.props.ash_vk.vk_device.clone(),
            phy_gpu.clone(),
            loading.props.device.mem.texture_memory_usage.clone(),
            loading.props.device.mem.buffer_memory_usage.clone(),
            loading.props.device.mem.stream_memory_usage.clone(),
            loading.props.device.mem.staging_memory_usage.clone(),
            vk::BufferUsageFlags::VERTEX_BUFFER,
            std::mem::size_of::<GlVertexTex3DStream>(),
            StreamDataMax::MaxVertices as usize,
            1,
        );

        let streamed_uniform_buffers_pool = StreamMemoryPool::new(
            loading.props.dbg.clone(),
            instance.clone(),
            loading.props.ash_vk.vk_device.clone(),
            phy_gpu.clone(),
            loading.props.device.mem.texture_memory_usage.clone(),
            loading.props.device.mem.buffer_memory_usage.clone(),
            loading.props.device.mem.stream_memory_usage.clone(),
            loading.props.device.mem.staging_memory_usage.clone(),
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            GRAPHICS_DEFAULT_UNIFORM_SIZE,
            GRAPHICS_MAX_UNIFORM_RENDER_COUNT,
            128,
        );

        let cur_stream_vertex_buffer = StreamMemoryBlock::new(
            &streamed_vertex_buffers_pool.block_pool,
            streamed_vertex_buffers_pool.vec_pool.new(),
            streamed_vertex_buffers_pool.pool.clone(),
        );
        let cur_stream_uniform_buffers = StreamMemoryBlock::new(
            &streamed_uniform_buffers_pool.block_pool,
            streamed_uniform_buffers_pool.vec_pool.new(),
            streamed_uniform_buffers_pool.pool.clone(),
        );
        benchmark.bench("creating the vk streamed buffers & pools");

        let mut res = Box::new(Self {
            props: loading.props,
            ash_surf: VulkanBackendSurfaceAsh {
                vk_swap_chain_ash: swap_chain,
                surface,
            },
            runtime_threadpool: runtime_threadpool.clone(),

            streamed_vertex_buffers_pool,
            streamed_uniform_buffers_pool,
            cur_stream_vertex_buffer,
            cur_stream_uniform_buffers,

            render_threads: Default::default(),
            render,

            render_index_buffer,
            render_index_buffer_memory,
            cur_render_index_primitive_count: index_prim_count as u64,

            cur_render_call_count_in_pipe: Default::default(),
            commands_in_pipe: Default::default(),
            render_calls_in_pipe: Default::default(),

            last_render_thread_index: Default::default(),

            recreate_swap_chain: Default::default(),
            rendering_paused: Default::default(),
            has_dynamic_viewport: Default::default(),
            dynamic_viewport_offset: Default::default(),
            dynamic_viewport_size: Default::default(),

            main_render_command_buffer: Default::default(),
            wait_semaphores: Default::default(),
            sig_semaphores: Default::default(),
            memory_sempahores: Default::default(),
            frame_fences: Default::default(),
            image_fences: Default::default(),
            cur_frame: Default::default(),
            order_id_gen: Default::default(),
            image_last_frame_check: Default::default(),

            fetch_frame_buffer: Default::default(),
            last_presented_swap_chain_image_index: u32::MAX,
            frame_fetchers: Default::default(),
            frame_data_pool: MtPool::with_capacity(0),

            frame: Frame::new(),

            cur_frames: Default::default(),
            cur_image_index: Default::default(),
            canvas_width,
            canvas_height,
            clear_color: Default::default(),

            command_groups: Default::default(),
            current_command_group: Default::default(),
            current_frame_resources: frame_resouces,
            frame_resources: Default::default(),

            frame_resources_pool,

            pipeline_cache,
        });
        benchmark.bench("creating vk backend instance");

        res.streamed_vertex_buffers_pool
            .try_alloc(|_, _, set_count: usize| Ok(vec![(); set_count]), 4 * 2)?;
        res.cur_stream_vertex_buffer = res.streamed_vertex_buffers_pool.try_get(1).unwrap();
        benchmark.bench("creating initial stream vertex buffers");
        res.uniform_stream_alloc_func(128 * 4 * 2)?;
        res.cur_stream_uniform_buffers = res.streamed_uniform_buffers_pool.try_get(128).unwrap();
        benchmark.bench("creating initial stream uniform buffers");

        // start threads
        assert!(
            thread_count >= 1,
            "At least one rendering thread must exist."
        );

        for _ in 0..thread_count {
            let render_thread = Arc::new(RenderThread {
                inner: parking_lot::Mutex::new(RenderThreadInner {
                    thread: None,
                    finished: false,
                    started: false,
                    events: Default::default(),
                    render_calls: Default::default(),
                }),
                cond: parking_lot::Condvar::new(),
            });
            res.render_threads.push(render_thread);
        }
        for i in 0..thread_count {
            let render_thread = &res.render_threads[i];

            let render_thread_param = render_thread.clone();
            let frame = res.frame.clone();
            let device = res.props.ash_vk.vk_device.clone();
            let queue_index = res.props.ash_vk.vk_device.phy_device.queue_node_index;
            let custom_pipes = res.props.custom_pipes.clone();

            let mut g = render_thread.inner.lock();

            g.thread = std::thread::Builder::new()
                .name(format!("render thread {i}"))
                .spawn(move || {
                    Self::run_thread(
                        render_thread_param,
                        frame,
                        device,
                        queue_index,
                        custom_pipes,
                    )
                })
                .ok();
            // wait until thread started
            render_thread
                .cond
                .wait_while(&mut g, |render_thread| !render_thread.started);
        }

        benchmark.bench("creating vk render threads");

        res.init()?;

        benchmark.bench("init vk backend instance");

        Ok(res)
    }

    /****************
     * RENDER THREADS
     *****************/

    fn run_thread(
        thread: Arc<RenderThread>,
        frame: HiArc<parking_lot::Mutex<Frame>>,
        device: HiArc<LogicalDevice>,
        queue_family_index: u32,
        custom_pipes: Arc<VulkanCustomPipes>,
    ) {
        let command_pool = create_command_pools(device.clone(), queue_family_index, 1, 0, 5)
            .unwrap()
            .remove(0);

        let frame_resources_pool: Pool<Vec<RenderThreadFrameResources>> = Pool::with_capacity(16);
        let mut frame_resources: HashMap<u32, PoolVec<RenderThreadFrameResources>> =
            Default::default();

        let mut guard = thread.inner.lock();
        guard.started = true;
        thread.cond.notify_one();

        let frame_resource_pool = RenderThreadFrameResourcesPool::new();

        while !guard.finished {
            thread.cond.wait_while(&mut guard, |thread| -> bool {
                thread.render_calls.is_empty() && thread.events.is_empty() && !thread.finished
            });
            thread.cond.notify_one();

            // set this to true, if you want to benchmark the render thread times
            let benchmark = Benchmark::new(false);

            if !guard.finished {
                let mut has_error_from_cmd: bool = false;
                while let Some(event) = guard.events.pop() {
                    match event {
                        RenderThreadEvent::ClearFrame(frame_index) => {
                            frame_resources.remove(&frame_index);
                        }
                        RenderThreadEvent::ClearFrames => {
                            frame_resources.clear();
                        }
                    }
                }
                while let Some((mut cmd_group, render)) = guard.render_calls.pop() {
                    let mut frame_resource =
                        RenderThreadFrameResources::new(Some(&frame_resource_pool));
                    let command_buffer = CommandPool::get_render_buffer(
                        &command_pool,
                        AutoCommandBufferType::Secondary {
                            render: &render,
                            cur_image_index: cmd_group.cur_frame_index,
                            render_pass_type: cmd_group.render_pass,
                            render_pass_frame_index: cmd_group.render_pass_index,
                            buffer_in_order_id: cmd_group.in_order_id,
                            canvas_index: cmd_group.canvas_index,
                            frame: &frame,
                        },
                        &mut frame_resource,
                    )
                    .unwrap();
                    for mut next_cmd in cmd_group.cmds.drain(..) {
                        let cmd = next_cmd.raw_render_command.take().unwrap();
                        if !command_cb_render(
                            &custom_pipes,
                            &device,
                            &render,
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

                    if !frame_resources.contains_key(&cmd_group.cur_frame_index) {
                        frame_resources
                            .insert(cmd_group.cur_frame_index, frame_resources_pool.new());
                    }
                    frame_resources
                        .get_mut(&cmd_group.cur_frame_index)
                        .unwrap()
                        .push(frame_resource);
                }
                if has_error_from_cmd {
                    panic!("TODO:")
                }
            }

            benchmark.bench("vulkan render thread");
        }
    }

    pub fn create_mt_backend(&self) -> VulkanBackendMt {
        VulkanBackendMt {
            mem_allocator: self.props.device.mem_allocator.clone(),
            flush_lock: Default::default(),
        }
    }
}

impl DriverBackendInterface for VulkanBackend {
    fn get_presented_image_data(
        &mut self,
        ignore_alpha: bool,
    ) -> anyhow::Result<BackendPresentedImageData> {
        self.get_presented_image_data_impl(false, ignore_alpha, FetchCanvasIndex::Onscreen)
    }

    fn attach_frame_fetcher(&mut self, name: String, fetcher: Arc<dyn BackendFrameFetcher>) {
        self.frame_fetchers.insert(name, fetcher);
    }

    fn detach_frame_fetcher(&mut self, name: String) {
        self.frame_fetchers.remove(&name);
    }

    fn run_command(&mut self, cmd: AllCommands) -> anyhow::Result<()> {
        let mut buffer = RenderCommandExecuteBuffer::default();
        buffer.viewport_size = self.render.get().native.swap_img_and_viewport_extent;

        let mut can_start_thread: bool = false;
        if let AllCommands::Render(render_cmd) = &cmd {
            let thread_index = ((self.cur_render_call_count_in_pipe * self.props.thread_count)
                / self.render_calls_in_pipe.max(1))
                % self.props.thread_count;

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
                    self.current_command_group.canvas_index,
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
        self.cur_stream_vertex_buffer.memories[0].flush(
            &mut self.current_frame_resources,
            self.props.vk_gpu.limits.non_coherent_mem_alignment,
            stream_data.borrow().vertices_count() * std::mem::size_of::<GlVertex>(),
            &mut self.props.device.non_flushed_memory_ranges,
        );
        let uniform_instance_count = stream_data.borrow().uniform_instance_count();
        for i in 0..uniform_instance_count {
            let usage_count = stream_data.borrow().uniform_used_count_of_instance(i);
            self.cur_stream_uniform_buffers.memories[i].flush(
                &mut self.current_frame_resources,
                self.props.vk_gpu.limits.non_coherent_mem_alignment,
                match usage_count {
                    GraphicsStreamedUniformDataType::Sprites(count) => {
                        count * std::mem::size_of::<RenderSpriteInfo>()
                    }
                    GraphicsStreamedUniformDataType::Arbitrary {
                        element_size,
                        element_count,
                    } => element_count * element_size,
                    GraphicsStreamedUniformDataType::None => {
                        panic!("uniform usage was none, this should not happen.")
                    }
                },
                &mut self.props.device.non_flushed_memory_ranges,
            );
        }
    }

    fn end_commands(&mut self) -> anyhow::Result<GraphicsStreamedData> {
        self.commands_in_pipe = 0;
        self.render_calls_in_pipe = 0;
        self.last_render_thread_index = 0;

        self.cur_stream_vertex_buffer = self
            .streamed_vertex_buffers_pool
            .get(|_, _, set_count: usize| Ok(vec![(); set_count]), 1)?;
        self.current_frame_resources
            .stream_vertex_buffers
            .push(self.cur_stream_vertex_buffer.clone());

        self.uniform_stream_alloc_func(128)?;
        self.cur_stream_uniform_buffers = self
            .streamed_uniform_buffers_pool
            .try_get(128)
            .ok_or_else(|| anyhow!("stream uniform buffer pool returned None"))?;
        self.current_frame_resources
            .stream_uniform_buffers
            .push(self.cur_stream_uniform_buffers.clone());

        let mem = unsafe {
            self.cur_stream_vertex_buffer.memories[0]
                .mapped_memory
                .get_mem_typed::<GlVertex>(StreamDataMax::MaxVertices as usize)
        };

        let mut graphics_uniform_data = self.props.graphics_uniform_buffers.new();
        graphics_uniform_data.extend(self.cur_stream_uniform_buffers.memories.iter().map(
            |uni| unsafe {
                GraphicsStreamedUniformData::new(
                    uni.mapped_memory
                        .get_mem_typed::<RenderSpriteInfo>(GRAPHICS_MAX_UNIFORM_RENDER_COUNT),
                    uni.mapped_memory
                        .get_mem(GRAPHICS_MAX_UNIFORM_RENDER_COUNT * GRAPHICS_DEFAULT_UNIFORM_SIZE),
                )
            },
        ));
        Ok(GraphicsStreamedData::new(mem, graphics_uniform_data))
    }
}

impl Drop for VulkanBackend {
    fn drop(&mut self) {
        unsafe {
            let _g = self.props.queue.queues.lock();
            self.props
                .ash_vk
                .vk_device
                .device
                .device_wait_idle()
                .unwrap()
        };

        self.cleanup_vulkan::<true>();

        // clean all images, buffers, buffer containers
        self.props.device.textures.clear();
        self.props.device.buffer_objects.clear();
    }
}

#[derive(Debug)]
pub struct VulkanBackendMt {
    pub mem_allocator: HiArc<parking_lot::Mutex<VulkanAllocator>>,
    pub flush_lock: spin::Mutex<()>,
}

#[derive(Debug)]
pub struct VulkanBackendDellocator {
    pub mem_allocator: HiArc<parking_lot::Mutex<VulkanAllocator>>,
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
