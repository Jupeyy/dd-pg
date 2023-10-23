use std::sync::Arc;

use base_fs_traits::traits::FileSystemInterface;

use crate::io_batcher::TokIOBatcher;

#[derive(Debug, Clone)]
pub struct IO {
    pub fs: Arc<dyn FileSystemInterface>,
    pub io_batcher: TokIOBatcher,
}

impl IO {
    pub fn new(fs: Arc<dyn FileSystemInterface>) -> Self {
        // tokio runtime for client side tasks
        #[cfg(not(target_arch = "wasm32"))]
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4) // should be at least 4
            .max_blocking_threads(4) // must be at least 4
            .build()
            .unwrap();

        #[cfg(target_arch = "wasm32")]
        let rt = async_executor::LocalExecutor::new();

        Self {
            fs,
            io_batcher: TokIOBatcher::new(rt),
        }
    }
}
