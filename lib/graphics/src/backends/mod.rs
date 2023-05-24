use arrayvec::ArrayString;
use graphics_types::{
    command_buffer::{AllCommands, ERunCommandReturnTypes, SBackendCapabilites},
    rendering::GL_SVertex,
    types::GraphicsMemoryAllocationType,
};

use crate::backend::BackendBuffer;

pub mod null;
pub mod vulkan;

pub trait GraphicsBackendInterface {
    fn set_files(&mut self, files: Vec<(String, Vec<u8>)>);

    fn init_while_io(
        &mut self,
        capabilities: &mut SBackendCapabilites,
    ) -> Result<(), ArrayString<4096>>;

    fn init(&mut self) -> Result<(), ArrayString<4096>>;

    /**
     * the lifetime of cmd must be available until end_commands was called
     */
    #[must_use]
    fn run_command(&mut self, cmd: &AllCommands) -> ERunCommandReturnTypes;

    fn start_commands(
        &mut self,
        backend_buffer: &BackendBuffer,
        command_count: usize,
        estimated_render_call_count: usize,
    );

    #[must_use]
    fn end_commands(&mut self) -> Result<&'static mut [GL_SVertex], ()>;
}

pub trait GraphicsBackendMtInterface {
    /**
     * The alloc_type is useful to reduce memory footprint by
     * putting similar memory types into the same heap
     */
    fn mem_alloc(
        &self,
        alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> &'static mut [u8];
    fn mem_free(&self, mem: &'static mut [u8]);
}
