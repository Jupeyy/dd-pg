use std::sync::Arc;

use api::read_param_from_host;
use api::upload_return_val;
use api::GRAPHICS;
use api::GRAPHICS_BACKEND;
use api::SOUND;
use api_wasm_macros::{guest_func_call_from_host_auto, impl_guest_functions_editor};

// TODO: remove them
use api::read_param_from_host_ex;
use config::config::ConfigEngine;
use editor::editor::EditorInterface;
use editor::editor::EditorResult;
use graphics_types::types::WindowProps;
use ui_base::font_data::UiFontData;

extern "Rust" {
    /// returns an instance of the game state and the game tick speed
    fn mod_editor_new(font_data: &Arc<UiFontData>) -> Box<dyn EditorInterface>;
}

pub struct ApiEditor {
    state: Option<Box<dyn EditorInterface>>,
}

impl ApiEditor {
    fn new(&mut self, font_data: &Arc<UiFontData>) {
        let state = unsafe { mod_editor_new(font_data) };
        self.state = Some(state);
    }
}

static mut API_EDITOR: once_cell::unsync::Lazy<ApiEditor> =
    once_cell::unsync::Lazy::new(|| ApiEditor { state: None });

#[no_mangle]
pub fn editor_new() {
    let window_props: WindowProps = read_param_from_host(0);
    let font_data: Arc<UiFontData> = read_param_from_host(1);
    unsafe { GRAPHICS.borrow().canvas_handle.resized(window_props) };
    unsafe {
        API_EDITOR.new(&font_data);
    };
}

#[impl_guest_functions_editor]
impl EditorInterface for ApiEditor {
    #[guest_func_call_from_host_auto(option)]
    fn render(&mut self, input: egui::RawInput, config: &ConfigEngine) -> EditorResult {
        unsafe {
            GRAPHICS_BACKEND.actual_run_cmds.set(false);
            let graphics = &mut *GRAPHICS;
            graphics
                .borrow()
                .backend_handle
                .run_backend_buffer(graphics.borrow().stream_handle.stream_data());
            GRAPHICS_BACKEND.actual_run_cmds.set(true);
            SOUND.borrow().backend_handle.run_cmds();
        }
    }
}
