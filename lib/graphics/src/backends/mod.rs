use arrayvec::ArrayString;
use graphics_types::{
    command_buffer::{AllCommands, ERunCommandReturnTypes, SBackendCapabilites},
    rendering::GlVertex,
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType, ImageFormat},
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

    fn destroy(self);

    #[must_use]
    fn get_presented_image_data(
        &mut self,
        width: &mut u32,
        height: &mut u32,
        dest_data_buffer: &mut Vec<u8>,
    ) -> anyhow::Result<ImageFormat>;

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
    fn end_commands(&mut self) -> Result<&'static mut [GlVertex], ()>;
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
    ) -> GraphicsBackendMemory;
    fn mem_free(&self, mem: GraphicsBackendMemory);
}
