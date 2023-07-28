use std::{
    pin::Pin,
    sync::{atomic::AtomicU64, Arc, Mutex},
};

use graphics_traits::GraphicsBackendBufferInterface;

use crate::{
    backend_mt::GraphicsBackendMtType,
    traits::{GraphicsLoadIOPipe, GraphicsLoadWhileIOPipe},
};

use native::native::NativeImpl;

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

impl GraphicsBackendBufferInterface for BackendBuffer {
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

use base::{benchmark, system::SystemTimeInterface};

use base_fs::io_batcher::{TokIOBatcher, TokIOBatcherTask};

use super::{
    backend_mt::GraphicsBackendMultiThreaded,
    backends::{
        null::NullBackend,
        vulkan::{common::TTWGraphicsGPUList, vulkan::VulkanBackend, Options},
        GraphicsBackendInterface,
    },
};

use graphics_types::{
    command_buffer::{
        AllCommands, CommandSwap, CommandUpdateViewport, Commands, SBackendCapabilites,
    },
    rendering::SVertex,
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType, ImageFormat, WindowProps},
};

enum GraphicsBackendType {
    Vulkan(Pin<Box<VulkanBackend>>),
    Null(NullBackend),
    None,
}

impl GraphicsBackendType {
    pub fn unwrap(&mut self) -> &mut dyn GraphicsBackendInterface {
        match self {
            GraphicsBackendType::Vulkan(backend) => Pin::as_mut(backend).get_mut(),
            GraphicsBackendType::Null(backend) => backend,
            GraphicsBackendType::None => panic!("Use of 'none' backend"),
        }
    }

    pub fn destroy(self) {
        match self {
            GraphicsBackendType::Vulkan(backend) => Pin::into_inner(backend).destroy(),
            GraphicsBackendType::Null(backend) => backend.destroy(),
            GraphicsBackendType::None => panic!("Use of 'none' backend"),
        }
    }
}

pub struct GraphicsBackend {
    backend: GraphicsBackendType,
    backend_mt: Arc<GraphicsBackendMultiThreaded>,
    backend_buffer_in_use: BackendBuffer,
    backend_files: Option<TokIOBatcherTask<Vec<(String, Vec<u8>)>>>,

    texture_memory_usage: Arc<AtomicU64>,
    buffer_memory_usage: Arc<AtomicU64>,
    stream_memory_usage: Arc<AtomicU64>,
    staging_memory_usage: Arc<AtomicU64>,

    window_props: WindowProps,
}

