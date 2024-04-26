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
    pub async fn load_ctf(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        fs: &dyn FileSystemInterface,
        ctf_name: &str,
    ) -> anyhow::Result<Self> {
        let ctf_path = Path::new("ctfs/");

        Ok(Self {
            flag_red: load_file_part_and_upload(
                graphics_mt,
                fs,
                &ctf_path,
                ctf_name,
                &[],
                "flag_red",
            )
            .await?,
            flag_blue: load_file_part_and_upload(
                graphics_mt,
                fs,
                &ctf_path,
                ctf_name,
                &[],
                "flag_blue",
            )
            .await?,

            capture: load_sound_file_part_and_upload(
                sound_mt,
                fs,
                ctf_path,
                ctf_name,
                &["audio"],
                "capture",
            )
            .await?,
            collect_team: load_sound_file_part_and_upload(
                sound_mt,
                fs,
                ctf_path,
                ctf_name,
                &["audio"],
                "collect_team",
            )
            .await?,
            collect_opponents: load_sound_file_part_and_upload(
                sound_mt,
                fs,
                ctf_path,
                ctf_name,
                &["audio"],
                "collect_opponents",
            )
            .await?,
            drop: load_sound_file_part_and_upload(
                sound_mt,
                fs,
                ctf_path,
                ctf_name,
                &["audio"],
                "drop",
            )
            .await?,
            return_sound: load_sound_file_part_and_upload(
                sound_mt,
                fs,
                ctf_path,
                ctf_name,
                &["audio"],
                "return",
            )
            .await?,

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

#[async_trait]
impl ContainerLoad<CTF> for LoadCTF {
    async fn load(
        item_name: &str,
        fs: &Arc<dyn FileSystemInterface>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
    ) -> anyhow::Result<Self> {
        LoadCTF::load_ctf(graphics_mt, sound_mt, fs.as_ref(), item_name).await
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
