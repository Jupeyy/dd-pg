use std::{path::Path, sync::Arc};

use async_trait::async_trait;

use base_io_traits::fs_traits::FileSystemInterface;
use graphics::{
    graphics_mt::GraphicsMultiThreaded,
    handles::texture::texture::{GraphicsTextureHandle, TextureContainer2dArray},
};
use graphics_types::{
    commands::{TexFlags, TexFormat},
    types::ImageFormat,
};
use hiarc::Hiarc;
use sound::{sound_handle::SoundObjectHandle, sound_mt::SoundMultiThreaded};

use super::container::{
    load_file_part_and_convert_3d_and_upload, Container, ContainerItemLoadData, ContainerLoad,
};

#[derive(Debug, Hiarc, Clone)]
pub struct Entities {
    pub physics: TextureContainer2dArray,
    pub text_overlay_top: TextureContainer2dArray,
    pub text_overlay_bottom: TextureContainer2dArray,
    pub text_overlay_center: TextureContainer2dArray,
}

#[derive(Debug, Hiarc)]
pub struct LoadEntities {
    physics: ContainerItemLoadData,
    text_overlay_top: ContainerItemLoadData,
    text_overlay_bottom: ContainerItemLoadData,
    text_overlay_center: ContainerItemLoadData,

    entities_name: String,
}

impl LoadEntities {
    pub async fn load_entities(
        graphics_mt: &GraphicsMultiThreaded,
        fs: &dyn FileSystemInterface,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
        entities_name: &str,
    ) -> anyhow::Result<Self> {
        let entities_path = Path::new("entities/");

        Ok(Self {
            physics: load_file_part_and_convert_3d_and_upload(
                graphics_mt,
                fs,
                runtime_thread_pool,
                &entities_path,
                entities_name,
                &[],
                "physics",
            )
            .await?,
            text_overlay_top: load_file_part_and_convert_3d_and_upload(
                graphics_mt,
                fs,
                runtime_thread_pool,
                &entities_path,
                entities_name,
                &[],
                "text_overlay_top",
            )
            .await?,
            text_overlay_bottom: load_file_part_and_convert_3d_and_upload(
                graphics_mt,
                fs,
                runtime_thread_pool,
                &entities_path,
                entities_name,
                &[],
                "text_overlay_bottom",
            )
            .await?,
            text_overlay_center: load_file_part_and_convert_3d_and_upload(
                graphics_mt,
                fs,
                runtime_thread_pool,
                &entities_path,
                entities_name,
                &[],
                "text_overlay_center",
            )
            .await?,

            entities_name: entities_name.to_string(),
        })
    }

    fn load_file_into_texture(
        texture_handle: &GraphicsTextureHandle,
        img: ContainerItemLoadData,
        name: &str,
    ) -> TextureContainer2dArray {
        texture_handle
            .load_texture_3d(
                img.width as usize,
                img.height as usize,
                img.depth as usize,
                ImageFormat::Rgba,
                img.data,
                TexFormat::RGBA,
                TexFlags::empty(),
                name,
            )
            .unwrap()
    }
}

#[async_trait]
impl ContainerLoad<Entities> for LoadEntities {
    async fn load(
        item_name: &str,
        fs: &Arc<dyn FileSystemInterface>,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
        _sound_mt: &SoundMultiThreaded,
    ) -> anyhow::Result<Self> {
        Self::load_entities(graphics_mt, fs.as_ref(), runtime_thread_pool, item_name).await
    }

    fn convert(
        self,
        texture_handle: &GraphicsTextureHandle,
        _sound_object_handle: &SoundObjectHandle,
    ) -> Entities {
        Entities {
            physics: Self::load_file_into_texture(
                texture_handle,
                self.physics,
                &self.entities_name,
            ),
            text_overlay_top: Self::load_file_into_texture(
                texture_handle,
                self.text_overlay_top,
                &self.entities_name,
            ),
            text_overlay_bottom: Self::load_file_into_texture(
                texture_handle,
                self.text_overlay_bottom,
                &self.entities_name,
            ),
            text_overlay_center: Self::load_file_into_texture(
                texture_handle,
                self.text_overlay_center,
                &self.entities_name,
            ),
        }
    }
}

pub type EntitiesContainer = Container<Entities, LoadEntities>;
