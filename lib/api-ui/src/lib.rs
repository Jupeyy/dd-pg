use std::time::Duration;

use api::{graphics::graphics::GraphicsBackend, read_param_from_host, GRAPHICS, GRAPHICS_BACKEND};
use config::config::Config;
use graphics_base_traits::traits::{GraphicsBackendHandleInterface, GraphicsSizeQuery};
use graphics_types::types::WindowProps;
use ui_base::{
    types::{RawInputWrapper, UIFeedbackInterface, UIPipe, UIRawInputGenerator, UIState},
    ui::{UIInterface, UI as RealUI},
    ui_render::render_ui,
};
use ui_traits::traits::UIRenderCallbackFunc;

pub struct UIWinitWrapper {}

pub struct UIWinitWrapperPipe {
    inp: RawInputWrapper,
}

impl UIRawInputGenerator<UIWinitWrapper> for UIWinitWrapperPipe {
    fn get_raw_input(&self, _state: &mut UIState<UIWinitWrapper>) -> egui::RawInput {
        self.inp.input.clone()
    }
}

pub struct UI {
    pub ui: RealUI<UIWinitWrapper>,
}

impl UI {
    pub fn new(zoom_level: f32) -> Self {
        Self {
            ui: RealUI::new(UIWinitWrapper {}, zoom_level),
        }
    }
}

pub struct ClientStatsUIFeedbackDummy {}

impl UIFeedbackInterface for ClientStatsUIFeedbackDummy {}

extern "Rust" {
    /// returns an instance of the game state and the game tick speed
    fn mod_ui_new() -> Box<dyn UIRenderCallbackFunc<UIWinitWrapper, GraphicsBackend>>;
}

static mut API_UI: once_cell::unsync::Lazy<UI> = once_cell::unsync::Lazy::new(|| UI::new(2.0));

static mut API_UI_USER: once_cell::unsync::Lazy<
    Box<dyn UIRenderCallbackFunc<UIWinitWrapper, GraphicsBackend>>,
> = once_cell::unsync::Lazy::new(|| unsafe { mod_ui_new() });

#[no_mangle]
pub fn ui_new() {}

#[no_mangle]
pub fn ui_run() {
    let cur_time = read_param_from_host::<Duration>(0);
    let window_props = read_param_from_host::<WindowProps>(1);
    let inp = read_param_from_host::<RawInputWrapper>(2);
    let zoom_level = read_param_from_host::<f32>(3);

    unsafe { &mut API_UI }.ui.ui_state.zoom_level = zoom_level;
    unsafe { &mut GRAPHICS }.resized(window_props);

    let (screen_rect, full_output) = unsafe { &mut API_UI.ui }.render(
        unsafe { &mut GRAPHICS }.window_width(),
        unsafe { &mut GRAPHICS }.window_height(),
        |ui, pipe, ui_state| {
            unsafe { &mut API_UI_USER }.render(ui, pipe, ui_state, unsafe { &mut *GRAPHICS })
        },
        &mut UIPipe {
            ui_feedback: &mut ClientStatsUIFeedbackDummy {}, // TODO: the interface/trait is not implemented (wasm caller functions)
            cur_time: cur_time,
            config: &mut Config::default(),
            raw_inp_generator: &UIWinitWrapperPipe { inp }, // TODO: get input from host
        },
    );

    render_ui(
        unsafe { &mut API_UI.ui },
        full_output,
        &screen_rect,
        unsafe { &mut *GRAPHICS },
    );

    *unsafe { &mut *GRAPHICS_BACKEND }
        .actual_run_cmds
        .borrow_mut() = false;
    let graphics = unsafe { &mut *GRAPHICS };
    graphics
        .backend_handle
        .run_backend_buffer(&graphics.stream_handle.stream_data);
    *unsafe { &mut *GRAPHICS_BACKEND }
        .actual_run_cmds
        .borrow_mut() = true;
}
