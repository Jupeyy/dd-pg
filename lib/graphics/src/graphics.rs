use std::{cell::RefCell, collections::HashMap, rc::Rc};

use anyhow::anyhow;

use base::shared_index::{SharedIndexCleanup, SharedIndexGetIndexUnsafe};

use graphics_backend_traits::{
    frame_fetcher_plugin::BackendPresentedImageData, traits::GraphicsBackendInterface,
};
use graphics_base_traits::traits::GraphicsStreamDataInterface;
use hiarc_macro::{hiarc_safer_rc_refcell, Hiarc};
use image::png::save_png_image;
use num_traits::FromPrimitive;

use pool::mt_pool::Pool;

use crate::{
    graphics_mt::GraphicsMultiThreaded,
    handles::{
        backend::GraphicsBackendHandle, buffer_object::GraphicsBufferObjectHandle,
        canvas::GraphicsCanvasHandle, quad_container::GraphicsQuadContainerHandle,
        stream::GraphicsStreamHandle,
    },
    types::TextureContainer,
};

use graphics_types::{
    commands::{
        AllCommands, CommandIndicesRequiredNumNotify, CommandTextureCreate, CommandTextureDestroy,
        CommandTextureUpdate, Commands, StreamDataMax, TexFlags, TexFormat,
    },
    textures_handle::TextureIndex,
    types::{
        GraphicsBackendMemory, GraphicsMemoryAllocationType, ImageFormat, VideoMode, WindowProps,
    },
};

