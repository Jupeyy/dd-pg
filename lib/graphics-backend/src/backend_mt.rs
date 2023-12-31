use super::backends::{null::NullBackendMt, vulkan::vulkan::VulkanBackendMt};

use graphics_backend_traits::traits::GraphicsBackendMtInterface;
use graphics_types::types::{GraphicsBackendMemory, GraphicsMemoryAllocationType};

#[derive(Debug)]
pub enum GraphicsBackendMtType {
    Vulkan(VulkanBackendMt),
    Null(NullBackendMt),
}

impl GraphicsBackendMtType {
    pub fn unwrap(&self) -> &dyn GraphicsBackendMtInterface {
        match self {
            Self::Vulkan(backend) => backend,
            Self::Null(backend) => backend,
        }
    }
}

/**
 * The multi-threaded backend part to `GraphicsMultiThreaded`
 */
#[derive(Debug)]
pub struct GraphicsBackendMultiThreaded {
    pub backend_mt: GraphicsBackendMtType,
}

impl GraphicsBackendMtInterface for GraphicsBackendMultiThreaded {
    fn mem_alloc(&self, alloc_type: GraphicsMemoryAllocationType) -> GraphicsBackendMemory {
        self.backend_mt.unwrap().mem_alloc(alloc_type)
    }

    fn try_flush_mem(
        &self,
        mem: &mut GraphicsBackendMemory,
        do_expensive_flushing: bool,
    ) -> anyhow::Result<()> {
        self.backend_mt
            .unwrap()
            .try_flush_mem(mem, do_expensive_flushing)
    }
}
