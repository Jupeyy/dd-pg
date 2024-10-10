use std::{rc::Rc, sync::Arc};

use graphics::{
    graphics_mt::GraphicsMultiThreaded, handles::texture::texture::GraphicsTextureHandle,
};
use sound::{
    sound_handle::SoundObjectHandle, sound_mt::SoundMultiThreaded,
    sound_mt_types::SoundBackendMemory, sound_object::SoundObject,
};

use crate::{
    container::{load_sound_file_part_and_upload_ex, ContainerLoadedItem, ContainerLoadedItemDir},
    skins::{LoadSkin, Skin},
};

use super::container::{Container, ContainerLoad};

#[derive(Debug)]
pub struct Freeze {
    pub skin: Rc<Skin>,

    pub attacks: Vec<SoundObject>,
}

#[derive(Debug)]
pub struct LoadFreeze {
    skin: LoadSkin,

    attacks: Vec<SoundBackendMemory>,

    _freeze_name: String,
}

impl LoadFreeze {
    pub fn new(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        freeze_name: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            skin: LoadSkin::new(
                graphics_mt,
                sound_mt,
                &files,
                default_files,
                freeze_name,
                Some("skin"),
            )?,

            attacks: {
                let mut sounds = Vec::new();
                let mut i = 0;
                let mut allow_default = true;
                loop {
                    match load_sound_file_part_and_upload_ex(
                        sound_mt,
                        &files,
                        default_files,
                        freeze_name,
                        &["audio"],
                        &format!("attack{}", i + 1),
                        allow_default,
                    ) {
                        Ok(sound) => {
                            allow_default &= sound.from_default;
                            sounds.push(sound.mem);
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
                sounds
            },

            _freeze_name: freeze_name.to_string(),
        })
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
                Self::new(graphics_mt, sound_mt, files, default_files, item_name)
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
            skin: self.skin.convert(texture_handle, sound_object_handle),

            attacks: self
                .attacks
                .into_iter()
                .map(|s| sound_object_handle.create(s))
                .collect(),
        }
    }
}

pub type FreezeContainer = Container<Freeze, LoadFreeze>;
pub const FREEZE_CONTAINER_PATH: &str = "freezes/";
