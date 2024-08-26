use std::sync::Arc;

pub use api::*;
pub use api_editor::*;
use editor::editor::{Editor, EditorInterface};
use ui_base::font_data::UiFontData;

#[no_mangle]
fn mod_editor_new(font_data: &Arc<UiFontData>) -> Box<dyn EditorInterface> {
    let editor = Editor::new(
        unsafe { &SOUND.borrow() },
        unsafe { &GRAPHICS.borrow() },
        unsafe { &IO.borrow() },
        &RUNTIME_THREAD_POOL,
        font_data,
    );
    Box::new(editor)
}
