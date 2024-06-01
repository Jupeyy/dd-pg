pub mod graphics {
    use std::rc::Rc;

    use graphics_backend_traits::{
        frame_fetcher_plugin::BackendPresentedImageData, traits::GraphicsBackendInterface,
    };
    use graphics_base_traits::traits::GraphicsStreamedData;
    use hiarc::Hiarc;
    use image::png::save_png_image;

    use crate::{
        graphics_mt::GraphicsMultiThreaded,
        handles::{
            backend::backend::GraphicsBackendHandle,
            buffer_object::buffer_object::GraphicsBufferObjectHandle,
            canvas::canvas::GraphicsCanvasHandle,
            quad_container::quad_container::GraphicsQuadContainerHandle,
            stream::stream::GraphicsStreamHandle, texture::texture::GraphicsTextureHandle,
        },
    };

    use graphics_types::{
        commands::{AllCommands, Commands},
        types::{VideoMode, WindowProps},
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

    #[derive(Debug, Hiarc)]
    pub struct Graphics {
        pub backend_handle: GraphicsBackendHandle,

        pub canvas_handle: GraphicsCanvasHandle,

        pub quad_container_handle: GraphicsQuadContainerHandle,

        pub buffer_object_handle: GraphicsBufferObjectHandle,

        pub stream_handle: GraphicsStreamHandle,

        pub texture_handle: GraphicsTextureHandle,
    }

    impl Graphics {
        pub fn new(
            backend: Rc<dyn GraphicsBackendInterface>,
            stream_data: GraphicsStreamedData,
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
}
