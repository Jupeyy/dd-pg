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
pub struct Hud {
    pub heart: TextureIndex,
    pub shield: TextureIndex,
}

#[derive(Default, Clone)]
pub struct LoadHud {
    heart: Vec<u8>,
    shield: Vec<u8>,

    hud_name: String,
}

impl LoadHud {
    pub async fn load_hud(&mut self, fs: &FileSystem, hud_name: &str) -> anyhow::Result<()> {
        let hud_path = ArrayString::<4096>::from_str("huds/").unwrap();

        // heart
        self.heart = load_file_part(fs, &hud_path, hud_name, &[], "heart").await?;
        // cursor
        self.shield = load_file_part(fs, &hud_path, hud_name, &[], "shield").await?;

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

    fn load_files_into_textures(self, graphics: &mut Graphics) -> Hud {
        Hud {
            heart: Self::load_file_into_texture(graphics, &self.heart, &self.hud_name),
            shield: Self::load_file_into_texture(graphics, &self.shield, &self.hud_name),
        }
    }
}

#[async_trait]
impl ContainerLoad<Hud> for LoadHud {
    async fn load(&mut self, item_name: &str, fs: &Arc<FileSystem>) -> anyhow::Result<()> {
        self.load_hud(fs, &item_name).await?;
        self.hud_name = item_name.to_string();
        Ok(())
    }

    fn convert(self, graphics: &mut Graphics) -> Hud {
        self.load_files_into_textures(graphics)
    }
}

pub type HudContainer = Container<Hud, LoadHud>;
