use std::{
    cell::RefCell,
    pin::Pin,
    rc::Rc,
    sync::{atomic::AtomicU64, Arc},
};

use graphics_backend_traits::{
    traits::{DriverBackendInterface, GraphicsBackendInterface, GraphicsBackendMtInterface},
    types::{BackendCommands, BackendStreamData},
};
use graphics_base_traits::traits::GraphicsStreamDataInterface;

use crate::{
    backend_mt::GraphicsBackendMtType,
    types::{GraphicsBackendLoadIOPipe, GraphicsBackendLoadWhileIOPipe},
};

use native::native::NativeImpl;

use base::{benchmark, system::SystemTimeInterface};

use base_fs::io_batcher::TokIOBatcherTask;

use super::{
    backend_mt::GraphicsBackendMultiThreaded,
    backends::{
        null::NullBackend,
        vulkan::{common::TTWGraphicsGPUList, vulkan::VulkanBackend, Options},
    },
};

use graphics_types::{
    command_buffer::{
        AllCommands, CommandSwap, CommandUpdateViewport, Commands, ERunCommandReturnTypes,
        SBackendCapabilites,
    },
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType, ImageFormat, WindowProps},
};

#[derive(Debug)]
enum GraphicsBackendType {
    Vulkan(Pin<Box<VulkanBackend>>),
    Null(NullBackend),
    None,
}

