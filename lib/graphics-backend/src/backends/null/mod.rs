use std::{cell::RefCell, rc::Rc};

use anyhow::anyhow;
use graphics_backend_traits::{
    frame_fetcher_plugin::BackendPresentedImageData,
    traits::{DriverBackendInterface, GraphicsBackendMtInterface},
    types::BackendCommands,
};
use graphics_base_traits::traits::{GraphicsStreamDataInterface, GraphicsStreamedData};
use graphics_types::{
    commands::AllCommands,
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType},
};
use pool::mt_datatypes::PoolVec;

#[derive(Debug)]
pub struct NullBackend {}

impl NullBackend {
    pub fn get_mt_backend(&self) -> NullBackendMt {
        NullBackendMt {}
    }
}

impl DriverBackendInterface for NullBackend {
    fn get_presented_image_data(
        &mut self,
        _ignore_alpha: bool,
    ) -> anyhow::Result<BackendPresentedImageData> {
        Err(anyhow!("not implemented for null backend"))
    }

    fn attach_frame_fetcher(
        &mut self,
        _name: String,
        _fetcher: std::sync::Arc<
            dyn graphics_backend_traits::frame_fetcher_plugin::BackendFrameFetcher,
        >,
    ) {
        // do nothing
    }

    fn detach_frame_fetcher(&mut self, _name: String) {
        // do nothing
    }

    fn run_command(&mut self, _cmd: AllCommands) -> anyhow::Result<()> {
        // nothing to do
        Ok(())
    }

    fn start_commands(
        &mut self,
        _backend_buffer: &BackendCommands,
        _stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>,
        _command_count: usize,
        _estimated_render_call_count: usize,
    ) {
        // nothing to do
    }

    fn end_commands(&mut self) -> anyhow::Result<GraphicsStreamedData> {
        // nothing to do
        Ok(GraphicsStreamedData::new(
            &mut [],
            PoolVec::new_without_pool(),
        ))
    }
}

#[derive(Debug)]
pub struct NullBackendMt {}

impl GraphicsBackendMtInterface for NullBackendMt {
    fn mem_alloc(&self, alloc_type: GraphicsMemoryAllocationType) -> GraphicsBackendMemory {
        let mut mem: Vec<u8> = Default::default();
        match alloc_type {
            GraphicsMemoryAllocationType::Texture {
                width,
                height,
                depth,
                ..
            } => mem.resize(width * height * depth * 4, Default::default()),
            GraphicsMemoryAllocationType::Buffer { required_size } => {
                mem.resize(required_size, Default::default())
            }
        }
        GraphicsBackendMemory::Vector(mem)
    }

    fn try_flush_mem(
        &self,
        _mem: &mut GraphicsBackendMemory,
        _do_expensive_flushing: bool,
    ) -> anyhow::Result<()> {
        Err(anyhow!("this operation is not supported."))
    }
}
