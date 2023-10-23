use std::fmt::Debug;

use base::shared_index::{SharedIndex, SharedIndexCleanup};

use crate::command_buffer::TexFlags;

pub type TextureIndex = SharedIndex<dyn GraphicsTextureHandleInterface>;

pub trait GraphicsTextureHandleInterface: SharedIndexCleanup + Debug {
    fn load_texture_slow(
        &mut self,
        width: usize,
        height: usize,
        format: i32,
        data: Vec<u8>,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<TextureIndex>;
    fn load_texture_3d_slow(
        &mut self,
        width: usize,
        height: usize,
        depth: usize,
        format: i32,
        data: Vec<u8>,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<TextureIndex>;
}
