use std::{rc::Rc, sync::Arc};

use base::benchmark::Benchmark;
use base_fs::filesys::FileSystem;
use base_http::http::HttpClient;
use base_io::io::{Io, IoFileSys};
use config::{config::ConfigBackend, types::ConfRgb};
use graphics::graphics::graphics::Graphics;
use graphics_backend::{
    backend::{
        GraphicsBackend, GraphicsBackendBase, GraphicsBackendIoLoading, GraphicsBackendLoading,
    },
    window::{BackendRawDisplayHandle, BackendWindow},
};
use graphics_base_traits::traits::GraphicsStreamedData;
use graphics_types::types::WindowProps;
use rayon::ThreadPool;
use sound::sound::SoundManager;
use sound_backend::sound_backend::SoundBackend;

fn prepare_backend(
    io: &Io,
    tp: &Arc<ThreadPool>,
    config_gl: &ConfigBackend,
    config_wnd: &config::config::ConfigWindow,
    backend_validation: bool,
) -> (Rc<GraphicsBackend>, GraphicsStreamedData) {
    let config_gfx = config::config::ConfigGfx::default();
    let io_loading = GraphicsBackendIoLoading::new(&config_gfx, &io.clone().into());
    let config_dbg = config::config::ConfigDebug {
        bench: true,
        gfx: if backend_validation {
            config::config::GfxDebugModes::All
        } else {
            config::config::GfxDebugModes::None
        },
        ..Default::default()
    };

    let bench = Benchmark::new(true);
    let backend_loading = GraphicsBackendLoading::new(
        &config_gfx,
        &config_dbg,
        config_gl,
        BackendRawDisplayHandle::Headless,
        None,
        io.clone().into(),
    )
    .unwrap();
    bench.bench("backend loading");
    let (backend_base, stream_data) = GraphicsBackendBase::new(
        io_loading,
        backend_loading,
        tp,
        BackendWindow::Headless {
            width: config_wnd.width,
            height: config_wnd.height,
        },
    )
    .unwrap();
    bench.bench("backend base init");
    let backend = GraphicsBackend::new(backend_base);
    bench.bench("backend init");

    (backend, stream_data)
}

pub fn get_base(
    backend_validation: bool,
) -> (
    Io,
    Arc<ThreadPool>,
    Graphics,
    Rc<GraphicsBackend>,
    SoundManager,
) {
    let io = IoFileSys::new(|rt| {
        Arc::new(FileSystem::new(
            rt,
            "ddnet-test",
            "ddnet-test",
            "ddnet-test",
            "ddnet-test",
        ))
    });
    let tp = Arc::new(
        rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap(),
    );

    let io = Io::from(io, Arc::new(HttpClient::new()));

    let config_gl = ConfigBackend {
        clear_color: ConfRgb::grey(),
        full_pipeline_creation: false,
        ..Default::default()
    };
    let config_wnd = Default::default();
    let (backend, stream_data) =
        prepare_backend(&io, &tp, &config_gl, &config_wnd, backend_validation);

    let sound_backend = SoundBackend::new(&config::config::ConfigSound {
        backend: "None".to_string(),
        limits: Default::default(),
    })
    .unwrap();
    let sound = SoundManager::new(sound_backend.clone()).unwrap();

    (
        io,
        tp,
        Graphics::new(
            backend.clone(),
            stream_data,
            WindowProps {
                canvas_width: config_wnd.width as f64,
                canvas_height: config_wnd.height as f64,
                window_width: config_wnd.width,
                window_height: config_wnd.height,
            },
        ),
        backend,
        sound,
    )
}
