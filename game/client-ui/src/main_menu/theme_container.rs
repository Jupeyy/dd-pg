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
use sound::{sound_handle::SoundObjectHandle, sound_mt::SoundMultiThreaded};

use client_containers::container::{
    load_file_part_and_upload, ContainerLoadedItem, ContainerLoadedItemDir,
};

use client_containers::container::{Container, ContainerItemLoadData, ContainerLoad};

#[derive(Debug, Hiarc, Clone)]
pub struct Theme {
    pub icon: TextureContainer,
}

#[derive(Debug, Hiarc)]
pub struct LoadTheme {
    icon: ContainerItemLoadData,

    theme_name: String,
}

impl LoadTheme {
    pub fn new(
        graphics_mt: &GraphicsMultiThreaded,
        files: ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        theme_name: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            icon: load_file_part_and_upload(
                graphics_mt,
                &files,
                default_files,
                theme_name,
                &[],
                "icon",
            )?
            .img,

            theme_name: theme_name.to_string(),
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
                TexFormat::Rgba,
                TexFlags::empty(),
                name,
            )
            .unwrap()
    }
}

impl ContainerLoad<Theme> for LoadTheme {
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
                Self::new(graphics_mt, files, default_files, item_name)
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
    ) -> Theme {
        Theme {
            icon: LoadTheme::load_file_into_texture(texture_handle, self.icon, &self.theme_name),
        }
    }
}

pub type ThemeContainer = Container<Theme, LoadTheme>;
pub const THEME_CONTAINER_PATH: &str = "themes/";
