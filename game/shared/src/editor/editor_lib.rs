pub mod editor_lib {
    use std::{sync::Arc, time::Duration};

    use base_io::io::IO;
    use config::config::ConfigEngine;
    use editor::editor::EditorInterface;
    use graphics::graphics::graphics::Graphics;
    use sound::sound::SoundManager;
    use ui_base::font_data::UiFontData;

    // TODO: remove this stuff, rust ABI is not stable, not even between different inocations of the same rustc
    // if it works, it works, but nothing more
    pub struct EditorLib {
        lib: Option<libloading::Library>,
    }

    impl EditorLib {
        pub fn new(
            sound: &SoundManager,
            graphics: &Graphics,
            io: &IO,
            font_data: &Arc<UiFontData>,
            lib: libloading::Library,
        ) -> Self {
            let func: libloading::Symbol<
                unsafe extern "Rust" fn(
                    sound: &SoundManager,
                    graphics: &Graphics,
                    io: &IO,
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
        fn render(
            &mut self,
            input: egui::RawInput,
            config: &ConfigEngine,
        ) -> Option<egui::PlatformOutput> {
            unsafe {
                let func: libloading::Symbol<
                    unsafe extern "Rust" fn(
                        input: egui::RawInput,
                        config: &ConfigEngine,
                    ) -> Option<egui::PlatformOutput>,
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

            // TODO: completely random value, without it, it crashes in `dlopen`
            // i tried to sync_all from std::fs::File.. made it less like but didn't help
            // entirely.
            // tried dlopen with RTLD_NOLOAD to check if the .so is still loaded, didn't help
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
