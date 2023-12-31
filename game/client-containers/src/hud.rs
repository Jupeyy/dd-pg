use std::{str::FromStr, sync::Arc};

use arrayvec::ArrayString;
use async_trait::async_trait;

use base_io_traits::fs_traits::FileSystemInterface;
use graphics::{graphics::GraphicsTextureHandle, graphics_mt::GraphicsMultiThreaded};
use graphics_types::{
    commands::{TexFlags, TexFormat},
    textures_handle::TextureIndex,
    types::ImageFormat,
};

use super::container::{
    load_file_part_and_upload, Container, ContainerItemLoadData, ContainerLoad,
};

#[derive(Clone)]
pub struct Hud {
    pub heart: TextureIndex,
    pub shield: TextureIndex,
}

#[derive(Debug)]
pub struct LoadHud {
    heart: ContainerItemLoadData,
    shield: ContainerItemLoadData,

    hud_name: String,
}

impl LoadHud {
    pub async fn load_hud(
        graphics_mt: &GraphicsMultiThreaded,
        fs: &dyn FileSystemInterface,
        hud_name: &str,
    ) -> anyhow::Result<Self> {
        let hud_path = ArrayString::<4096>::from_str("huds/").unwrap();

        Ok(Self {
            // heart
            heart: load_file_part_and_upload(graphics_mt, fs, &hud_path, hud_name, &[], "heart")
                .await?,
            // cursor
            shield: load_file_part_and_upload(graphics_mt, fs, &hud_path, hud_name, &[], "shield")
                .await?,

            hud_name: hud_name.to_string(),
        })
    }

    fn load_file_into_texture(
        texture_handle: &GraphicsTextureHandle,
        img: ContainerItemLoadData,
        name: &str,
    ) -> TextureIndex {
        texture_handle
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

    fn load_files_into_textures(self, texture_handle: &GraphicsTextureHandle) -> Hud {
        Hud {
            heart: Self::load_file_into_texture(texture_handle, self.heart, &self.hud_name),
            shield: Self::load_file_into_texture(texture_handle, self.shield, &self.hud_name),
        }
    }
}

#[async_trait]
impl ContainerLoad<Hud> for LoadHud {
    async fn load(
        item_name: &str,
        fs: &Arc<dyn FileSystemInterface>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
    ) -> anyhow::Result<Self> {
        Self::load_hud(graphics_mt, fs.as_ref(), item_name).await
    }

    fn convert(self, texture_handle: &GraphicsTextureHandle) -> Hud {
        self.load_files_into_textures(texture_handle)
    }
}

pub type HudContainer = Container<Hud, LoadHud>;
