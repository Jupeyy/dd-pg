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
pub struct Hook {
    pub hook_chain: TextureIndex,
    pub hook_head: TextureIndex,
}

#[derive(Debug)]
pub struct LoadHook {
    hook_chain: ContainerItemLoadData,
    hook_head: ContainerItemLoadData,

    hook_name: String,
}

impl LoadHook {
    pub async fn load_hook(
        graphics_mt: &GraphicsMultiThreaded,
        fs: &dyn FileSystemInterface,
        hook_name: &str,
    ) -> anyhow::Result<Self> {
        let hook_path = ArrayString::<4096>::from_str("hooks/").unwrap();

        Ok(Self {
            hook_chain: load_file_part_and_upload(
                graphics_mt,
                fs,
                &hook_path,
                hook_name,
                &[],
                "hook_chain",
            )
            .await?,
            hook_head: load_file_part_and_upload(
                graphics_mt,
                fs,
                &hook_path,
                hook_name,
                &[],
                "hook_head",
            )
            .await?,

            hook_name: hook_name.to_string(),
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
impl ContainerLoad<Hook> for LoadHook {
    async fn load(
        item_name: &str,
        fs: &Arc<dyn FileSystemInterface>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
    ) -> anyhow::Result<Self> {
        Self::load_hook(graphics_mt, fs.as_ref(), item_name).await
    }

    fn convert<B: GraphicsBackendInterface>(self, graphics: &mut GraphicsBase<B>) -> Hook {
        Hook {
            hook_chain: Self::load_file_into_texture(graphics, self.hook_chain, &self.hook_name),
            hook_head: Self::load_file_into_texture(graphics, self.hook_head, &self.hook_name),
        }
    }
}

pub type HookContainer = Container<Hook, LoadHook>;
