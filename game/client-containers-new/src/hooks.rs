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

#[derive(Debug, Hiarc, Clone)]
pub struct Hook {
    pub hook_chain: TextureContainer,
    pub hook_head: TextureContainer,

    pub hit_hookable: [SoundObject; 3],
    pub hit_player: [SoundObject; 3],
    pub hit_unhookable: [SoundObject; 3],
}

#[derive(Debug, Hiarc)]
pub struct LoadHook {
    hook_chain: ContainerItemLoadData,
    hook_head: ContainerItemLoadData,

    hit_hookable: [SoundBackendMemory; 3],
    hit_player: [SoundBackendMemory; 3],
    hit_unhookable: [SoundBackendMemory; 3],

    hook_name: String,
}

impl LoadHook {
    pub fn load_hook(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        hook_name: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            hook_chain: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                hook_name,
                &[],
                "hook_chain",
            )?,
            hook_head: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                hook_name,
                &[],
                "hook_head",
            )?,

            hit_hookable: {
                let mut sounds: Vec<_> = Vec::new();
                for i in 0..3 {
                    sounds.push(load_sound_file_part_and_upload(
                        sound_mt,
                        &files,
                        default_files,
                        hook_name,
                        &[],
                        &format!("hit_hookable{}", i + 1),
                    )?)
                }
                sounds.try_into().unwrap()
            },
            hit_player: {
                let mut sounds: Vec<_> = Vec::new();
                for i in 0..3 {
                    sounds.push(load_sound_file_part_and_upload(
                        sound_mt,
                        &files,
                        default_files,
                        hook_name,
                        &[],
                        &format!("hit_player{}", i + 1),
                    )?)
                }
                sounds.try_into().unwrap()
            },
            hit_unhookable: {
                let mut sounds: Vec<_> = Vec::new();
                for i in 0..3 {
                    sounds.push(load_sound_file_part_and_upload(
                        sound_mt,
                        &files,
                        default_files,
                        hook_name,
                        &[],
                        &format!("hit_unhookable{}", i + 1),
                    )?)
                }
                sounds.try_into().unwrap()
            },

            hook_name: hook_name.to_string(),
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

impl ContainerLoad<Hook> for LoadHook {
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
                Self::load_hook(graphics_mt, sound_mt, files, default_files, item_name)
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
    ) -> Hook {
        Hook {
            hook_chain: Self::load_file_into_texture(
                texture_handle,
                self.hook_chain,
                &self.hook_name,
            ),
            hook_head: Self::load_file_into_texture(
                texture_handle,
                self.hook_head,
                &self.hook_name,
            ),

            hit_hookable: self
                .hit_hookable
                .into_iter()
                .map(|sound| sound_object_handle.create(sound))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            hit_player: self
                .hit_player
                .into_iter()
                .map(|sound| sound_object_handle.create(sound))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            hit_unhookable: self
                .hit_unhookable
                .into_iter()
                .map(|sound| sound_object_handle.create(sound))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        }
    }
}

pub type HookContainer = Container<Hook, LoadHook>;
pub const HOOK_CONTAINER_PATH: &str = "hooks/";
