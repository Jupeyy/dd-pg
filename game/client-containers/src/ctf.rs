use std::{str::FromStr, sync::Arc};

use arrayvec::ArrayString;
use async_trait::async_trait;

use base_fs_traits::traits::FileSystemInterface;
use graphics::{graphics::GraphicsBase, graphics_mt::GraphicsMultiThreaded};
use graphics_backend_traits::traits::GraphicsBackendInterface;
use graphics_types::{
    command_buffer::{TexFlags, TexFormat},
    textures_handle::TextureIndex,
    types::ImageFormat,
};

use super::container::{
    load_file_part_and_upload, Container, ContainerItemLoadData, ContainerLoad,
};

#[derive(Clone)]
pub struct CTF {
    pub flag_red: TextureIndex,
    pub flag_blue: TextureIndex,
}

#[derive(Debug)]
pub struct LoadCTF {
    flag_red: ContainerItemLoadData,
    flag_blue: ContainerItemLoadData,

    ctf_name: String,
}

impl LoadCTF {
    pub async fn load_ctf(
        graphics_mt: &GraphicsMultiThreaded,
        fs: &dyn FileSystemInterface,
        ctf_name: &str,
    ) -> anyhow::Result<Self> {
        let ctf_path = ArrayString::<4096>::from_str("ctfs/").unwrap();

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
            ctf_name: ctf_name.to_string(),
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
impl ContainerLoad<CTF> for LoadCTF {
    async fn load(
        item_name: &str,
        fs: &Arc<dyn FileSystemInterface>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
    ) -> anyhow::Result<Self> {
        LoadCTF::load_ctf(graphics_mt, fs.as_ref(), item_name).await
    }

    fn convert<B: GraphicsBackendInterface>(self, graphics: &mut GraphicsBase<B>) -> CTF {
        CTF {
            flag_red: Self::load_file_into_texture(graphics, self.flag_red, &self.ctf_name),
            flag_blue: Self::load_file_into_texture(graphics, self.flag_blue, &self.ctf_name),
        }
    }
}

pub type CTFContainer = Container<CTF, LoadCTF>;
