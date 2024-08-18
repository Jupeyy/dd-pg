pub mod editor_lib {
    use std::sync::Arc;

    use base_io::io::Io;
    use config::config::ConfigEngine;
    use editor::editor::{EditorInterface, EditorResult};
    use graphics::graphics::graphics::Graphics;
    use sound::sound::SoundManager;
    use ui_base::font_data::UiFontData;

    // TODO: remove this stuff, rust ABI is not stable,
    // not even between different inocations of the same rustc
    // if it works, it works, but nothing more, only luck
    // i assume it's because the compiler makes same struct layouts
    // bcs code is too huge to optimize all cases or smth
    pub struct EditorLib {
        lib: Option<libloading::Library>,
    }

    impl EditorLib {
        pub fn new(
            sound: &SoundManager,
            graphics: &Graphics,
            io: &Io,
            font_data: &Arc<UiFontData>,
            lib: libloading::Library,
        ) -> Self {
            let func: libloading::Symbol<
                unsafe extern "Rust" fn(
                    sound: &SoundManager,
                    graphics: &Graphics,
                    io: &Io,
                    font_data: &Arc<UiFontData>,
                ) -> (),
            > = unsafe { lib.get(b"editor_new").unwrap() };
            unsafe {
                func(sound, graphics, io, font_data);
            }
            Self { lib: Some(lib) }
        }
    }

    impl EditorInterface for EditorLib {
        fn render(&mut self, input: egui::RawInput, config: &ConfigEngine) -> EditorResult {
            unsafe {
                let func: libloading::Symbol<
                    unsafe extern "Rust" fn(
                        input: egui::RawInput,
                        config: &ConfigEngine,
                    ) -> EditorResult,
                > = self.lib.as_ref().unwrap().get(b"editor_render").unwrap();

                func(input, config)
            }
        }
    }

    impl Drop for EditorLib {
        fn drop(&mut self) {
            unsafe {
                let func: libloading::Symbol<unsafe extern "Rust" fn() -> ()> =
                    self.lib.as_ref().unwrap().get(b"editor_destroy").unwrap();

                func();
            }
            self.lib.take().unwrap().close().unwrap();
        }
    }
}
