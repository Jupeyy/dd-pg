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
    load_file_part_and_convert_3d_and_upload, Container, ContainerItemLoadData, ContainerLoad,
};

#[derive(Clone)]
pub struct Entities {
    pub vanilla: TextureIndex,
    pub text_overlay_top: TextureIndex,
    pub text_overlay_bottom: TextureIndex,
    pub text_overlay_center: TextureIndex,
}

#[derive(Debug)]
pub struct LoadEntities {
    vanilla: ContainerItemLoadData,
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
        let entities_path = ArrayString::<4096>::from_str("entities/").unwrap();

        Ok(Self {
            vanilla: load_file_part_and_convert_3d_and_upload(
                graphics_mt,
                fs,
                runtime_thread_pool,
                &entities_path,
                entities_name,
                &[],
                "vanilla",
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
    ) -> TextureIndex {
        texture_handle
            .load_texture_3d(
                img.width as usize,
                img.height as usize,
                img.depth as usize,
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
impl ContainerLoad<Entities> for LoadEntities {
    async fn load(
        item_name: &str,
        fs: &Arc<dyn FileSystemInterface>,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
    ) -> anyhow::Result<Self> {
        Self::load_entities(graphics_mt, fs.as_ref(), runtime_thread_pool, item_name).await
    }

    fn convert(self, texture_handle: &GraphicsTextureHandle) -> Entities {
        Entities {
            vanilla: Self::load_file_into_texture(
                texture_handle,
                self.vanilla,
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
