use std::sync::Arc;

use graphics::{
    graphics_mt::GraphicsMultiThreaded,
    handles::texture::texture::{GraphicsTextureHandle, TextureContainer},
};
use graphics_types::{
    commands::{TexFlags, TexFormat},
    types::ImageFormat,
};
use sound::{
    sound_handle::SoundObjectHandle, sound_mt::SoundMultiThreaded,
    sound_mt_types::SoundBackendMemory, sound_object::SoundObject,
};

use crate::container::{
    load_sound_file_part_and_upload, ContainerLoadedItem, ContainerLoadedItemDir,
};

use super::container::{
    load_file_part_and_upload, Container, ContainerItemLoadData, ContainerLoad,
};

#[derive(Clone)]
pub struct Pickup<const C: usize> {
    pub tex: TextureContainer,

    pub spawn: SoundObject,
    pub collects: [SoundObject; C],
}

#[derive(Clone)]
pub struct Game {
    pub heart: Pickup<2>,
    pub shield: Pickup<4>,

    pub lose_grenade: TextureContainer,
    pub lose_laser: TextureContainer,
    pub lose_ninja: TextureContainer,
    pub lose_shotgun: TextureContainer,

    pub stars: [TextureContainer; 3],
}

#[derive(Debug)]
pub struct LoadPickup<const C: usize> {
    pub tex: ContainerItemLoadData,

    pub spawn: SoundBackendMemory,
    pub collects: [SoundBackendMemory; C],
}

#[derive(Debug)]
pub struct LoadGame {
    pub heart: LoadPickup<2>,
    pub shield: LoadPickup<4>,

    pub lose_grenade: ContainerItemLoadData,
    pub lose_laser: ContainerItemLoadData,
    pub lose_ninja: ContainerItemLoadData,
    pub lose_shotgun: ContainerItemLoadData,

    pub stars: [ContainerItemLoadData; 3],

    game_name: String,
}

impl LoadGame {
    pub fn load_game(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        game_name: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            heart: LoadPickup {
                tex: load_file_part_and_upload(
                    graphics_mt,
                    &files,
                    default_files,
                    game_name,
                    &[],
                    "heart",
                )?,

                spawn: load_sound_file_part_and_upload(
                    sound_mt,
                    &files,
                    default_files,
                    game_name,
                    &["audio", "heart"],
                    "spawn",
                )?,
                collects: {
                    let mut sounds = Vec::new();
                    for i in 0..2 {
                        sounds.push(load_sound_file_part_and_upload(
                            sound_mt,
                            &files,
                            default_files,
                            game_name,
                            &["audio", "heart"],
                            &format!("collect{}", i + 1),
                        )?);
                    }
                    sounds.try_into().unwrap()
                },
            },
            shield: LoadPickup {
                tex: load_file_part_and_upload(
                    graphics_mt,
                    &files,
                    default_files,
                    game_name,
                    &[],
                    "shield",
                )?,

                spawn: load_sound_file_part_and_upload(
                    sound_mt,
                    &files,
                    default_files,
                    game_name,
                    &["audio", "shield"],
                    "spawn",
                )?,
                collects: {
                    let mut sounds = Vec::new();
                    for i in 0..4 {
                        sounds.push(load_sound_file_part_and_upload(
                            sound_mt,
                            &files,
                            default_files,
                            game_name,
                            &["audio", "shield"],
                            &format!("collect{}", i + 1),
                        )?);
                    }
                    sounds.try_into().unwrap()
                },
            },

            lose_grenade: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                game_name,
                &[],
                "lose_grenade",
            )?,
            lose_laser: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                game_name,
                &[],
                "lose_laser",
            )?,
            lose_ninja: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                game_name,
                &[],
                "lose_ninja",
            )?,
            lose_shotgun: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                game_name,
                &[],
                "lose_shotgun",
            )?,

            stars: {
                let mut stars = Vec::new();
                for index in 0..3 {
                    stars.push(load_file_part_and_upload(
                        graphics_mt,
                        &files,
                        default_files,
                        game_name,
                        &[],
                        &format!("star{}", index + 1),
                    )?);
                }
                stars.try_into().unwrap()
            },

            game_name: game_name.to_string(),
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
}

impl ContainerLoad<Game> for LoadGame {
    fn load(
        item_name: &str,
        files: ContainerLoadedItem,
        default_files: &ContainerLoadedItemDir,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
    ) -> anyhow::Result<Self> {
        match files {
            ContainerLoadedItem::Directory(files) => {
                Self::load_game(graphics_mt, sound_mt, files, default_files, item_name)
            }
            ContainerLoadedItem::SingleFile(_) => Err(anyhow::anyhow!(
                "single file mode is currently not supported"
            )),
        }
    }

    fn convert(
        self,
        texture_handle: &GraphicsTextureHandle,
        sound_object_handle: &SoundObjectHandle,
    ) -> Game {
        Game {
            heart: Pickup {
                tex: Self::load_file_into_texture(texture_handle, self.heart.tex, &self.game_name),

                spawn: sound_object_handle.create(self.heart.spawn),
                collects: self
                    .heart
                    .collects
                    .into_iter()
                    .map(|collect| sound_object_handle.create(collect))
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            },
            shield: Pickup {
                tex: Self::load_file_into_texture(texture_handle, self.shield.tex, &self.game_name),

                spawn: sound_object_handle.create(self.shield.spawn),
                collects: self
                    .shield
                    .collects
                    .into_iter()
                    .map(|collect| sound_object_handle.create(collect))
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            },

            lose_grenade: Self::load_file_into_texture(
                texture_handle,
                self.lose_grenade,
                &self.game_name,
            ),
            lose_laser: Self::load_file_into_texture(
                texture_handle,
                self.lose_laser,
                &self.game_name,
            ),
            lose_ninja: Self::load_file_into_texture(
                texture_handle,
                self.lose_ninja,
                &self.game_name,
            ),
            lose_shotgun: Self::load_file_into_texture(
                texture_handle,
                self.lose_shotgun,
                &self.game_name,
            ),

            stars: self
                .stars
                .into_iter()
                .map(|star| Self::load_file_into_texture(texture_handle, star, &self.game_name))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        }
    }
}

pub type GameContainer = Container<Game, LoadGame>;
pub const GAME_CONTAINER_PATH: &str = "games/";
