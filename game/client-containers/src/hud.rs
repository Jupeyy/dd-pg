use std::{path::Path, sync::Arc};

use async_trait::async_trait;

use base_io_traits::fs_traits::FileSystemInterface;
use graphics::{
    graphics_mt::GraphicsMultiThreaded,
    handles::texture::texture::{GraphicsTextureHandle, TextureContainer},
};
use graphics_types::{
    commands::{TexFlags, TexFormat},
    types::ImageFormat,
};
use sound::{sound_handle::SoundObjectHandle, sound_mt::SoundMultiThreaded};

use super::container::{
    load_file_part_and_upload, Container, ContainerItemLoadData, ContainerLoad,
};

#[derive(Clone)]
pub struct Hud {
    pub heart: TextureContainer,
    pub heart_empty: TextureContainer,
    pub shield: TextureContainer,
    pub shield_empty: TextureContainer,
}

#[derive(Debug)]
pub struct LoadHud {
    heart: ContainerItemLoadData,
    heart_empty: ContainerItemLoadData,
    shield: ContainerItemLoadData,
    shield_empty: ContainerItemLoadData,

    hud_name: String,
}

impl LoadHud {
    pub async fn load_hud(
        graphics_mt: &GraphicsMultiThreaded,
        fs: &dyn FileSystemInterface,
        hud_name: &str,
    ) -> anyhow::Result<Self> {
        let hud_path = Path::new("huds/");

        Ok(Self {
            // heart
            heart: load_file_part_and_upload(graphics_mt, fs, &hud_path, hud_name, &[], "heart")
                .await?,
            heart_empty: load_file_part_and_upload(
                graphics_mt,
                fs,
                &hud_path,
                hud_name,
                &[],
                "heart_empty",
            )
            .await?,
            // cursor
            shield: load_file_part_and_upload(graphics_mt, fs, &hud_path, hud_name, &[], "shield")
                .await?,
            shield_empty: load_file_part_and_upload(
                graphics_mt,
                fs,
                &hud_path,
                hud_name,
                &[],
                "shield_empty",
            )
            .await?,

            hud_name: hud_name.to_string(),
        })
    }

    fn load_file_into_texture(
        texture_handle: &GraphicsTextureHandle,
        img: ContainerItemLoadData,
        name: &str,
    ) -> TextureContainer {
        texture_handle
            .load_texture(
                img.width as usize,
                img.height as usize,
                ImageFormat::Rgba,
                img.data,
                TexFormat::RGBA,
                TexFlags::empty(),
                name,
            )
            .unwrap()
    }

    fn load_files_into_textures(self, texture_handle: &GraphicsTextureHandle) -> Hud {
        Hud {
            heart: Self::load_file_into_texture(texture_handle, self.heart, &self.hud_name),
            heart_empty: Self::load_file_into_texture(
                texture_handle,
                self.heart_empty,
                &self.hud_name,
            ),
            shield: Self::load_file_into_texture(texture_handle, self.shield, &self.hud_name),
            shield_empty: Self::load_file_into_texture(
                texture_handle,
                self.shield_empty,
                &self.hud_name,
            ),
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
        _sound_mt: &SoundMultiThreaded,
    ) -> anyhow::Result<Self> {
        Self::load_hud(graphics_mt, fs.as_ref(), item_name).await
    }

    fn convert(
        self,
        texture_handle: &GraphicsTextureHandle,
        _sound_object_handle: &SoundObjectHandle,
    ) -> Hud {
        self.load_files_into_textures(texture_handle)
    }
}

pub type HudContainer = Container<Hud, LoadHud>;
