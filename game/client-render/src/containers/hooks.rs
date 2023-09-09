use std::{str::FromStr, sync::Arc};

use arrayvec::ArrayString;
use async_trait::async_trait;
use base_fs::filesys::FileSystem;

use graphics_backend::types::Graphics;
use graphics_types::{
    command_buffer::{TexFlags, TexFormat},
    textures_handle::{GraphicsTextureHandleInterface, TextureIndex},
    types::ImageFormat,
};
use image::png::load_png_image;

use super::container::{load_file_part, Container, ContainerLoad};

#[derive(Clone)]
pub struct Hook {
    pub hook_chain: TextureIndex,
    pub hook_head: TextureIndex,
}

#[derive(Default, Clone)]
pub struct LoadHook {
    hook_chain: Vec<u8>,
    hook_head: Vec<u8>,

    hook_name: String,
}

impl LoadHook {
    pub async fn load_hook(&mut self, fs: &FileSystem, hook_name: &str) -> anyhow::Result<()> {
        let hook_path = ArrayString::<4096>::from_str("hooks/").unwrap();

        self.hook_chain = load_file_part(fs, &hook_path, hook_name, &[], "hook_chain").await?;
        self.hook_head = load_file_part(fs, &hook_path, hook_name, &[], "hook_head").await?;

        Ok(())
    }

    fn load_file_into_texture(graphics: &mut Graphics, file: &Vec<u8>, name: &str) -> TextureIndex {
        let mut img_data = Vec::<u8>::new();
        let part_img = load_png_image(&file, |size| {
            img_data = vec![0; size];
            &mut img_data
        })
        .unwrap();
        let texture_id = graphics
            .texture_handle
            .load_texture_slow(
                part_img.width as usize,
                part_img.height as usize,
                ImageFormat::Rgba as i32,
                part_img.data.to_vec(),
                TexFormat::RGBA as i32,
                TexFlags::empty(),
                name,
            )
            .unwrap();
        texture_id
    }
}

#[async_trait]
impl ContainerLoad<Hook> for LoadHook {
    async fn load(
        &mut self,
        item_name: &str,
        fs: &Arc<FileSystem>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
    ) -> anyhow::Result<()> {
        self.load_hook(fs, item_name).await?;
        self.hook_name = item_name.to_string();
        Ok(())
    }

    fn convert(self, graphics: &mut Graphics) -> Hook {
        Hook {
            hook_chain: Self::load_file_into_texture(graphics, &self.hook_chain, &self.hook_name),
            hook_head: Self::load_file_into_texture(graphics, &self.hook_head, &self.hook_name),
        }
    }
}

pub type HookContainer = Container<Hook, LoadHook>;
