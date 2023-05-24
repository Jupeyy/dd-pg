use std::sync::Arc;

use crate::backend::BackendBuffer;
use base::{config::Config, filesys::FileSystem, io_batcher::IOBatcher, system::System};
use graphics_types::types::DrawModes;

pub struct GraphicsLoadWhileIOPipe<'a> {
    pub config: &'a Config,
    pub runtime_threadpool: &'a Arc<rayon::ThreadPool>,
    pub sys: &'a System,
}

pub struct GraphicsLoadIOPipe<'a> {
    pub fs: &'a Arc<FileSystem>,
    pub batcher: &'a Arc<std::sync::Mutex<IOBatcher>>,
    pub config: &'a Config,
}