const _FAKE_MODES: [VideoMode; 68] = [
    VideoMode::new(8192, 4320, 8192, 4320, 0, 8, 8, 8, 0),
    VideoMode::new(7680, 4320, 7680, 4320, 0, 8, 8, 8, 0),
    VideoMode::new(5120, 2880, 5120, 2880, 0, 8, 8, 8, 0),
    VideoMode::new(4096, 2160, 4096, 2160, 0, 8, 8, 8, 0),
    VideoMode::new(3840, 2160, 3840, 2160, 0, 8, 8, 8, 0),
    VideoMode::new(2560, 1440, 2560, 1440, 0, 8, 8, 8, 0),
    VideoMode::new(2048, 1536, 2048, 1536, 0, 8, 8, 8, 0),
    VideoMode::new(1920, 2400, 1920, 2400, 0, 8, 8, 8, 0),
    VideoMode::new(1920, 1440, 1920, 1440, 0, 8, 8, 8, 0),
    VideoMode::new(1920, 1200, 1920, 1200, 0, 8, 8, 8, 0),
    VideoMode::new(1920, 1080, 1920, 1080, 0, 8, 8, 8, 0),
    VideoMode::new(1856, 1392, 1856, 1392, 0, 8, 8, 8, 0),
    VideoMode::new(1800, 1440, 1800, 1440, 0, 8, 8, 8, 0),
    VideoMode::new(1792, 1344, 1792, 1344, 0, 8, 8, 8, 0),
    VideoMode::new(1680, 1050, 1680, 1050, 0, 8, 8, 8, 0),
    VideoMode::new(1600, 1200, 1600, 1200, 0, 8, 8, 8, 0),
    VideoMode::new(1600, 1000, 1600, 1000, 0, 8, 8, 8, 0),
    VideoMode::new(1440, 1050, 1440, 1050, 0, 8, 8, 8, 0),
    VideoMode::new(1440, 900, 1440, 900, 0, 8, 8, 8, 0),
    VideoMode::new(1400, 1050, 1400, 1050, 0, 8, 8, 8, 0),
    VideoMode::new(1368, 768, 1368, 768, 0, 8, 8, 8, 0),
    VideoMode::new(1280, 1024, 1280, 1024, 0, 8, 8, 8, 0),
    VideoMode::new(1280, 960, 1280, 960, 0, 8, 8, 8, 0),
    VideoMode::new(1280, 800, 1280, 800, 0, 8, 8, 8, 0),
    VideoMode::new(1280, 768, 1280, 768, 0, 8, 8, 8, 0),
    VideoMode::new(1152, 864, 1152, 864, 0, 8, 8, 8, 0),
    VideoMode::new(1024, 768, 1024, 768, 0, 8, 8, 8, 0),
    VideoMode::new(1024, 600, 1024, 600, 0, 8, 8, 8, 0),
    VideoMode::new(800, 600, 800, 600, 0, 8, 8, 8, 0),
    VideoMode::new(768, 576, 768, 576, 0, 8, 8, 8, 0),
    VideoMode::new(720, 400, 720, 400, 0, 8, 8, 8, 0),
    VideoMode::new(640, 480, 640, 480, 0, 8, 8, 8, 0),
    VideoMode::new(400, 300, 400, 300, 0, 8, 8, 8, 0),
    VideoMode::new(320, 240, 320, 240, 0, 8, 8, 8, 0),
    VideoMode::new(8192, 4320, 8192, 4320, 0, 5, 6, 5, 0),
    VideoMode::new(7680, 4320, 7680, 4320, 0, 5, 6, 5, 0),
    VideoMode::new(5120, 2880, 5120, 2880, 0, 5, 6, 5, 0),
    VideoMode::new(4096, 2160, 4096, 2160, 0, 5, 6, 5, 0),
    VideoMode::new(3840, 2160, 3840, 2160, 0, 5, 6, 5, 0),
    VideoMode::new(2560, 1440, 2560, 1440, 0, 5, 6, 5, 0),
    VideoMode::new(2048, 1536, 2048, 1536, 0, 5, 6, 5, 0),
    VideoMode::new(1920, 2400, 1920, 2400, 0, 5, 6, 5, 0),
    VideoMode::new(1920, 1440, 1920, 1440, 0, 5, 6, 5, 0),
    VideoMode::new(1920, 1200, 1920, 1200, 0, 5, 6, 5, 0),
    VideoMode::new(1920, 1080, 1920, 1080, 0, 5, 6, 5, 0),
    VideoMode::new(1856, 1392, 1856, 1392, 0, 5, 6, 5, 0),
    VideoMode::new(1800, 1440, 1800, 1440, 0, 5, 6, 5, 0),
    VideoMode::new(1792, 1344, 1792, 1344, 0, 5, 6, 5, 0),
    VideoMode::new(1680, 1050, 1680, 1050, 0, 5, 6, 5, 0),
    VideoMode::new(1600, 1200, 1600, 1200, 0, 5, 6, 5, 0),
    VideoMode::new(1600, 1000, 1600, 1000, 0, 5, 6, 5, 0),
    VideoMode::new(1440, 1050, 1440, 1050, 0, 5, 6, 5, 0),
    VideoMode::new(1440, 900, 1440, 900, 0, 5, 6, 5, 0),
    VideoMode::new(1400, 1050, 1400, 1050, 0, 5, 6, 5, 0),
    VideoMode::new(1368, 768, 1368, 768, 0, 5, 6, 5, 0),
    VideoMode::new(1280, 1024, 1280, 1024, 0, 5, 6, 5, 0),
    VideoMode::new(1280, 960, 1280, 960, 0, 5, 6, 5, 0),
    VideoMode::new(1280, 800, 1280, 800, 0, 5, 6, 5, 0),
    VideoMode::new(1280, 768, 1280, 768, 0, 5, 6, 5, 0),
    VideoMode::new(1152, 864, 1152, 864, 0, 5, 6, 5, 0),
    VideoMode::new(1024, 768, 1024, 768, 0, 5, 6, 5, 0),
    VideoMode::new(1024, 600, 1024, 600, 0, 5, 6, 5, 0),
    VideoMode::new(800, 600, 800, 600, 0, 5, 6, 5, 0),
    VideoMode::new(768, 576, 768, 576, 0, 5, 6, 5, 0),
    VideoMode::new(720, 400, 720, 400, 0, 5, 6, 5, 0),
    VideoMode::new(640, 480, 640, 480, 0, 5, 6, 5, 0),
    VideoMode::new(400, 300, 400, 300, 0, 5, 6, 5, 0),
    VideoMode::new(320, 240, 320, 240, 0, 5, 6, 5, 0),
];

/**
 * This buffer is useful if you want to
 * allocate memory that is going to be used by the
 * graphics engine. It might optimize away expensive
 * heap allocations and copying.
 */
pub struct GraphicsRawMemoryBuffer<'a> {
    memory: Option<GraphicsBackendMemory>,
    graphics: Option<&'a Graphics>,
}

/**
 * implements minimal graphics traits that are useful for
 * memory management from the backend
 */
pub struct StagingGraphics<'a> {
    memory: Option<GraphicsBackendMemory>,
    graphics: &'a Graphics,
}

impl<'a> StagingGraphics<'a> {
    pub fn load_texture(
        &mut self,
        width: usize,
        height: usize,
        format: i32,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<TextureIndex> {
        let mut memory: Option<GraphicsBackendMemory> = None;
        std::mem::swap(&mut memory, &mut self.memory);
        self.graphics.texture_handle.load_texture(
            width,
            height,
            format,
            memory.unwrap(),
            store_format,
            flags,
            tex_name,
        )
    }
}

impl<'a> GraphicsRawMemoryBuffer<'a> {
    pub fn exec(mut self) -> StagingGraphics<'a> {
        let mut memory: Option<GraphicsBackendMemory> = None;
        let mut graphics: Option<&'a Graphics> = None;
        std::mem::swap(&mut memory, &mut self.memory);
        std::mem::swap(&mut graphics, &mut self.graphics);
        StagingGraphics {
            graphics: graphics.unwrap(),
            memory: memory,
        }
    }

