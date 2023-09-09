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
pub struct Particle {
    pub tex: TextureIndex,
}

#[derive(Default, Clone)]
pub struct LoadParticle {
    tex: Vec<u8>,

    particle_name: String,
}

impl LoadParticle {
    pub async fn load_particle(
        &mut self,
        fs: &FileSystem,
        particle_name: &str,
    ) -> anyhow::Result<()> {
        let particle_path = ArrayString::<4096>::from_str("particles/").unwrap();

        self.tex = load_file_part(fs, &particle_path, particle_name, &[], "particle").await?;

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
impl ContainerLoad<Particle> for LoadParticle {
    async fn load(
        &mut self,
        item_name: &str,
        fs: &Arc<FileSystem>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
    ) -> anyhow::Result<()> {
        self.load_particle(fs, item_name).await.unwrap();
        self.particle_name = item_name.to_string();
        Ok(())
    }

    fn convert(self, graphics: &mut Graphics) -> Particle {
        Particle {
            tex: Self::load_file_into_texture(graphics, &self.tex, &self.particle_name),
        }
    }
}

pub type ParticleContainer = Container<Particle, LoadParticle>;