impl GraphicsBackend {
    pub fn new() -> GraphicsBackend {
        GraphicsBackend {
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

            {
                // TODO: this is an example to use naga to compile GLSL to SPIR-V
                // currently https://github.com/gfx-rs/naga/issues/2231 is blocking it from working
                // generally the crate is not ready for use yet
                // don't remove this code, since the goal is to use this instead of compiling at build time
                // by using a shader cache etc.
                /*let mut files: Vec<(String, Vec<u8>)> = Default::default();
                let mut cb =
                    &mut |name: String, file: Vec<u8>| files.push((name.to_string(), file.clone()));
                fs.files_of_dir("shader/glsl/", &mut cb).await;
                for (name, file) in &files {
                    let mut glsl_parser = naga::front::glsl::Frontend::default();
                    let glsl_options = naga::front::glsl::Options {
                        stage: match &name[name.len() - 4..name.len()] {
                            "vert" => naga::ShaderStage::Vertex,
                            "frag" => naga::ShaderStage::Fragment,
                            "comp" => naga::ShaderStage::Compute,
                            _ => {
                                panic!("{} is not a valid GLSL shader file", name);
                            }
                        },
                        defines: Default::default(),
                    };
                    if name == "blur.frag" {
                        println!("now");
                    }
                    println!("compiling: {}", name);
                    let module_res =
                        glsl_parser.parse(&glsl_options, &String::from_utf8(file.clone()).unwrap());
                    match &module_res {
                        Ok(module) => {
                            naga::back::spv::write_vec(
                                &module,
                                &naga::valid::Validator::new(
                                    naga::valid::ValidationFlags::empty(),
                                    naga::valid::Capabilities::empty(),
                                )
                                .validate(module)
                                .unwrap(),
                                &naga::back::spv::Options::default(),
                                None,
                            );
                        }
                        Err(err) => {
                            println!("{} failed to compile.", name);
                            module_res.unwrap();
                        }
                    }
                }*/
            }

            Ok(files)
        }));
    }

    pub fn init_while_io(&mut self, pipe: &mut GraphicsLoadWhileIOPipe) {
        let window = pipe.window_handling.borrow_window();
        let size = pipe.window_handling.borrow_window().inner_size();
        let scale_factor = pipe.window_handling.borrow_window().scale_factor();

        // get window & canvas properties
        (
            self.window_props.window_width,
            self.window_props.window_height,
        ) = (size.width, size.height);

        (
            self.window_props.canvas_width,
            self.window_props.canvas_height,
        ) = (
            size.width as f64 * scale_factor,
            size.height as f64 * scale_factor,
        );

        // prepare the GL instance
        let mut gpu_list = TTWGraphicsGPUList::default();
        let options = Options {
            thread_count: pipe.config.gfx_thread_count,
            dbg_gfx: pipe.config.dbg_gfx,
        };

        let backend = &pipe.config.gfx_backend;

        self.backend = benchmark!(
            pipe.config.dbg_bench,
            pipe.sys,
            "\tinitializing the backend instance (while io)",
            || {
                match backend.to_ascii_lowercase().as_str() {
                    "vulkan" => GraphicsBackendType::Vulkan(
                        VulkanBackend::init_instance_while_io(
                            window,
                            &mut gpu_list,
                            self.texture_memory_usage.clone(),
                            self.buffer_memory_usage.clone(),
                            self.stream_memory_usage.clone(),
                            self.staging_memory_usage.clone(),
                            self.window_props.canvas_width,
                            self.window_props.canvas_height,
                            &pipe.runtime_threadpool,
                            pipe.sys,
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
    }

    #[must_use]
    pub fn init(&mut self, io_batcher: &Arc<Mutex<TokIOBatcher>>) -> Result<BackendBuffer, ()> {
        // the actual backend requires shader files, so it should be initialized only at this point
        // TODO: split backend initialization to fit the current initialization style

        let task = self.backend_files.as_mut().unwrap();
        io_batcher.lock().unwrap().wait_finished_and_drop(task);
        let backend_files = task.get_storage().unwrap();
        self.backend.unwrap().set_files(backend_files);

        self.backend.unwrap().init().unwrap();

        // clear first frame
        let cmd_swap = Commands::Swap(CommandSwap {});

        let mut buffer = BackendBuffer {
            cmds: vec![AllCommands::Misc(cmd_swap)],
            num_vertices: 0,
            vertices: &mut [],
        };
        self.run_cmds(&mut buffer);
        Ok(buffer)
    }

    pub fn destroy(self) {
        self.backend.destroy();
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

    pub fn resized(
        &mut self,
        window_handling: &mut dyn NativeImpl,
        new_width: u32,
        new_height: u32,
    ) {
        // TODO make sure backend is idle

        let cmd_viewport = Commands::UpdateViewport(CommandUpdateViewport {
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

        let inner_size = window_handling.borrow_window().inner_size();
        let scale_factor = window_handling.borrow_window().scale_factor();

        self.window_props.canvas_width = inner_size.width as f64 * scale_factor;
        self.window_props.canvas_height = inner_size.height as f64 * scale_factor;
    }

    pub fn do_screenshot(
        &mut self,
        width: &mut u32,
        height: &mut u32,
        dest_data_buffer: &mut Vec<u8>,
    ) -> anyhow::Result<ImageFormat> {
        self.backend
            .unwrap()
            .get_presented_image_data(width, height, dest_data_buffer)
    }

    pub fn mem_alloc(
        &mut self,
        alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> GraphicsBackendMemory {
        self.backend_mt.mem_alloc(alloc_type, req_size)
    }

    pub fn mem_free(&mut self, mem: GraphicsBackendMemory) {
        self.backend_mt.mem_free(mem)
    }

    pub fn get_backend_mt(&self) -> Arc<GraphicsBackendMultiThreaded> {
        self.backend_mt.clone()
    }
}