    pub fn mem<'b>(&'b mut self) -> &'b mut GraphicsBackendMemory {
        self.memory.as_mut().unwrap()
    }
}

#[hiarc_safer_rc_refcell]
#[derive(Debug, Hiarc)]
pub struct GraphicsTextureHandle {
    texture_indices: HashMap<u128, TextureContainer>,
    id_gen: u128,

    #[hiarc]
    backend_handle: GraphicsBackendHandle,
}

#[hiarc_safer_rc_refcell]
impl SharedIndexCleanup for GraphicsTextureHandle {
    #[hiarc_trait_is_immutable_self]
    fn destroy_from_index(&mut self, index: u128) {
        // unwrap is important to prevent corruptions
        let _ = self.texture_indices.remove(&index).unwrap();

        let cmd = CommandTextureDestroy {
            texture_index: index,
        };
        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::TextureDestroy(cmd)));
    }
}

#[hiarc_safer_rc_refcell]
impl GraphicsTextureHandle {
    pub(crate) fn new(backend_handle: GraphicsBackendHandle) -> Self {
        Self {
            texture_indices: HashMap::with_capacity(StreamDataMax::MaxTextures as usize),
            id_gen: Default::default(),

            backend_handle,
        }
    }

    fn image_format_to_pixel_size(format: i32) -> usize {
        let f = ImageFormat::from_i32(format);
        if let Some(f) = f {
            match f {
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
        return 4;
    }

    fn convert_to_rgba(
        &self,
        mut src_mem: GraphicsBackendMemory,
        src_width: usize,
        src_height: usize,
        src_format: i32,
        flags: TexFlags,
    ) -> anyhow::Result<GraphicsBackendMemory> {
        if src_format == TexFormat::RGBA as i32 {
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
                                &src_mem.as_mut_slice()[img_offset_src..img_offset_src + copy_size],
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

    pub fn update_texture(
        &self,
        tex_index: &TextureIndex,
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
            texture_index: tex_index.get_index_unsafe(),
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

    fn load_texture_impl(
        &mut self,
        width: usize,
        height: usize,
        depth: usize,
        is_3d_tex: bool,
        format: i32,
        data: GraphicsBackendMemory,
        _store_format: i32,
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

            self.texture_indices.insert(
                tex_index,
                TextureContainer {
                    width: width,
                    height: height,
                    depth: depth,
                },
            );

            Ok(tex_index)
        } else {
            // TODO: add logging dbg_msg("graphics", "converted image %s to RGBA, consider making its file format RGBA", pTexName ? pTexName : "(no name)");
            Err(anyhow!("image could not be converted to rgba"))
        }
    }

    #[hiarc_force_impl]
    fn load_texture_inner(
        &mut self,
        width: usize,
        height: usize,
        format: i32,
        data: GraphicsBackendMemory,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<u128> {
        self.load_texture_impl(
            width,
            height,
            1,
            false,
            format,
            data,
            store_format,
            flags,
            tex_name,
        )
    }

    #[hiarc_force_impl]
    fn load_texture_3d_inner(
        &mut self,
        width: usize,
        height: usize,
        depth: usize,
        format: i32,
        data: GraphicsBackendMemory,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<u128> {
        self.load_texture_impl(
            width,
            height,
            depth,
            true,
            format,
            data,
            store_format,
            flags,
            tex_name,
        )
    }

    #[hiarc_force_impl]
    fn load_texture_slow_inner(
        &mut self,
        width: usize,
        height: usize,
        format: i32,
        data: Vec<u8>,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<u128> {
        let mut mem = self
            .backend_handle
            .mem_alloc(GraphicsMemoryAllocationType::Texture {
                width,
                height,
                depth: 1,
                is_3d_tex: false,
                flags,
            });
        mem.copy_from_slice(data.as_slice());
        self.load_texture_impl(
            width,
            height,
            1,
            false,
            format,
            mem,
            store_format,
            flags,
            tex_name,
        )
    }

    #[hiarc_force_impl]
    fn load_texture_3d_slow_inner(
        &mut self,
        width: usize,
        height: usize,
        depth: usize,
        format: i32,
        data: Vec<u8>,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<u128> {
        let mut mem = self
            .backend_handle
            .mem_alloc(GraphicsMemoryAllocationType::Texture {
                width,
                height,
                depth,
                is_3d_tex: true,
                flags,
            });
        mem.copy_from_slice(data.as_slice());
        self.load_texture_impl(
            width,
            height,
            depth,
            true,
            format,
            mem,
            store_format,
            flags,
            tex_name,
        )
    }
}

impl GraphicsTextureHandle {
    pub fn load_texture(
        &self,
        width: usize,
        height: usize,
        format: i32,
        data: GraphicsBackendMemory,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<TextureIndex> {
        self.load_texture_inner(width, height, format, data, store_format, flags, tex_name)
            .map(|index| TextureIndex::new(index, Rc::new(self.clone())))
    }

    pub fn load_texture_3d(
        &self,
        width: usize,
        height: usize,
        depth: usize,
        format: i32,
        data: GraphicsBackendMemory,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<TextureIndex> {
        self.load_texture_3d_inner(
            width,
            height,
            depth,
            format,
            data,
            store_format,
            flags,
            tex_name,
        )
        .map(|index| TextureIndex::new(index, Rc::new(self.clone())))
    }

    pub fn load_texture_slow(
        &self,
        width: usize,
        height: usize,
        format: i32,
        data: Vec<u8>,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<TextureIndex> {
        self.load_texture_slow_inner(width, height, format, data, store_format, flags, tex_name)
            .map(|index| TextureIndex::new(index, Rc::new(self.clone())))
    }

    pub fn load_texture_3d_slow(
        &self,
        width: usize,
        height: usize,
        depth: usize,
        format: i32,
        data: Vec<u8>,
        store_format: i32,
        flags: TexFlags,
        tex_name: &str,
    ) -> anyhow::Result<TextureIndex> {
        self.load_texture_3d_slow_inner(
            width,
            height,
            depth,
            format,
            data,
            store_format,
            flags,
            tex_name,
        )
        .map(|index| TextureIndex::new(index, Rc::new(self.clone())))
    }
}

#[derive(Debug, Hiarc)]
pub struct Graphics {
    #[hiarc]
    pub backend_handle: GraphicsBackendHandle,

    #[hiarc]
    pub canvas_handle: GraphicsCanvasHandle,

    #[hiarc]
    pub quad_container_handle: GraphicsQuadContainerHandle,

    #[hiarc]
    pub buffer_object_handle: GraphicsBufferObjectHandle,

    #[hiarc]
    pub stream_handle: GraphicsStreamHandle,

    #[hiarc]
    pub texture_handle: GraphicsTextureHandle,

    pub index_offset_or_draw_count_pool: Pool<Vec<usize>>,
}

impl Graphics {
    pub fn new(
        backend: Rc<dyn GraphicsBackendInterface>,
        stream_data: Rc<RefCell<dyn GraphicsStreamDataInterface>>,
        window_props: WindowProps,
    ) -> Graphics {
        let backend_handle = GraphicsBackendHandle::new(backend);
        let buffer_object_handle = GraphicsBufferObjectHandle::new(backend_handle.clone());
        Graphics {
            // handles
            canvas_handle: GraphicsCanvasHandle::new(backend_handle.clone(), window_props),

            quad_container_handle: GraphicsQuadContainerHandle::new(
                backend_handle.clone(),
                buffer_object_handle.clone(),
            ),
            buffer_object_handle,
            stream_handle: GraphicsStreamHandle::new(stream_data, backend_handle.clone()),
            texture_handle: GraphicsTextureHandle::new(backend_handle.clone()),
            backend_handle,

            // pools
            index_offset_or_draw_count_pool: Pool::with_sized(64 * 2, || Vec::with_capacity(128)),
        }
    }

    pub fn get_graphics_mt(&self) -> GraphicsMultiThreaded {
        GraphicsMultiThreaded::new(self.backend_handle.backend.get_backend_mt())
    }

    pub fn resized(&mut self, window_props: WindowProps) {
        self.canvas_handle.resized(window_props)
    }

    pub fn swap(&self) {
        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::Swap));
        self.backend_handle
            .run_backend_buffer(self.stream_handle.stream_data());
    }

    pub fn next_switch_pass(&self) {
        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::NextSwitchPass));
    }

    /**
     * Allocates memory to be used in the backend
     */
    pub fn mem_alloc(&self, alloc_type: GraphicsMemoryAllocationType) -> GraphicsRawMemoryBuffer {
        GraphicsRawMemoryBuffer {
            memory: Some(self.backend_handle.mem_alloc(alloc_type)),
            graphics: Some(self),
        }
    }

    pub fn indices_num_required_notify(&mut self, required_indices_count: u64) {
        let mut cmd = CommandIndicesRequiredNumNotify::default();
        cmd.required_indices_num = required_indices_count;

        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::IndicesRequiredNumNotify(cmd)));
    }

    pub fn do_screenshot(&self) -> anyhow::Result<Vec<u8>> {
        let BackendPresentedImageData {
            width,
            height,
            dest_data_buffer,
            ..
        } = self.backend_handle.backend.do_screenshot()?;
        Ok(save_png_image(&dest_data_buffer, width, height)?)
    }
}

impl Drop for Graphics {
    fn drop(&mut self) {
        self.backend_handle
            .run_backend_buffer(self.stream_handle.stream_data());
    }
}
