use std::{
    cell::{Cell, RefCell},
    sync::Arc,
};

use graphics_backend_traits::{
    plugin::GraphicsObjectRewriteFunc,
    traits::{GraphicsBackendInterface, GraphicsBackendMtInterface},
    types::BackendCommands,
};
use graphics_base_traits::traits::GraphicsStreamedData;
use graphics_types::{
    gpu::{CurGpu, Gpu, GpuType, Gpus},
    types::GraphicsBackendMemory,
};
use pool::{mixed_pool::PoolSyncPoint, mt_datatypes::PoolVec};

use crate::upload_param;

use super::graphics_mt::GraphicsBackendMultiThreaded;

extern "C" {
    fn run_cmds();
}

#[derive(Debug)]
pub struct GraphicsBackend {
    /// only if run_cmds is called explicity, it should also run it on the host
    /// but e.g. if the module is about to end it's call, it will still flush its
    /// cmds
    pub actual_run_cmds: Cell<bool>,
    pub sync_points: RefCell<Vec<Box<dyn PoolSyncPoint>>>,
}

impl GraphicsBackendInterface for GraphicsBackend {
    fn run_cmds(&self, buffer: &BackendCommands, stream_data: &GraphicsStreamedData) {
        let cmds = buffer.take();
        upload_param(0, &cmds);
        let vertices = stream_data.used_vertices_as_vec();
        upload_param(1, vertices);
        let uniform_buffers = stream_data.serialize_uniform_instances_as_vec();
        upload_param(2, uniform_buffers);
        upload_param(3, self.actual_run_cmds.get());
        unsafe { run_cmds() };
        stream_data.reset_vertices_count();
        stream_data.reset_uniform_instances();

        for sync_point in self.sync_points.borrow().iter() {
            sync_point.sync();
        }
    }

    fn mem_alloc(
        &self,
        alloc_type: graphics_types::types::GraphicsMemoryAllocationType,
    ) -> GraphicsBackendMemory {
        let mut mem = Vec::new();
        match alloc_type {
            graphics_types::types::GraphicsMemoryAllocationType::Texture {
                width,
                height,
                depth,
                ..
            } => {
                assert!(
                    width * height * depth > 0,
                    "an allocation of zero size is not allowed."
                );
                mem.resize(width * height * depth * 4, Default::default())
            }
            graphics_types::types::GraphicsMemoryAllocationType::Buffer { required_size } => {
                assert!(
                    required_size > 0,
                    "an allocation of zero size is not allowed."
                );
                mem.resize(required_size, Default::default())
            }
        };
        GraphicsBackendMemory::Vector(mem)
    }

    fn attach_frame_fetcher(
        &self,
        _name: String,
        _fetcher: Arc<dyn graphics_backend_traits::frame_fetcher_plugin::BackendFrameFetcher>,
    ) -> anyhow::Result<()> {
        panic!("this is currently not supported over the wasm api")
    }

    fn detach_frame_fetcher(&self, _name: String) -> anyhow::Result<()> {
        panic!("this is currently not supported over the wasm api")
    }

    fn wait_idle(&self) -> anyhow::Result<()> {
        panic!("this is currently not supported over the wasm api")
    }

    fn get_backend_mt(&self) -> Arc<dyn GraphicsBackendMtInterface + Sync + Send + 'static> {
        Arc::new(GraphicsBackendMultiThreaded::new())
    }

    fn check_mod_cmd(
        &self,
        _mod_name: &str,
        _cmd: &mut PoolVec<u8>,
        _f: &dyn Fn(GraphicsObjectRewriteFunc),
    ) {
        panic!("this is not intended for a call inside the WASM module");
    }

    fn add_sync_point(&self, sync_point: Box<dyn pool::mixed_pool::PoolSyncPoint>) {
        self.sync_points.borrow_mut().push(sync_point);
    }

    fn gpus(&self) -> Arc<Gpus> {
        Arc::new(Gpus {
            gpus: Default::default(),
            auto: Gpu {
                name: "wasm".to_string(),
                ty: GpuType::Invalid,
            },
            cur: CurGpu {
                msaa_sampling_count: 1,
                name: "wasm".to_string(),
            },
        })
    }
}
