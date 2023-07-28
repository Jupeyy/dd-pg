use std::{str::FromStr, sync::Arc};

use arrayvec::ArrayString;
use async_trait::async_trait;
use base_fs::filesys::FileSystem;
use graphics::graphics::{Graphics, GraphicsTextureAllocations};
use graphics_types::{
    command_buffer::{TexFlags, TexFormat},
    rendering::TextureIndex,
    types::ImageFormat,
};

use crate::client::image::png::load_png_image;

use super::container::{load_file_part, Container, ContainerItemInterface, ContainerLoad};

#[derive(Clone)]
pub struct Emoticon {
    pub tex: TextureIndex,
}

impl ContainerItemInterface for Emoticon {
    fn destroy(self, graphics: &mut Graphics) {
        graphics.unload_texture(self.tex);
    }
}

#[derive(Default, Clone)]
pub struct LoadEmoticon {
    tex: Vec<u8>,

    emoticon_name: String,
}

impl LoadEmoticon {
    pub async fn load_emoticon(
        &mut self,
        fs: &FileSystem,
        emoticon_name: &str,
    ) -> anyhow::Result<()> {
        let emoticon_path = ArrayString::<4096>::from_str("emoticons/").unwrap();

        self.tex = load_file_part(fs, &emoticon_path, emoticon_name, &[], "emoticon").await?;

        Ok(())
    }

    fn load_file_into_texture(graphics: &mut Graphics, file: &Vec<u8>, name: &str) -> TextureIndex {
        let mut img_data = Vec::<u8>::new();
        let part_img = load_png_image(&file, |size| {
            img_data = vec![0; size];
            &mut img_data
        })
        .unwrap();
        let texture_id = graphics
            .load_texture_slow(
                part_img.width as usize,
                part_img.height as usize,
                ImageFormat::Rgba as i32,
                part_img.data.to_vec(),
                TexFormat::RGBA as i32,
                TexFlags::empty(),
                name,
            )
            .unwrap();
        texture_id
    }
}

#[async_trait]
impl ContainerLoad<Emoticon> for LoadEmoticon {
    async fn load(&mut self, item_name: &str, fs: &Arc<FileSystem>) -> anyhow::Result<()> {
        self.load_emoticon(fs, item_name).await.unwrap();
        self.emoticon_name = item_name.to_string();
        Ok(())
    }

    fn convert(self, graphics: &mut Graphics) -> Emoticon {
        Emoticon {
            tex: Self::load_file_into_texture(graphics, &self.tex, &self.emoticon_name),
        }
    }
}

pub type EmoticonContainer = Container<Emoticon, LoadEmoticon>;
