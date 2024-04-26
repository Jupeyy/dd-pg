use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc,
    },
    thread::JoinHandle,
};

use anyhow::anyhow;
use base::system::System;
use base_io::io::IOFileSys;
use config::config::{ConfigBackend, ConfigDebug};
use graphics_backend_traits::{
    frame_fetcher_plugin::{BackendFrameFetcher, BackendPresentedImageData},
    plugin::BackendCustomPipeline,
    traits::DriverBackendInterface,
};
use graphics_base_traits::traits::{
    GraphicsStreamVertices, GraphicsStreamedData, GraphicsStreamedDataSyncSend,
    GraphicsStreamedUniformData, GraphicsStreamedUniformRawData,
};
use graphics_types::{
    commands::{
        AllCommands, StreamDataMax, GRAPHICS_DEFAULT_UNIFORM_SIZE,
        GRAPHICS_MAX_UNIFORM_RENDER_COUNT,
    },
    rendering::GlVertex,
};
use hiarc::Hiarc;
use parking_lot::{Condvar, Mutex};
use pool::{mixed_pool::PoolSyncPoint, mt_datatypes::PoolVec};

use crate::{
    backend_mt::GraphicsBackendMtType,
    backends::{
        null::NullBackend,
        types::BackendWriteFiles,
        vulkan::{
            common::TTWGraphicsGPUList,
            vulkan::{
                VulkanBackend, VulkanBackendLoadedIO, VulkanBackendLoading, VulkanInUseStreamData,
                VulkanMainThreadData, VulkanMainThreadInit,
            },
            Options,
        },
    },
    window::{BackendDisplayRequirements, BackendWindow},
};

#[derive(Debug)]
enum GraphicsBackendLoadingType {
    Vulkan((VulkanBackendLoading, TTWGraphicsGPUList)),
    Null(NullBackend),
}

#[derive(Debug, Hiarc)]
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

#[derive(Debug, Hiarc)]
pub enum BackendThreadInitData {
    Vulkan {
        data: VulkanBackendLoadedIO,

        runtime_threadpool: Arc<rayon::ThreadPool>,

        canvas_width: f64,
        canvas_height: f64,
        dbg: ConfigDebug,
        gl: ConfigBackend,
    },
    Null,
}

#[derive(Debug, Hiarc)]
pub enum BackendThreadInitFromMainThread {
    Vulkan(VulkanMainThreadData),
    Null,
}

#[derive(Debug, Hiarc)]
pub enum BackendThreadMainThreadInit {
    Vulkan(VulkanMainThreadInit),
    Null,
}

#[derive(Debug, Hiarc)]
pub enum BackendThreadBackendEvent {
    Init {
        backend_ty: String,
        display_requirements: BackendDisplayRequirements,
        config_dbg: ConfigDebug,
        config_gl: ConfigBackend,
        sys: System,
        #[hiarc_skip_unsafe]
        custom_pipes: Option<Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>>,
        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,
        write_files: BackendWriteFiles,
    },
    FinishInit {
        data: BackendThreadInitData,
        main_thread_init: BackendThreadMainThreadInit,
    },
    RunCmds {
        cmds: Vec<AllCommands>,
        stream_data: GraphicsStreamedDataSyncSend,
    },
    TakeScreenshot,
    AttachFrameFetcher {
        name: String,
        #[hiarc_skip_unsafe]
        fetcher: Arc<dyn BackendFrameFetcher>,
    },
    DetachFrameFetcher {
        name: String,
    },
    Stop,
}

#[derive(Debug, Hiarc)]
pub enum BackendThreadFrontendEvent {
    InitFromMainThread(BackendThreadInitFromMainThread),
    BuffersFromBackend {
        streamed_data: GraphicsStreamedDataSyncSend,
        /// empty cmd buffer, can be reused
        cmds: Vec<AllCommands>,
    },
    Screenshot(anyhow::Result<BackendPresentedImageData>),
}

#[derive(Debug, Hiarc, Default)]
pub struct BackendThreadEvents {
    pub backend_events: VecDeque<BackendThreadBackendEvent>,
    pub frontend_events: VecDeque<BackendThreadFrontendEvent>,
}

