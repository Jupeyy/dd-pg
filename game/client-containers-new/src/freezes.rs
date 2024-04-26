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

pub struct Freeze {
    pub cursor: TextureContainer,
    pub weapon: TextureContainer,
    pub muzzles: Vec<TextureContainer>,

    pub skin: Skin,

    pub attack: SoundObject,
}

#[derive(Debug)]
pub struct LoadFreeze {
    cursor: ContainerItemLoadData,
    muzzles: Vec<ContainerItemLoadData>,
    weapon: ContainerItemLoadData,

    skin: LoadSkin,

    attack: SoundBackendMemory,

    freeze_name: String,
}

impl LoadFreeze {
    pub fn load_freeze(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        freeze_name: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            cursor: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                freeze_name,
                &[],
                "cursor",
            )?,
            weapon: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                freeze_name,
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
                        freeze_name,
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
                freeze_name,
                Some("skin"),
            )?,

            attack: load_sound_file_part_and_upload(
                sound_mt,
                &files,
                default_files,
                freeze_name,
                &["audio"],
                "attack",
            )?,

            freeze_name: freeze_name.to_string(),
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

impl ContainerLoad<Freeze> for LoadFreeze {
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
                Self::load_freeze(graphics_mt, sound_mt, files, default_files, item_name)
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
    ) -> Freeze {
        Freeze {
            cursor: Self::load_file_into_texture(texture_handle, self.cursor, &self.freeze_name),
            weapon: Self::load_file_into_texture(texture_handle, self.weapon, &self.freeze_name),
            muzzles: self
                .muzzles
                .into_iter()
                .map(|muzzle| {
                    Self::load_file_into_texture(texture_handle, muzzle, &self.freeze_name)
                })
                .collect(),

            skin: self.skin.convert(texture_handle, sound_object_handle),

            attack: sound_object_handle.create(self.attack),
        }
    }
}

pub type FreezeContainer = Container<Freeze, LoadFreeze>;
pub const FREEZE_CONTAINER_PATH: &str = "freezes/";
