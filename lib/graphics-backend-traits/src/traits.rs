use std::{cell::RefCell, fmt::Debug, rc::Rc, sync::Arc};

use arrayvec::ArrayString;
use graphics_base_traits::traits::GraphicsStreamDataInterface;
use graphics_types::{
    command_buffer::{AllCommands, ERunCommandReturnTypes, SBackendCapabilites},
    rendering::GlVertex,
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType, ImageFormat},
};

use crate::types::BackendCommands;

pub trait DriverBackendInterface: Debug {
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
        ignore_alpha: bool,
    ) -> anyhow::Result<ImageFormat>;

    /**
     * the lifetime of cmd must be available until end_commands was called
     */
    #[must_use]
    fn run_command(&mut self, cmd: &AllCommands) -> ERunCommandReturnTypes;

    fn start_commands(
        &mut self,
        backend_buffer: &BackendCommands,
        stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>,
        command_count: usize,
        estimated_render_call_count: usize,
    );

    #[must_use]
    fn end_commands(&mut self) -> Result<&'static mut [GlVertex], ()>;
}

pub trait GraphicsBackendInterface: Debug {
    /**
     * Runs a backend buffer and swaps out the buffers the next to use
     */
    fn run_cmds(
        &self,
        buffer: &BackendCommands,
        stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>,
    );

    /**
     * The alloc_type is useful to reduce memory footprint by
     * putting similar memory types into the same heap
     */
    fn mem_alloc(
        &self,
        alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> GraphicsBackendMemory;

    fn do_screenshot(
        &self,
        width: &mut u32,
        height: &mut u32,
        dest_data_buffer: &mut Vec<u8>,
    ) -> anyhow::Result<ImageFormat>;

    fn get_backend_mt(&self) -> Arc<dyn GraphicsBackendMtInterface + Sync + Send + 'static>;
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
}
