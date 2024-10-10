use std::sync::Arc;

use graphics::{
    graphics_mt::GraphicsMultiThreaded,
    handles::texture::texture::{GraphicsTextureHandle, TextureContainer},
};
use graphics_types::{
    commands::{TexFlags, TexFormat},
    types::ImageFormat,
};
use hiarc::Hiarc;
use sound::{sound_handle::SoundObjectHandle, sound_mt::SoundMultiThreaded};

use crate::container::{load_file_part_and_upload_ex, ContainerLoadedItem, ContainerLoadedItemDir};

use super::container::{
    load_file_part_and_upload, Container, ContainerItemLoadData, ContainerLoad,
};

#[derive(Debug, Clone, Hiarc)]
pub struct Particle {
    pub slice: TextureContainer,
    pub ball: TextureContainer,
    pub splats: Vec<TextureContainer>,

    pub smoke: TextureContainer,
    pub shell: TextureContainer,
    pub explosions: Vec<TextureContainer>,
    pub airjump: TextureContainer,
    pub hits: Vec<TextureContainer>,
    pub stars: Vec<TextureContainer>,
}

impl Particle {
    pub fn get_by_name(&self, name: &str) -> &TextureContainer {
        match name {
            "slice" => &self.slice,
            "ball" => &self.ball,
            "splats0" | "splats00" | "splat0" | "splat00" => &self.splats[0],
            "splats1" | "splats01" | "splat1" | "splat01" => &self.splats[1],
            "splats2" | "splats02" | "splat2" | "splat02" => &self.splats[2],
            "smoke" => &self.smoke,
            "shell" => &self.shell,
            "explosions0" | "explosions00" | "explosion0" | "explosion00" => &self.explosions[0],
            "airjump" => &self.airjump,
            "hits0" | "hits00" | "hit0" | "hit00" => &self.hits[0],
            "stars0" | "stars00" | "star0" | "star00" => &self.stars[0],
            "stars1" | "stars01" | "star1" | "star01" => &self.stars[1],
            "stars2" | "stars02" | "star2" | "star02" => &self.stars[2],
            _ => panic!("this is not a member of particle, or was not implemented."),
        }
    }
}

#[derive(Debug, Hiarc)]
pub struct LoadParticle {
    slice: ContainerItemLoadData,
    ball: ContainerItemLoadData,
    splats: Vec<ContainerItemLoadData>,

    smoke: ContainerItemLoadData,
    shell: ContainerItemLoadData,
    explosions: Vec<ContainerItemLoadData>,
    airjump: ContainerItemLoadData,
    hits: Vec<ContainerItemLoadData>,
    stars: Vec<ContainerItemLoadData>,

    particle_name: String,
}

