use std::sync::Arc;

use base::system::System;
use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use config::config::Config;

use graphics::graphics::GraphicsBase;
use native::native::NativeImpl;

use crate::backend::GraphicsBackend;

pub struct GraphicsBackendLoadIOPipe<'a> {
    pub fs: &'a Arc<FileSystem>,
    pub io_batcher: &'a TokIOBatcher,
    pub config: &'a Config,
}

pub struct GraphicsBackendLoadWhileIOPipe<'a> {
    pub config: &'a Config,
    pub runtime_threadpool: &'a Arc<rayon::ThreadPool>,
    pub sys: &'a System,
    pub window_handling: &'a mut dyn NativeImpl,
}

pub type Graphics = GraphicsBase<GraphicsBackend>;
