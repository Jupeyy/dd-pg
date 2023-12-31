use std::{cell::RefCell, rc::Rc};

use graphics_backend_traits::{traits::GraphicsBackendInterface, types::BackendCommands};
use graphics_base_traits::traits::GraphicsStreamDataInterface;
use graphics_types::{
    commands::AllCommands,
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType},
};
use hiarc_macro::Hiarc;

#[derive(Debug, Hiarc)]
pub struct GraphicsBackendHandle {
    pub backend_cmds: BackendCommands,
    pub(crate) backend: Rc<dyn GraphicsBackendInterface>,
}

impl Clone for GraphicsBackendHandle {
    fn clone(&self) -> Self {
        Self {
            backend_cmds: self.backend_cmds.clone(),
            backend: self.backend.clone(),
        }
    }
}

impl GraphicsBackendHandle {
    pub fn run_backend_buffer(&self, stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>) {
        self.backend.run_cmds(&self.backend_cmds, stream_data);
    }

    pub fn add_cmd(&self, cmd: AllCommands) {
        self.backend_cmds.add_cmd(cmd);
    }

    pub fn mem_alloc(&self, alloc_type: GraphicsMemoryAllocationType) -> GraphicsBackendMemory {
        self.backend.mem_alloc(alloc_type)
    }
}

impl GraphicsBackendHandle {
    pub fn new(backend: Rc<dyn GraphicsBackendInterface>) -> Self {
        Self {
            backend_cmds: BackendCommands::default(),
            backend,
        }
    }
}
