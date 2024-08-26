use std::{
    collections::HashMap,
    ffi::CStr,
    num::NonZeroUsize,
    os::raw::c_void,
    rc::Rc,
    sync::{atomic::AtomicU64, Arc},
};

use base_io::{io::IoFileSys, io_batcher::IoBatcherTask};
use graphics_backend_traits::{
    frame_fetcher_plugin::{
        BackendFrameFetcher, BackendPresentedImageData, FetchCanvasError, FetchCanvasIndex,
    },
    plugin::{BackendCustomPipeline, BackendRenderExecuteInterface},
    traits::{DriverBackendInterface, GraphicsBackendMtInterface},
};
use graphics_base_traits::traits::{
    GraphicsStreamedData, GraphicsStreamedUniformData, GraphicsStreamedUniformDataType,
};

use anyhow::anyhow;
use graphics_types::{
    commands::{
        AllCommands, CommandClear, CommandCreateBufferObject, CommandDeleteBufferObject,
        CommandIndicesForQuadsRequiredNotify, CommandRecreateBufferObject, CommandRender,
        CommandRenderQuadContainer, CommandRenderQuadContainerAsSpriteMultiple,
        CommandSwitchCanvasMode, CommandSwitchCanvasModeType, CommandTextureCreate,
        CommandTextureDestroy, CommandTextureUpdate, CommandUpdateBufferObject,
        CommandUpdateViewport, CommandsMisc, CommandsRender, CommandsRenderMod,
        CommandsRenderQuadContainer, CommandsRenderStream, GlVertexTex3DStream, StreamDataMax,
        GRAPHICS_DEFAULT_UNIFORM_SIZE, GRAPHICS_MAX_UNIFORM_RENDER_COUNT,
        GRAPHICS_UNIFORM_INSTANCE_COUNT,
    },
    rendering::{GlVertex, State, StateTexture},
    types::{
        GraphicsBackendMemory, GraphicsBackendMemoryStatic, GraphicsBackendMemoryStaticCleaner,
        GraphicsMemoryAllocationType, ImageFormat,
    },
};

use ash::vk::{self};
use hashlink::LinkedHashMap;
use hiarc::Hiarc;
use log::{info, warn};
use pool::mt_pool::Pool as MtPool;
use pool::{datatypes::PoolVec, pool::Pool, rc::PoolRc};

use crate::{
    backend::CustomPipelines,
    backends::{
        types::BackendWriteFiles,
        vulkan::{pipeline_cache::PipelineCache, vulkan_types::RenderThreadInner},
    },
    window::{
        BackendDisplayRequirements, BackendSurface, BackendSurfaceAndHandles, BackendSwapchain,
        BackendWindow,
    },
};

use base::benchmark::Benchmark;
use config::config::{AtomicGfxDebugModes, ConfigDebug, GfxDebugModes};

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
    frame::{Frame, FrameCanvasIndex},
    frame_collection::FrameCollector,
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
    semaphore::Semaphore,
    stream_memory_pool::{StreamMemoryBlock, StreamMemoryPool},
    swapchain::Swapchain,
    vulkan_allocator::{
        VulkanAllocator, VulkanAllocatorImageCacheEntryData, VulkanDeviceInternalMemory,
    },
    vulkan_dbg::is_verbose,
    vulkan_device::Device,
    vulkan_types::{
        CTexture, DescriptorPoolType, DeviceDescriptorPools, EMemoryBlockUsage, RenderPassSubType,
        RenderPassType, RenderThread, RenderThreadEvent, StreamedUniformBuffer, TextureData,
        ThreadCommandGroup,
    },
    Options,
};

#[derive(Debug, Hiarc)]
pub struct VulkanBackendLoadedIo {
    pub shader_compiler: ShaderCompiler,
    pub pipeline_cache: Option<Vec<u8>>,
}

#[derive(Debug)]
pub struct VulkanBackendLoadingIo {
    pub shader_compiler: IoBatcherTask<ShaderCompiler>,
    pub pipeline_cache: IoBatcherTask<Option<Vec<u8>>>,
}

impl VulkanBackendLoadingIo {
    pub fn new(io: &IoFileSys) -> Self {
        let fs = io.fs.clone();
        let backend_files = io.io_batcher.spawn(async move {
            let mut shader_compiler = ShaderCompiler::new(ShaderCompilerType::WgslInSpvOut, fs);

            shader_compiler
                .compile("shader/wgsl".as_ref(), "compile.json".as_ref())
                .await?;

            Ok(shader_compiler)
        });

        let pipeline_cache = PipelineCache::load_previous_cache(io);

        Self {
            shader_compiler: backend_files,
            pipeline_cache,
        }
    }
}

#[derive(Hiarc)]
pub struct VulkanBackendAsh {
    pub(crate) vk_device: Arc<LogicalDevice>,
}

impl std::fmt::Debug for VulkanBackendAsh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanBackendAsh").finish()
    }
}

#[derive(Hiarc)]
pub struct VulkanBackendSurfaceAsh {
    vk_swap_chain_ash: BackendSwapchain,
    surface: BackendSurface,
}

impl std::fmt::Debug for VulkanBackendSurfaceAsh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanBackendSurfaceAsh").finish()
    }
}

#[derive(Debug, Hiarc)]
pub struct VulkanFetchFramebuffer {
    get_presented_img_data_helper_mem: Arc<DeviceMemoryBlock>,
    get_presented_img_data_helper_image: Arc<Image>,
    get_presented_img_data_helper_mapped_memory: Arc<MappedMemory>,
    get_presented_img_data_helper_mapped_layout_offset: vk::DeviceSize,
    get_presented_img_data_helper_mapped_layout_pitch: vk::DeviceSize,
    get_presented_img_data_helper_width: u32,
    get_presented_img_data_helper_height: u32,
    get_presented_img_data_helper_fence: Arc<Fence>,
}

#[derive(Debug, Hiarc)]
pub(crate) struct VulkanCustomPipes {
    #[hiarc_skip_unsafe]
    pub(crate) pipes: CustomPipelines,

    pub(crate) pipe_indices: HashMap<String, usize>,
}

