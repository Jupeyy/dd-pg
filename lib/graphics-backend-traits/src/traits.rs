use std::{cell::RefCell, fmt::Debug, rc::Rc, sync::Arc};

use graphics_base_traits::traits::GraphicsStreamDataInterface;
use graphics_types::{
    command_buffer::{AllCommands, SBackendCapabilites},
    rendering::GlVertex,
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType, ImageFormat},
};

use crate::types::BackendCommands;

pub trait DriverBackendInterface: Debug {
    fn set_files(&mut self, files: Vec<(String, Vec<u8>)>);

    fn init_while_io(&mut self, capabilities: &mut SBackendCapabilites) -> anyhow::Result<()>;

    fn init(&mut self) -> anyhow::Result<()>;

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
    fn run_command(&mut self, cmd: AllCommands) -> anyhow::Result<()>;

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

pub trait GraphicsBackendInterface: Debug + Clone + 'static {
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
    fn mem_alloc(&self, alloc_type: GraphicsMemoryAllocationType) -> GraphicsBackendMemory;

    fn do_screenshot(
        &self,
        width: &mut u32,
        height: &mut u32,
        dest_data_buffer: &mut Vec<u8>,
    ) -> anyhow::Result<ImageFormat>;

    fn get_backend_mt(&self) -> Arc<dyn GraphicsBackendMtInterface + Sync + Send + 'static>;
}

pub trait GraphicsBackendMtInterface {
    /// The alloc_type is useful to reduce memory footprint by
    /// putting similar memory types into the same heap
    fn mem_alloc(&self, alloc_type: GraphicsMemoryAllocationType) -> GraphicsBackendMemory;

    /// Tries to flush backend memory, which allows to skip doing so
    /// on runtime on the main thread. It's important however to understand
    /// that the backend can have various reasons not to flush in this moment.
    /// In any case the memory should still be applied to the graphics implementation
    /// normally (create texture, create buffer).
    /// The rule of thumb is to always try to call this, when you are loading inside
    /// a thread
    /// `do_expensive_flushing` determines if the implementation is allowed to use a expensive flushing mechanism
    /// this is generally recommended since this function should only be called from a function anyway,
    /// but it might cost more performance in some cases which makes it undesirable for situations where
    /// it is better to have fewest possible runtime (e.g. loading a map, which is required to proceed the game)
    /// ### Note: any memory related operations after flushing might be ignore until another flush operation
    /// ### is called. In other words, if you tried to flushed manually once, you always have to flush.
    /// ### The implementation has no mechanism to know if memory was changed afterwards.
    /// ### If you don't flush at all, the backend will do it automatically.
    fn try_flush_mem(
        &self,
        mem: &mut GraphicsBackendMemory,
        do_expensive_flushing: bool,
    ) -> anyhow::Result<()>;
}
