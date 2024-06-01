use std::collections::HashMap;

use graphics::handles::{
    backend::backend::GraphicsBackendHandle, stream::stream::GraphicsStreamHandle,
};
use graphics_types::{
    commands::{
        AllCommands, CommandDeleteBufferObject, CommandRender, CommandTextureDestroy, Commands,
        CommandsRender, CommandsRenderMod, CommandsRenderQuadContainer, CommandsRenderStream,
        PrimType,
    },
    rendering::{StateTexture, StateTexture2dArray},
};
use hiarc::{hiarc_safer_refcell, Hiarc};

#[derive(Debug, Hiarc)]
struct TextureContainer {}

#[derive(Debug, Hiarc)]
struct BufferObject {
    alloc_size: usize,
}

/// rewrites graphics commands to make them use resource indices
/// that are not colliding with host indices
/// Important: Does not validate correctness in any kind.
#[hiarc_safer_refcell]
#[derive(Debug, Hiarc)]
pub struct GraphicsContainersAPI {
    textures: HashMap<u128, TextureContainer>,
    buffers: HashMap<u128, BufferObject>,

    graphics_backend: GraphicsBackendHandle,
    id_offset: u128,
}

#[hiarc_safer_refcell]
impl GraphicsContainersAPI {
    pub fn new(id_offset: u128, graphics_backend: GraphicsBackendHandle) -> Self {
        Self {
            textures: Default::default(),
            buffers: Default::default(),
            id_offset,
            graphics_backend,
        }
    }

    fn process_texture(&self, texture_index: &mut StateTexture) {
        if let StateTexture::Texture(index) = texture_index {
            assert!(*index < u64::MAX as u128, "invalid index");
            let real_index = *index + self.id_offset;

            assert!(
                self.textures.contains_key(&real_index),
                "texture does not exists, this is not allowed"
            );
            *texture_index = StateTexture::Texture(real_index);
        }
    }

    fn process_texture_2d_array(&self, texture_index: &mut StateTexture2dArray) {
        if let StateTexture2dArray::Texture(index) = texture_index {
            assert!(*index < u64::MAX as u128, "invalid index");
            let real_index = *index + self.id_offset;

            assert!(
                self.textures.contains_key(&real_index),
                "texture does not exists, this is not allowed"
            );
            *texture_index = StateTexture2dArray::Texture(real_index);
        }
    }

    fn process_buffer_object(&self, index: &mut u128) {
        assert!(*index < u64::MAX as u128, "invalid index");
        let real_index = *index + self.id_offset;

        assert!(
            self.buffers.contains_key(&real_index),
            "texture does not exists, this is not allowed"
        );
        *index = real_index;
    }

    fn check_render_cmd(
        &self,
        stream_handle: &GraphicsStreamHandle,
        cmd: &mut CommandRender,
        vertices_offset_before_commands: usize,
    ) {
        self.process_texture(&mut cmd.texture_index);

        let count = stream_handle.stream_data().vertices_count();
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
        &mut self,
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
                        self.process_texture(&mut cmd.texture_index);
                    }
                    CommandsRenderQuadContainer::RenderAsSpriteMultiple(cmd) => {
                        assert!(cmd.buffer_object_index < u64::MAX as u128, "invalid index");
                        let real_index = cmd.buffer_object_index + self.id_offset;
                        cmd.buffer_object_index = real_index;
                        self.process_texture(&mut cmd.texture_index);
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
                        self.process_texture(&mut cmd.texture_index);

                        let count = stream_handle.stream_data().vertices_count();
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
                CommandsRender::Mod(CommandsRenderMod { mod_name, cmd }) => {
                    self.graphics_backend.check_mod_cmd(
                        mod_name,
                        cmd,
                        &|textures, textures_2d_array, buffer_objects| {
                            for texture in textures {
                                self.process_texture(texture)
                            }
                            for texture_2d_array in textures_2d_array {
                                self.process_texture_2d_array(texture_2d_array)
                            }
                            for buffer_object in buffer_objects {
                                self.process_buffer_object(buffer_object)
                            }
                        },
                    );
                }
            },
            AllCommands::Misc(cmd) => match cmd {
                Commands::TextureCreate(cmd) => {
                    assert!(cmd.texture_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.texture_index + self.id_offset;
                    cmd.texture_index = real_index;
                    assert!(
                        !self.textures.contains_key(&real_index),
                        "texture already exists, this is not allowed"
                    );
                    self.textures.insert(real_index, TextureContainer {});
                }
                Commands::TextureDestroy(cmd) => {
                    assert!(cmd.texture_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.texture_index + self.id_offset;
                    cmd.texture_index = real_index;
                    assert!(
                        self.textures.contains_key(&real_index),
                        "texture does not exists, this is not allowed"
                    );
                    self.textures.remove(&real_index);
                }
                Commands::TextureUpdate(cmd) => {
                    assert!(cmd.texture_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.texture_index + self.id_offset;
                    cmd.texture_index = real_index;
                    assert!(
                        self.textures.contains_key(&real_index),
                        "texture does not exists, this is not allowed"
                    );
                }
                // TODO: check out of bounds? Commands::TextureUpdate(cmd) => {}
                Commands::CreateBufferObject(cmd) => {
                    assert!(cmd.buffer_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.buffer_index + self.id_offset;
                    cmd.buffer_index = real_index;
                    assert!(
                        !self.buffers.contains_key(&real_index),
                        "buffer already exists, this is not allowed"
                    );
                    self.buffers.insert(
                        real_index,
                        BufferObject {
                            alloc_size: cmd.upload_data.len(),
                        },
                    );
                }
                Commands::RecreateBufferObject(cmd) => {
                    assert!(cmd.buffer_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.buffer_index + self.id_offset;
                    cmd.buffer_index = real_index;
                    assert!(
                        self.buffers.contains_key(&real_index),
                        "buffer does not exists, this is not allowed"
                    );
                    self.buffers.get_mut(&real_index).unwrap().alloc_size = cmd.upload_data.len();
                }
                Commands::DeleteBufferObject(cmd) => {
                    assert!(cmd.buffer_index < u64::MAX as u128, "invalid index");
                    let real_index = cmd.buffer_index + self.id_offset;
                    cmd.buffer_index = real_index;
                    assert!(
                        self.buffers.contains_key(&real_index),
                        "buffer does not exists, this is not allowed"
                    );
                    self.buffers.remove(&real_index);
                }
                _ => {}
            },
        }
    }

    pub fn process_commands(
        &mut self,
        stream_handle: &GraphicsStreamHandle,
        cmds: &mut Vec<AllCommands>,
        vertices_offset_before_commands: usize,
    ) {
        for cmd in cmds {
            self.process_command(stream_handle, cmd, vertices_offset_before_commands);
        }
    }
}

#[hiarc_safer_refcell]
impl Drop for GraphicsContainersAPI {
    fn drop(&mut self) {
        self.buffers.drain().for_each(|(buffer_index, _)| {
            self.graphics_backend
                .add_cmd(AllCommands::Misc(Commands::DeleteBufferObject(
                    CommandDeleteBufferObject { buffer_index },
                )))
        });

        self.textures.drain().for_each(|(texture_index, _)| {
            self.graphics_backend
                .add_cmd(AllCommands::Misc(Commands::TextureDestroy(
                    CommandTextureDestroy { texture_index },
                )))
        });
    }
}
