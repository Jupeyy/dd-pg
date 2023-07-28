use super::backends::{
    null::NullBackendMt, vulkan::vulkan::VulkanBackendMt, GraphicsBackendMtInterface,
};

use graphics_types::types::{GraphicsBackendMemory, GraphicsMemoryAllocationType};

pub enum GraphicsBackendMtType {
    Vulkan(VulkanBackendMt),
    Null(NullBackendMt),
    None,
}

impl GraphicsBackendMtType {
    pub fn unwrap(&self) -> &dyn GraphicsBackendMtInterface {
        match self {
            GraphicsBackendMtType::Vulkan(backend) => backend,
            GraphicsBackendMtType::Null(backend) => backend,
            GraphicsBackendMtType::None => panic!("Use of 'none' mt backend"),
        }
    }
}

/**
 * The multi-threaded backend part to `GraphicsMultiThreaded`
 */
pub struct GraphicsBackendMultiThreaded {
    pub backend_mt: GraphicsBackendMtType,
}

impl GraphicsBackendMultiThreaded {
    pub fn new() -> Self {
        Self {
            backend_mt: GraphicsBackendMtType::None,
        }
    }

    pub fn mem_alloc(
        &self,
        alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> GraphicsBackendMemory {
        self.backend_mt.unwrap().mem_alloc(alloc_type, req_size)
    }

    pub fn mem_free(&self, mem: GraphicsBackendMemory) {
        self.backend_mt.unwrap().mem_free(mem)
    }
}
