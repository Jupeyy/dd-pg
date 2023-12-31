use std::{
    cell::RefCell,
    rc::Rc,
    sync::{atomic::AtomicU64, Arc},
};

use anyhow::anyhow;
use base_io::io::IOFileSys;
use config::config::{ConfigBackend, ConfigDebug, ConfigGFX};
use graphics_backend_traits::{
    frame_fetcher_plugin::{BackendFrameFetcher, BackendPresentedImageData},
    plugin::BackendCustomPipeline,
    traits::{DriverBackendInterface, GraphicsBackendInterface, GraphicsBackendMtInterface},
    types::BackendCommands,
};
use graphics_base_traits::traits::{GraphicsStreamDataInterface, GraphicsStreamedData};
use pool::mt_datatypes::PoolVec;

use crate::{
    backend_mt::GraphicsBackendMtType,
    backends::vulkan::vulkan::{VulkanBackendLoading, VulkanBackendLoadingIO},
    window::{BackendDisplayRequirements, BackendRawDisplayHandle, BackendWindow},
};

use native::native::NativeImpl;

use base::{benchmark::Benchmark, system::System};

use super::{
    backend_mt::GraphicsBackendMultiThreaded,
    backends::{
        null::NullBackend,
        vulkan::{common::TTWGraphicsGPUList, vulkan::VulkanBackend, Options},
    },
};

use graphics_types::{
    commands::{AllCommands, CommandUpdateViewport, Commands},
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType, WindowProps},
};

#[derive(Debug)]
enum GraphicsBackendLoadingIOType {
    Vulkan(VulkanBackendLoadingIO),
    Null,
}

#[derive(Debug)]
enum GraphicsBackendLoadingType {
    Vulkan(std::thread::JoinHandle<anyhow::Result<(VulkanBackendLoading, TTWGraphicsGPUList)>>),
    Null(NullBackend),
}

#[derive(Debug)]
enum GraphicsBackendType {
    Vulkan(Box<VulkanBackend>),
    Null(NullBackend),
}

impl GraphicsBackendType {
    pub fn as_mut(&mut self) -> &mut dyn DriverBackendInterface {
        match self {
            GraphicsBackendType::Vulkan(backend) => backend.as_mut(),
            GraphicsBackendType::Null(backend) => backend,
        }
    }
}

#[derive(Debug)]
pub struct GraphicsBackendIOLoading {
    backend_io: GraphicsBackendLoadingIOType,
}

impl GraphicsBackendIOLoading {
    pub fn new(config_gfx: &ConfigGFX, io: &IOFileSys) -> Self {
        Self {
            backend_io: match config_gfx.backend.to_ascii_lowercase().as_str() {
                "vulkan" => GraphicsBackendLoadingIOType::Vulkan(VulkanBackendLoadingIO::new(io)),
                _ => GraphicsBackendLoadingIOType::Null,
            },
        }
    }
}

#[derive(Debug)]
pub struct GraphicsBackendProps {
    texture_memory_usage: Arc<AtomicU64>,
    buffer_memory_usage: Arc<AtomicU64>,
    stream_memory_usage: Arc<AtomicU64>,
    staging_memory_usage: Arc<AtomicU64>,
}

#[derive(Debug)]
pub struct GraphicsBackendLoading {
    props: GraphicsBackendProps,

    backend: GraphicsBackendLoadingType,
}

