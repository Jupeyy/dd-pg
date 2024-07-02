use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{atomic::AtomicU64, Arc},
};

use base_io::io::IoFileSys;
use config::config::{ConfigBackend, ConfigDebug, ConfigGfx};
use graphics_backend_traits::{
    frame_fetcher_plugin::{BackendFrameFetcher, BackendPresentedImageData},
    plugin::BackendCustomPipeline,
    traits::{GraphicsBackendInterface, GraphicsBackendMtInterface},
    types::BackendCommands,
};
use graphics_base_traits::traits::{GraphicsStreamVertices, GraphicsStreamedData};
use hiarc::Hiarc;
use pool::{mixed_pool::PoolSyncPoint, mt_datatypes::PoolVec};

use crate::{
    backend_thread::{BackendThread, BackendThreadInitData},
    backends::vulkan::vulkan::{VulkanBackendLoadedIo, VulkanBackendLoadingIo},
    window::{BackendDisplayRequirements, BackendRawDisplayHandle, BackendWindow},
};

use native::native::NativeImpl;

use base::{benchmark::Benchmark, system::System};

use super::backend_mt::GraphicsBackendMultiThreaded;

use graphics_types::{
    commands::{AllCommands, CommandUpdateViewport, Commands},
    rendering::{StateTexture, StateTexture2dArray},
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType, WindowProps},
};

#[derive(Debug)]
enum GraphicsBackendLoadingIoType {
    Vulkan(VulkanBackendLoadingIo),
    Null,
}

#[derive(Debug)]
pub struct GraphicsBackendIoLoading {
    backend_io: GraphicsBackendLoadingIoType,
}