#[derive(Debug, Hiarc)]
pub struct BackendThread {
    thread: Option<JoinHandle<anyhow::Result<()>>>,

    is_finished: Arc<AtomicBool>,
    nty: Arc<Condvar>,
    events: Arc<Mutex<BackendThreadEvents>>,

    write_files: BackendWriteFiles,
    io: IOFileSys,

    #[hiarc_skip_unsafe]
    sync_points: Vec<Box<dyn PoolSyncPoint>>,
}

impl BackendThread {
    pub fn new(
        backend_ty: String,
        display_requirements: BackendDisplayRequirements,
        config_dbg: ConfigDebug,
        config_gl: ConfigBackend,
        sys: System,
        custom_pipes: Option<Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>>,
        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,
        io: IOFileSys,
    ) -> Self {
        let nty: Arc<Condvar> = Default::default();
        let events: Arc<Mutex<BackendThreadEvents>> = Default::default();

        let write_files = BackendWriteFiles::default();

        events
            .lock()
            .backend_events
            .push_back(BackendThreadBackendEvent::Init {
                backend_ty,
                display_requirements,
                config_dbg,
                config_gl,
                sys,
                custom_pipes,
                texture_memory_usage,
                buffer_memory_usage,
                stream_memory_usage,
                staging_memory_usage,
                write_files: write_files.clone(),
            });

        let is_finished: Arc<AtomicBool> = Default::default();

        let nty_thread = nty.clone();
        let events_thread = events.clone();

        let is_finished_thread = is_finished.clone();

        let thread = thread_priority::ThreadBuilder::default()
            .name("backend-thread".to_string())
            .priority(thread_priority::ThreadPriority::Max)
            .spawn_careless(move || {
                match BackendThread::run(nty_thread, events_thread, is_finished_thread) {
                    Ok(_) => Ok(()),
                    Err(err) => {
                        println!("graphics backend thread died: {err}");
                        Err(err)
                    }
                }
            })
            .ok();

        Self {
            thread,
            events,
            nty,
            is_finished,
            write_files,
            io,
            sync_points: Default::default(),
        }
    }

    pub fn init(
        &self,
        data: BackendThreadInitData,
        dbg: &ConfigDebug,
        window: &BackendWindow,
    ) -> anyhow::Result<GraphicsBackendMtType> {
        let mut g = self.events.lock();
        self.nty
            .wait_while(&mut g, |g| g.frontend_events.is_empty());
        let Some(BackendThreadFrontendEvent::InitFromMainThread(init_ev)) =
            g.frontend_events.pop_front()
        else {
            return Err(anyhow!("Frontend event was not sent from the backend thread, maybe it died? else it's a logic bug"));
        };
        let (main_thread_init, backend_mt) = match init_ev {
            BackendThreadInitFromMainThread::Vulkan(data) => {
                let backend_mt = VulkanBackend::create_mt_backend(&data);
                (
                    BackendThreadMainThreadInit::Vulkan(VulkanBackend::init_from_main_thread(
                        data, window, dbg,
                    )?),
                    GraphicsBackendMtType::Vulkan(backend_mt),
                )
            }
            BackendThreadInitFromMainThread::Null => (
                BackendThreadMainThreadInit::Null,
                GraphicsBackendMtType::Null(NullBackend::get_mt_backend()),
            ),
        };
        g.backend_events
            .push_back(BackendThreadBackendEvent::FinishInit {
                data,
                main_thread_init,
            });
        self.nty.notify_all();

        Ok(backend_mt)
    }

    /// add a pool sync pointer before the [`BackendThread::run_cmds`] command is called
    /// sync points can not be removed, so call carefully
    pub fn add_sync_point(&mut self, sync_point: Box<dyn PoolSyncPoint>) {
        self.sync_points.push(sync_point);
    }

