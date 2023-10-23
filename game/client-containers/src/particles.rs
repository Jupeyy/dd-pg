use std::{str::FromStr, sync::Arc};

use arrayvec::{ArrayString, ArrayVec};
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
pub struct Particle {
    pub slice: TextureIndex,
    pub ball: TextureIndex,
    pub splats: [TextureIndex; 3],

    pub smoke: TextureIndex,
    pub shell: TextureIndex,
    pub explosions: [TextureIndex; 1],
    pub airjump: TextureIndex,
    pub hits: [TextureIndex; 1],
}

impl Particle {
    pub fn get_by_name(&self, name: &str) -> &TextureIndex {
        match name {
            "slice" => &self.slice,
            "ball" => &self.ball,
            "splats1" | "splats01" => &self.splats[0],
            "splats2" | "splats02" => &self.splats[1],
            "splats3" | "splats03" => &self.splats[2],
            "smoke" => &self.smoke,
            "shell" => &self.shell,
            "explosions1" | "explosions01" => &self.explosions[0],
            "airjump" => &self.airjump,
            "hits1" | "hits01" => &self.hits[0],
            _ => panic!("this is not a member of particle, or was not implemented."),
        }
    }
}

#[derive(Debug)]
pub struct LoadParticle {
    slice: ContainerItemLoadData,
    ball: ContainerItemLoadData,
    splats: [ContainerItemLoadData; 3],

    smoke: ContainerItemLoadData,
    shell: ContainerItemLoadData,
    explosions: [ContainerItemLoadData; 1],
    airjump: ContainerItemLoadData,
    hits: [ContainerItemLoadData; 1],

    particle_name: String,
}

impl LoadParticle {
    pub async fn load_particle(
        graphics_mt: &GraphicsMultiThreaded,
        fs: &dyn FileSystemInterface,
        particle_name: &str,
    ) -> anyhow::Result<Self> {
        let particle_path = ArrayString::<4096>::from_str("particles/").unwrap();

        let mut splats: [Option<ContainerItemLoadData>; 3] = Default::default();
        for (index, split) in splats.iter_mut().enumerate() {
            *split = Some(
                load_file_part_and_upload(
                    graphics_mt,
                    fs,
                    &particle_path,
                    particle_name,
                    &[],
                    &("splat".to_string() + &index.to_string()),
                )
                .await?,
            );
        }
        let mut explosions: [Option<ContainerItemLoadData>; 1] = Default::default();
        for (index, explosion) in explosions.iter_mut().enumerate() {
            *explosion = Some(
                load_file_part_and_upload(
                    graphics_mt,
                    fs,
                    &particle_path,
                    particle_name,
                    &[],
                    &("explosion".to_string() + &index.to_string()),
                )
                .await?,
            );
        }
        let mut hits: [Option<ContainerItemLoadData>; 1] = Default::default();
        for (index, hit) in hits.iter_mut().enumerate() {
            *hit = Some(
                load_file_part_and_upload(
                    graphics_mt,
                    fs,
                    &particle_path,
                    particle_name,
                    &[],
                    &("hit".to_string() + &index.to_string()),
                )
                .await?,
            );
        }

        Ok(Self {
            slice: load_file_part_and_upload(
                graphics_mt,
                fs,
                &particle_path,
                particle_name,
                &[],
                "slice",
            )
            .await?,
            ball: load_file_part_and_upload(
                graphics_mt,
                fs,
                &particle_path,
                particle_name,
                &[],
                "ball",
            )
            .await?,
            splats: splats
                .into_iter()
                .map(|splat| splat.unwrap())
                .collect::<ArrayVec<_, 3>>()
                .into_inner()
                .unwrap(),

            smoke: load_file_part_and_upload(
                graphics_mt,
                fs,
                &particle_path,
                particle_name,
                &[],
                "smoke",
            )
            .await?,
            shell: load_file_part_and_upload(
                graphics_mt,
                fs,
                &particle_path,
                particle_name,
                &[],
                "shell",
            )
            .await?,

            explosions: explosions
                .into_iter()
                .map(|explosion| explosion.unwrap())
                .collect::<ArrayVec<_, 1>>()
                .into_inner()
                .unwrap(),
            airjump: load_file_part_and_upload(
                graphics_mt,
                fs,
                &particle_path,
                particle_name,
                &[],
                "airjump",
            )
            .await?,
            hits: hits
                .into_iter()
                .map(|hit| hit.unwrap())
                .collect::<ArrayVec<_, 1>>()
                .into_inner()
                .unwrap(),

            particle_name: particle_name.to_string(),
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
impl ContainerLoad<Particle> for LoadParticle {
    async fn load(
        item_name: &str,
        fs: &Arc<dyn FileSystemInterface>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
    ) -> anyhow::Result<Self> {
        Self::load_particle(graphics_mt, fs.as_ref(), item_name).await
    }

    fn convert<B: GraphicsBackendInterface>(self, graphics: &mut GraphicsBase<B>) -> Particle {
        Particle {
            slice: Self::load_file_into_texture(graphics, self.slice, &self.particle_name),
            ball: Self::load_file_into_texture(graphics, self.ball, &self.particle_name),
            splats: self
                .splats
                .into_iter()
                .map(|splat| Self::load_file_into_texture(graphics, splat, &self.particle_name))
                .collect::<ArrayVec<_, 3>>()
                .into_inner()
                .unwrap(),

            smoke: Self::load_file_into_texture(graphics, self.smoke, &self.particle_name),
            shell: Self::load_file_into_texture(graphics, self.shell, &self.particle_name),
            explosions: self
                .explosions
                .into_iter()
                .map(|explosion| {
                    Self::load_file_into_texture(graphics, explosion, &self.particle_name)
                })
                .collect::<ArrayVec<_, 1>>()
                .into_inner()
                .unwrap(),
            airjump: Self::load_file_into_texture(graphics, self.airjump, &self.particle_name),
            hits: self
                .hits
                .into_iter()
                .map(|hit| Self::load_file_into_texture(graphics, hit, &self.particle_name))
                .collect::<ArrayVec<_, 1>>()
                .into_inner()
                .unwrap(),
        }
    }
}

pub type ParticlesContainer = Container<Particle, LoadParticle>;