impl GraphicsBackendIoLoading {
    pub fn new(config_gfx: &ConfigGfx, io: &IoFileSys) -> Self {
        Self {
            backend_io: match config_gfx.backend.to_ascii_lowercase().as_str() {
                "vulkan" => GraphicsBackendLoadingIoType::Vulkan(VulkanBackendLoadingIo::new(io)),
                _ => GraphicsBackendLoadingIoType::Null,
            },
        }
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct GraphicsBackendMemoryUsage {
    pub texture_memory_usage: Arc<AtomicU64>,
    pub buffer_memory_usage: Arc<AtomicU64>,
    pub stream_memory_usage: Arc<AtomicU64>,
    pub staging_memory_usage: Arc<AtomicU64>,
}

#[derive(Debug)]
pub struct GraphicsBackendLoading {
    memory_usage: GraphicsBackendMemoryUsage,

    backend: BackendThread,

    custom_pipes: Option<Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>>,
}

impl GraphicsBackendLoading {
    pub fn new(
        config_gfx: &ConfigGfx,
        config_dbg: &ConfigDebug,
        config_gl: &ConfigBackend,
        sys: &System,
        raw_display_handle: BackendRawDisplayHandle,
        custom_pipes: Option<Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>>,
        io: IoFileSys,
    ) -> anyhow::Result<Self> {
        let backend = &config_gfx.backend;

        let benchmark = Benchmark::new(config_dbg.bench);

        let texture_memory_usage: Arc<AtomicU64> = Default::default();
        let buffer_memory_usage: Arc<AtomicU64> = Default::default();
        let stream_memory_usage: Arc<AtomicU64> = Default::default();
        let staging_memory_usage: Arc<AtomicU64> = Default::default();

        let backend = BackendThread::new(
            backend.clone(),
            BackendDisplayRequirements {
                extensions: raw_display_handle.enumerate_required_vk_extensions()?,
                is_headless: raw_display_handle.is_headless(),
            },
            config_dbg.clone(),
            config_gl.clone(),
            sys.clone(),
            custom_pipes.clone(),
            texture_memory_usage.clone(),
            buffer_memory_usage.clone(),
            stream_memory_usage.clone(),
            staging_memory_usage.clone(),
            io,
        );
        benchmark.bench("initializing the backend instance");

        Ok(Self {
            memory_usage: GraphicsBackendMemoryUsage {
                texture_memory_usage,
                buffer_memory_usage,
                stream_memory_usage,
                staging_memory_usage,
            },

            backend,
            custom_pipes,
        })
    }
}

#[derive(Debug, Hiarc)]
pub struct GraphicsBackendBase {
    backend: BackendThread,
    backend_mt: Arc<GraphicsBackendMultiThreaded>,

    backend_cmds_in_use: Vec<AllCommands>,

    window_props: WindowProps,
    memory_usage: GraphicsBackendMemoryUsage,

    #[hiarc_skip_unsafe]
    custom_pipes: Option<Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>>,
    pipeline_names: HashMap<String, usize>,
}

impl GraphicsBackendBase {
    /// returns the base and the stream_data
    pub fn new(
        io_loading: GraphicsBackendIoLoading,
        backend_loading: GraphicsBackendLoading,
        runtime_threadpool: &Arc<rayon::ThreadPool>,
        window: BackendWindow,
        config_dbg: &ConfigDebug,
        config_gl: &ConfigBackend,
    ) -> anyhow::Result<(Self, GraphicsStreamedData)> {
        let benchmark = Benchmark::new(config_dbg.bench);

        let window = &window;
        let size = window.inner_size();
        let scale_factor = window.scale_factor();

        // get window & canvas properties
        let (window_width, window_height) = (size.width, size.height);

        let (canvas_width, canvas_height) = (
            size.width as f64 / scale_factor,
            size.height as f64 / scale_factor,
        );

        let backend_mt = backend_loading.backend.init(
            match io_loading.backend_io {
                GraphicsBackendLoadingIoType::Vulkan(data) => BackendThreadInitData::Vulkan {
                    data: VulkanBackendLoadedIo {
                        shader_compiler: data.shader_compiler.get_storage()?,
                        pipeline_cache: data.pipeline_cache.get_storage()?,
                    },
                    runtime_threadpool: runtime_threadpool.clone(),
                    canvas_width,
                    canvas_height,
                    dbg: config_dbg.clone(),
                    gl: config_gl.clone(),
                },
                GraphicsBackendLoadingIoType::Null => BackendThreadInitData::Null,
            },
            config_dbg,
            window,
        )?;
        benchmark.bench("gl backend loading");

        let backend_mt = Arc::new(GraphicsBackendMultiThreaded { backend_mt });

        // clear first frame
        let cmd_swap = Commands::Swap;

        let buffer = BackendCommands::default();
        buffer.add_cmd(AllCommands::Misc(cmd_swap));
        let stream_data: GraphicsStreamedData = GraphicsStreamedData::new(
            GraphicsStreamVertices::Static(&mut []),
            PoolVec::new_without_pool(),
        );

        let mut pipeline_names: HashMap<String, usize> = Default::default();
        if let Some(custom_pipes) = &backend_loading.custom_pipes {
            let pipes = custom_pipes.read();
            for (index, pipe) in pipes.iter().enumerate() {
                pipeline_names.insert(pipe.pipe_name(), index);
            }
        }

        let mut res = GraphicsBackendBase {
            backend: backend_loading.backend,
            backend_mt,

            backend_cmds_in_use: Default::default(),

            window_props: WindowProps {
                window_width,
                window_height,
                canvas_width,
                canvas_height,
            },
            memory_usage: backend_loading.memory_usage,

            custom_pipes: backend_loading.custom_pipes,
            pipeline_names,
        };
        res.run_cmds(&buffer, &stream_data)?;
        benchmark.bench("gl first swap");

        Ok((res, stream_data))
    }

    fn run_cmds_impl(
        &mut self,
        buffer: &BackendCommands,
        stream_data: &GraphicsStreamedData,
    ) -> anyhow::Result<()> {
        self.backend_cmds_in_use.clear();
        buffer.replace(&mut self.backend_cmds_in_use);

        self.backend
            .run_cmds(stream_data, &mut self.backend_cmds_in_use)?;

        Ok(())
    }

    pub fn get_window_props(&self) -> WindowProps {
        self.window_props
    }

    #[must_use]
    fn resized(
        &mut self,
        buffer: &BackendCommands,
        stream_data: &GraphicsStreamedData,
        window_handling: &dyn NativeImpl,
        new_width: u32,
        new_height: u32,
    ) -> WindowProps {
        // TODO make sure backend is idle

        let cmd_viewport = Commands::UpdateViewport(CommandUpdateViewport {
            x: 0,
            y: 0,
            width: new_width,
            height: new_height,
            by_resize: true,
        });

        buffer.add_cmd(AllCommands::Misc(cmd_viewport));
        self.run_cmds_impl(buffer, stream_data).unwrap(); // TODO: unwrap here?

        let inner_size = window_handling.borrow_window().inner_size();
        let scale_factor = window_handling.borrow_window().scale_factor();

        self.window_props.window_width = new_width;
        self.window_props.window_height = new_height;
        self.window_props.canvas_width = inner_size.width as f64 / scale_factor;
        self.window_props.canvas_height = inner_size.height as f64 / scale_factor;

        self.window_props
    }

    fn run_cmds(
        &mut self,
        buffer: &BackendCommands,
        stream_data: &GraphicsStreamedData,
    ) -> anyhow::Result<()> {
        self.run_cmds_impl(buffer, stream_data)
    }

    fn mem_alloc(&self, alloc_type: GraphicsMemoryAllocationType) -> GraphicsBackendMemory {
        self.backend_mt.mem_alloc(alloc_type)
    }

    fn do_screenshot(&mut self) -> anyhow::Result<BackendPresentedImageData> {
        self.backend.do_screenshot()
    }

    fn attach_frame_fetcher(&mut self, name: String, fetcher: Arc<dyn BackendFrameFetcher>) {
        self.backend.attach_frame_fetcher(name, fetcher)
    }

    fn detach_frame_fetcher(&mut self, name: String) {
        self.backend.detach_frame_fetcher(name)
    }

    fn get_backend_mt(&self) -> Arc<dyn GraphicsBackendMtInterface + Sync + Send + 'static> {
        self.backend_mt.clone()
    }

    fn add_sync_point(&mut self, sync_point: Box<dyn PoolSyncPoint>) {
        self.backend.add_sync_point(sync_point);
    }

    fn check_mod_cmd(
        &self,
        mod_name: &str,
        cmd: &mut PoolVec<u8>,
        f: &dyn Fn(&mut [&mut StateTexture], &mut [&mut StateTexture2dArray], &mut [&mut u128]),
    ) {
        let backend_plugins = self
            .custom_pipes
            .as_ref()
            .expect("no backend custom pipeline plugins registered.");
        let pipe_index = self
            .pipeline_names
            .get(mod_name)
            .expect("pipeline with that name not found");
        backend_plugins.read()[*pipe_index].rewrite_texture_and_buffer_object_indices(cmd, f)
    }
}

#[derive(Debug, Hiarc)]
pub struct GraphicsBackend(RefCell<GraphicsBackendBase>);

impl GraphicsBackend {
    pub fn new(backend_base: GraphicsBackendBase) -> Rc<Self> {
        Rc::new(Self(RefCell::new(backend_base)))
    }

