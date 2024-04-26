use std::sync::Arc;

use arrayvec::ArrayVec;

use game_interface::types::emoticons::{EmoticonType, EMOTICONS_COUNT};
use graphics::{
    graphics_mt::GraphicsMultiThreaded,
    handles::texture::texture::{GraphicsTextureHandle, TextureContainer},
};
use graphics_types::{
    commands::{TexFlags, TexFormat},
    types::ImageFormat,
};
use num_traits::FromPrimitive;
use sound::{sound_handle::SoundObjectHandle, sound_mt::SoundMultiThreaded};

use crate::container::{ContainerLoadedItem, ContainerLoadedItemDir};

use super::container::{
    load_file_part_and_upload, Container, ContainerItemLoadData, ContainerLoad,
};

#[derive(Clone)]
pub struct Emoticons {
    pub emoticons: [TextureContainer; EMOTICONS_COUNT],
}

#[derive(Debug)]
pub struct LoadEmoticons {
    emoticons: [ContainerItemLoadData; EMOTICONS_COUNT],

    emoticon_name: String,
}

impl LoadEmoticons {
    pub fn load_emoticon(
        graphics_mt: &GraphicsMultiThreaded,
        files: ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        emoticon_name: &str,
    ) -> anyhow::Result<Self> {
        let mut emoticons: [Option<ContainerItemLoadData>; EMOTICONS_COUNT] = Default::default();
        for (i, emoticon) in emoticons.iter_mut().enumerate() {
            let emoticon_type = EmoticonType::from_usize(i).unwrap();

            let name: &'static str = emoticon_type.into();
            *emoticon = Some(load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                emoticon_name,
                &[],
                &name.to_string().to_lowercase(),
            )?);
        }

        Ok(Self {
            emoticons: emoticons
                .into_iter()
                .map(|item| item.unwrap())
                .collect::<ArrayVec<ContainerItemLoadData, EMOTICONS_COUNT>>()
                .into_inner()
                .unwrap(),
            emoticon_name: emoticon_name.to_string(),
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

impl ContainerLoad<Emoticons> for LoadEmoticons {
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
                Self::load_emoticon(graphics_mt, files, default_files, item_name)
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
    ) -> Emoticons {
        Emoticons {
            emoticons: self
                .emoticons
                .into_iter()
                .map(|e| Self::load_file_into_texture(texture_handle, e, &self.emoticon_name))
                .collect::<ArrayVec<TextureContainer, EMOTICONS_COUNT>>()
                .into_inner()
                .unwrap(),
        }
    }
}

pub type EmoticonsContainer = Container<Emoticons, LoadEmoticons>;
pub const EMOTICONS_CONTAINER_PATH: &str = "emoticons/";
