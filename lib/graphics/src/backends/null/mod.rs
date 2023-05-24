use arrayvec::ArrayString;
use graphics_types::{
    command_buffer::{AllCommands, ERunCommandReturnTypes, SBackendCapabilites},
    rendering::GL_SVertex,
    types::GraphicsMemoryAllocationType,
};

use crate::backend::BackendBuffer;

use super::{GraphicsBackendInterface, GraphicsBackendMtInterface};

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

    #[must_use]
    fn run_command(&mut self, _cmd: &AllCommands) -> ERunCommandReturnTypes {
        // nothing to do
        ERunCommandReturnTypes::RUN_COMMAND_COMMAND_HANDLED
    }

    fn start_commands(
        &mut self,
        _backend_buffer: &BackendBuffer,
        _command_count: usize,
        _estimated_render_call_count: usize,
    ) {
        // nothing to do
    }

    fn end_commands(&mut self) -> Result<&'static mut [GL_SVertex], ()> {
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
    ) -> &'static mut [u8] {
        unsafe {
            let mem = libc::malloc(req_size);
            std::slice::from_raw_parts_mut(mem as *mut u8, req_size)
        }
    }

    fn mem_free(&self, mem: &'static mut [u8]) {
        unsafe { libc::free(mem.as_ptr() as *mut libc::c_void) }
    }
}