    #[must_use]
    pub fn resized(
        &self,
        buffer: &BackendCommands,
        stream_data: &GraphicsStreamedData,
        window_handling: &dyn NativeImpl,
        new_width: u32,
        new_height: u32,
    ) -> WindowProps {
        self.0
            .borrow_mut()
            .resized(buffer, stream_data, window_handling, new_width, new_height)
    }

    #[must_use]
    pub fn memory_usage(&self) -> GraphicsBackendMemoryUsage {
        self.0.borrow().memory_usage.clone()
    }
}

impl GraphicsBackendInterface for GraphicsBackend {
    fn run_cmds(&self, buffer: &BackendCommands, stream_data: &GraphicsStreamedData) {
        self.0.borrow_mut().run_cmds(buffer, stream_data).unwrap(); // TODO: unwrap?
    }

    fn mem_alloc(&self, alloc_type: GraphicsMemoryAllocationType) -> GraphicsBackendMemory {
        self.0.borrow().mem_alloc(alloc_type)
    }

    fn do_screenshot(&self) -> anyhow::Result<BackendPresentedImageData> {
        self.0.borrow_mut().do_screenshot()
    }

    fn attach_frame_fetcher(
        &self,
        name: String,
        fetcher: Arc<dyn graphics_backend_traits::frame_fetcher_plugin::BackendFrameFetcher>,
    ) {
        self.0.borrow_mut().attach_frame_fetcher(name, fetcher)
    }

    fn detach_frame_fetcher(&self, name: String) {
        self.0.borrow_mut().detach_frame_fetcher(name)
    }

    fn get_backend_mt(&self) -> Arc<dyn GraphicsBackendMtInterface + Sync + Send + 'static> {
        self.0.borrow().get_backend_mt()
    }

    fn add_sync_point(&self, sync_point: Box<dyn PoolSyncPoint>) {
        self.0.borrow_mut().add_sync_point(sync_point)
    }

    fn check_mod_cmd(
        &self,
        mod_name: &str,
        cmd: &mut PoolVec<u8>,
        f: &dyn Fn(&mut [&mut StateTexture], &mut [&mut StateTexture2dArray], &mut [&mut u128]),
    ) {
        self.0.borrow().check_mod_cmd(mod_name, cmd, f)
    }
}
