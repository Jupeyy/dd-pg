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
use sound::{
    sound_handle::SoundObjectHandle, sound_mt::SoundMultiThreaded,
    sound_mt_types::SoundBackendMemory, sound_object::SoundObject,
};

use crate::container::load_sound_file_part_and_upload;

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
    pub async fn load_game(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        fs: &dyn FileSystemInterface,
        game_name: &str,
    ) -> anyhow::Result<Self> {
        let game_path = Path::new("games/");

        Ok(Self {
            heart: LoadPickup {
                tex: load_file_part_and_upload(
                    graphics_mt,
                    fs,
                    &game_path,
                    game_name,
                    &[],
                    "heart",
                )
                .await?,

                spawn: load_sound_file_part_and_upload(
                    sound_mt,
                    fs,
                    game_path,
                    game_name,
                    &["audio", "heart"],
                    "spawn",
                )
                .await?,
                collects: {
                    let mut sounds = Vec::new();
                    for i in 0..2 {
                        sounds.push(
                            load_sound_file_part_and_upload(
                                sound_mt,
                                fs,
                                game_path,
                                game_name,
                                &["audio", "heart"],
                                &format!("collect{}", i + 1),
                            )
                            .await?,
                        );
                    }
                    sounds.try_into().unwrap()
                },
            },
            shield: LoadPickup {
                tex: load_file_part_and_upload(
                    graphics_mt,
                    fs,
                    &game_path,
                    game_name,
                    &[],
                    "shield",
                )
                .await?,

                spawn: load_sound_file_part_and_upload(
                    sound_mt,
                    fs,
                    game_path,
                    game_name,
                    &["audio", "shield"],
                    "spawn",
                )
                .await?,
                collects: {
                    let mut sounds = Vec::new();
                    for i in 0..4 {
                        sounds.push(
                            load_sound_file_part_and_upload(
                                sound_mt,
                                fs,
                                game_path,
                                game_name,
                                &["audio", "shield"],
                                &format!("collect{}", i + 1),
                            )
                            .await?,
                        );
                    }
                    sounds.try_into().unwrap()
                },
            },

            lose_grenade: load_file_part_and_upload(
                graphics_mt,
                fs,
                &game_path,
                game_name,
                &[],
                "lose_grenade",
            )
            .await?,
            lose_laser: load_file_part_and_upload(
                graphics_mt,
                fs,
                &game_path,
                game_name,
                &[],
                "lose_laser",
            )
            .await?,
            lose_ninja: load_file_part_and_upload(
                graphics_mt,
                fs,
                &game_path,
                game_name,
                &[],
                "lose_ninja",
            )
            .await?,
            lose_shotgun: load_file_part_and_upload(
                graphics_mt,
                fs,
                &game_path,
                game_name,
                &[],
                "lose_shotgun",
            )
            .await?,

            stars: {
                let mut stars = Vec::new();
                for index in 0..3 {
                    stars.push(
                        load_file_part_and_upload(
                            graphics_mt,
                            fs,
                            &game_path,
                            game_name,
                            &[],
                            &format!("star{}", index + 1),
                        )
                        .await?,
                    );
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

#[async_trait]
impl ContainerLoad<Game> for LoadGame {
    async fn load(
        item_name: &str,
        fs: &Arc<dyn FileSystemInterface>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
    ) -> anyhow::Result<Self> {
        LoadGame::load_game(graphics_mt, sound_mt, fs.as_ref(), item_name).await
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
