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
use image::png::PngResultPersistent;

use super::container::{load_file_part_as_png_and_convert_3d, Container, ContainerLoad};

#[derive(Clone)]
pub struct Entities {
    pub vanilla: TextureIndex,
    pub text_overlay_top: TextureIndex,
    pub text_overlay_bottom: TextureIndex,
    pub text_overlay_center: TextureIndex,
}

#[derive(Default, Clone)]
pub struct LoadEntities {
    vanilla: PngResultPersistent,
    text_overlay_top: PngResultPersistent,
    text_overlay_bottom: PngResultPersistent,
    text_overlay_center: PngResultPersistent,

    entities_name: String,
}

impl LoadEntities {
    pub async fn load_entities(
        &mut self,
        fs: &FileSystem,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
        entities_name: &str,
    ) -> anyhow::Result<()> {
        let entities_path = ArrayString::<4096>::from_str("entities/").unwrap();

        self.vanilla = load_file_part_as_png_and_convert_3d(
            fs,
            runtime_thread_pool,
            &entities_path,
            entities_name,
            &[],
            "vanilla",
        )
        .await?;
        self.text_overlay_top = load_file_part_as_png_and_convert_3d(
            fs,
            runtime_thread_pool,
            &entities_path,
            entities_name,
            &[],
            "text_overlay_top",
        )
        .await?;
        self.text_overlay_bottom = load_file_part_as_png_and_convert_3d(
            fs,
            runtime_thread_pool,
            &entities_path,
            entities_name,
            &[],
            "text_overlay_bottom",
        )
        .await?;
        self.text_overlay_center = load_file_part_as_png_and_convert_3d(
            fs,
            runtime_thread_pool,
            &entities_path,
            entities_name,
            &[],
            "text_overlay_center",
        )
        .await?;

        Ok(())
    }

    fn load_file_into_texture(
        graphics: &mut Graphics,
        img: PngResultPersistent,
        name: &str,
    ) -> TextureIndex {
        let texture_id = graphics
            .texture_handle
            .load_texture_3d_slow(
                img.width as usize,
                img.height as usize,
                256,
                ImageFormat::Rgba as i32,
                img.data.to_vec(),
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
    async fn load(
        &mut self,
        item_name: &str,
        fs: &Arc<FileSystem>,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
    ) -> anyhow::Result<()> {
        self.load_entities(fs, runtime_thread_pool, item_name)
            .await
            .unwrap();
        self.entities_name = item_name.to_string();
        Ok(())
    }

    fn convert(self, graphics: &mut Graphics) -> Entities {
        Entities {
            vanilla: Self::load_file_into_texture(graphics, self.vanilla, &self.entities_name),
            text_overlay_top: Self::load_file_into_texture(
                graphics,
                self.text_overlay_top,
                &self.entities_name,
            ),
            text_overlay_bottom: Self::load_file_into_texture(
                graphics,
                self.text_overlay_bottom,
                &self.entities_name,
            ),
            text_overlay_center: Self::load_file_into_texture(
                graphics,
                self.text_overlay_center,
                &self.entities_name,
            ),
        }
    }
}

pub type EntitiesContainer = Container<Entities, LoadEntities>;
