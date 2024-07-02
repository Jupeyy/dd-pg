use std::sync::Arc;

use base_io_traits::{fs_traits::FileSystemInterface, http_traits::HttpClientInterface};
use hiarc::Hiarc;

use crate::io_batcher::IoBatcher;

#[derive(Debug, Hiarc, Clone)]
pub struct IoFileSys {
    #[hiarc_skip_unsafe]
    pub fs: Arc<dyn FileSystemInterface>,
    pub io_batcher: IoBatcher,
}

impl From<Io> for IoFileSys {
    fn from(value: Io) -> Self {
        Self {
            fs: value.fs,
            io_batcher: value.io_batcher,
        }
    }
}

impl From<&Io> for IoFileSys {
    fn from(value: &Io) -> Self {
        Self {
            fs: value.fs.clone(),
            io_batcher: value.io_batcher.clone(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub type AsyncRuntime = tokio::runtime::Runtime;
#[cfg(target_arch = "wasm32")]
pub type AsyncRuntime<'a> = async_executor::LocalExecutor<'a>;

impl IoFileSys {
    pub fn new(fs_builder: impl FnOnce(&AsyncRuntime) -> Arc<dyn FileSystemInterface>) -> Self {
        // tokio runtime for client side tasks
        #[cfg(not(target_arch = "wasm32"))]
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4) // should be at least 4
            .max_blocking_threads(4) // must be at least 4
            .enable_time()
            .enable_io()
            .build()
            .unwrap();

        #[cfg(target_arch = "wasm32")]
        let rt = async_executor::LocalExecutor::new();

        Self {
            fs: fs_builder(&rt),
            io_batcher: IoBatcher::new(rt),
        }
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct Io {
    #[hiarc_skip_unsafe]
    pub fs: Arc<dyn FileSystemInterface>,
    #[hiarc_skip_unsafe]
    pub http: Arc<dyn HttpClientInterface>,
    pub io_batcher: IoBatcher,
}

impl Io {
    pub fn new(
        fs_builder: impl FnOnce(&AsyncRuntime) -> Arc<dyn FileSystemInterface>,
        http: Arc<dyn HttpClientInterface>,
    ) -> Self {
        let io_fs = IoFileSys::new(fs_builder);

        Self {
            fs: io_fs.fs,
            http,
            io_batcher: io_fs.io_batcher,
        }
    }

    pub fn from(io: IoFileSys, http: Arc<dyn HttpClientInterface>) -> Self {
        Self {
            fs: io.fs,
            http,
            io_batcher: io.io_batcher,
        }
    }
}
