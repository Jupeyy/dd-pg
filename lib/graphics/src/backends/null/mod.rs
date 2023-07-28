use anyhow::anyhow;
use arrayvec::ArrayString;
use graphics_types::{
    command_buffer::{AllCommands, ERunCommandReturnTypes, SBackendCapabilites},
    rendering::GlVertex,
    types::GraphicsMemoryAllocationType,
};

use crate::backend::BackendBuffer;

use super::{GraphicsBackendInterface, GraphicsBackendMemory, GraphicsBackendMtInterface};

pub struct NullBackend {}

impl GraphicsBackendInterface for NullBackend {
    fn set_files(&mut self, _files: Vec<(String, Vec<u8>)>) {
        // nothing to do
    }

    #[must_use]
    fn init_while_io(
        &mut self,
        _capabilities: &mut SBackendCapabilites,
    ) -> Result<(), ArrayString<4096>> {
        // nothing to do
        Ok(())
    }

    #[must_use]
    fn init(&mut self) -> Result<(), ArrayString<4096>> {
        // nothing to do
        Ok(())
    }

    fn destroy(self) {}

    #[must_use]
    fn get_presented_image_data(
        &mut self,
        width: &mut u32,
        height: &mut u32,
        dest_data_buffer: &mut Vec<u8>,
    ) -> anyhow::Result<graphics_types::types::ImageFormat> {
        Err(anyhow!("not implemented for null backend"))
    }

    #[must_use]
    fn run_command(&mut self, _cmd: &AllCommands) -> ERunCommandReturnTypes {
        // nothing to do
        ERunCommandReturnTypes::CmdHandled
    }

    fn start_commands(
        &mut self,
        _backend_buffer: &BackendBuffer,
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

pub struct NullBackendMt {}

impl GraphicsBackendMtInterface for NullBackendMt {
    fn mem_alloc(
        &self,
        _alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> GraphicsBackendMemory {
        let mut mem: Vec<u8> = Default::default();
        mem.resize(req_size, Default::default());
        GraphicsBackendMemory::Vector(mem)
    }

    fn mem_free(&self, mem: GraphicsBackendMemory) {
        if let GraphicsBackendMemory::Static(_) = mem {
            panic!("Do not allocate custom memory!");
        }
    }
}
