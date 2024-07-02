pub mod texture {
    use anyhow::anyhow;
    use graphics_types::{
        commands::{
            AllCommands, CommandTextureCreate, CommandTextureDestroy, CommandTextureUpdate,
            Commands, TexFlags, TexFormat,
        },
        rendering::{StateTexture, StateTexture2dArray},
        types::{GraphicsBackendMemory, GraphicsMemoryAllocationType, ImageFormat},
    };
    use hiarc::{hiarc_safer_rc_refcell, Hiarc};

    use crate::handles::backend::backend::GraphicsBackendHandle;

    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Hiarc)]
    pub struct GraphicsTextureHandle {
        id_gen: u128,

        backend_handle: GraphicsBackendHandle,
    }

    #[hiarc_safer_rc_refcell]
    impl GraphicsTextureHandle {
        pub(crate) fn new(backend_handle: GraphicsBackendHandle) -> Self {
            Self {
                id_gen: Default::default(),

                backend_handle,
            }
        }

        fn image_format_to_pixel_size(format: ImageFormat) -> usize {
            match format {
                ImageFormat::Rgb => {
                    return 3;
                }
                ImageFormat::SingleComponent => {
                    return 1;
                }
                _ => {
                    return 4;
                }
            }
        }

        fn convert_to_rgba(
            &self,
            mut src_mem: GraphicsBackendMemory,
            src_width: usize,
            src_height: usize,
            src_format: ImageFormat,
            flags: TexFlags,
        ) -> anyhow::Result<GraphicsBackendMemory> {
            if src_format == ImageFormat::Rgba {
                return Ok(src_mem);
            } else {
                let mut data_slice =
                    self.backend_handle
                        .mem_alloc(GraphicsMemoryAllocationType::Texture {
                            width: src_width,
                            height: src_height,
                            depth: 1,
                            flags,
                            is_3d_tex: false,
                        });

                let src_channel_count = Self::image_format_to_pixel_size(src_format);
                let dst_channel_count = 4;
                for y in 0..src_height {
                    for x in 0..src_width {
                        let img_offset_src =
                            (y * src_width * src_channel_count) + (x * src_channel_count);
                        let img_offset_dest =
                            (y * src_width * dst_channel_count) + (x * dst_channel_count);
                        let copy_size = src_channel_count;
                        if src_channel_count == 3 {
                            data_slice.as_mut_slice()[img_offset_dest..img_offset_dest + copy_size]
                                .copy_from_slice(
                                    &src_mem.as_mut_slice()
                                        [img_offset_src..img_offset_src + copy_size],
                                );
                            data_slice.as_mut_slice()[img_offset_dest + 3] = 255;
                        } else if src_channel_count == 1 {
                            data_slice.as_mut_slice()[img_offset_dest + 0] = 255;
                            data_slice.as_mut_slice()[img_offset_dest + 1] = 255;
                            data_slice.as_mut_slice()[img_offset_dest + 2] = 255;
                            data_slice.as_mut_slice()[img_offset_dest + 3] =
                                src_mem.as_mut_slice()[img_offset_src];
                        }
                    }
                }

                return Ok(data_slice);
            }
        }

        fn load_texture_impl(
            &mut self,
            width: usize,
            height: usize,
            depth: usize,
            is_3d_tex: bool,
            format: ImageFormat,
            data: GraphicsBackendMemory,
            _store_format: TexFormat,
            flags: TexFlags,
            _tex_name: &str,
        ) -> anyhow::Result<u128> {
            if width == 0 || height == 0 {
                return Err(anyhow!("width and/or height was 0"));
            }

            // grab texture
            self.id_gen += 1;
            let tex_index = self.id_gen;

            // flags
            let mut cmd_flags = TexFlags::empty();
            if !(flags & TexFlags::TEXFLAG_NOMIPMAPS).is_empty() {
                cmd_flags |= TexFlags::TEXFLAG_NOMIPMAPS;
            }

            let pixel_size = 4;

            // copy texture data
            let _mem_size = width * height * pixel_size;
            let tmp_buff_data =
                self.convert_to_rgba(data, width as usize, height as usize, format, flags);

            if let Ok(data) = tmp_buff_data {
                let cmd = CommandTextureCreate {
                    texture_index: tex_index,
                    width,
                    height,
                    depth,
                    is_3d_tex,
                    pixel_size,
                    format: TexFormat::RGBA as i32,
                    store_format: TexFormat::RGBA as i32,
                    flags: cmd_flags,
                    data,
                };

                self.backend_handle
                    .add_cmd(AllCommands::Misc(Commands::TextureCreate(cmd)));

                Ok(tex_index)
            } else {
                // TODO: add logging dbg_msg("graphics", "converted image %s to RGBA, consider making its file format RGBA", pTexName ? pTexName : "(no name)");
                Err(anyhow!("image could not be converted to rgba"))
            }
        }

        pub fn load_texture(
            &mut self,
            width: usize,
            height: usize,
            format: ImageFormat,
            data: GraphicsBackendMemory,
            store_format: TexFormat,
            flags: TexFlags,
            tex_name: &str,
        ) -> anyhow::Result<TextureContainer> {
            let tex_index = self.load_texture_impl(
                width,
                height,
                1,
                false,
                format,
                data,
                store_format,
                flags,
                tex_name,
            )?;
            Ok(TextureContainer::new(
                tex_index,
                self.backend_handle.clone(),
            ))
        }

        pub fn load_texture_3d(
            &mut self,
            width: usize,
            height: usize,
            depth: usize,
            format: ImageFormat,
            data: GraphicsBackendMemory,
            store_format: TexFormat,
            flags: TexFlags,
            tex_name: &str,
        ) -> anyhow::Result<TextureContainer2dArray> {
            let tex_index = self.load_texture_impl(
                width,
                height,
                depth,
                true,
                format,
                data,
                store_format,
                flags,
                tex_name,
            )?;
            Ok(TextureContainer2dArray::new(
                tex_index,
                self.backend_handle.clone(),
            ))
        }
    }

    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Hiarc)]
    pub struct TextureContainer {
        index: u128,
        backend_handle: GraphicsBackendHandle,
    }

    #[hiarc_safer_rc_refcell]
    impl Drop for TextureContainer {
        fn drop(&mut self) {
            let cmd = CommandTextureDestroy {
                texture_index: self.index,
            };
            self.backend_handle
                .add_cmd(AllCommands::Misc(Commands::TextureDestroy(cmd)));
        }
    }

    #[hiarc_safer_rc_refcell]
    impl TextureContainer {
        pub fn new(index: u128, backend_handle: GraphicsBackendHandle) -> Self {
            Self {
                index,
                backend_handle,
            }
        }

        /// updates the texture with specific limitations:
        /// - all commands that use this texture before this command was issued __might__ see the texture update too
        /// - all commands that are issued after this update are guaranteed to see the texture update
        pub fn update_texture(
            &self,
            x: isize,
            y: isize,
            width: usize,
            height: usize,
            data: Vec<u8>,
        ) -> anyhow::Result<()> {
            if width == 0 || height == 0 {
                return Err(anyhow!("width and/or height was 0"));
            }

            let cmd = CommandTextureUpdate {
                texture_index: self.index,
                x: x as i32,
                y: y as i32,
                width: width as u32,
                height: height as u32,
                format: TexFormat::RGBA as i32,
                data,
            };

            self.backend_handle
                .add_cmd(AllCommands::Misc(Commands::TextureUpdate(cmd)));

            Ok(())
        }

        pub fn get_index_unsafe(&self) -> u128 {
            self.index
        }
    }

    #[derive(Debug, Hiarc, Default, Clone)]
    pub enum TextureType {
        #[default]
        None,
        Texture(TextureContainer),
        ColorAttachmentOfPreviousPass,
    }

    impl Into<StateTexture> for TextureType {
        fn into(self) -> StateTexture {
            match self {
                TextureType::None => StateTexture::None,
                TextureType::Texture(tex) => StateTexture::Texture(tex.get_index_unsafe()),
                TextureType::ColorAttachmentOfPreviousPass => {
                    StateTexture::ColorAttachmentOfPreviousPass
                }
            }
        }
    }

    impl<'a> From<&'a TextureContainer> for TextureType {
        fn from(value: &'a TextureContainer) -> Self {
            Self::Texture(value.clone())
        }
    }

    impl From<TextureContainer> for TextureType {
        fn from(value: TextureContainer) -> Self {
            Self::Texture(value)
        }
    }

    impl<'a> From<Option<&'a TextureContainer>> for TextureType {
        fn from(value: Option<&'a TextureContainer>) -> Self {
            match value {
                Some(value) => Self::Texture(value.clone()),
                None => Self::None,
            }
        }
    }

    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Hiarc)]
    pub struct TextureContainer2dArray {
        index: u128,
        backend_handle: GraphicsBackendHandle,
    }

    #[hiarc_safer_rc_refcell]
    impl Drop for TextureContainer2dArray {
        fn drop(&mut self) {
            let cmd = CommandTextureDestroy {
                texture_index: self.index,
            };
            self.backend_handle
                .add_cmd(AllCommands::Misc(Commands::TextureDestroy(cmd)));
        }
    }

    #[hiarc_safer_rc_refcell]
    impl TextureContainer2dArray {
        pub fn new(index: u128, backend_handle: GraphicsBackendHandle) -> Self {
            Self {
                index,
                backend_handle,
            }
        }

        pub fn get_index_unsafe(&self) -> u128 {
            self.index
        }
    }

    #[derive(Debug, Hiarc, Default, Clone)]
    pub enum TextureType2dArray {
        #[default]
        None,
        Texture(TextureContainer2dArray),
    }

    impl Into<StateTexture2dArray> for TextureType2dArray {
        fn into(self) -> StateTexture2dArray {
            match self {
                TextureType2dArray::None => StateTexture2dArray::None,
                TextureType2dArray::Texture(tex) => {
                    StateTexture2dArray::Texture(tex.get_index_unsafe())
                }
            }
        }
    }

    impl<'a> From<&'a TextureContainer2dArray> for TextureType2dArray {
        fn from(value: &'a TextureContainer2dArray) -> Self {
            Self::Texture(value.clone())
        }
    }

    impl From<TextureContainer2dArray> for TextureType2dArray {
        fn from(value: TextureContainer2dArray) -> Self {
            Self::Texture(value)
        }
    }

    impl<'a> From<Option<&'a TextureContainer2dArray>> for TextureType2dArray {
        fn from(value: Option<&'a TextureContainer2dArray>) -> Self {
            match value {
                Some(value) => Self::Texture(value.clone()),
                None => Self::None,
            }
        }
    }
}
