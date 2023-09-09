use std::{str::FromStr, sync::Arc};

use arrayvec::ArrayString;
use async_trait::async_trait;
use base_fs::filesys::FileSystem;

use graphics_backend::types::Graphics;
use graphics_types::{
    command_buffer::{TexFlags, TexFormat},
    textures_handle::{GraphicsTextureHandleInterface, TextureIndex},
    types::ImageFormat,
};
use image::png::load_png_image;

use super::container::{load_file_part, Container, ContainerLoad};

#[derive(Clone)]
pub struct CTF {
    pub flag_red: TextureIndex,
    pub flag_blue: TextureIndex,
}

#[derive(Default, Clone)]
pub struct LoadCTF {
    flag_red: Vec<u8>,
    flag_blue: Vec<u8>,

    ctf_name: String,
}

impl LoadCTF {
    pub async fn load_ctf(&mut self, fs: &FileSystem, ctf_name: &str) -> anyhow::Result<()> {
        let ctf_path = ArrayString::<4096>::from_str("ctfs/").unwrap();

        self.flag_red = load_file_part(fs, &ctf_path, ctf_name, &[], "flag_red").await?;
        self.flag_blue = load_file_part(fs, &ctf_path, ctf_name, &[], "flag_blue").await?;

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
            .texture_handle
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
impl ContainerLoad<CTF> for LoadCTF {
    async fn load(
        &mut self,
        item_name: &str,
        fs: &Arc<FileSystem>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
    ) -> anyhow::Result<()> {
        self.load_ctf(fs, item_name).await.unwrap();
        self.ctf_name = item_name.to_string();
        Ok(())
    }

    fn convert(self, graphics: &mut Graphics) -> CTF {
        CTF {
            flag_red: Self::load_file_into_texture(graphics, &self.flag_red, &self.ctf_name),
            flag_blue: Self::load_file_into_texture(graphics, &self.flag_blue, &self.ctf_name),
        }
    }
}

pub type CTFContainer = Container<CTF, LoadCTF>;