impl VulkanCustomPipes {
    pub fn new(pipes: CustomPipelines) -> Arc<Self> {
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

#[derive(Debug, Hiarc)]
pub(crate) struct VulkanBackendProps {
    /************************
     * MEMBER VARIABLES
     ************************/
    #[hiarc_skip_unsafe]
    dbg: Arc<AtomicGfxDebugModes>,
    gfx_vsync: bool,

    thread_count: usize,

    pub(crate) graphics_uniform_buffers: MtPool<Vec<GraphicsStreamedUniformData>>,

    pub(crate) ash_vk: VulkanBackendAsh,

    vk_gpu: Arc<PhyDevice>,
    pub(crate) device: Device,
    queue: Arc<Queue>,

    // never read from, but automatic cleanup
    _debug_messenger: Option<Arc<DebugUtilsMessengerEXT>>,

    command_pool: Rc<CommandPool>,

    uniform_buffer_descr_pools: Arc<parking_lot::Mutex<DeviceDescriptorPools>>,

    /************************
     * ERROR MANAGEMENT
     ************************/
    custom_pipes: Arc<VulkanCustomPipes>,
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

#[derive(Debug)]
pub struct VulkanBackendLoading {
    props: VulkanBackendProps,
}

type InitNativeResult = (
    Arc<LogicalDevice>,
    Arc<PhyDevice>,
    Arc<Queue>,
    Device,
    Option<Arc<DebugUtilsMessengerEXT>>,
    Vec<Rc<CommandPool>>,
);

type ArcRwLock<T> = Arc<parking_lot::RwLock<T>>;

type InitialIndexBuffer = ((Arc<Buffer>, Arc<DeviceMemoryBlock>), usize);

impl VulkanBackendLoading {
    unsafe extern "system" fn vk_debug_callback(
        message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
        _message_type: vk::DebugUtilsMessageTypeFlagsEXT,
        ptr_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
        _ptr_raw_user: *mut c_void,
    ) -> vk::Bool32 {
        if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
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
    ) -> anyhow::Result<Arc<DebugUtilsMessengerEXT>> {
        let mut create_info = vk::DebugUtilsMessengerCreateInfoEXT::default();
        create_info.message_severity = vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
            | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
            | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR;
        create_info.message_type = vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE; // | vk::DebugUtilsMessageTypeFlagsEXT::GENERAL <- too annoying
        create_info.pfn_user_callback = Some(Self::vk_debug_callback);

        let res_dbg = DebugUtilsMessengerEXT::new(entry, instance, &create_info)
            .map_err(|err| anyhow!("Debug extension could not be loaded: {err}"))?;

        warn!("enabled vulkan debug context.");
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

    fn init_vulkan_with_native(
        display_requirements: &BackendDisplayRequirements,
        dbg_mode: GfxDebugModes,
        dbg: Arc<AtomicGfxDebugModes>,
        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,
        options: &Options,
    ) -> anyhow::Result<InitNativeResult> {
        let benchmark = Benchmark::new(options.dbg.bench);
        let instance = Instance::new(display_requirements, dbg_mode)?;
        benchmark.bench("creating vk instance");

        let mut dbg_callback = None;
        if dbg_mode == GfxDebugModes::Minimum || dbg_mode == GfxDebugModes::All {
            let dbg_res = Self::setup_debug_callback(&instance.vk_entry, &instance.vk_instance);
            if let Ok(dbg) = dbg_res {
                dbg_callback = Some(dbg);
            }
        }

        let physical_gpu =
            PhyDevice::new(instance.clone(), options, display_requirements.is_headless)?;
        benchmark.bench("selecting vk physical device");

        let device = LogicalDevice::new(
            physical_gpu.clone(),
            physical_gpu.queue_node_index,
            &instance.vk_instance,
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

        options: &Options,

        custom_pipes: Option<ArcRwLock<Vec<Box<dyn BackendCustomPipeline>>>>,
    ) -> anyhow::Result<(Self, TTWGraphicsGPUList)> {
        let dbg_mode = options.dbg.gfx; // TODO config / options
        let dbg = Arc::new(AtomicGfxDebugModes::new(dbg_mode));

        // thread count
        let thread_count = (options.gl.thread_count as usize).clamp(
            1,
            std::thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(1).unwrap())
                .get(),
        );

        let (device, phy_gpu, queue, device_instance, dbg_utils_messenger, mut command_pools) =
            Self::init_vulkan_with_native(
                &display_requirements,
                dbg_mode,
                dbg.clone(),
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

                graphics_uniform_buffers: MtPool::with_capacity(
                    GRAPHICS_UNIFORM_INSTANCE_COUNT * 2,
                ),

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

                custom_pipes: VulkanCustomPipes::new(custom_pipes.unwrap_or_default()),
            },
        };
        benchmark.bench("creating initial vk props");

        let gpu_list = res.props.ash_vk.vk_device.phy_device.gpu_list.clone();
        Ok((res, gpu_list))
    }
}

#[derive(Debug, Hiarc)]
pub struct VulkanMainThreadData {
    instance: Arc<Instance>,
    phy_gpu: Arc<PhyDevice>,
    mem_allocator: Arc<parking_lot::Mutex<VulkanAllocator>>,
}

#[derive(Debug, Hiarc)]
pub struct VulkanMainThreadInit {
    surface: BackendSurface,
}

#[derive(Debug, Hiarc)]
pub struct VulkanInUseStreamData {
    pub(crate) cur_stream_vertex_buffer: PoolRc<StreamMemoryBlock<()>>,
    pub(crate) cur_stream_uniform_buffers: PoolRc<StreamMemoryBlock<StreamedUniformBuffer>>,
}

#[derive(Debug, Hiarc)]
pub struct VulkanBackend {
    pub(crate) props: VulkanBackendProps,
    ash_surf: VulkanBackendSurfaceAsh,
    #[hiarc_skip_unsafe]
    runtime_threadpool: Arc<rayon::ThreadPool>,

    pub(crate) in_use_data: VulkanInUseStreamData,

    streamed_vertex_buffers_pool: StreamMemoryPool<()>,
    streamed_uniform_buffers_pool: StreamMemoryPool<StreamedUniformBuffer>,

    pub(crate) render_index_buffer: Arc<Buffer>,
    render_index_buffer_memory: Arc<DeviceMemoryBlock>,
    cur_render_index_primitive_count: u64,

    last_render_thread_index: usize,
    recreate_swap_chain: bool,
    pub(crate) has_dynamic_viewport: bool,
    #[hiarc_skip_unsafe]
    pub(crate) dynamic_viewport_offset: vk::Offset2D,
    #[hiarc_skip_unsafe]
    pub(crate) dynamic_viewport_size: vk::Extent2D,
    cur_render_call_count_in_pipe: usize,

    commands_in_pipe: usize,
    render_calls_in_pipe: usize,

    main_render_command_buffer: Option<AutoCommandBuffer>,
    pub(crate) frame: Arc<parking_lot::Mutex<Frame>>,

    // swapped by use case
    wait_semaphores: Vec<Arc<Semaphore>>,
    sig_semaphores: Vec<Arc<Semaphore>>,

    frame_fences: Vec<Arc<Fence>>,
    image_fences: Vec<Option<Arc<Fence>>>,

    order_id_gen: usize,
    cur_frame: u64,
    image_last_frame_check: Vec<u64>,

    fetch_frame_buffer: Option<VulkanFetchFramebuffer>,
    last_presented_swap_chain_image_index: u32,
    #[hiarc_skip_unsafe]
    frame_fetchers: LinkedHashMap<String, Arc<dyn BackendFrameFetcher>>,
    frame_data_pool: MtPool<Vec<u8>>,

    render_threads: Vec<Arc<RenderThread>>,
    pub(crate) render: RenderSetup,
    pub(crate) multi_sampling_count: u32,
    next_multi_sampling_count: u32,

    cur_semaphore_index: u32,
    pub(crate) cur_image_index: u32,

    canvas_width: f64,
    canvas_height: f64,

    pub(crate) clear_color: [f32; 4],

    pub(crate) current_command_group: ThreadCommandGroup,
    command_groups: Vec<ThreadCommandGroup>,
    pub(crate) current_frame_resources: FrameResources,
    frame_resources: HashMap<u32, FrameResources>,

    frame_resources_pool: FrameResourcesPool,

    pipeline_cache: Option<PipelineCache>,
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

        let alloc_func = |buffer: &Arc<Buffer>,
                          mem_offset: vk::DeviceSize,
                          set_count: usize|
         -> anyhow::Result<Vec<StreamedUniformBuffer>> {
            let mut res: Vec<StreamedUniformBuffer> = Vec::with_capacity(set_count);
            let descr1: Vec<Arc<DescriptorSet>> = VulkanAllocator::create_uniform_descriptor_sets(
                device,
                pools,
                sprite_descr_layout,
                set_count,
                buffer,
                GRAPHICS_MAX_UNIFORM_RENDER_COUNT * GRAPHICS_DEFAULT_UNIFORM_SIZE,
                mem_offset,
            )?
            .into_iter()
            .flat_map(|sets| split_descriptor_sets(&sets))
            .collect();
            let descr2: Vec<Arc<DescriptorSet>> = VulkanAllocator::create_uniform_descriptor_sets(
                device,
                pools,
                quad_descr_layout,
                set_count,
                buffer,
                GRAPHICS_MAX_UNIFORM_RENDER_COUNT * GRAPHICS_DEFAULT_UNIFORM_SIZE,
                mem_offset,
            )?
            .into_iter()
            .flat_map(|sets| split_descriptor_sets(&sets))
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
    fn command_cb_misc(&mut self, cmd_param: CommandsMisc) -> anyhow::Result<()> {
        match cmd_param {
            CommandsMisc::TextureCreate(cmd) => self.cmd_texture_create(cmd),
            CommandsMisc::TextureDestroy(cmd) => self.cmd_texture_destroy(&cmd),
            CommandsMisc::TextureUpdate(cmd) => self.cmd_texture_update(&cmd),
            CommandsMisc::CreateBufferObject(cmd) => self.cmd_create_buffer_object(cmd),
            CommandsMisc::RecreateBufferObject(cmd) => self.cmd_recreate_buffer_object(cmd),
            CommandsMisc::UpdateBufferObject(cmd) => self.cmd_update_buffer_object(cmd),
            CommandsMisc::DeleteBufferObject(cmd) => self.cmd_delete_buffer_object(&cmd),
            CommandsMisc::IndicesForQuadsRequiredNotify(cmd) => {
                self.cmd_indices_required_num_notify(&cmd)
            }
            CommandsMisc::Swap => self.cmd_swap(),
            CommandsMisc::NextSwitchPass => self.cmd_switch_to_switching_passes(),
            CommandsMisc::ConsumeMultiSamplingTargets => self.cmd_consume_multi_sampling_targets(),
            CommandsMisc::SwitchCanvas(cmd) => self.cmd_switch_canvas_mode(cmd),
            CommandsMisc::UpdateViewport(cmd) => self.cmd_update_viewport(&cmd),
            CommandsMisc::Multisampling => todo!(),
            CommandsMisc::VSync => todo!(),
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
            CommandsRender::Mod(CommandsRenderMod { mod_name, cmd }) => {
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
                        presented_img_data_helper_image.img(&mut FrameResources::new(None)),
                    )
            };

            let mut mem_alloc_info = vk::MemoryAllocateInfo::default();
            mem_alloc_info.allocation_size = mem_requirements.size;
            mem_alloc_info.memory_type_index = self.props.device.mem.find_memory_type(
                self.props.vk_gpu.cur_device,
                mem_requirements.memory_type_bits,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
            )?;

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

            let sub_resource = vk::ImageSubresource::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .mip_level(0)
                .array_layer(0);
            let sub_resource_layout = unsafe {
                self.props
                    .ash_vk
                    .vk_device
                    .device
                    .get_image_subresource_layout(
                        presented_img_data_helper_image.img(&mut FrameResources::new(None)),
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
    ) -> anyhow::Result<BackendPresentedImageData, FetchCanvasError> {
        let width: u32;
        let height: u32;
        let mut dest_data_buff = self.frame_data_pool.new();
        let render = match fetch_index {
            FetchCanvasIndex::Onscreen => &self.render.onscreen,
            FetchCanvasIndex::Offscreen(id) => self
                .render
                .offscreens
                .get(&id)
                .ok_or(FetchCanvasError::CanvasNotFound)?,
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
                        swap_img.img(&mut self.current_frame_resources),
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        fetch_frame_buffer
                            .get_presented_img_data_helper_image
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
                        swap_img.img(&mut self.current_frame_resources),
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        fetch_frame_buffer
                            .get_presented_img_data_helper_image
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

            let command_buffers = [command_buffer];
            let submit_info = vk::SubmitInfo::default().command_buffers(&command_buffers);

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
        } else if !uses_rgba_like_format {
            Err(FetchCanvasError::DriverErr("Swap chain image was not ready to be copied, because it was not in a RGBA like format.".to_string()))
        } else {
            Err(FetchCanvasError::DriverErr(
                "Swap chain image was not ready to be copied.".to_string(),
            ))
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
            let command_buffer = memory_command_buffer.command_buffer;
            drop(memory_command_buffer);

            let command_buffers = [command_buffer];
            let submit_info = vk::SubmitInfo::default().command_buffers(&command_buffers);
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
            RenderPassType::Normal(ty) => match ty {
                RenderPassSubType::Single | RenderPassSubType::Switching2 => {
                    self.start_new_render_pass(RenderPassType::Normal(
                        RenderPassSubType::Switching1,
                    ))?;
                }
                RenderPassSubType::Switching1 => {
                    self.start_new_render_pass(RenderPassType::Normal(
                        RenderPassSubType::Switching2,
                    ))?;
                }
            },
            RenderPassType::MultiSampling => {
                self.start_new_render_pass(RenderPassType::Normal(RenderPassSubType::Switching1))?;
            }
        }
        Ok(())
    }

    fn cmd_consume_multi_sampling_targets(&mut self) -> anyhow::Result<()> {
        // if and only if multi sampling is currently active, start a new render pass
        if let RenderPassType::MultiSampling = self.current_command_group.render_pass {
            self.start_new_render_pass(RenderPassType::Normal(RenderPassSubType::Single))?;
        }
        Ok(())
    }

    fn cmd_switch_canvas_mode(&mut self, cmd: CommandSwitchCanvasMode) -> anyhow::Result<()> {
        let (canvas_index, has_multi_sampling) = match &cmd.mode {
            // even if onscreen has multi-sampling. this is not allowed
            CommandSwitchCanvasModeType::Onscreen => (FrameCanvasIndex::Onscreen, false),
            CommandSwitchCanvasModeType::Offscreen {
                id,
                has_multi_sampling,
                ..
            } => (
                FrameCanvasIndex::Offscreen(*id),
                has_multi_sampling.is_some(),
            ),
        };
        self.new_command_group(
            canvas_index,
            0,
            if has_multi_sampling {
                RenderPassType::MultiSampling
            } else {
                RenderPassType::default()
            },
        );
        let mut frame_g = self.frame.lock();
        let frame = &mut *frame_g;
        match canvas_index {
            FrameCanvasIndex::Onscreen => {}
            FrameCanvasIndex::Offscreen(index) => frame.new_offscreen(index),
        }
        drop(frame_g);
        match &cmd.mode {
            CommandSwitchCanvasModeType::Offscreen {
                id,
                width,
                height,
                has_multi_sampling,
            } => self.render.switch_canvas(CanvasMode::Offscreen {
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
                has_multi_sampling: *has_multi_sampling,
            })?,
            CommandSwitchCanvasModeType::Onscreen => {
                self.render.switch_canvas(CanvasMode::Onscreen)?
            }
        }
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

        FrameCollector::collect(self)?;

        // add frame resources
        self.frame_resources.insert(
            self.cur_image_index,
            self.current_frame_resources
                .take(Some(&self.frame_resources_pool)),
        );

        self.main_render_command_buffer = None;

        let wait_semaphore = self.wait_semaphores[self.cur_semaphore_index as usize].semaphore;

        let mut submit_info = vk::SubmitInfo::default();

        let mut command_buffers: [vk::CommandBuffer; 2] = Default::default();
        command_buffers[0] = command_buffer;

        if let Some(memory_command_buffer) = self.props.device.memory_command_buffer.take() {
            let memory_command_buffer = memory_command_buffer.command_buffer;

            command_buffers[0] = memory_command_buffer;
            command_buffers[1] = command_buffer;
            submit_info = submit_info.command_buffers(&command_buffers[..]);
        } else {
            submit_info = submit_info.command_buffers(&command_buffers[..1]);
        }

        let wait_semaphores = [wait_semaphore];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let signal_semaphores = [self.sig_semaphores[self.cur_semaphore_index as usize].semaphore];
        submit_info = submit_info
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .signal_semaphores(&signal_semaphores);

        let mut timeline_submit_info: vk::TimelineSemaphoreSubmitInfo;
        let wait_counter: [u64; 1];
        let signal_counter: [u64; 1];

        if self.props.device.is_headless && self.ash_surf.surface.can_render() {
            wait_counter = [unsafe {
                self.props
                    .ash_vk
                    .vk_device
                    .device
                    .get_semaphore_counter_value(wait_semaphore)
                    .unwrap()
            }];
            signal_counter = [unsafe {
                self.props
                    .ash_vk
                    .vk_device
                    .device
                    .get_semaphore_counter_value(signal_semaphores[0])
                    .unwrap()
            } + 1];
            timeline_submit_info = vk::TimelineSemaphoreSubmitInfo::default()
                .wait_semaphore_values(&wait_counter)
                .signal_semaphore_values(&signal_counter);
            submit_info = submit_info.push_next(&mut timeline_submit_info);
        } else if !self.ash_surf.surface.can_render() {
            unsafe { self.props.device.ash_vk.device.device.device_wait_idle()? }
        }

        unsafe {
            self.props
                .ash_vk
                .vk_device
                .device
                .reset_fences(&[self.frame_fences[self.cur_semaphore_index as usize].fence])
                .map_err(|err| anyhow!("could not reset fences {err}"))
        }?;

        unsafe {
            let queue = &self.props.queue.queues.lock();
            self.props.ash_vk.vk_device.device.queue_submit(
                queue.graphics_queue,
                &[submit_info],
                self.frame_fences[self.cur_semaphore_index as usize].fence,
            )
        }
        .map_err(|err| anyhow!("Submitting to graphics queue failed: {err}"))?;

        std::mem::swap(
            &mut self.wait_semaphores[self.cur_semaphore_index as usize],
            &mut self.sig_semaphores[self.cur_semaphore_index as usize],
        );

        let image_indices = [self.cur_image_index];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&signal_semaphores)
            .image_indices(&image_indices);

        self.last_presented_swap_chain_image_index = self.cur_image_index;

        let queue_present_res = unsafe {
            let queue = &self.props.queue.queues.lock();
            self.ash_surf
                .vk_swap_chain_ash
                .queue_present(queue.present_queue, present_info)
        };

        let (needs_recreate, is_err) = if queue_present_res
            .is_err_and(|err| err == vk::Result::ERROR_OUT_OF_DATE_KHR)
        {
            (true, true)
        } else if queue_present_res.is_err_and(|err| err == vk::Result::ERROR_SURFACE_LOST_KHR) {
            let surface = self.create_fake_surface()?;
            self.reinit_vulkan_swap_chain(|_| &surface)?;
            self.ash_surf.surface.replace(surface);
            self.recreate_swap_chain = false;
            self.prepare_frame()?;
            (false, true)
        } else {
            (
                queue_present_res
                    .map_err(|err| anyhow!("Presenting graphics queue failed: {err}"))?,
                false,
            )
        };

        if needs_recreate {
            self.recreate_swap_chain = true;
        }

        self.cur_semaphore_index =
            (self.cur_semaphore_index + 1) % self.sig_semaphores.len() as u32;

        if !is_err && !self.frame_fetchers.is_empty() {
            // TODO: removed cloning
            let keys: Vec<String> = self.frame_fetchers.keys().cloned().collect();
            for i in keys.iter() {
                // get current frame and fill the frame fetcher with it
                let fetch_index = self.frame_fetchers.get(i).unwrap().current_fetch_index();
                let img_data = self.get_presented_image_data_impl(false, false, fetch_index);
                if let Ok(img_data) = img_data {
                    let frame_fetcher = self.frame_fetchers.get(i).unwrap();
                    frame_fetcher.next_frame(img_data);
                }
            }
        }

        Ok(())
    }

    fn prepare_frame(&mut self) -> anyhow::Result<()> {
        if self.recreate_swap_chain {
            self.recreate_swap_chain = false;
            if is_verbose(&self.props.dbg) {
                info!("recreating swap chain requested by user (prepare frame).");
            }
            self.recreate_swap_chain()?;
        }

        let acquire_res = unsafe {
            self.ash_surf.vk_swap_chain_ash.acquire_next_image(
                u64::MAX,
                self.sig_semaphores[self.cur_semaphore_index as usize].semaphore,
                vk::Fence::null(),
            )
        };

        if acquire_res.is_err_and(|err| err == vk::Result::ERROR_OUT_OF_DATE_KHR) {
            self.recreate_swap_chain = false;
            if is_verbose(&self.props.dbg) {
                info!("recreating swap chain requested by acquire next image (prepare frame).");
            }
            self.recreate_swap_chain()?;
            return self.prepare_frame();
        } else if acquire_res.is_err_and(|err| err == vk::Result::ERROR_SURFACE_LOST_KHR) {
            let surface = self.create_fake_surface()?;
            self.reinit_vulkan_swap_chain(|_| &surface)?;
            self.ash_surf.surface.replace(surface);
            self.recreate_swap_chain = false;
            self.prepare_frame()?;
            return Ok(());
        }

        let (next_image_index, is_suboptimal) =
            acquire_res.map_err(|err| anyhow!("Acquiring next image failed: {err}"))?;
        if is_suboptimal {
            self.recreate_swap_chain = true;
            if is_verbose(&self.props.dbg) {
                info!("recreating swap chain requested by acquire next image (prepare frame).");
            }
        }

        self.cur_image_index = next_image_index;
        std::mem::swap(
            &mut self.wait_semaphores[self.cur_semaphore_index as usize],
            &mut self.sig_semaphores[self.cur_semaphore_index as usize],
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
            Some(self.frame_fences[self.cur_semaphore_index as usize].clone());

        // next frame
        self.cur_frame += 1;
        self.order_id_gen = 0;
        self.image_last_frame_check[self.cur_image_index as usize] = self.cur_frame;
        self.current_command_group = Default::default();
        self.current_command_group.render_pass = if self.render.onscreen.multi_sampling.is_some() {
            RenderPassType::MultiSampling
        } else {
            RenderPassType::default()
        };
        self.current_command_group.cur_frame_index = self.cur_image_index;
        self.current_command_group.canvas_index = Default::default();
        self.render.new_frame(&mut self.current_frame_resources)?;

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
        if self.ash_surf.surface.can_render() {
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
        data: &[u8],
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

        let tex = self
            .props
            .device
            .textures
            .get(&texture_slot)
            .ok_or(anyhow!("texture with that index does not exist"))?;
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
        let sync_object_count = self.render.onscreen.swap_chain_image_count() + 1;
        for _ in 0..sync_object_count {
            self.wait_semaphores.push(Semaphore::new(
                self.props.ash_vk.vk_device.clone(),
                self.props.device.is_headless,
            )?)
        }
        for _ in 0..sync_object_count {
            self.sig_semaphores.push(Semaphore::new(
                self.props.ash_vk.vk_device.clone(),
                self.props.device.is_headless,
            )?)
        }

        for _ in 0..sync_object_count {
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

        self.frame_fences.clear();
        self.image_fences.clear();

        self.cur_semaphore_index = 0;
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

        if is_verbose(&self.props.dbg) {
            info!("recreating swap chain.");
        }

        let old_swap_chain_image_count = self.render.onscreen.swap_chain_image_count();

        // set new multi sampling if it was requested
        if self.next_multi_sampling_count != u32::MAX {
            self.multi_sampling_count = self.next_multi_sampling_count;
            self.next_multi_sampling_count = u32::MAX;
        }

        self.reinit_vulkan_swap_chain(|s| s)?;

        if old_swap_chain_image_count != self.render.onscreen.swap_chain_image_count() {
            self.cleanup_vulkan::<false>();
            self.init_vulkan()?;
        }

        Ok(())
    }

    fn reinit_vulkan_swap_chain<'a>(
        &'a mut self,
        surf_func: impl FnOnce(&'a BackendSurface) -> &'a BackendSurface,
    ) -> anyhow::Result<()> {
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
                surf_func(&self.ash_surf.surface),
                &mut self.ash_surf.vk_swap_chain_ash,
                &super::swapchain::SwapchainCreateOptions {
                    vsync: self.props.gfx_vsync,
                },
                &self.props.dbg,
                (self.canvas_width as u32, self.canvas_height as u32),
            )?,
            &self.ash_surf.vk_swap_chain_ash,
            ShaderCompiler::new_with_files(ty, fs, shader_files),
            true,
            (self.multi_sampling_count > 0).then_some(self.multi_sampling_count),
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
        match cmd.texture_index {
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
            if is_verbose(&self.props.dbg) {
                info!("queueing swap chain recreation because the viewport changed");
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
                self.dynamic_viewport_offset = vk::Offset2D { x: cmd.x, y: cmd.y };
                self.dynamic_viewport_size = vk::Extent2D {
                    width: cmd.width,
                    height: cmd.height,
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

    fn cmd_update_buffer_object(&mut self, cmd: CommandUpdateBufferObject) -> anyhow::Result<()> {
        let update_buffer: Vec<u8> = cmd.update_data;
        let copy_regions = cmd.update_regions;
        anyhow::ensure!(
            !copy_regions.is_empty(),
            anyhow!("copy regions shall not be empty.")
        );
        anyhow::ensure!(
            !copy_regions.iter().any(|region| region.size == 0),
            anyhow!("copy regions sizes must be bigger than zero.")
        );

        let mut staging_allocation = self.props.device.mem_allocator.lock().get_staging_buffer(
            update_buffer.as_ptr() as _,
            update_buffer.len() as vk::DeviceSize,
        );

        if let Err(_) = staging_allocation {
            self.skip_frames_until_current_frame_is_used_again()?;
            staging_allocation = self.props.device.mem_allocator.lock().get_staging_buffer(
                update_buffer.as_ptr() as _,
                update_buffer.len() as vk::DeviceSize,
            );
        }
        let staging_buffer = staging_allocation?;

        let buffer = self
            .props
            .device
            .buffer_objects
            .get(&cmd.buffer_index)
            .ok_or(anyhow!("buffer object with that index does not exist"))?;
        let cur_buffer = buffer.cur_buffer.clone();
        let dst_buffer_align = buffer.buffer_object.mem.heap_data.offset_to_align;
        let src_buffer = staging_buffer
            .buffer(&mut self.current_frame_resources)
            .clone()
            .ok_or(anyhow!("staging mem had no buffer attached to it"))?;

        let min_dst_off = copy_regions
            .iter()
            .map(|region| region.dst_offset)
            .min()
            .unwrap();
        let max_dst_off = copy_regions
            .iter()
            .map(|region| region.dst_offset + region.size)
            .max()
            .unwrap();
        self.props.device.memory_barrier(
            &mut self.current_frame_resources,
            &cur_buffer,
            min_dst_off as vk::DeviceSize + dst_buffer_align as vk::DeviceSize,
            (max_dst_off - min_dst_off) as vk::DeviceSize,
            vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            true,
        )?;
        self.props.device.copy_buffer(
            &mut self.current_frame_resources,
            &src_buffer,
            &cur_buffer,
            &copy_regions
                .into_iter()
                .map(|region| vk::BufferCopy {
                    src_offset: staging_buffer.heap_data.offset_to_align as vk::DeviceSize
                        + region.src_offset as vk::DeviceSize,
                    dst_offset: region.dst_offset as vk::DeviceSize
                        + dst_buffer_align as vk::DeviceSize,
                    size: region.size as vk::DeviceSize,
                })
                .collect::<Vec<_>>(),
        )?;
        self.props.device.memory_barrier(
            &mut self.current_frame_resources,
            &cur_buffer,
            min_dst_off as vk::DeviceSize + dst_buffer_align as vk::DeviceSize,
            (max_dst_off - min_dst_off) as vk::DeviceSize,
            vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            false,
        )?;
        self.props
            .device
            .upload_and_free_staging_mem_block(&mut self.current_frame_resources, staging_buffer);

        Ok(())
    }

    fn cmd_delete_buffer_object(&mut self, cmd: &CommandDeleteBufferObject) -> anyhow::Result<()> {
        let buffer_index = cmd.buffer_index;
        self.props.device.delete_buffer_object(buffer_index);

        Ok(())
    }

    fn cmd_indices_required_num_notify(
        &mut self,
        cmd: &CommandIndicesForQuadsRequiredNotify,
    ) -> anyhow::Result<()> {
        let quad_count = cmd.quad_count_required;
        if self.cur_render_index_primitive_count < quad_count {
            let mut upload_indices = Vec::<u32>::new();
            upload_indices.resize((quad_count * 6) as usize, Default::default());
            let mut primitive_count: u32 = 0;
            for i in (0..(quad_count as usize * 6)).step_by(6) {
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
            self.cur_render_index_primitive_count = quad_count;
        }

        Ok(())
    }

    fn buffer_object_fill_execute_buffer(
        render_execute_manager: &mut RenderCommandExecuteManager,
        state: &State,
        texture_index: &StateTexture,
        buffer_object_index: u128,
        draw_calls: usize,
    ) {
        render_execute_manager.set_vertex_buffer(buffer_object_index);

        let address_mode_index: usize = get_address_mode_index(state);
        match texture_index {
            StateTexture::Texture(texture_index) => {
                render_execute_manager.set_texture(0, *texture_index, address_mode_index as u64);
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

        render_execute_manager.exec_buffer_fill_dynamic_states(state);
    }

    fn cmd_render_quad_container_ex_fill_execute_buffer(
        render_execute_manager: &mut RenderCommandExecuteManager,
        cmd: &CommandRenderQuadContainer,
    ) {
        Self::buffer_object_fill_execute_buffer(
            render_execute_manager,
            &cmd.state,
            &cmd.texture_index,
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
            &cmd.texture_index,
            cmd.buffer_object_index,
            ((cmd.instance_count - 1) / GRAPHICS_MAX_UNIFORM_RENDER_COUNT) + 1,
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
    ) -> anyhow::Result<InitialIndexBuffer> {
        let mut indices_upload: Vec<u32> =
            Vec::with_capacity(StreamDataMax::MaxVertices as usize / 4 * 6);
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
        surface: BackendSurfaceAndHandles,
        instance: &ash::Instance,
        phy_gpu: &vk::PhysicalDevice,
        queue_family_index: u32,
        mem_allocator: &Arc<parking_lot::Mutex<VulkanAllocator>>,
    ) -> anyhow::Result<BackendSurface> {
        let surface = unsafe { surface.create_vk_surface(entry, instance, mem_allocator) }?;

        let is_supported =
            unsafe { surface.get_physical_device_surface_support(*phy_gpu, queue_family_index) }?;
        if !is_supported {
            return Err(anyhow!("The device surface does not support presenting the framebuffer to a screen. (maybe the wrong GPU was selected?)"));
        }

        Ok(surface)
    }

    pub fn get_main_thread_data(&self) -> VulkanMainThreadData {
        let phy_gpu = self.props.ash_vk.vk_device.phy_device.clone();
        let instance = phy_gpu.instance.clone();
        VulkanMainThreadData {
            instance,
            mem_allocator: self.props.device.mem_allocator.clone(),
            phy_gpu,
        }
    }

    pub fn main_thread_data(loading: &VulkanBackendLoading) -> VulkanMainThreadData {
        let instance = loading.props.ash_vk.vk_device.phy_device.instance.clone();
        let phy_gpu = loading.props.ash_vk.vk_device.phy_device.clone();
        let mem_allocator = loading.props.device.mem_allocator.clone();
        VulkanMainThreadData {
            instance,
            mem_allocator,
            phy_gpu,
        }
    }

    fn create_fake_surface(&self) -> anyhow::Result<BackendSurface> {
        let phy_gpu = &self.props.ash_vk.vk_device.phy_device;
        let instance = &phy_gpu.instance;
        unsafe {
            BackendWindow::create_fake_headless_surface().create_vk_surface(
                &instance.vk_entry,
                &instance.vk_instance,
                &self.props.device.mem_allocator,
            )
        }
    }

    pub fn try_create_surface_impl(
        instance: &Arc<Instance>,
        phy_gpu: &Arc<PhyDevice>,
        mem_allocator: &Arc<parking_lot::Mutex<VulkanAllocator>>,
        window: &BackendWindow,
        dbg: &ConfigDebug,
    ) -> anyhow::Result<BackendSurface> {
        let benchmark = Benchmark::new(dbg.bench);
        let surface = window.create_surface(&instance.vk_entry, &instance.vk_instance)?;
        let surface = Self::create_surface(
            &instance.vk_entry,
            surface,
            &instance.vk_instance,
            &phy_gpu.cur_device,
            phy_gpu.queue_node_index,
            mem_allocator,
        )?;
        benchmark.bench("creating vk surface");

        Ok(surface)
    }

    pub fn init_from_main_thread(
        data: VulkanMainThreadData,
        window: &BackendWindow,
        dbg: &ConfigDebug,
    ) -> anyhow::Result<VulkanMainThreadInit> {
        Ok(VulkanMainThreadInit {
            surface: Self::try_create_surface_impl(
                &data.instance,
                &data.phy_gpu,
                &data.mem_allocator,
                window,
                dbg,
            )?,
        })
    }

    pub fn set_from_main_thread(&mut self, data: VulkanMainThreadInit) -> anyhow::Result<()> {
        self.wait_frame()?;
        self.reinit_vulkan_swap_chain(|_| &data.surface)?;
        self.ash_surf.surface.replace(data.surface);
        self.recreate_swap_chain = false;
        self.prepare_frame()?;
        Ok(())
    }
    pub fn surface_lost(&mut self) -> anyhow::Result<()> {
        self.wait_frame()?;
        let surface = self.create_fake_surface()?;
        self.reinit_vulkan_swap_chain(|_| &surface)?;
        self.ash_surf.surface.replace(surface);
        self.recreate_swap_chain = false;
        self.prepare_frame()?;
        Ok(())
    }

    pub fn new(
        mut loading: VulkanBackendLoading,
        loaded_io: VulkanBackendLoadedIo,
        runtime_threadpool: &Arc<rayon::ThreadPool>,

        main_thread_data: VulkanMainThreadInit,
        canvas_width: f64,
        canvas_height: f64,
        options: &Options,

        write_files: BackendWriteFiles,
    ) -> anyhow::Result<Box<Self>> {
        let benchmark = Benchmark::new(options.dbg.bench);

        let phy_gpu = &loading.props.ash_vk.vk_device.phy_device;
        let instance = &loading.props.ash_vk.vk_device.phy_device.instance;
        let surface = main_thread_data.surface;

        // thread count
        let thread_count = loading.props.thread_count;

        let shader_compiler = loaded_io.shader_compiler;
        benchmark.bench("getting compiled shaders");

        let pipeline_cache = PipelineCache::new(
            loading.props.device.ash_vk.device.clone(),
            loaded_io.pipeline_cache.as_ref(),
            write_files,
        )
        .ok();
        benchmark.bench("creating the pipeline cache");

        let mut swap_chain = surface.create_swapchain(
            &instance.vk_instance, /* TODO: use the wrapper func */
            &loading.props.ash_vk.vk_device.device,
            &loading.props.queue,
        )?;
        benchmark.bench("creating vk swap chain");

        let multi_sampling_count = options.gl.msaa_samples & 0xFFFFFFFE; // ignore the uneven bit, only even multi sampling works

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
                &loading.props.dbg,
                (canvas_width as u32, canvas_height as u32),
            )?,
            &swap_chain,
            shader_compiler,
            true,
            (multi_sampling_count > 0).then_some(multi_sampling_count),
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
            GRAPHICS_UNIFORM_INSTANCE_COUNT,
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

            in_use_data: VulkanInUseStreamData {
                cur_stream_vertex_buffer,
                cur_stream_uniform_buffers,
            },

            render_threads: Default::default(),
            render,

            multi_sampling_count,
            next_multi_sampling_count: Default::default(),

            render_index_buffer,
            render_index_buffer_memory,
            cur_render_index_primitive_count: index_prim_count as u64,

            cur_render_call_count_in_pipe: Default::default(),
            commands_in_pipe: Default::default(),
            render_calls_in_pipe: Default::default(),

            last_render_thread_index: Default::default(),

            recreate_swap_chain: Default::default(),
            has_dynamic_viewport: Default::default(),
            dynamic_viewport_offset: Default::default(),
            dynamic_viewport_size: Default::default(),

            main_render_command_buffer: Default::default(),
            wait_semaphores: Default::default(),
            sig_semaphores: Default::default(),
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

            cur_semaphore_index: Default::default(),
            cur_image_index: Default::default(),
            canvas_width,
            canvas_height,
            clear_color: [
                options.gl.clear_color.r as f32 / 255.0,
                options.gl.clear_color.g as f32 / 255.0,
                options.gl.clear_color.b as f32 / 255.0,
                1.0,
            ],

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
        res.in_use_data.cur_stream_vertex_buffer =
            res.streamed_vertex_buffers_pool.try_get(1).unwrap();
        benchmark.bench("creating initial stream vertex buffers");
        res.uniform_stream_alloc_func(GRAPHICS_UNIFORM_INSTANCE_COUNT * 4 * 2)?;
        res.in_use_data.cur_stream_uniform_buffers = res
            .streamed_uniform_buffers_pool
            .try_get(GRAPHICS_UNIFORM_INSTANCE_COUNT)
            .unwrap();
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
        frame: Arc<parking_lot::Mutex<Frame>>,
        device: Arc<LogicalDevice>,
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

                    let resources = frame_resources
                        .entry(cmd_group.cur_frame_index)
                        .or_insert_with(|| frame_resources_pool.new());
                    resources.push(frame_resource);
                }
                if has_error_from_cmd {
                    panic!("TODO:")
                }
            }

            benchmark.bench("vulkan render thread");
        }
    }

    pub fn get_stream_data(&mut self) -> anyhow::Result<VulkanInUseStreamData> {
        let cur_stream_vertex_buffer = self
            .streamed_vertex_buffers_pool
            .get(|_, _, set_count: usize| Ok(vec![(); set_count]), 1)?;

        self.uniform_stream_alloc_func(GRAPHICS_UNIFORM_INSTANCE_COUNT)?;
        let cur_stream_uniform_buffers = self
            .streamed_uniform_buffers_pool
            .try_get(GRAPHICS_UNIFORM_INSTANCE_COUNT)
            .ok_or_else(|| anyhow!("stream uniform buffer pool returned None"))?;

        Ok(VulkanInUseStreamData {
            cur_stream_vertex_buffer,
            cur_stream_uniform_buffers,
        })
    }
    pub fn set_stream_data_in_use(
        &mut self,
        stream_data: &GraphicsStreamedData,
        data: &VulkanInUseStreamData,
    ) -> anyhow::Result<()> {
        self.current_frame_resources
            .stream_vertex_buffers
            .push(data.cur_stream_vertex_buffer.clone());

        self.current_frame_resources
            .stream_uniform_buffers
            .push(data.cur_stream_uniform_buffers.clone());

        data.cur_stream_vertex_buffer.memories[0].flush(
            &mut self.current_frame_resources,
            self.props.vk_gpu.limits.non_coherent_mem_alignment,
            stream_data.vertices_count() * std::mem::size_of::<GlVertex>(),
            &mut self.props.device.non_flushed_memory_ranges,
        );
        let uniform_instance_count = stream_data.uniform_instance_count();
        for i in 0..uniform_instance_count {
            let usage_count = stream_data.uniform_used_count_of_instance(i);
            data.cur_stream_uniform_buffers.memories[i].flush(
                &mut self.current_frame_resources,
                self.props.vk_gpu.limits.non_coherent_mem_alignment,
                match usage_count {
                    GraphicsStreamedUniformDataType::Arbitrary {
                        element_size,
                        element_count,
                    } => element_count * element_size,
                    GraphicsStreamedUniformDataType::None => 0,
                },
                &mut self.props.device.non_flushed_memory_ranges,
            );
        }

        self.in_use_data = VulkanInUseStreamData {
            cur_stream_vertex_buffer: data.cur_stream_vertex_buffer.clone(),
            cur_stream_uniform_buffers: data.cur_stream_uniform_buffers.clone(),
        };

        Ok(())
    }

    pub fn create_mt_backend(data: &VulkanMainThreadData) -> VulkanBackendMt {
        VulkanBackendMt {
            mem_allocator: data.mem_allocator.clone(),
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
            .map_err(|err| match err {
                FetchCanvasError::CanvasNotFound => anyhow!("onscreen canvas not found... weird."),
                FetchCanvasError::DriverErr(err) => anyhow!(err),
            })
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
            self.fill_execute_buffer(render_cmd, &mut buffer);
            self.cur_render_call_count_in_pipe += buffer.estimated_render_call_count;
        }
        let mut is_misc_cmd = false;
        if let AllCommands::Misc(_) = cmd {
            is_misc_cmd = true;
        }
        if is_misc_cmd {
            if let AllCommands::Misc(cmd) = cmd {
                self.command_cb_misc(cmd)?;
            }
        } else if self.ash_surf.surface.can_render() {
            if let AllCommands::Render(render_cmd) = cmd {
                buffer.raw_render_command = Some(render_cmd)
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

    fn start_commands(&mut self, command_count: usize, estimated_render_call_count: usize) {
        self.commands_in_pipe = command_count;
        self.render_calls_in_pipe = estimated_render_call_count;
        self.cur_render_call_count_in_pipe = 0;
    }

    fn end_commands(&mut self) -> anyhow::Result<()> {
        self.commands_in_pipe = 0;
        self.render_calls_in_pipe = 0;
        self.last_render_thread_index = 0;

        Ok(())
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

#[derive(Debug, Hiarc)]
pub struct VulkanBackendMt {
    pub mem_allocator: Arc<parking_lot::Mutex<VulkanAllocator>>,
    pub flush_lock: parking_lot::Mutex<()>,
}

#[derive(Debug)]
pub struct VulkanBackendDellocator {
    pub mem_allocator: Arc<parking_lot::Mutex<VulkanAllocator>>,
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
                assert!(
                    required_size > 0,
                    "an allocation of zero size is not allowed."
                );
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
                assert!(
                    width * height * depth > 0,
                    "an allocation of zero size is not allowed."
                );
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
