use std::{cell::RefCell, time::Duration};

use api::{read_param_from_host, upload_return_val, GRAPHICS, GRAPHICS_BACKEND};

use egui::FullOutput;
use graphics::graphics::graphics::Graphics;
use graphics_types::types::WindowProps;
use ui_base::{
    style::default_style,
    types::{RawInputWrapper, RawOutputWrapper, UIFonts, UIPipe},
    ui::UI,
};
use ui_traits::traits::UIRenderCallbackFunc;

extern "Rust" {
    /// returns an instance of the game state and the game tick speed
    fn mod_ui_new() -> Box<dyn UIRenderCallbackFunc<()>>;

    /// #Example
    /// ```rust
    /// #[no_mangle]
    /// pub fn mod_render_ui(
    ///     ui: &mut UI,
    ///     full_output: FullOutput,
    ///     screen_rect: &egui::Rect,
    ///     zoom_level: f32,
    ///     graphics: &mut Graphics,
    ///     as_stencil: bool,
    /// ) -> egui::PlatformOutput {
    ///     render_ui(
    ///         ui,
    ///         full_output,
    ///         screen_rect,
    ///         zoom_level,
    ///         graphics,
    ///         as_stencil,
    ///     )
    /// }
    ///
    fn mod_render_ui(
        ui: &mut UI,
        full_output: FullOutput,
        screen_rect: &egui::Rect,
        zoom_level: f32,
        graphics: &mut Graphics,
        as_stencil: bool,
    ) -> egui::PlatformOutput;
}

type U = ();

static mut API_UI: once_cell::unsync::Lazy<RefCell<UI>> =
    once_cell::unsync::Lazy::new(|| RefCell::new(UI::new(None)));

static mut API_UI_USER: once_cell::unsync::Lazy<RefCell<Box<dyn UIRenderCallbackFunc<U>>>> =
    once_cell::unsync::Lazy::new(|| RefCell::new(unsafe { mod_ui_new() }));

#[no_mangle]
pub fn ui_new() {
    let fonts = read_param_from_host::<UIFonts>(0);
    unsafe {
        API_UI
            .borrow()
            .context
            .egui_ctx
            .set_fonts(fonts.fonts.clone())
    };
    unsafe {
        API_UI
            .borrow()
            .stencil_context
            .egui_ctx
            .set_fonts(fonts.fonts)
    };
}

/// returns platform output and zoom level
fn ui_run_impl(
    cur_time: Duration,
    window_props: WindowProps,
    inp: RawInputWrapper,
    zoom_level: Option<f32>,
    main_frame_only: bool,
    mut user_data: U,
) -> egui::PlatformOutput {
    if !main_frame_only || unsafe { API_UI_USER.borrow().has_blur() } {
        unsafe { API_UI.borrow_mut().ui_state.zoom_level = zoom_level };
        unsafe { GRAPHICS.borrow_mut().resized(window_props) };

        unsafe {
            API_UI
                .borrow_mut()
                .context
                .egui_ctx
                .set_style(default_style());
        };
        unsafe {
            API_UI
                .borrow_mut()
                .stencil_context
                .egui_ctx
                .set_style(default_style())
        };

        let (screen_rect, full_output, zoom_level) = unsafe {
            API_UI.borrow_mut().render(
                GRAPHICS.borrow().canvas_handle.window_width(),
                GRAPHICS.borrow().canvas_handle.window_height(),
                GRAPHICS.borrow().canvas_handle.window_pixels_per_point(),
                |ui, pipe, ui_state| {
                    if main_frame_only {
                        API_UI_USER
                            .borrow_mut()
                            .render_main_frame(ui, pipe, ui_state);
                    } else {
                        API_UI_USER.borrow_mut().render(ui, pipe, ui_state);
                    }
                },
                &mut UIPipe::new(cur_time, &mut user_data),
                inp.input,
                main_frame_only,
            )
        };

        let platform_output = unsafe {
            mod_render_ui(
                &mut *API_UI.borrow_mut(),
                full_output,
                &screen_rect,
                zoom_level,
                &mut *GRAPHICS.borrow_mut(),
                main_frame_only,
            )
        };

        unsafe { &mut *GRAPHICS_BACKEND }.actual_run_cmds.set(false);
        let graphics = unsafe { &mut *GRAPHICS };
        graphics
            .borrow()
            .backend_handle
            .run_backend_buffer(graphics.borrow().stream_handle.stream_data());
        unsafe { &mut *GRAPHICS_BACKEND }.actual_run_cmds.set(true);

        platform_output
    } else {
        Default::default()
    }
}

#[no_mangle]
pub fn ui_has_blur() -> u8 {
    match unsafe { API_UI_USER.borrow().has_blur() } {
        true => 1,
        false => 0,
    }
}

#[no_mangle]
pub fn ui_main_frame() {
    let cur_time = read_param_from_host::<Duration>(0);
    let window_props = read_param_from_host::<WindowProps>(1);
    let zoom_level = read_param_from_host::<Option<f32>>(2);
    let user_data = read_param_from_host::<U>(3);

    ui_run_impl(
        cur_time,
        window_props,
        RawInputWrapper {
            input: Default::default(),
        },
        zoom_level,
        true,
        user_data,
    );
}

#[no_mangle]
pub fn ui_run() {
    let cur_time = read_param_from_host::<Duration>(0);
    let window_props = read_param_from_host::<WindowProps>(1);
    let inp = read_param_from_host::<RawInputWrapper>(2);
    let zoom_level = read_param_from_host::<Option<f32>>(3);
    let user_data = read_param_from_host::<U>(4);

    let output = ui_run_impl(cur_time, window_props, inp, zoom_level, false, user_data);
    upload_return_val(RawOutputWrapper {
        output,
        zoom_level: unsafe { API_UI.borrow_mut().ui_state.zoom_level },
    });
}
