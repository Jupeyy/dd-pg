use std::sync::Arc;

use super::backend_mt::GraphicsBackendMultiThreaded;

use graphics_types::types::{GraphicsBackendMemory, GraphicsMemoryAllocationType};

/**
 * Graphics related functions that can be called from a multi-threaded environment
 * e.g. memory management which makes it faster to communicate with what the backend needs
 */
pub struct GraphicsMultiThreaded {
    backend_mt: Arc<GraphicsBackendMultiThreaded>,
}

impl GraphicsMultiThreaded {
    pub fn new(backend_mt: Arc<GraphicsBackendMultiThreaded>) -> Self {
        Self {
            backend_mt: backend_mt,
        }
    }

    /**
     * Allocates memory to be used in the backend
     */
    pub fn mem_alloc(
        &self,
        alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> GraphicsBackendMemory {
        self.backend_mt.mem_alloc(alloc_type, req_size)
    }
}
