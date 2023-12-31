use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use graphics_backend_traits::{
    frame_fetcher_plugin::BackendPresentedImageData,
    traits::{GraphicsBackendInterface, GraphicsBackendMtInterface},
    types::BackendCommands,
};
use graphics_base_traits::traits::GraphicsStreamDataInterface;
use graphics_types::{commands::StreamDataMax, rendering::SVertex, types::GraphicsBackendMemory};

use crate::upload_param;

use super::graphics_mt::GraphicsBackendMultiThreaded;

extern "C" {
    fn run_cmds();
}

#[derive(Debug)]
pub struct BackendStreamData {
    vertices: Vec<SVertex>,
    vertices_count: usize,
}

impl BackendStreamData {
    pub fn new() -> Self {
        let mut vertices: Vec<_> = Vec::new();
        vertices.resize(StreamDataMax::MaxVertices as usize, Default::default());
        Self {
            vertices,
            vertices_count: 0,
        }
    }
}

impl GraphicsStreamDataInterface for BackendStreamData {
    fn vertices(&self) -> &[SVertex] {
        &self.vertices[..]
    }

    fn vertices_mut(&mut self) -> &mut [SVertex] {
        &mut self.vertices[..]
    }

    fn vertices_count(&self) -> usize {
        self.vertices_count
    }

    fn vertices_count_mut(&mut self) -> &mut usize {
        &mut self.vertices_count
    }

    fn vertices_and_count(&self) -> (&[SVertex], &usize) {
        (&self.vertices, &self.vertices_count)
    }

    fn vertices_and_count_mut(&mut self) -> (&mut [SVertex], &mut usize) {
        (&mut self.vertices, &mut self.vertices_count)
    }

    fn allocate_uniform_instance(&mut self) -> anyhow::Result<usize> {
        todo!()
    }

    fn get_sprites_uniform_instance(
        &mut self,
        _instance: usize,
    ) -> graphics_base_traits::traits::GraphicsStreamedUniformDataSpritesBorrowMut {
        todo!()
    }

    fn get_arbitrary_uniform_instance(
        &mut self,
        _instance: usize,
        _size_of_el: usize,
    ) -> graphics_base_traits::traits::GraphicsArbitraryUniformDataSpritesBorrowMut {
        todo!()
    }

    fn uniform_instance_count(&self) -> usize {
        todo!()
    }

    fn uniform_used_count_of_instance(
        &self,
        _instance_index: usize,
    ) -> graphics_base_traits::traits::GraphicsStreamedUniformDataType {
        todo!()
    }

    fn set_from_graphics_streamed_data(
        &mut self,
        _streamed_data: graphics_base_traits::traits::GraphicsStreamedData,
    ) {
        panic!("this should not be called from a mod")
    }
}

#[derive(Debug)]
pub struct GraphicsBackend {
    /// only if run_cmds is called explicity, it should also run it on the host
    /// but e.g. if the module is about to end it's call, it will still flush its
    /// cmds
    pub actual_run_cmds: Cell<bool>,
}

impl GraphicsBackendInterface for GraphicsBackend {
    fn run_cmds(
        &self,
        buffer: &BackendCommands,
        stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>,
    ) {
        let cmds = buffer.take();
        upload_param(0, &cmds);
        let stream_data_ref = stream_data.borrow();
        let (vertices, count) = stream_data_ref.vertices_and_count();
        upload_param(1, vertices[0..*count].to_vec());
        upload_param(2, self.actual_run_cmds.get());
        unsafe { run_cmds() };
        drop(stream_data_ref);
        *stream_data.borrow_mut().vertices_count_mut() = 0;
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
}
