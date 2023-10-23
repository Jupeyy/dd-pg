use std::{str::FromStr, sync::Arc};

use arrayvec::ArrayString;
use async_trait::async_trait;

use base_fs_traits::traits::FileSystemInterface;
use graphics::{graphics::GraphicsBase, graphics_mt::GraphicsMultiThreaded};
use graphics_backend_traits::traits::GraphicsBackendInterface;
use graphics_types::{
    command_buffer::{TexFlags, TexFormat},
    textures_handle::TextureIndex,
    types::ImageFormat,
};

use super::container::{
    load_file_part_and_upload, Container, ContainerItemLoadData, ContainerLoad,
};

#[derive(Clone)]
pub struct Pickup {
    pub armor: TextureIndex,
    pub health: TextureIndex,
}

#[derive(Debug)]
pub struct LoadPickup {
    armor: ContainerItemLoadData,
    health: ContainerItemLoadData,

    pickup_name: String,
}

impl LoadPickup {
    pub async fn load_pickup(
        graphics_mt: &GraphicsMultiThreaded,
        fs: &dyn FileSystemInterface,
        pickup_name: &str,
    ) -> anyhow::Result<Self> {
        let pickup_path = ArrayString::<4096>::from_str("pickups/").unwrap();

        Ok(Self {
            armor: load_file_part_and_upload(
                graphics_mt,
                fs,
                &pickup_path,
                pickup_name,
                &[],
                "armor",
            )
            .await?,
            health: load_file_part_and_upload(
                graphics_mt,
                fs,
                &pickup_path,
                pickup_name,
                &[],
                "health",
            )
            .await?,

            pickup_name: pickup_name.to_string(),
        })
    }

    fn load_file_into_texture<B: GraphicsBackendInterface>(
        graphics: &mut GraphicsBase<B>,
        img: ContainerItemLoadData,
        name: &str,
    ) -> TextureIndex {
        graphics
            .texture_handle
            .load_texture(
                img.width as usize,
                img.height as usize,
                ImageFormat::Rgba as i32,
                img.data,
                TexFormat::RGBA as i32,
                TexFlags::empty(),
                name,
            )
            .unwrap()
    }
}

#[async_trait]
impl ContainerLoad<Pickup> for LoadPickup {
    async fn load(
        item_name: &str,
        fs: &Arc<dyn FileSystemInterface>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
    ) -> anyhow::Result<Self> {
        Self::load_pickup(graphics_mt, fs.as_ref(), item_name).await
    }

    fn convert<B: GraphicsBackendInterface>(self, graphics: &mut GraphicsBase<B>) -> Pickup {
        Pickup {
            armor: Self::load_file_into_texture(graphics, self.armor, &self.pickup_name),
            health: Self::load_file_into_texture(graphics, self.health, &self.pickup_name),
        }
    }
}

pub type PickupContainer = Container<Pickup, LoadPickup>;
