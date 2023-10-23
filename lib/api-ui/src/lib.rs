#![allow(clippy::all)]

use std::{cell::RefCell, time::Duration};

use api::{
    graphics::graphics::GraphicsBackend, read_param_from_host, upload_return_val, GRAPHICS,
    GRAPHICS_BACKEND, IO, RUNTIME_THREAD_POOL,
};
use base_log::log::SystemLog;
use client_containers::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use config::config::Config;
use graphics_base_traits::traits::{GraphicsBackendHandleInterface, GraphicsSizeQuery};
use graphics_types::types::WindowProps;
use ui_base::{
    style::default_style,
    types::{
        RawInputWrapper, RawOutputWrapper, UIFeedbackInterface, UIFonts, UINativePipe,
        UINativeState, UIPipe, UIRawInputGenerator,
    },
    ui::{UIInterface, UI as RealUI},
    ui_render::render_ui_2,
};
use ui_traits::traits::UIRenderCallbackFunc;

pub struct UIWinitWrapper {}

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

pub struct UI {
    pub ui: RealUI<UIWinitWrapper>,
}

impl UI {
    pub fn new(zoom_level: Option<f32>) -> Self {
        Self {
            ui: RealUI::new(UIWinitWrapper {}, zoom_level),
        }
    }
}

pub struct ClientStatsUIFeedbackDummy {}

impl UIFeedbackInterface for ClientStatsUIFeedbackDummy {}

extern "Rust" {
    /// returns an instance of the game state and the game tick speed
    fn mod_ui_new() -> Box<dyn UIRenderCallbackFunc<(), GraphicsBackend>>;
}

static mut SYS_LOG: once_cell::unsync::Lazy<SystemLog> =
    once_cell::unsync::Lazy::new(|| SystemLog::new());

static mut API_UI: once_cell::unsync::Lazy<UI> = once_cell::unsync::Lazy::new(|| UI::new(None));

static mut API_UI_USER: once_cell::unsync::Lazy<
    Box<dyn UIRenderCallbackFunc<(), GraphicsBackend>>,
> = once_cell::unsync::Lazy::new(|| unsafe { mod_ui_new() });

static mut API_CONFIG: once_cell::unsync::Lazy<Config> =
    once_cell::unsync::Lazy::new(|| Config::default());

static mut SKIN_CONTAINER: once_cell::unsync::Lazy<SkinContainer> =
    once_cell::unsync::Lazy::new(|| {
        let default_skin = SkinContainer::load(
            "default",
            unsafe { &IO },
            unsafe { &RUNTIME_THREAD_POOL },
            unsafe { &mut *GRAPHICS },
        );
        SkinContainer::new(
            unsafe { IO.clone() },
            unsafe { RUNTIME_THREAD_POOL.clone() },
            default_skin,
            unsafe { &SYS_LOG },
            "skin-container",
        )
    });

static mut TEE_RENDER: once_cell::unsync::Lazy<RenderTee> =
    once_cell::unsync::Lazy::new(|| RenderTee::new(unsafe { &mut *GRAPHICS }));

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
            unsafe { &mut GRAPHICS }.window_width(),
            unsafe { &mut GRAPHICS }.window_height(),
            unsafe { &mut GRAPHICS }.window_pixels_per_point(),
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

        render_ui_2(
            unsafe { &mut API_UI.ui },
            &mut native_pipe,
            unsafe { &mut *SKIN_CONTAINER },
            unsafe { &mut *TEE_RENDER },
            full_output,
            &screen_rect,
            zoom_level,
            unsafe { &mut *GRAPHICS },
            main_frame_only,
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
