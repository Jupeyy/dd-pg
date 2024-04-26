use std::{
    cell::{Cell, RefCell},
    sync::Arc,
};

use graphics_backend_traits::{
    frame_fetcher_plugin::BackendPresentedImageData,
    traits::{GraphicsBackendInterface, GraphicsBackendMtInterface},
    types::BackendCommands,
};
use graphics_base_traits::traits::GraphicsStreamedData;
use graphics_types::{
    rendering::{StateTexture, StateTexture2dArray},
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
            } => mem.resize(width * height * depth * 4, Default::default()),
            graphics_types::types::GraphicsMemoryAllocationType::Buffer { required_size } => {
                mem.resize(required_size, Default::default())
            }
        };
        GraphicsBackendMemory::Vector(mem)
    }

    fn do_screenshot(&self) -> anyhow::Result<BackendPresentedImageData> {
        panic!("this is currently not supported over the wasm api")
    }

    fn attach_frame_fetcher(
        &self,
        _name: String,
        _fetcher: Arc<dyn graphics_backend_traits::frame_fetcher_plugin::BackendFrameFetcher>,
    ) {
        panic!("this is currently not supported over the wasm api")
    }

    fn detach_frame_fetcher(&self, _name: String) {
        panic!("this is currently not supported over the wasm api")
    }

    fn get_backend_mt(&self) -> Arc<dyn GraphicsBackendMtInterface + Sync + Send + 'static> {
        Arc::new(GraphicsBackendMultiThreaded::new())
    }

    fn check_mod_cmd(
        &self,
        _mod_name: &str,
        _cmd: &mut PoolVec<u8>,
        _f: &dyn Fn(&mut [&mut StateTexture], &mut [&mut StateTexture2dArray], &mut [&mut u128]),
    ) {
        panic!("this is not intended for a call inside the WASM module");
    }

    fn add_sync_point(&self, sync_point: Box<dyn pool::mixed_pool::PoolSyncPoint>) {
        self.sync_points.borrow_mut().push(sync_point);
    }
}
