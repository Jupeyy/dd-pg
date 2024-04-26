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

use crate::{
    container::load_sound_file_part_and_upload,
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
    pub async fn load_freeze(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        fs: &dyn FileSystemInterface,
        freeze_name: &str,
    ) -> anyhow::Result<Self> {
        let freeze_path = Path::new("freezes/");

        Ok(Self {
            cursor: load_file_part_and_upload(
                graphics_mt,
                fs,
                &freeze_path,
                freeze_name,
                &[],
                "cursor",
            )
            .await?,
            weapon: load_file_part_and_upload(
                graphics_mt,
                fs,
                &freeze_path,
                freeze_name,
                &[],
                "weapon",
            )
            .await?,
            muzzles: {
                let mut muzzles: Vec<_> = Default::default();
                for i in 0..3 {
                    muzzles.push(
                        load_file_part_and_upload(
                            graphics_mt,
                            fs,
                            &freeze_path,
                            freeze_name,
                            &[],
                            &format!("muzzle{i}"),
                        )
                        .await?,
                    );
                }
                muzzles
            },

            skin: LoadSkin::load_skin(
                graphics_mt,
                sound_mt,
                fs,
                freeze_name,
                Some("skin"),
                freeze_path,
            )
            .await?,

            attack: load_sound_file_part_and_upload(
                sound_mt,
                fs,
                freeze_path,
                freeze_name,
                &["audio"],
                "attack",
            )
            .await?,

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

#[async_trait]
impl ContainerLoad<Freeze> for LoadFreeze {
    async fn load(
        item_name: &str,
        fs: &Arc<dyn FileSystemInterface>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
    ) -> anyhow::Result<Self> {
        Self::load_freeze(graphics_mt, sound_mt, fs.as_ref(), item_name).await
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
