use anyhow::anyhow;
use graphics_backend_traits::traits::GraphicsBackendMtInterface;
use graphics_types::types::{GraphicsBackendMemory, GraphicsMemoryAllocationType};

/// The multi-threaded backend part to [`graphics::graphics_mt::GraphicsMultiThreaded`]
#[derive(Debug)]
pub struct GraphicsBackendMultiThreaded {}

impl GraphicsBackendMultiThreaded {
    pub fn new() -> Self {
        Self {}
    }
}

impl GraphicsBackendMtInterface for GraphicsBackendMultiThreaded {
    fn mem_alloc(&self, alloc_type: GraphicsMemoryAllocationType) -> GraphicsBackendMemory {
        match alloc_type {
            GraphicsMemoryAllocationType::Texture {
                width,
                height,
                depth,
                ..
            } => {
                let mut res = Vec::new();
                res.resize(width * height * depth * 4, Default::default());
                GraphicsBackendMemory::Vector(res)
            }
            GraphicsMemoryAllocationType::Buffer { required_size } => {
                let mut res = Vec::new();
                res.resize(required_size, Default::default());
                GraphicsBackendMemory::Vector(res)
            }
        }
    }

    fn try_flush_mem(
        &self,
        _mem: &mut GraphicsBackendMemory,
        _do_expensive_flushing: bool,
    ) -> anyhow::Result<()> {
        Err(anyhow!("not supported."))
    }
}