impl LoadParticle {
    pub fn new(
        graphics_mt: &GraphicsMultiThreaded,
        files: ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        particle_name: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            slice: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                particle_name,
                &[],
                "slice",
            )?
            .img,
            ball: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                particle_name,
                &[],
                "ball",
            )?
            .img,
            splats: {
                let mut textures = Vec::new();
                let mut i = 0;
                let mut allow_default = true;
                loop {
                    match load_file_part_and_upload_ex(
                        graphics_mt,
                        &files,
                        default_files,
                        particle_name,
                        &[],
                        &format!("splat{i}"),
                        allow_default,
                    ) {
                        Ok(img) => {
                            allow_default &= img.from_default;
                            textures.push(img.img);
                        }
                        Err(err) => {
                            if i == 0 {
                                return Err(err);
                            } else {
                                break;
                            }
                        }
                    }

                    i += 1;
                }
                textures
            },

            smoke: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                particle_name,
                &[],
                "smoke",
            )?
            .img,
            shell: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                particle_name,
                &[],
                "shell",
            )?
            .img,

            explosions: {
                let mut textures = Vec::new();
                let mut i = 0;
                let mut allow_default = true;
                loop {
                    match load_file_part_and_upload_ex(
                        graphics_mt,
                        &files,
                        default_files,
                        particle_name,
                        &[],
                        &format!("explosion{i}"),
                        allow_default,
                    ) {
                        Ok(img) => {
                            allow_default &= img.from_default;
                            textures.push(img.img);
                        }
                        Err(err) => {
                            if i == 0 {
                                return Err(err);
                            } else {
                                break;
                            }
                        }
                    }

                    i += 1;
                }
                textures
            },
            airjump: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                particle_name,
                &[],
                "airjump",
            )?
            .img,
            hits: {
                let mut textures = Vec::new();
                let mut i = 0;
                let mut allow_default = true;
                loop {
                    match load_file_part_and_upload_ex(
                        graphics_mt,
                        &files,
                        default_files,
                        particle_name,
                        &[],
                        &format!("hit{i}"),
                        allow_default,
                    ) {
                        Ok(img) => {
                            allow_default &= img.from_default;
                            textures.push(img.img);
                        }
                        Err(err) => {
                            if i == 0 {
                                return Err(err);
                            } else {
                                break;
                            }
                        }
                    }

                    i += 1;
                }
                textures
            },
            stars: {
                let mut textures = Vec::new();
                let mut i = 0;
                let mut allow_default = true;
                loop {
                    match load_file_part_and_upload_ex(
                        graphics_mt,
                        &files,
                        default_files,
                        particle_name,
                        &[],
                        &format!("star{i}"),
                        allow_default,
                    ) {
                        Ok(img) => {
                            allow_default &= img.from_default;
                            textures.push(img.img);
                        }
                        Err(err) => {
                            if i == 0 {
                                return Err(err);
                            } else {
                                break;
                            }
                        }
                    }

                    i += 1;
                }
                textures
            },

            particle_name: particle_name.to_string(),
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
                TexFormat::Rgba,
                TexFlags::empty(),
                name,
            )
            .unwrap()
    }
}

impl ContainerLoad<Particle> for LoadParticle {
    fn load(
        item_name: &str,
        files: ContainerLoadedItem,
        default_files: &ContainerLoadedItemDir,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
        _sound_mt: &SoundMultiThreaded,
    ) -> anyhow::Result<Self> {
        match files {
            ContainerLoadedItem::Directory(files) => {
                Self::new(graphics_mt, files, default_files, item_name)
            }
            ContainerLoadedItem::SingleFile(_) => Err(anyhow::anyhow!(
                "single file mode is currently not supported"
            )),
        }
    }

    fn convert(
        self,
        texture_handle: &GraphicsTextureHandle,
        _sound_object_handle: &SoundObjectHandle,
    ) -> Particle {
        Particle {
            slice: Self::load_file_into_texture(texture_handle, self.slice, &self.particle_name),
            ball: Self::load_file_into_texture(texture_handle, self.ball, &self.particle_name),
            splats: self
                .splats
                .into_iter()
                .map(|splat| {
                    Self::load_file_into_texture(texture_handle, splat, &self.particle_name)
                })
                .collect(),

            smoke: Self::load_file_into_texture(texture_handle, self.smoke, &self.particle_name),
            shell: Self::load_file_into_texture(texture_handle, self.shell, &self.particle_name),
            explosions: self
                .explosions
                .into_iter()
                .map(|explosion| {
                    Self::load_file_into_texture(texture_handle, explosion, &self.particle_name)
                })
                .collect(),
            airjump: Self::load_file_into_texture(
                texture_handle,
                self.airjump,
                &self.particle_name,
            ),
            hits: self
                .hits
                .into_iter()
                .map(|hit| Self::load_file_into_texture(texture_handle, hit, &self.particle_name))
                .collect(),
            stars: self
                .stars
                .into_iter()
                .map(|star| Self::load_file_into_texture(texture_handle, star, &self.particle_name))
                .collect(),
        }
    }
}

pub type ParticlesContainer = Container<Particle, LoadParticle>;
pub const PARTICLES_CONTAINER_PATH: &str = "particles/";