    pub fn run_cmds(
        &self,
        stream_data: &GraphicsStreamedData,
        cmds: &mut Vec<AllCommands>,
    ) -> anyhow::Result<()> {
        let mut g = self.events.lock();
        self.nty
            .wait_while(&mut g, |g| g.frontend_events.is_empty());
        let Some(BackendThreadFrontendEvent::BuffersFromBackend {
            streamed_data: stream_data_cmd,
            cmds: mut cmds_cmd,
        }) = g.frontend_events.pop_front()
        else {
            return Err(anyhow!("frontend commands other than stream data is not supported yet, also there must be a stream data command every frame"));
        };

        std::mem::swap(cmds, &mut cmds_cmd);

        let stream_data_cmd = GraphicsStreamedData::from_sync_send_wrapper(stream_data_cmd);
        let stream_data = stream_data
            .try_replace_inner(stream_data_cmd)
            .map_err(|err| anyhow!(err))?;

        for sync_point in &self.sync_points {
            sync_point.sync();
        }

        g.backend_events
            .push_back(BackendThreadBackendEvent::RunCmds {
                cmds: cmds_cmd,
                stream_data: stream_data.try_into_sync_send_wrapper()?,
            });
        self.nty.notify_all();
        Ok(())
    }

    pub fn do_screenshot(&mut self) -> anyhow::Result<BackendPresentedImageData> {
        let mut g = self.events.lock();

        g.backend_events
            .push_back(BackendThreadBackendEvent::TakeScreenshot);
        self.nty.notify_all();

        self.nty.wait_while(&mut g, |g| {
            !g.frontend_events
                .back()
                .is_some_and(|ev| matches!(ev, BackendThreadFrontendEvent::Screenshot(_)))
        });

        let Some(BackendThreadFrontendEvent::Screenshot(screenshot_data)) =
            g.frontend_events.pop_back()
        else {
            return Err(anyhow!("frontend commands other than stream data is not supported yet, also there must be a stream data command every frame"));
        };
        screenshot_data
    }

    pub fn attach_frame_fetcher(&mut self, name: String, fetcher: Arc<dyn BackendFrameFetcher>) {
        let mut g = self.events.lock();

        g.backend_events
            .push_back(BackendThreadBackendEvent::AttachFrameFetcher { name, fetcher });
        self.nty.notify_all();
    }

    pub fn detach_frame_fetcher(&mut self, name: String) {
        let mut g = self.events.lock();

        g.backend_events
            .push_back(BackendThreadBackendEvent::DetachFrameFetcher { name });
        self.nty.notify_all();
    }

