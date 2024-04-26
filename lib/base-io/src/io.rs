use std::sync::Arc;

use base_io_traits::{fs_traits::FileSystemInterface, http_traits::HttpClientInterface};
use hiarc::Hiarc;

use crate::io_batcher::IOBatcher;

#[derive(Debug, Hiarc, Clone)]
pub struct IOFileSys {
    #[hiarc_skip_unsafe]
    pub fs: Arc<dyn FileSystemInterface>,
    pub io_batcher: IOBatcher,
}

impl From<IO> for IOFileSys {
    fn from(value: IO) -> Self {
        Self {
            fs: value.fs,
            io_batcher: value.io_batcher,
        }
    }
}

impl From<&IO> for IOFileSys {
    fn from(value: &IO) -> Self {
        Self {
            fs: value.fs.clone(),
            io_batcher: value.io_batcher.clone(),
        }
    }
}

impl IOFileSys {
    pub fn new(fs: Arc<dyn FileSystemInterface>) -> Self {
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
            fs,
            io_batcher: IOBatcher::new(rt),
        }
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct IO {
    #[hiarc_skip_unsafe]
    pub fs: Arc<dyn FileSystemInterface>,
    #[hiarc_skip_unsafe]
    pub http: Arc<dyn HttpClientInterface>,
    pub io_batcher: IOBatcher,
}

impl IO {
    pub fn new(fs: Arc<dyn FileSystemInterface>, http: Arc<dyn HttpClientInterface>) -> Self {
        let io_fs = IOFileSys::new(fs);

        Self {
            fs: io_fs.fs,
            http,
            io_batcher: io_fs.io_batcher,
        }
    }

    pub fn from(io: IOFileSys, http: Arc<dyn HttpClientInterface>) -> Self {
        Self {
            fs: io.fs,
            http,
            io_batcher: io.io_batcher,
        }
    }
}