impl GraphicsBackendType {
    pub fn unwrap(&mut self) -> &mut dyn DriverBackendInterface {
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

#[derive(Debug)]
pub struct GraphicsBackendBase {
    backend: GraphicsBackendType,
    backend_mt: Arc<GraphicsBackendMultiThreaded>,

    backend_cmds_in_use: Vec<AllCommands>,

    backend_files: Option<TokIOBatcherTask<Vec<(String, Vec<u8>)>>>,

    texture_memory_usage: Arc<AtomicU64>,
    buffer_memory_usage: Arc<AtomicU64>,
    stream_memory_usage: Arc<AtomicU64>,
    staging_memory_usage: Arc<AtomicU64>,

    window_props: WindowProps,
}

impl GraphicsBackendBase {
    pub fn new() -> GraphicsBackendBase {
        GraphicsBackendBase {
            backend: GraphicsBackendType::None,
            backend_mt: Arc::new(GraphicsBackendMultiThreaded::new()),

            backend_cmds_in_use: Default::default(),

            backend_files: None,

            texture_memory_usage: Arc::<AtomicU64>::default(),
            buffer_memory_usage: Arc::<AtomicU64>::default(),
            stream_memory_usage: Arc::<AtomicU64>::default(),
            staging_memory_usage: Arc::<AtomicU64>::default(),

            window_props: WindowProps {
                canvas_width: 0.0,
                canvas_height: 0.0,
                window_width: 0,
                window_height: 0,
            },
        }
    }

    pub fn load_io(&mut self, io_pipe: &mut GraphicsBackendLoadIOPipe) {
        let fs = io_pipe.fs.clone();
        self.backend_files = Some(io_pipe.io_batcher.spawn(async move {
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

    pub fn init_while_io(&mut self, pipe: &mut GraphicsBackendLoadWhileIOPipe) {
        let window = &pipe.window_handling;
        let size = pipe.window_handling.inner_size();
        let scale_factor = pipe.window_handling.scale_factor();

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
            thread_count: pipe.config.gfx.thread_count,
            dbg_gfx: pipe.config.dbg.gfx,
        };

        let backend = &pipe.config.gfx.backend;

        self.backend = benchmark!(
            pipe.config.dbg.bench,
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
            pipe.config.dbg.bench,
            pipe.sys,
            "\tinitializing the backend (while io)",
            || {
                self.backend
                    .unwrap()
                    .init_while_io(&mut capabilities)
                    .unwrap();
            }
        );

        self.backend_mt = Arc::new(GraphicsBackendMultiThreaded {
            backend_mt: match &self.backend {
                GraphicsBackendType::Vulkan(vk_backend) => {
                    GraphicsBackendMtType::Vulkan(vk_backend.get_mt_backend())
                }
                GraphicsBackendType::Null(null_bk) => {
                    GraphicsBackendMtType::Null(null_bk.get_mt_backend())
                }
                _ => todo!(),
            },
        });
    }

    #[must_use]
    pub fn init(&mut self) -> Result<Rc<RefCell<dyn GraphicsStreamDataInterface>>, ()> {
        // the actual backend requires shader files, so it should be initialized only at this point
        // TODO: split backend initialization to fit the current initialization style

        let task = self.backend_files.take().unwrap();
        let backend_files = task.get_storage().unwrap();
        self.backend.unwrap().set_files(backend_files);

        self.backend.unwrap().init().unwrap();

        // clear first frame
        let cmd_swap = Commands::Swap(CommandSwap {});

        let mut buffer = BackendCommands::default();
        buffer.add_cmd(AllCommands::Misc(cmd_swap));
        let mut stream_data: Rc<RefCell<dyn GraphicsStreamDataInterface>> =
            Rc::new(RefCell::new(BackendStreamData::default()));
        self.run_cmds(&mut buffer, &mut stream_data);
        Ok(stream_data)
    }

    fn run_cmds_impl(
        &mut self,
        buffer: &BackendCommands,
        stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>,
    ) {
        let backend = self.backend.unwrap();

        self.backend_cmds_in_use.clear();
        buffer.replace(&mut self.backend_cmds_in_use);
        backend.start_commands(buffer, &stream_data, self.backend_cmds_in_use.len(), 0);

        for cmd in &self.backend_cmds_in_use {
            match backend.run_command(cmd) {
                ERunCommandReturnTypes::CmdHandled => {}
                ERunCommandReturnTypes::CmdUnhandled => todo!(),
                ERunCommandReturnTypes::CmdWarning => todo!(),
                ERunCommandReturnTypes::CmdError => todo!(),
            }
        }
        let res = backend.end_commands();

        stream_data.borrow_mut().set_vertices_unsafe(res.unwrap());
    }

    pub fn get_window_props(&self) -> &WindowProps {
        &self.window_props
    }

    #[must_use]
    fn resized(
        &mut self,
        buffer: &BackendCommands,
        stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>,
        window_handling: &mut dyn NativeImpl,
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
        self.run_cmds_impl(buffer, &stream_data);

        let inner_size = window_handling.borrow_window().inner_size();
        let scale_factor = window_handling.borrow_window().scale_factor();

        self.window_props.window_width = new_width;
        self.window_props.window_height = new_height;
        self.window_props.canvas_width = inner_size.width as f64 * scale_factor;
        self.window_props.canvas_height = inner_size.height as f64 * scale_factor;

        self.window_props
    }
}

impl GraphicsBackendBase {
    fn run_cmds(
        &mut self,
        buffer: &BackendCommands,
        stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>,
    ) {
        self.run_cmds_impl(buffer, stream_data);
    }

    fn mem_alloc(
        &self,
        alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> GraphicsBackendMemory {
        self.backend_mt.mem_alloc(alloc_type, req_size)
    }

    fn do_screenshot(
        &mut self,
        width: &mut u32,
        height: &mut u32,
        dest_data_buffer: &mut Vec<u8>,
    ) -> anyhow::Result<ImageFormat> {
        self.backend
            .unwrap()
            .get_presented_image_data(width, height, dest_data_buffer, true)
    }

    fn get_backend_mt(&self) -> Arc<dyn GraphicsBackendMtInterface + Sync + Send + 'static> {
        self.backend_mt.clone()
    }
}

impl Drop for GraphicsBackendBase {
    fn drop(&mut self) {
        let mut backend = GraphicsBackendType::None;
        std::mem::swap(&mut self.backend, &mut backend);
        backend.destroy();
    }
}

#[derive(Debug, Clone)]
pub struct GraphicsBackend(Rc<RefCell<GraphicsBackendBase>>);

impl GraphicsBackend {
    pub fn new(backend_base: GraphicsBackendBase) -> Self {
        Self {
            0: Rc::new(RefCell::new(backend_base)),
        }
    }

    #[must_use]
    pub fn resized(
        &self,
        buffer: &BackendCommands,
        stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>,
        window_handling: &mut dyn NativeImpl,
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
        self.0.borrow_mut().run_cmds(buffer, stream_data)
    }

    fn mem_alloc(
        &self,
        alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> GraphicsBackendMemory {
        self.0.borrow().mem_alloc(alloc_type, req_size)
    }

    fn do_screenshot(
        &self,
        width: &mut u32,
        height: &mut u32,
        dest_data_buffer: &mut Vec<u8>,
    ) -> anyhow::Result<ImageFormat> {
        self.0
            .borrow_mut()
            .do_screenshot(width, height, dest_data_buffer)
    }

    fn get_backend_mt(&self) -> Arc<dyn GraphicsBackendMtInterface + Sync + Send + 'static> {
        self.0.borrow().get_backend_mt()
    }
}