    fn run(
        nty: Arc<Condvar>,
        events: Arc<Mutex<BackendThreadEvents>>,

        is_finished: Arc<AtomicBool>,
    ) -> anyhow::Result<()> {
        let mut g = events.lock();

        // handle loading
        let load_ev = g.backend_events.pop_front();
        let Some(BackendThreadBackendEvent::Init {
            backend_ty,
            display_requirements,
            config_dbg,
            config_gl,
            sys,
            custom_pipes,
            texture_memory_usage,
            buffer_memory_usage,
            stream_memory_usage,
            staging_memory_usage,
            write_files,
        }) = load_ev
        else {
            return Err(anyhow!("first event is always the load event"));
        };
        let backend_loading = match backend_ty.to_ascii_lowercase().as_str() {
            "vulkan" => {
                let options = Options {
                    dbg: &config_dbg,
                    gl: &config_gl,
                };
                // prepare the GL instance
                let backend = VulkanBackendLoading::new(
                    display_requirements,
                    texture_memory_usage,
                    buffer_memory_usage,
                    stream_memory_usage,
                    staging_memory_usage,
                    &sys,
                    &options,
                    custom_pipes,
                )?;
                GraphicsBackendLoadingType::Vulkan(backend)
            }
            "null" => GraphicsBackendLoadingType::Null(NullBackend {}),
            _ => panic!("backend not found"),
        };

        g.frontend_events
            .push_back(BackendThreadFrontendEvent::InitFromMainThread(
                match &backend_loading {
                    GraphicsBackendLoadingType::Vulkan((loading, _)) => {
                        BackendThreadInitFromMainThread::Vulkan(VulkanBackend::main_thread_data(
                            loading,
                        ))
                    }
                    GraphicsBackendLoadingType::Null(_) => BackendThreadInitFromMainThread::Null,
                },
            ));

        nty.notify_all();

        // wait for io stuff to arrive
        nty.wait_while(&mut g, |g| g.backend_events.is_empty());

        let load_ev = g.backend_events.pop_front();
        let Some(BackendThreadBackendEvent::FinishInit {
            data,
            main_thread_init,
        }) = load_ev
        else {
            return Err(anyhow!("finish init event is always the second event"));
        };

        let mut backend = match data {
            BackendThreadInitData::Vulkan {
                data,
                runtime_threadpool,
                canvas_width,
                canvas_height,
                dbg,
                gl,
            } => {
                let GraphicsBackendLoadingType::Vulkan((loading, _)) = backend_loading else {
                    return Err(anyhow!("loading was not of type vulkan"));
                };
                let BackendThreadMainThreadInit::Vulkan(main_thread_init) = main_thread_init else {
                    return Err(anyhow!("main thread init data was not of type vulkan"));
                };
                GraphicsBackendType::Vulkan(VulkanBackend::new(
                    loading,
                    data,
                    &runtime_threadpool,
                    main_thread_init,
                    canvas_width,
                    canvas_height,
                    &Options { dbg: &dbg, gl: &gl },
                    write_files,
                )?)
            }
            BackendThreadInitData::Null => GraphicsBackendType::Null(NullBackend {}),
        };

        enum InUseDataPerBackend {
            Vulkan(VulkanInUseStreamData),
            Null,
        }

        let (mut stream_data, mut next_in_use_data, mut in_use_data) = match &mut backend {
            GraphicsBackendType::Vulkan(backend) => {
                let stream_data = backend.get_stream_data()?;
                let next_stream_data = backend.get_stream_data()?;

                let mem = unsafe {
                    stream_data.cur_stream_vertex_buffer.memories[0]
                        .mapped_memory
                        .get_mem_typed::<GlVertex>(StreamDataMax::MaxVertices as usize)
                };

                let mut graphics_uniform_data = backend.props.graphics_uniform_buffers.new();
                graphics_uniform_data.extend(
                    stream_data
                        .cur_stream_uniform_buffers
                        .memories
                        .iter()
                        .map(|uni| unsafe {
                            GraphicsStreamedUniformData::new(GraphicsStreamedUniformRawData::Raw(
                                uni.mapped_memory.get_mem(
                                    GRAPHICS_MAX_UNIFORM_RENDER_COUNT
                                        * GRAPHICS_DEFAULT_UNIFORM_SIZE,
                                ),
                            ))
                        }),
                );

                (
                    GraphicsStreamedData::new(
                        GraphicsStreamVertices::Static(mem),
                        graphics_uniform_data,
                    ),
                    InUseDataPerBackend::Vulkan(stream_data),
                    InUseDataPerBackend::Vulkan(next_stream_data),
                )
            }
            GraphicsBackendType::Null(_) => (
                GraphicsStreamedData::new(
                    GraphicsStreamVertices::Static(&mut []),
                    PoolVec::new_without_pool(),
                ),
                InUseDataPerBackend::Null,
                InUseDataPerBackend::Null,
            ),
        };

        g.frontend_events
            .push_back(BackendThreadFrontendEvent::BuffersFromBackend {
                streamed_data: stream_data.try_into_sync_send_wrapper()?,
                cmds: Vec::new(),
            });
        nty.notify_all();

        'outer: while !is_finished.load(std::sync::atomic::Ordering::SeqCst) {
            nty.wait_while(&mut g, |g| g.backend_events.is_empty());

            let evs = &mut *g;
            for event in evs.backend_events.drain(..) {
                match event {
                    BackendThreadBackendEvent::Init { .. } => {
                        panic!("backend is already initialized")
                    }
                    BackendThreadBackendEvent::FinishInit { .. } => {
                        panic!("backend is already initialized")
                    }
                    BackendThreadBackendEvent::RunCmds {
                        mut cmds,
                        stream_data: stream_data_cmd,
                    } => {
                        let stream_data_cmd =
                            GraphicsStreamedData::from_sync_send_wrapper(stream_data_cmd);
                        match &in_use_data {
                            InUseDataPerBackend::Vulkan(data) => {
                                let GraphicsBackendType::Vulkan(backend) = &mut backend else {
                                    return Err(anyhow!("not a vulkan backend"));
                                };
                                backend.set_stream_data_in_use(&stream_data_cmd, data)?;
                            }
                            InUseDataPerBackend::Null => {
                                // nothing to do
                            }
                        }
                        in_use_data = next_in_use_data;
                        let backend_ref = backend.as_mut();
                        backend_ref.start_commands(cmds.len(), 0);

                        for cmd in cmds.drain(..) {
                            backend_ref.run_command(cmd)?;
                        }
                        backend_ref.end_commands()?;

                        (stream_data, next_in_use_data) = match &mut backend {
                            GraphicsBackendType::Vulkan(backend) => {
                                let stream_data = backend.get_stream_data()?;

                                let mem = unsafe {
                                    stream_data.cur_stream_vertex_buffer.memories[0]
                                        .mapped_memory
                                        .get_mem_typed::<GlVertex>(
                                            StreamDataMax::MaxVertices as usize,
                                        )
                                };

                                let mut graphics_uniform_data =
                                    backend.props.graphics_uniform_buffers.new();
                                graphics_uniform_data.extend(
                                    stream_data.cur_stream_uniform_buffers.memories.iter().map(
                                        |uni| unsafe {
                                            GraphicsStreamedUniformData::new(
                                                GraphicsStreamedUniformRawData::Raw(
                                                    uni.mapped_memory.get_mem(
                                                        GRAPHICS_MAX_UNIFORM_RENDER_COUNT
                                                            * GRAPHICS_DEFAULT_UNIFORM_SIZE,
                                                    ),
                                                ),
                                            )
                                        },
                                    ),
                                );

                                (
                                    GraphicsStreamedData::new(
                                        GraphicsStreamVertices::Static(mem),
                                        graphics_uniform_data,
                                    ),
                                    InUseDataPerBackend::Vulkan(stream_data),
                                )
                            }
                            GraphicsBackendType::Null(_) => (
                                GraphicsStreamedData::new(
                                    GraphicsStreamVertices::Static(&mut []),
                                    PoolVec::new_without_pool(),
                                ),
                                InUseDataPerBackend::Null,
                            ),
                        };

                        evs.frontend_events.push_back(
                            BackendThreadFrontendEvent::BuffersFromBackend {
                                streamed_data: stream_data.try_into_sync_send_wrapper()?,
                                cmds,
                            },
                        );
                    }
                    BackendThreadBackendEvent::Stop => break 'outer,
                    BackendThreadBackendEvent::TakeScreenshot => {
                        let res = backend.as_mut().get_presented_image_data(true);

                        evs.frontend_events
                            .push_back(BackendThreadFrontendEvent::Screenshot(res));
                    }
                    BackendThreadBackendEvent::AttachFrameFetcher { name, fetcher } => {
                        backend.as_mut().attach_frame_fetcher(name, fetcher)
                    }
                    BackendThreadBackendEvent::DetachFrameFetcher { name } => {
                        backend.as_mut().detach_frame_fetcher(name)
                    }
                }
            }

            nty.notify_all();
        }

        g.backend_events.clear();
        nty.notify_all();

        Ok(())
    }
}

impl Drop for BackendThread {
    fn drop(&mut self) {
        let mut g = self.events.lock();
        self.is_finished
            .store(true, std::sync::atomic::Ordering::SeqCst);
        g.backend_events.push_back(BackendThreadBackendEvent::Stop);
        self.nty.notify_all();
        self.nty
            .wait_while(&mut g, |g| !g.backend_events.is_empty());
        drop(g);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        for (path, write_file) in std::mem::take(&mut *self.write_files.lock()) {
            let fs = self.io.fs.clone();
            self.io.io_batcher.spawn_without_lifetime(async move {
                if let Some(dir) = path.parent() {
                    fs.create_dir(dir).await?;
                }
                fs.write_file(&path, write_file).await?;
                anyhow::Ok(())
            });
        }
    }
}
