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
pub struct CTF {
    pub flag_red: TextureContainer,
    pub flag_blue: TextureContainer,

    pub capture: SoundObject,
    pub collect_team: SoundObject,
    pub collect_opponents: SoundObject,
    pub drop: SoundObject,
    pub return_sound: SoundObject,
}

#[derive(Debug)]
pub struct LoadCTF {
    flag_red: ContainerItemLoadData,
    flag_blue: ContainerItemLoadData,

    capture: SoundBackendMemory,
    collect_team: SoundBackendMemory,
    collect_opponents: SoundBackendMemory,
    drop: SoundBackendMemory,
    return_sound: SoundBackendMemory,

    ctf_name: String,
}

impl LoadCTF {
    pub fn load_ctf(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        ctf_name: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            flag_red: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                ctf_name,
                &[],
                "flag_red",
            )?,
            flag_blue: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                ctf_name,
                &[],
                "flag_blue",
            )?,

            capture: load_sound_file_part_and_upload(
                sound_mt,
                &files,
                default_files,
                ctf_name,
                &["audio"],
                "capture",
            )?,
            collect_team: load_sound_file_part_and_upload(
                sound_mt,
                &files,
                default_files,
                ctf_name,
                &["audio"],
                "collect_team",
            )?,
            collect_opponents: load_sound_file_part_and_upload(
                sound_mt,
                &files,
                default_files,
                ctf_name,
                &["audio"],
                "collect_opponents",
            )?,
            drop: load_sound_file_part_and_upload(
                sound_mt,
                &files,
                default_files,
                ctf_name,
                &["audio"],
                "drop",
            )?,
            return_sound: load_sound_file_part_and_upload(
                sound_mt,
                &files,
                default_files,
                ctf_name,
                &["audio"],
                "return",
            )?,

            ctf_name: ctf_name.to_string(),
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

impl ContainerLoad<CTF> for LoadCTF {
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
                LoadCTF::load_ctf(graphics_mt, sound_mt, files, default_files, item_name)
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
    ) -> CTF {
        CTF {
            flag_red: Self::load_file_into_texture(texture_handle, self.flag_red, &self.ctf_name),
            flag_blue: Self::load_file_into_texture(texture_handle, self.flag_blue, &self.ctf_name),

            capture: sound_object_handle.create(self.capture),
            collect_team: sound_object_handle.create(self.collect_team),
            collect_opponents: sound_object_handle.create(self.collect_opponents),
            drop: sound_object_handle.create(self.drop),
            return_sound: sound_object_handle.create(self.return_sound),
        }
    }
}

pub type CTFContainer = Container<CTF, LoadCTF>;
pub const CTF_CONTAINER_PATH: &str = "ctfs/";
