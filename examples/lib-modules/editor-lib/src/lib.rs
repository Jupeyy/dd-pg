use std::{num::NonZeroUsize, sync::Arc};

use base_io::io::Io;
use config::config::ConfigEngine;
use editor::editor::{Editor, EditorInterface, EditorResult};
use graphics::graphics::graphics::Graphics;
use sound::sound::SoundManager;
use ui_base::font_data::UiFontData;

pub struct ApiEditor {
    state: Option<Box<dyn EditorInterface>>,
}

impl ApiEditor {
    fn create(
        &mut self,
        sound: &SoundManager,
        graphics: &Graphics,
        io: &Io,
        tp: &Arc<rayon::ThreadPool>,
        font_data: &Arc<UiFontData>,
    ) {
        let state = Editor::new(sound, graphics, io, tp, font_data);
        self.state = Some(Box::new(state));
    }
}

static mut API_EDITOR: once_cell::unsync::Lazy<ApiEditor> =
    once_cell::unsync::Lazy::new(|| ApiEditor { state: None });

#[no_mangle]
pub fn editor_new(sound: &SoundManager, graphics: &Graphics, io: &Io, font_data: &Arc<UiFontData>) {
    unsafe {
        let thread_pool = Arc::new(
            rayon::ThreadPoolBuilder::new()
                .thread_name(|index| format!("editor-rayon {index}"))
                .num_threads(
                    std::thread::available_parallelism()
                        .unwrap_or(NonZeroUsize::new(2).unwrap())
                        .get()
                        .max(4)
                        - 2,
                )
                .build()
                .unwrap(),
        );
        API_EDITOR.create(sound, graphics, io, &thread_pool, font_data);
    };
}

#[no_mangle]
pub fn editor_render(input: egui::RawInput, config: &ConfigEngine) -> EditorResult {
    unsafe { API_EDITOR.state.as_mut().unwrap().render(input, config) }
}

#[no_mangle]
pub fn editor_destroy() {
    unsafe {
        API_EDITOR.state = None;
    }
}
