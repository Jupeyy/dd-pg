use std::{str::FromStr, sync::Arc};

use arrayvec::{ArrayString, ArrayVec};
use async_trait::async_trait;

use base_fs_traits::traits::FileSystemInterface;
use graphics::{graphics::GraphicsBase, graphics_mt::GraphicsMultiThreaded};
use graphics_backend_traits::traits::GraphicsBackendInterface;
use graphics_types::{
    command_buffer::{TexFlags, TexFormat},
    textures_handle::TextureIndex,
    types::ImageFormat,
};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum_macros::IntoStaticStr;

use super::container::{
    load_file_part_and_upload, Container, ContainerItemLoadData, ContainerLoad,
};

#[derive(
    Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, FromPrimitive, IntoStaticStr,
)]
pub enum EmoticonType {
    OOP,
    EXCLAMATION,
    HEARTS,
    DROP,
    DOTDOT,
    MUSIC,
    SORRY,
    GHOST,
    SUSHI,
    SPLATTEE,
    DEVILTEE,
    ZOMG,
    ZZZ,
    WTF,
    EYES,
    QUESTION,
}
const EMOTICONS_COUNT: usize = EmoticonType::QUESTION as usize + 1;

#[derive(Clone)]
pub struct Emoticons {
    pub emoticons: [TextureIndex; EMOTICONS_COUNT],
}

#[derive(Debug)]
pub struct LoadEmoticons {
    emoticons: [ContainerItemLoadData; EMOTICONS_COUNT],

    emoticon_name: String,
}

impl LoadEmoticons {
    pub async fn load_emoticon(
        graphics_mt: &GraphicsMultiThreaded,
        fs: &dyn FileSystemInterface,
        emoticon_name: &str,
    ) -> anyhow::Result<Self> {
        let emoticon_path = ArrayString::<4096>::from_str("emoticons/").unwrap();

        let mut emoticons: [Option<ContainerItemLoadData>; EMOTICONS_COUNT] = Default::default();
        for (i, emoticon) in emoticons.iter_mut().enumerate() {
            let emoticon_type = EmoticonType::from_usize(i).unwrap();

            let name: &'static str = emoticon_type.into();
            *emoticon = Some(
                load_file_part_and_upload(
                    graphics_mt,
                    fs,
                    &emoticon_path,
                    emoticon_name,
                    &[],
                    &name.to_string().to_lowercase(),
                )
                .await?,
            );
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

    fn load_file_into_texture<B: GraphicsBackendInterface>(
        graphics: &mut GraphicsBase<B>,
        img: ContainerItemLoadData,
        name: &str,
    ) -> TextureIndex {
        graphics
            .texture_handle
            .load_texture(
                img.width as usize,
                img.height as usize,
                ImageFormat::Rgba as i32,
                img.data,
                TexFormat::RGBA as i32,
                TexFlags::empty(),
                name,
            )
            .unwrap()
    }
}

#[async_trait]
impl ContainerLoad<Emoticons> for LoadEmoticons {
    async fn load(
        item_name: &str,
        fs: &Arc<dyn FileSystemInterface>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
    ) -> anyhow::Result<Self> {
        Self::load_emoticon(graphics_mt, fs.as_ref(), item_name).await
    }

    fn convert<B: GraphicsBackendInterface>(self, graphics: &mut GraphicsBase<B>) -> Emoticons {
        Emoticons {
            emoticons: self
                .emoticons
                .into_iter()
                .map(|e| Self::load_file_into_texture(graphics, e, &self.emoticon_name))
                .collect::<ArrayVec<TextureIndex, EMOTICONS_COUNT>>()
                .into_inner()
                .unwrap(),
        }
    }
}

pub type EmoticonsContainer = Container<Emoticons, LoadEmoticons>;