impl GraphicsBackendLoading {
    pub fn new(
        config_gfx: &ConfigGFX,
        config_dbg: &ConfigDebug,
        config_gl: &ConfigBackend,
        sys: &System,
        raw_display_handle: BackendRawDisplayHandle,
        custom_pipes: Option<Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>>,
    ) -> anyhow::Result<Self> {
        let backend = &config_gfx.backend;

        let benchmark = Benchmark::new(config_dbg.bench);

        let texture_memory_usage: Arc<AtomicU64> = Default::default();
        let buffer_memory_usage: Arc<AtomicU64> = Default::default();
        let stream_memory_usage: Arc<AtomicU64> = Default::default();
        let staging_memory_usage: Arc<AtomicU64> = Default::default();

        let backend = match backend.to_ascii_lowercase().as_str() {
            "vulkan" => {
                let display_requirements = BackendDisplayRequirements {
                    extensions: raw_display_handle.enumerate_required_vk_extensions()?,
                    is_headless: raw_display_handle.is_headless(),
                };
                let custom_pipes = custom_pipes.clone();
                let config_dbg = config_dbg.clone();
                let config_gl = config_gl.clone();
                let texture_memory_usage = texture_memory_usage.clone();
                let buffer_memory_usage = buffer_memory_usage.clone();
                let stream_memory_usage = stream_memory_usage.clone();
                let staging_memory_usage = staging_memory_usage.clone();
                let sys = sys.clone();
                GraphicsBackendLoadingType::Vulkan(std::thread::spawn(move || {
                    // prepare the GL instance
                    let options = Options {
                        dbg: &config_dbg,
                        gl: &config_gl,
                    };
                    VulkanBackendLoading::new(
                        display_requirements,
                        texture_memory_usage,
                        buffer_memory_usage,
                        stream_memory_usage,
                        staging_memory_usage,
                        &sys,
                        &options,
                        custom_pipes,
                    )
                }))
            }
            "null" => GraphicsBackendLoadingType::Null(NullBackend {}),
            _ => panic!("backend not found"),
        };
        benchmark.bench("initializing the backend instance");

        Ok(Self {
            props: GraphicsBackendProps {
                texture_memory_usage,
                buffer_memory_usage,
                stream_memory_usage,
                staging_memory_usage,
            },

            backend,
        })
    }
}

#[derive(Debug)]
pub struct GraphicsBackendBase {
    backend: GraphicsBackendType,
    backend_mt: Arc<GraphicsBackendMultiThreaded>,

    backend_cmds_in_use: Vec<AllCommands>,

    window_props: WindowProps,
    props: GraphicsBackendProps,
}

impl GraphicsBackendBase {
    /// returns the base and the stream_data
    pub fn new(
        io_loading: GraphicsBackendIOLoading,
        backend_loading: GraphicsBackendLoading,
        runtime_threadpool: &Arc<rayon::ThreadPool>,
        window: BackendWindow,
        config_dbg: &ConfigDebug,
        config_gl: &ConfigBackend,
    ) -> anyhow::Result<(
        GraphicsBackendBase,
        Rc<RefCell<dyn GraphicsStreamDataInterface>>,
    )> {
        let benchmark = Benchmark::new(config_dbg.bench);

        let options = Options {
            dbg: config_dbg,
            gl: config_gl,
        };

        let window = &window;
        let size = window.inner_size();
        let scale_factor = window.scale_factor();

        // get window & canvas properties
        let (window_width, window_height) = (size.width, size.height);

        let (canvas_width, canvas_height) = (
            size.width as f64 / scale_factor,
            size.height as f64 / scale_factor,
        );

        let backend = match backend_loading.backend {
            GraphicsBackendLoadingType::Vulkan(loading) => {
                let loading = loading
                    .join()
                    .map_err(|_| anyhow!("joining vk load thread failed"))?;
                let (backend_loading, _) = loading?; // TODO: GPU list is unused var
                GraphicsBackendType::Vulkan(VulkanBackend::new(
                    backend_loading,
                    {
                        let GraphicsBackendLoadingIOType::Vulkan(res) = io_loading.backend_io
                        else {
                            panic!("not a vulkan io backend")
                        };
                        res
                    },
                    runtime_threadpool,
                    window,
                    canvas_width,
                    canvas_height,
                    &options,
                )?)
            }
            GraphicsBackendLoadingType::Null(backend) => GraphicsBackendType::Null(backend),
        };
        benchmark.bench("gl backend loading");

        let backend_mt = Arc::new(GraphicsBackendMultiThreaded {
            backend_mt: match &backend {
                GraphicsBackendType::Vulkan(vk_backend) => {
                    GraphicsBackendMtType::Vulkan(vk_backend.create_mt_backend())
                }
                GraphicsBackendType::Null(null_bk) => {
                    GraphicsBackendMtType::Null(null_bk.get_mt_backend())
                }
            },
        });

        // clear first frame
        let cmd_swap = Commands::Swap;

        let buffer = BackendCommands::default();
        buffer.add_cmd(AllCommands::Misc(cmd_swap));
        let stream_data: Rc<RefCell<dyn GraphicsStreamDataInterface>> = Rc::new(RefCell::new(
            GraphicsStreamedData::new(&mut [], PoolVec::new_without_pool()),
        ));
        let mut res = GraphicsBackendBase {
            backend,
            backend_mt,

            backend_cmds_in_use: Default::default(),

            window_props: WindowProps {
                window_width,
                window_height,
                canvas_width,
                canvas_height,
            },
            props: backend_loading.props,
        };
        res.run_cmds(&buffer, &stream_data)?;
        benchmark.bench("gl first swap");

        Ok((res, stream_data))
    }

