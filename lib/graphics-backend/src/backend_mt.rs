use std::sync::Arc;

use super::backends::{null::NullBackendMt, vulkan::vulkan::VulkanBackendMt};

use graphics_backend_traits::traits::GraphicsBackendMtInterface;
use graphics_types::{
    gpu::{CurGpu, Gpu, GpuType, Gpus},
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType},
};
use hiarc::Hiarc;

#[derive(Debug, Hiarc)]
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

    pub fn gpus(&self) -> Arc<Gpus> {
        match self {
            GraphicsBackendMtType::Vulkan(backend) => backend.gpus.clone(),
            GraphicsBackendMtType::Null(_) => Arc::new(Gpus {
                auto: Gpu {
                    name: "null".to_string(),
                    ty: GpuType::Invalid,
                },
                gpus: Default::default(),
                cur: CurGpu {
                    msaa_sampling_count: 1,
                    name: "null".to_string(),
                },
            }),
        }
    }
}

/// The multi-threaded backend part to [`graphics::graphics_mt::GraphicsMultiThreaded`]
#[derive(Debug, Hiarc)]
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
