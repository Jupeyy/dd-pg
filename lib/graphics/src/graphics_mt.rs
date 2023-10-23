use std::sync::Arc;

use graphics_backend_traits::traits::GraphicsBackendMtInterface;
use graphics_types::types::{GraphicsBackendMemory, GraphicsMemoryAllocationType};

/**
 * Graphics related functions that can be called from a multi-threaded environment
 * e.g. memory management which makes it faster to communicate with what the backend needs
 */
pub struct GraphicsMultiThreaded {
    backend_mt: Arc<dyn GraphicsBackendMtInterface + Send + Sync + 'static>,
}

impl GraphicsMultiThreaded {
    pub fn new(backend_mt: Arc<dyn GraphicsBackendMtInterface + Send + Sync + 'static>) -> Self {
        Self { backend_mt }
    }

    /**
     * Allocates memory to be used in the backend
     */
    pub fn mem_alloc(&self, alloc_type: GraphicsMemoryAllocationType) -> GraphicsBackendMemory {
        self.backend_mt.mem_alloc(alloc_type)
    }

    pub fn try_flush_mem(
        &self,
        mem: &mut GraphicsBackendMemory,
        do_expensive_flushing: bool,
    ) -> anyhow::Result<()> {
        self.backend_mt.try_flush_mem(mem, do_expensive_flushing)
    }
}
