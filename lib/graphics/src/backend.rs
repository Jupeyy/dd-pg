use std::sync::{atomic::AtomicU64, Arc};

use graphics_traits::GraphicsBachendBufferInterface;
use sdl2::{video::Window, *};

use crate::{
    backend_mt::GraphicsBackendMtType,
    traits::{GraphicsLoadIOPipe, GraphicsLoadWhileIOPipe},
};

use native::native::Native;

pub struct BackendBuffer {
    pub cmds: Vec<AllCommands>,
    pub vertices: &'static mut [SVertex],
    pub num_vertices: usize,
}

impl Default for BackendBuffer {
    fn default() -> Self {
        let mut res = BackendBuffer {
            vertices: &mut [],
            cmds: Vec::new(),
            num_vertices: 0,
        };
        res.cmds.reserve(200);
        res
    }
}

impl GraphicsBachendBufferInterface for BackendBuffer {
    fn vertices_mut(&mut self) -> &mut [SVertex] {
        &mut self.vertices
    }

    fn vertices_count(&self) -> usize {
        self.num_vertices
    }

    fn vertices_count_mut(&mut self) -> &mut usize {
        &mut self.num_vertices
    }

    fn vertices_and_count_mut(&mut self) -> (&mut [SVertex], &mut usize) {
        (&mut self.vertices, &mut self.num_vertices)
    }
}

use base::{
    benchmark,
    config::Config,
    filesys::FileSystem,
    io_batcher::{IOBatcher, IOBatcherTask},
    system::{System, SystemTimeInterface},
};

use super::{
    backend_mt::GraphicsBackendMultiThreaded,
    backends::{
        null::{NullBackend, NullBackendMt},
        vulkan::{
            common::TTWGraphicsGPUList,
            vulkan::{VulkanBackend, VulkanBackendMt},
            Options,
        },
        GraphicsBackendInterface,
    },
};

use graphics_types::{
    command_buffer::{
        AllCommands, Commands, SBackendCapabilites, SCommand_Swap, SCommand_Update_Viewport,
    },
    rendering::SVertex,
    types::{GraphicsMemoryAllocationType, WindowProps},
};

const GPU_INFO_STR: usize = 256;

enum GraphicsBackendType {
    Vulkan(VulkanBackend),
    Null(NullBackend),
    None,
}

impl GraphicsBackendType {
    pub fn unwrap(&mut self) -> &mut dyn GraphicsBackendInterface {
        match self {
            GraphicsBackendType::Vulkan(backend) => backend,
            GraphicsBackendType::Null(backend) => backend,
            GraphicsBackendType::None => panic!("Use of 'none' backend"),
        }
    }
}

pub struct GraphicsBackend {
    sdl2: Sdl,
    window: Option<Window>,
    backend: GraphicsBackendType,
    backend_mt: Arc<GraphicsBackendMultiThreaded>,
    backend_buffer_in_use: BackendBuffer,
    backend_files: Option<IOBatcherTask<Vec<(String, Vec<u8>)>>>,

    texture_memory_usage: Arc<AtomicU64>,
    buffer_memory_usage: Arc<AtomicU64>,
    stream_memory_usage: Arc<AtomicU64>,
    staging_memory_usage: Arc<AtomicU64>,

    window_props: WindowProps,
}

impl GraphicsBackend {
    pub fn new(native: Native) -> GraphicsBackend {
        GraphicsBackend {
            sdl2: native.sdl2,
            window: None,
            backend: GraphicsBackendType::None,
            backend_mt: Arc::new(GraphicsBackendMultiThreaded::new()),
            backend_buffer_in_use: BackendBuffer::default(),
            backend_files: None,

            texture_memory_usage: Arc::<AtomicU64>::default(),
            buffer_memory_usage: Arc::<AtomicU64>::default(),
            stream_memory_usage: Arc::<AtomicU64>::default(),
            staging_memory_usage: Arc::<AtomicU64>::default(),

            window_props: Default::default(),
        }
    }

    pub fn load_io(&mut self, io_pipe: &mut GraphicsLoadIOPipe) {
        let fs = io_pipe.fs.clone();
        self.backend_files = Some(io_pipe.batcher.lock().unwrap().spawn(async move {
            let mut files: Vec<(String, Vec<u8>)> = Default::default();
            let mut cb =
                &mut |name: String, file: Vec<u8>| files.push((name.to_string(), file.clone()));
            fs.files_of_dir("shader/vulkan/", &mut cb).await;
            Ok(files)
        }));
    }

