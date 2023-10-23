use std::{cell::RefCell, rc::Rc};

use anyhow::anyhow;
use graphics_backend_traits::{
    traits::{DriverBackendInterface, GraphicsBackendMtInterface},
    types::BackendCommands,
};
use graphics_base_traits::traits::GraphicsStreamDataInterface;
use graphics_types::{
    command_buffer::{AllCommands, SBackendCapabilites},
    rendering::GlVertex,
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType},
};

#[derive(Debug)]
pub struct NullBackend {}

impl NullBackend {
    pub fn get_mt_backend(&self) -> NullBackendMt {
        NullBackendMt {}
    }
}

impl DriverBackendInterface for NullBackend {
    fn set_files(&mut self, _files: Vec<(String, Vec<u8>)>) {
        // nothing to do
    }

    fn init_while_io(&mut self, _capabilities: &mut SBackendCapabilites) -> anyhow::Result<()> {
        // nothing to do
        Ok(())
    }

    fn init(&mut self) -> anyhow::Result<()> {
        // nothing to do
        Ok(())
    }

    fn destroy(self) {}

    fn get_presented_image_data(
        &mut self,
        _width: &mut u32,
        _height: &mut u32,
        _dest_data_buffer: &mut Vec<u8>,
        _ignore_alpha: bool,
    ) -> anyhow::Result<graphics_types::types::ImageFormat> {
        Err(anyhow!("not implemented for null backend"))
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

    fn end_commands(&mut self) -> Result<&'static mut [GlVertex], ()> {
        // nothing to do
        Ok(&mut [])
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
