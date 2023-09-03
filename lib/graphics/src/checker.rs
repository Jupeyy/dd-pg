use std::{cell::RefCell, collections::HashMap};

use graphics_backend_traits::traits::GraphicsBackendInterface;
use graphics_types::{
    command_buffer::{AllCommands, Commands, CommandsRender, PrimType},
    rendering::State,
};

use crate::{
    graphics::GraphicsStreamHandle,
    types::{GraphicsBufferObject, TextureContainer},
};

/// rewrites graphics commands to make them use resource indices
/// that are not colliding with host indices
/// Important: Does not validate correctness in any kind.
pub struct GraphicsContainersAPI {
    textures: RefCell<HashMap<u128, TextureContainer>>,
    buffers: RefCell<HashMap<u128, GraphicsBufferObject>>,

    id_offset: u128,
}

impl GraphicsContainersAPI {
    pub fn new(id_offset: u128) -> Self {
        Self {
            textures: Default::default(),
            buffers: Default::default(),
            id_offset,
        }
    }

    fn process_state(&self, state: &mut State) {
        if let Some(index) = state.texture_index {
            assert!(index < u64::MAX as u128, "invalid index");
            let real_index = index + self.id_offset;

            assert!(
                self.textures.borrow().contains_key(&real_index),
                "texture does not exists, this is not allowed"
            );
            state.texture_index = Some(real_index);
        }
    }

    fn process_command<B: GraphicsBackendInterface>(
        &self,
        stream_handle: &GraphicsStreamHandle<B>,
        cmd: &mut AllCommands,
        vertices_offset_before_commands: usize,
    ) {
        match cmd {
            AllCommands::Render(cmd) => match cmd {
                CommandsRender::TileLayer(cmd) => {
                    assert!(cmd.buffer_object_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.buffer_object_index + self.id_offset;
                    cmd.buffer_object_index = real_index;
                    self.process_state(&mut cmd.state);
                }
                CommandsRender::BorderTile(cmd) => {
                    assert!(cmd.buffer_object_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.buffer_object_index + self.id_offset;
                    cmd.buffer_object_index = real_index;
                    self.process_state(&mut cmd.state);
                }
                CommandsRender::BorderTileLine(cmd) => {
                    assert!(cmd.buffer_object_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.buffer_object_index + self.id_offset;
                    cmd.buffer_object_index = real_index;
                    self.process_state(&mut cmd.state);
                }
                CommandsRender::QuadLayer(cmd) => {
                    assert!(cmd.buffer_object_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.buffer_object_index + self.id_offset;
                    cmd.buffer_object_index = real_index;
                    self.process_state(&mut cmd.state);
                }
                CommandsRender::QuadContainerEx(cmd) => {
                    assert!(cmd.buffer_object_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.buffer_object_index + self.id_offset;
                    cmd.buffer_object_index = real_index;
                    self.process_state(&mut cmd.state);
                }
                CommandsRender::QuadContainerSpriteMultiple(cmd) => {
                    assert!(cmd.buffer_object_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.buffer_object_index + self.id_offset;
                    cmd.buffer_object_index = real_index;
                    self.process_state(&mut cmd.state);
                }
                CommandsRender::Render(cmd) => {
                    self.process_state(&mut cmd.state);

                    let count = stream_handle.stream_data.borrow().vertices_count();
                    let vert_per_prim = match cmd.prim_type {
                        PrimType::Lines => 2,
                        PrimType::Quads => 4,
                        PrimType::Triangles => 3,
                    };
                    cmd.vertices_offset = cmd
                        .vertices_offset
                        .checked_add(vertices_offset_before_commands)
                        .unwrap();
                    assert!(
                        count >= cmd.prim_count * vert_per_prim + cmd.vertices_offset,
                        "Not enough vertices in the stream handle."
                    );
                }
                CommandsRender::RenderTex3D(cmd) => {
                    self.process_state(&mut cmd.state);

                    let count = stream_handle.stream_data.borrow().vertices_count();
                    let vert_per_prim = match cmd.prim_type {
                        PrimType::Lines => 2,
                        PrimType::Quads => 4,
                        PrimType::Triangles => 3,
                    };
                    cmd.vertices_offset = cmd
                        .vertices_offset
                        .checked_add(vertices_offset_before_commands)
                        .unwrap();
                    assert!(
                        count >= cmd.prim_count * vert_per_prim + cmd.vertices_offset,
                        "Not enough vertices in the stream handle."
                    );
                    todo!("this must use an extra stream handle designed for 3d texture support, so the current implementation is incorrect");
                }
                _ => {}
            },
            AllCommands::Misc(cmd) => match cmd {
                Commands::TextureCreate(cmd) => {
                    assert!(cmd.texture_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.texture_index + self.id_offset;
                    cmd.texture_index = real_index;
                    assert!(
                        !self.textures.borrow().contains_key(&real_index),
                        "texture already exists, this is not allowed"
                    );
                    self.textures.borrow_mut().insert(
                        real_index,
                        TextureContainer {
                            width: cmd.width,
                            height: cmd.height,
                            depth: cmd.depth,
                        },
                    );
                }
                Commands::TextureDestroy(cmd) => {
                    assert!(cmd.texture_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.texture_index + self.id_offset;
                    cmd.texture_index = real_index;
                    assert!(
                        self.textures.borrow().contains_key(&real_index),
                        "texture does not exists, this is not allowed"
                    );
                    self.textures.borrow_mut().remove(&real_index);
                }
                // TODO: check out of bounds? Commands::TextureUpdate(cmd) => {}
                Commands::CreateBufferObject(cmd) => {
                    assert!(cmd.buffer_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.buffer_index + self.id_offset;
                    cmd.buffer_index = real_index;
                    assert!(
                        !self.buffers.borrow().contains_key(&real_index),
                        "buffer already exists, this is not allowed"
                    );
                    self.buffers.borrow_mut().insert(
                        real_index,
                        GraphicsBufferObject {
                            alloc_size: cmd.upload_data.borrow().as_ref().unwrap().len(),
                        },
                    );
                }
                Commands::RecreateBufferObject(cmd) => {
                    assert!(cmd.buffer_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.buffer_index + self.id_offset;
                    cmd.buffer_index = real_index;
                    assert!(
                        self.buffers.borrow().contains_key(&real_index),
                        "buffer does not exists, this is not allowed"
                    );
                    self.buffers
                        .borrow_mut()
                        .get_mut(&real_index)
                        .unwrap()
                        .alloc_size = cmd.upload_data.borrow().as_ref().unwrap().len();
                }
                Commands::DeleteBufferObject(cmd) => {
                    assert!(cmd.buffer_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.buffer_index + self.id_offset;
                    cmd.buffer_index = real_index;
                    assert!(
                        self.buffers.borrow().contains_key(&real_index),
                        "buffer does not exists, this is not allowed"
                    );
                    self.buffers.borrow_mut().remove(&real_index);
                }
                _ => {}
            },
        }
    }

    pub fn process_commands<B: GraphicsBackendInterface>(
        &self,
        stream_handle: &GraphicsStreamHandle<B>,
        cmds: &mut Vec<AllCommands>,
        vertices_offset_before_commands: usize,
    ) {
        for cmd in cmds {
            self.process_command(stream_handle, cmd, vertices_offset_before_commands);
        }
    }
}
