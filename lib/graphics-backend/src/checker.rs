use std::{cell::RefCell, collections::HashMap};

use graphics::{
    handles::{backend::GraphicsBackendHandle, stream::GraphicsStreamHandle},
    types::{GraphicsBufferObject, TextureContainer},
};
use graphics_types::{
    commands::{
        AllCommands, CommandDeleteBufferObject, CommandRender, CommandTextureDestroy, Commands,
        CommandsRender, CommandsRenderQuadContainer, CommandsRenderStream, PrimType,
    },
    rendering::{State, StateTexture},
};
use hiarc_macro::Hiarc;

/// rewrites graphics commands to make them use resource indices
/// that are not colliding with host indices
/// Important: Does not validate correctness in any kind.
#[derive(Debug, Hiarc)]
pub struct GraphicsContainersAPI {
    textures: RefCell<HashMap<u128, TextureContainer>>,
    buffers: RefCell<HashMap<u128, GraphicsBufferObject>>,

    #[hiarc]
    graphics_backend: GraphicsBackendHandle,
    id_offset: u128,
}

impl GraphicsContainersAPI {
    pub fn new(id_offset: u128, graphics_backend: GraphicsBackendHandle) -> Self {
        Self {
            textures: Default::default(),
            buffers: Default::default(),
            id_offset,
            graphics_backend,
        }
    }

    fn process_state(&self, state: &mut State) {
        if let StateTexture::Texture(index) = state.texture_index {
            assert!(index < u64::MAX as u128, "invalid index");
            let real_index = index + self.id_offset;

            assert!(
                self.textures.borrow().contains_key(&real_index),
                "texture does not exists, this is not allowed"
            );
            state.texture_index = StateTexture::Texture(real_index);
        }
    }

    fn check_render_cmd(
        &self,
        stream_handle: &GraphicsStreamHandle,
        cmd: &mut CommandRender,
        vertices_offset_before_commands: usize,
    ) {
        self.process_state(&mut cmd.state);

        let count = stream_handle.stream_data().borrow().vertices_count();
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

    fn process_command(
        &self,
        stream_handle: &GraphicsStreamHandle,
        cmd: &mut AllCommands,
        vertices_offset_before_commands: usize,
    ) {
        match cmd {
            AllCommands::Render(cmd) => match cmd {
                CommandsRender::QuadContainer(cmd) => match cmd {
                    CommandsRenderQuadContainer::Render(cmd) => {
                        assert!(cmd.buffer_object_index < u64::MAX as u128, "invalid index");
                        let real_index = cmd.buffer_object_index + self.id_offset;
                        cmd.buffer_object_index = real_index;
                        self.process_state(&mut cmd.state);
                    }
                    CommandsRenderQuadContainer::RenderAsSpriteMultiple(cmd) => {
                        assert!(cmd.buffer_object_index < u64::MAX as u128, "invalid index");
                        let real_index = cmd.buffer_object_index + self.id_offset;
                        cmd.buffer_object_index = real_index;
                        self.process_state(&mut cmd.state);
                    }
                },
                CommandsRender::Stream(cmd) => match cmd {
                    CommandsRenderStream::Render(cmd) => {
                        self.check_render_cmd(stream_handle, cmd, vertices_offset_before_commands)
                    }
                    CommandsRenderStream::RenderBlurred { cmd, .. } => {
                        self.check_render_cmd(stream_handle, cmd, vertices_offset_before_commands)
                    }
                    CommandsRenderStream::RenderTex3D(cmd) => {
                        self.process_state(&mut cmd.state);

                        let count = stream_handle.stream_data().borrow().vertices_count();
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
                },
                CommandsRender::Clear(_) => {
                    todo!()
                }
                CommandsRender::Mod { .. } => todo!(),
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
                Commands::TextureUpdate(cmd) => {
                    assert!(cmd.texture_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.texture_index + self.id_offset;
                    cmd.texture_index = real_index;
                    assert!(
                        self.textures.borrow().contains_key(&real_index),
                        "texture does not exists, this is not allowed"
                    );
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
                            alloc_size: cmd.upload_data.len(),
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
                        .alloc_size = cmd.upload_data.len();
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

    pub fn process_commands(
        &self,
        stream_handle: &GraphicsStreamHandle,
        cmds: &mut Vec<AllCommands>,
        vertices_offset_before_commands: usize,
    ) {
        for cmd in cmds {
            self.process_command(stream_handle, cmd, vertices_offset_before_commands);
        }
    }
}

impl Drop for GraphicsContainersAPI {
    fn drop(&mut self) {
        self.buffers
            .borrow_mut()
            .drain()
            .for_each(|(buffer_index, _)| {
                self.graphics_backend
                    .add_cmd(AllCommands::Misc(Commands::DeleteBufferObject(
                        CommandDeleteBufferObject { buffer_index },
                    )))
            });

        self.textures
            .borrow_mut()
            .drain()
            .for_each(|(texture_index, _)| {
                self.graphics_backend
                    .add_cmd(AllCommands::Misc(Commands::TextureDestroy(
                        CommandTextureDestroy { texture_index },
                    )))
            });
    }
}
