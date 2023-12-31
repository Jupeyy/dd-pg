use std::{collections::HashMap, rc::Rc};

use base::shared_index::{SharedIndexCleanup, SharedIndexGetIndexUnsafe};
use graphics_types::{
    commands::{
        AllCommands, CommandCreateBufferObject, CommandDeleteBufferObject,
        CommandRecreateBufferObject, Commands,
    },
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType},
};
use hiarc_macro::{hiarc_safer_rc_refcell, Hiarc};

use crate::{buffer_object_handle::BufferObjectIndex, types::GraphicsBufferObject};

use super::backend::GraphicsBackendHandle;

#[hiarc_safer_rc_refcell]
#[derive(Debug, Hiarc)]
pub struct GraphicsBufferObjectHandle {
    buffer_objects: HashMap<u128, GraphicsBufferObject>,
    id_gen: u128,

    #[hiarc]
    backend_handle: GraphicsBackendHandle,
}

/*
impl Clone for GraphicsBufferObjectHandle {
    fn clone(&self) -> Self {
        Self {
            buffer_objects: self.buffer_objects.clone(),
            id_gen: self.id_gen.clone(),
            backend_handle: self.backend_handle.clone(),
        }
    }
}*/

#[hiarc_safer_rc_refcell]
impl SharedIndexCleanup for GraphicsBufferObjectHandle {
    #[hiarc_trait_is_immutable_self]
    fn destroy_from_index(&mut self, index: u128) {
        self.buffer_objects.remove(&index).unwrap();
        let cmd = CommandDeleteBufferObject {
            buffer_index: index,
        };

        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::DeleteBufferObject(cmd)));
    }
}

#[hiarc_safer_rc_refcell]
impl GraphicsBufferObjectHandle {
    pub fn new(backend_handle: GraphicsBackendHandle) -> Self {
        Self {
            buffer_objects: HashMap::new(),
            id_gen: Default::default(),

            backend_handle,
        }
    }

    #[hiarc_force_impl]
    fn create_buffer_object_inner(&mut self, upload_data: GraphicsBackendMemory) -> u128 {
        self.id_gen += 1;
        let index = self.id_gen;

        let upload_len = upload_data.len();

        let cmd = CommandCreateBufferObject {
            buffer_index: index,
            upload_data: upload_data,
        };

        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::CreateBufferObject(cmd)));

        self.buffer_objects.insert(
            index,
            GraphicsBufferObject {
                alloc_size: upload_len,
            },
        );
        index
    }

    pub fn recreate_buffer_object(
        &mut self,
        buffer_index: &BufferObjectIndex,
        upload_data: GraphicsBackendMemory,
    ) {
        let upload_len = upload_data.len();

        let cmd = CommandRecreateBufferObject {
            buffer_index: buffer_index.get_index_unsafe(),
            upload_data: upload_data,
        };

        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::RecreateBufferObject(cmd)));

        self.buffer_objects
            .get_mut(&buffer_index.get_index_unsafe())
            .unwrap()
            .alloc_size = upload_len;
    }

    pub fn create_buffer_object_slow_inner(&mut self, upload_data: Vec<u8>) -> u128 {
        let mut buffer_mem = self
            .backend_handle
            .mem_alloc(GraphicsMemoryAllocationType::Buffer {
                required_size: upload_data.len(),
            });
        buffer_mem.copy_from_slice(&upload_data);
        self.create_buffer_object_inner(buffer_mem)
    }

    pub fn recreate_buffer_object_slow(
        &mut self,
        buffer_index: &BufferObjectIndex,
        upload_data: Vec<u8>,
    ) {
        let mut buffer_mem = self
            .backend_handle
            .mem_alloc(GraphicsMemoryAllocationType::Buffer {
                required_size: upload_data.len(),
            });
        buffer_mem.copy_from_slice(&upload_data);
        self.recreate_buffer_object(buffer_index, buffer_mem)
    }
}

impl GraphicsBufferObjectHandle {
    pub fn create_buffer_object(&self, upload_data: GraphicsBackendMemory) -> BufferObjectIndex {
        BufferObjectIndex::new(
            self.create_buffer_object_inner(upload_data),
            Rc::new(self.clone()),
        )
    }

    pub fn create_buffer_object_slow(&self, upload_data: Vec<u8>) -> BufferObjectIndex {
        BufferObjectIndex::new(
            self.create_buffer_object_slow_inner(upload_data),
            Rc::new(self.clone()),
        )
    }
}
