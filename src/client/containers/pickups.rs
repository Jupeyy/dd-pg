use std::{str::FromStr, sync::Arc};

use arrayvec::ArrayString;
use async_trait::async_trait;
use base_fs::filesys::FileSystem;
use graphics::graphics::{Graphics, GraphicsTextureAllocations};
use graphics_types::{
    command_buffer::{TexFlags, TexFormat},
    rendering::TextureIndex,
    types::ImageFormat,
};

use crate::client::image::png::load_png_image;

use super::container::{load_file_part, Container, ContainerItemInterface, ContainerLoad};

#[derive(Clone)]
pub struct Pickup {
    pub armor: TextureIndex,
    pub health: TextureIndex,
}

impl ContainerItemInterface for Pickup {
    fn destroy(self, graphics: &mut Graphics) {
        graphics.unload_texture(self.armor);
        graphics.unload_texture(self.health);
    }
}

#[derive(Default, Clone)]
pub struct LoadPickup {
    armor: Vec<u8>,
    health: Vec<u8>,

    pickup_name: String,
}

impl LoadPickup {
    pub async fn load_pickup(&mut self, fs: &FileSystem, pickup_name: &str) -> anyhow::Result<()> {
        let pickup_path = ArrayString::<4096>::from_str("pickups/").unwrap();

        self.armor = load_file_part(fs, &pickup_path, pickup_name, &[], "armor").await?;
        self.health = load_file_part(fs, &pickup_path, pickup_name, &[], "health").await?;

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
impl ContainerLoad<Pickup> for LoadPickup {
    async fn load(&mut self, item_name: &str, fs: &Arc<FileSystem>) -> anyhow::Result<()> {
        self.load_pickup(fs, item_name).await.unwrap();
        self.pickup_name = item_name.to_string();
        Ok(())
    }

    fn convert(self, graphics: &mut Graphics) -> Pickup {
        Pickup {
            armor: Self::load_file_into_texture(graphics, &self.armor, &self.pickup_name),
            health: Self::load_file_into_texture(graphics, &self.health, &self.pickup_name),
        }
    }
}

pub type PickupContainer = Container<Pickup, LoadPickup>;
