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
pub struct Entities {
    pub vanilla: TextureIndex,
}

#[derive(Default, Clone)]
pub struct LoadEntities {
    vanilla: Vec<u8>,

    entities_name: String,
}

impl LoadEntities {
    pub async fn load_entities(
        &mut self,
        fs: &FileSystem,
        entities_name: &str,
    ) -> anyhow::Result<()> {
        let entities_path = ArrayString::<4096>::from_str("entities/").unwrap();

        self.vanilla = load_file_part(fs, &entities_path, entities_name, &[], "vanilla").await?;

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
impl ContainerLoad<Entities> for LoadEntities {
    async fn load(&mut self, item_name: &str, fs: &Arc<FileSystem>) -> anyhow::Result<()> {
        self.load_entities(fs, item_name).await.unwrap();
        self.entities_name = item_name.to_string();
        Ok(())
    }

    fn convert(self, graphics: &mut Graphics) -> Entities {
        Entities {
            vanilla: Self::load_file_into_texture(graphics, &self.vanilla, &self.entities_name),
        }
    }
}

pub type EntitiesContainer = Container<Entities, LoadEntities>;
