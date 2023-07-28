use std::sync::Arc;

use crate::backend::BackendBuffer;
use base::system::System;
use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use config::config::Config;
use graphics_types::types::DrawModes;
use native::native::NativeImpl;

pub struct GraphicsLoadWhileIOPipe<'a> {
    pub config: &'a Config,
    pub runtime_threadpool: &'a Arc<rayon::ThreadPool>,
    pub sys: &'a System,
    pub window_handling: &'a mut dyn NativeImpl,
}

pub struct GraphicsLoadIOPipe<'a> {
    pub fs: &'a Arc<FileSystem>,
    pub batcher: &'a Arc<std::sync::Mutex<TokIOBatcher>>,
    pub config: &'a Config,
}