    pub fn init_while_io(&mut self, pipe: &mut GraphicsLoadWhileIOPipe) {
        let target_width = pipe.config.gfx_window_width;
        let target_height = pipe.config.gfx_window_height;

        // prepare the window while waiting for IO
        let video_subsystem = self.sdl2.video().unwrap();
        let mut window = benchmark!(
            pipe.config.dbg_bench,
            pipe.sys,
            "\tinitializing the window",
            || {
                video_subsystem
                    .window("DDNet", target_width, target_height)
                    .vulkan()
                    .resizable()
                    .build()
                    .unwrap()
            }
        );
        window.show();

        // get window & canvas properties
        (
            self.window_props.window_width,
            self.window_props.window_height,
        ) = window.size();

        (
            self.window_props.canvas_width,
            self.window_props.canvas_height,
        ) = window.drawable_size();

        // prepare the GL instance
        let mut gpu_list = TTWGraphicsGPUList::default();
        let options = Options {
            thread_count: pipe.config.gfx_thread_count,
            dbg_gfx: pipe.config.dbg_gfx,
        };

        let backend = "vulkan";

        self.backend = benchmark!(
            pipe.config.dbg_bench,
            pipe.sys,
            "\tinitializing the backend instance (while io)",
            || {
                match backend.to_ascii_lowercase().as_str() {
                    "vulkan" => GraphicsBackendType::Vulkan(
                        VulkanBackend::init_instance_while_io(
                            &window,
                            &mut gpu_list,
                            self.texture_memory_usage.clone(),
                            self.buffer_memory_usage.clone(),
                            self.stream_memory_usage.clone(),
                            self.staging_memory_usage.clone(),
                            self.window_props.canvas_width,
                            self.window_props.canvas_height,
                            &pipe.runtime_threadpool,
                            &options,
                        )
                        .unwrap(),
                    ),
                    "null" => GraphicsBackendType::Null(NullBackend {}),
                    _ => panic!("backend not found"),
                }
            }
        );

        let mut capabilities = SBackendCapabilites::default();
        benchmark!(
            pipe.config.dbg_bench,
            pipe.sys,
            "\tinitializing the backend (while io)",
            || {
                self.backend.unwrap().init_while_io(&mut capabilities);
            }
        );

        self.backend_mt = Arc::new(GraphicsBackendMultiThreaded {
            backend_mt: match &self.backend {
                GraphicsBackendType::Vulkan(vk_backend) => {
                    GraphicsBackendMtType::Vulkan(vk_backend.get_mt_backend())
                }
                _ => todo!(),
            },
        });

        // finish the setup
        self.window = Some(window);
    }

    #[must_use]
    pub fn init(&mut self) -> Result<BackendBuffer, ()> {
        // the actual backend requires shader files, so it should be initialized only at this point
        // TODO: split backend initialization to fit the current initialization style

        let mut backend_files: Vec<(String, Vec<u8>)> = Default::default();
        backend_files = self.backend_files.as_mut().unwrap().get_storage().unwrap();
        self.backend.unwrap().set_files(backend_files);

        self.backend.unwrap().init().unwrap();

        // clear first frame
        let cmd_swap = Commands::CMD_SWAP(SCommand_Swap {});

        let mut buffer = BackendBuffer {
            cmds: vec![AllCommands::Misc(cmd_swap)],
            num_vertices: 0,
            vertices: &mut [],
        };
        self.run_cmds(&mut buffer);
        Ok(buffer)
    }

    fn run_cmds_impl(&mut self, buffer: &mut BackendBuffer, swap_buffers: bool) {
        let backend = self.backend.unwrap();
        backend.start_commands(buffer, buffer.cmds.len(), 0);

        for cmd in &buffer.cmds {
            backend.run_command(cmd);
        }
        let res = backend.end_commands();

        if swap_buffers {
            self.backend_buffer_in_use.cmds.clear();
            self.backend_buffer_in_use.num_vertices = 0;
            self.backend_buffer_in_use.vertices = res.unwrap();
            std::mem::swap(&mut self.backend_buffer_in_use, buffer);
        }
    }

    /**
     * Runs a backend buffer and swaps out the buffers the next to use
     */
    pub fn run_cmds(&mut self, buffer: &mut BackendBuffer) {
        self.run_cmds_impl(buffer, true);
    }

    pub fn get_window_props(&self) -> &WindowProps {
        &self.window_props
    }

    pub fn resized(&mut self, new_width: u32, new_height: u32) {
        // TODO make sure backend is idle

        let cmd_viewport = Commands::CMD_UPDATE_VIEWPORT(SCommand_Update_Viewport {
            x: 0,
            y: 0,
            width: new_width,
            height: new_height,
            by_resize: true,
        });

        let mut buffer = BackendBuffer {
            cmds: vec![AllCommands::Misc(cmd_viewport)],
            num_vertices: 0,
            vertices: &mut [],
        };
        self.run_cmds_impl(&mut buffer, false);

        (
            self.window_props.canvas_width,
            self.window_props.canvas_height,
        ) = self.window.as_ref().unwrap().drawable_size();
    }

    pub fn borrow_window(&self) -> &sdl2::video::Window {
        self.window.as_ref().unwrap()
    }

    pub fn mem_alloc(
        &mut self,
        alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> &'static mut [u8] {
        self.backend_mt.mem_alloc(alloc_type, req_size)
    }

    pub fn mem_free(&mut self, mem: &'static mut [u8]) {
        self.backend_mt.mem_free(mem)
    }

    pub fn get_backend_mt(&self) -> Arc<GraphicsBackendMultiThreaded> {
        self.backend_mt.clone()
    }
}
