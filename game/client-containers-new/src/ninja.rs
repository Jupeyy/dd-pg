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

use crate::{
    container::{load_sound_file_part_and_upload, ContainerLoadedItem, ContainerLoadedItemDir},
    skins::{LoadSkin, Skin},
};

use super::container::{
    load_file_part_and_upload, Container, ContainerItemLoadData, ContainerLoad,
};

pub struct Ninja {
    pub cursor: TextureContainer,
    pub weapon: TextureContainer,
    pub muzzles: Vec<TextureContainer>,

    pub skin: Skin,

    pub spawn: SoundObject,
    pub collect: SoundObject,
    pub attacks: [SoundObject; 4],
    pub hits: [SoundObject; 4],
}

#[derive(Debug)]
pub struct LoadNinja {
    cursor: ContainerItemLoadData,
    muzzles: Vec<ContainerItemLoadData>,
    weapon: ContainerItemLoadData,

    skin: LoadSkin,

    spawn: SoundBackendMemory,
    collect: SoundBackendMemory,
    attacks: [SoundBackendMemory; 4],
    hits: [SoundBackendMemory; 4],

    ninja_name: String,
}

impl LoadNinja {
    pub fn load_ninja(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        ninja_name: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            cursor: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                ninja_name,
                &[],
                "cursor",
            )?,
            weapon: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                ninja_name,
                &[],
                "weapon",
            )?,
            muzzles: {
                let mut muzzles: Vec<_> = Default::default();
                for i in 0..3 {
                    muzzles.push(load_file_part_and_upload(
                        graphics_mt,
                        &files,
                        default_files,
                        ninja_name,
                        &[],
                        &format!("muzzle{i}"),
                    )?);
                }
                muzzles
            },

            skin: LoadSkin::load_skin(
                graphics_mt,
                sound_mt,
                &files,
                default_files,
                ninja_name,
                Some("skin"),
            )?,

            spawn: load_sound_file_part_and_upload(
                sound_mt,
                &files,
                default_files,
                ninja_name,
                &["audio"],
                "spawn",
            )?,
            collect: load_sound_file_part_and_upload(
                sound_mt,
                &files,
                default_files,
                ninja_name,
                &["audio"],
                "collect",
            )?,
            attacks: {
                let mut sounds: Vec<_> = Vec::new();

                for i in 0..4 {
                    sounds.push(load_sound_file_part_and_upload(
                        sound_mt,
                        &files,
                        default_files,
                        ninja_name,
                        &["audio"],
                        &format!("attack{}", i + 1),
                    )?)
                }

                sounds.try_into().unwrap()
            },
            hits: {
                let mut sounds: Vec<_> = Vec::new();

                for i in 0..4 {
                    sounds.push(load_sound_file_part_and_upload(
                        sound_mt,
                        &files,
                        default_files,
                        ninja_name,
                        &["audio"],
                        &format!("hit{}", i + 1),
                    )?)
                }

                sounds.try_into().unwrap()
            },
            ninja_name: ninja_name.to_string(),
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

impl ContainerLoad<Ninja> for LoadNinja {
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
                Self::load_ninja(graphics_mt, sound_mt, files, default_files, item_name)
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
    ) -> Ninja {
        Ninja {
            cursor: Self::load_file_into_texture(texture_handle, self.cursor, &self.ninja_name),
            weapon: Self::load_file_into_texture(texture_handle, self.weapon, &self.ninja_name),
            muzzles: self
                .muzzles
                .into_iter()
                .map(|muzzle| {
                    Self::load_file_into_texture(texture_handle, muzzle, &self.ninja_name)
                })
                .collect(),

            skin: self.skin.convert(texture_handle, sound_object_handle),

            spawn: sound_object_handle.create(self.spawn),
            collect: sound_object_handle.create(self.collect),
            attacks: self
                .attacks
                .into_iter()
                .map(|attack| sound_object_handle.create(attack))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            hits: self
                .hits
                .into_iter()
                .map(|hit| sound_object_handle.create(hit))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        }
    }
}

pub type NinjaContainer = Container<Ninja, LoadNinja>;
pub const NINJA_CONTAINER_PATH: &str = "ninjas/";
