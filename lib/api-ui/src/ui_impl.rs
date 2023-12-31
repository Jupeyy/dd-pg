use std::{cell::RefCell, time::Duration};

use api::{read_param_from_host, upload_return_val, GRAPHICS, GRAPHICS_BACKEND};
use config::config::ConfigEngine;

use crate::types::{UIWinitWrapper, UI};
use egui::FullOutput;
use graphics::graphics::Graphics;
use graphics_types::types::WindowProps;
use ui_base::{
    style::default_style,
    types::{
        RawInputWrapper, RawOutputWrapper, UIFeedbackInterface, UIFonts, UINativePipe,
        UINativeState, UIPipe, UIRawInputGenerator,
    },
    ui::UI as RealUI,
};
use ui_traits::traits::UIRenderCallbackFunc;

pub struct UIWinitWrapperPipe {
    inp: RawInputWrapper,
    output: RefCell<Option<egui::PlatformOutput>>,
}

impl UIRawInputGenerator<UIWinitWrapper> for UIWinitWrapperPipe {
    fn get_raw_input(&self, _state: &mut UINativeState<UIWinitWrapper>) -> egui::RawInput {
        self.inp.input.clone()
    }
    fn process_output(
        &self,
        _state: &mut UINativeState<UIWinitWrapper>,
        _ctx: &egui::Context,
        output: egui::PlatformOutput,
    ) {
        *self.output.borrow_mut() = Some(output);
    }
}

pub struct ClientStatsUIFeedbackDummy {}

impl UIFeedbackInterface for ClientStatsUIFeedbackDummy {}

extern "Rust" {
    /// returns an instance of the game state and the game tick speed
    fn mod_ui_new() -> Box<dyn UIRenderCallbackFunc<()>>;

    /// #Example
    /// ```rust
    /// #[no_mangle]
    /// pub fn mod_render_ui(
    ///     ui: &mut UI<UIWinitWrapper>,
    ///     native_pipe: &mut UINativePipe<UIWinitWrapper>,
    ///     full_output: FullOutput,
    ///     screen_rect: &egui::Rect,
    ///     zoom_level: f32,
    ///     graphics: &mut Graphics,
    ///     as_stencil: bool,
    /// ) {
    ///     render_ui(
    ///         ui,
    ///         native_pipe,
    ///         full_output,
    ///         screen_rect,
    ///         zoom_level,
    ///         graphics,
    ///         as_stencil,
    ///     )
    /// }
    ///
    fn mod_render_ui(
        ui: &mut RealUI<UIWinitWrapper>,
        native_pipe: &mut UINativePipe<UIWinitWrapper>,
        full_output: FullOutput,
        screen_rect: &egui::Rect,
        zoom_level: f32,
        graphics: &mut Graphics,
        as_stencil: bool,
    );
}

static mut API_UI: once_cell::unsync::Lazy<UI> = once_cell::unsync::Lazy::new(|| UI::new(None));

static mut API_UI_USER: once_cell::unsync::Lazy<Box<dyn UIRenderCallbackFunc<()>>> =
    once_cell::unsync::Lazy::new(|| unsafe { mod_ui_new() });

static mut API_CONFIG: once_cell::unsync::Lazy<ConfigEngine> =
    once_cell::unsync::Lazy::new(|| ConfigEngine::default());

#[no_mangle]
pub fn ui_new() {
    let fonts = read_param_from_host::<UIFonts>(0);
    unsafe { &mut API_UI }
        .ui
        .context
        .egui_ctx
        .set_fonts(fonts.fonts);
}

/// returns platform output and zoom level
fn ui_run_impl(
    cur_time: Duration,
    window_props: WindowProps,
    inp: RawInputWrapper,
    zoom_level: f32,
    main_frame_only: bool,
) -> (Option<egui::PlatformOutput>, f32) {
    if !main_frame_only || unsafe { &mut API_UI_USER }.has_blur() {
        unsafe { &mut API_UI }.ui.ui_state.zoom_level = Some(zoom_level);
        unsafe { &mut GRAPHICS }.resized(window_props);

        unsafe { &mut API_UI.ui.context.egui_ctx }.set_style(default_style());
        unsafe { &mut API_UI.ui.stencil_context.egui_ctx }.set_style(default_style());

        let inp_generator = UIWinitWrapperPipe {
            inp,
            output: Default::default(),
        };
        let mut native_pipe = UINativePipe {
            raw_inp_generator: &inp_generator,
        };

        let (screen_rect, full_output, zoom_level) = unsafe { &mut API_UI.ui }.render(
            unsafe { &mut GRAPHICS }.canvas_handle.window_width(),
            unsafe { &mut GRAPHICS }.canvas_handle.window_height(),
            unsafe { &mut GRAPHICS }
                .canvas_handle
                .window_pixels_per_point(),
            |ui, pipe, ui_state| {
                if main_frame_only {
                    unsafe { &mut API_UI_USER }
                        .render_main_frame(ui, pipe, ui_state, unsafe { &mut *GRAPHICS })
                } else {
                    unsafe { &mut API_UI_USER }
                        .render(ui, pipe, ui_state, unsafe { &mut *GRAPHICS })
                }
            },
            &mut UIPipe::new(
                &mut ClientStatsUIFeedbackDummy {}, // TODO: the interface/trait is not implemented (wasm caller functions)
                cur_time,
                unsafe { &mut API_CONFIG },
                (),
            ),
            &mut native_pipe,
            main_frame_only,
        );

        unsafe {
            mod_render_ui(
                &mut API_UI.ui,
                &mut native_pipe,
                full_output,
                &screen_rect,
                zoom_level,
                &mut *GRAPHICS,
                main_frame_only,
            )
        };

        unsafe { &mut *GRAPHICS_BACKEND }
            .actual_run_cmds
            .set(false);
        let graphics = unsafe { &mut *GRAPHICS };
        graphics
            .backend_handle
            .run_backend_buffer(graphics.stream_handle.stream_data());
        unsafe { &mut *GRAPHICS_BACKEND }
            .actual_run_cmds
            .set(true);

        (inp_generator.output.take(), zoom_level)
    } else {
        (None, 0.0)
    }
}

#[no_mangle]
pub fn ui_has_blur() -> u8 {
    match unsafe { &mut API_UI_USER }.has_blur() {
        true => 1,
        false => 0,
    }
}

#[no_mangle]
pub fn ui_main_frame() {
    let cur_time = read_param_from_host::<Duration>(0);
    let window_props = read_param_from_host::<WindowProps>(1);
    let zoom_level = read_param_from_host::<f32>(2);

    ui_run_impl(
        cur_time,
        window_props,
        RawInputWrapper {
            input: Default::default(),
        },
        zoom_level,
        true,
    );
}

#[no_mangle]
pub fn ui_run() {
    let cur_time = read_param_from_host::<Duration>(0);
    let window_props = read_param_from_host::<WindowProps>(1);
    let inp = read_param_from_host::<RawInputWrapper>(2);
    let zoom_level = read_param_from_host::<f32>(3);

    let (output, zoom_level) = ui_run_impl(cur_time, window_props, inp, zoom_level, false);
    upload_return_val(RawOutputWrapper { output, zoom_level });
}