    fn run_cmds_impl(
        &mut self,
        buffer: &BackendCommands,
        stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>,
    ) -> anyhow::Result<()> {
        let backend = self.backend.as_mut();

        self.backend_cmds_in_use.clear();
        buffer.replace(&mut self.backend_cmds_in_use);
        backend.start_commands(buffer, &stream_data, self.backend_cmds_in_use.len(), 0);

        for cmd in self.backend_cmds_in_use.drain(..) {
            backend.run_command(cmd)?;
        }
        let res = backend.end_commands().unwrap();

        stream_data
            .borrow_mut()
            .set_from_graphics_streamed_data(res);

        Ok(())
    }

    pub fn get_window_props(&self) -> &WindowProps {
        &self.window_props
    }

    #[must_use]
    fn resized(
        &mut self,
        buffer: &BackendCommands,
        stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>,
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
        self.run_cmds_impl(buffer, &stream_data).unwrap(); // TODO: unwrap here?

        let inner_size = window_handling.borrow_window().inner_size();
        let scale_factor = window_handling.borrow_window().scale_factor();

        self.window_props.window_width = new_width;
        self.window_props.window_height = new_height;
        self.window_props.canvas_width = inner_size.width as f64 / scale_factor;
        self.window_props.canvas_height = inner_size.height as f64 / scale_factor;

        self.window_props
    }
}

impl GraphicsBackendBase {
    fn run_cmds(
        &mut self,
        buffer: &BackendCommands,
        stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>,
    ) -> anyhow::Result<()> {
        self.run_cmds_impl(buffer, stream_data)
    }

    fn mem_alloc(&self, alloc_type: GraphicsMemoryAllocationType) -> GraphicsBackendMemory {
        self.backend_mt.mem_alloc(alloc_type)
    }

    fn do_screenshot(&mut self) -> anyhow::Result<BackendPresentedImageData> {
        self.backend.as_mut().get_presented_image_data(true)
    }

    fn attach_frame_fetcher(&mut self, name: String, fetcher: Arc<dyn BackendFrameFetcher>) {
        self.backend.as_mut().attach_frame_fetcher(name, fetcher)
    }

    fn detach_frame_fetcher(&mut self, name: String) {
        self.backend.as_mut().detach_frame_fetcher(name)
    }

    fn get_backend_mt(&self) -> Arc<dyn GraphicsBackendMtInterface + Sync + Send + 'static> {
        self.backend_mt.clone()
    }
}

#[derive(Debug)]
pub struct GraphicsBackend(RefCell<GraphicsBackendBase>);

impl GraphicsBackend {
    pub fn new(backend_base: GraphicsBackendBase) -> Rc<Self> {
        Rc::new(Self {
            0: RefCell::new(backend_base),
        })
    }

    #[must_use]
    pub fn resized(
        &self,
        buffer: &BackendCommands,
        stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>,
        window_handling: &dyn NativeImpl,
        new_width: u32,
        new_height: u32,
    ) -> WindowProps {
        self.0
            .borrow_mut()
            .resized(buffer, stream_data, window_handling, new_width, new_height)
    }
}

impl GraphicsBackendInterface for GraphicsBackend {
    fn run_cmds(
        &self,
        buffer: &BackendCommands,
        stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>,
    ) {
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
}
