use std::{cell::RefCell, time::Duration};

use api::{read_param_from_host, upload_return_val, GRAPHICS, GRAPHICS_BACKEND};

use graphics_types::types::WindowProps;
use ui_base::{
    types::{RawInputWrapper, RawOutputWrapper, UiFonts, UiRenderPipe},
    ui::UiContainer,
    ui_render::render_ui,
};
use ui_traits::traits::UiPageInterface;

extern "Rust" {
    /// returns an instance of the game state and the game tick speed
    fn mod_ui_new() -> Box<dyn UiPageInterface<()>>;
}

type U = ();

thread_local! {
static API_UI: once_cell::unsync::Lazy<RefCell<UiContainer>> =
    once_cell::unsync::Lazy::new(|| RefCell::new(UiContainer::new(&Default::default())));

static API_UI_USER: once_cell::unsync::Lazy<RefCell<Box<dyn UiPageInterface<U>>>> =
    once_cell::unsync::Lazy::new(|| RefCell::new(unsafe { mod_ui_new() }));
}

#[no_mangle]
pub fn ui_new() {
    let fonts = read_param_from_host::<UiFonts>(0);
    API_UI.with(|g| {
        let mut ui = g.borrow_mut();
        if let Some(font_definitions) = fonts.fonts.as_ref() {
            ui.context.egui_ctx.set_fonts(font_definitions.clone());
        }
        ui.font_definitions = fonts.fonts;
    });
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
    if !main_frame_only || API_UI_USER.with(|g| g.borrow().has_blur()) {
        API_UI.with(|g| g.borrow_mut().zoom_level.set(zoom_level));
        GRAPHICS.with(|g| g.resized(window_props));

        let (screen_rect, full_output, zoom_level) = API_UI.with(|g| {
            g.borrow_mut().render(
                GRAPHICS.with(|g| g.canvas_handle.window_width()),
                GRAPHICS.with(|g| g.canvas_handle.window_height()),
                GRAPHICS.with(|g| g.canvas_handle.window_pixels_per_point()),
                |ui, pipe, ui_state| {
                    if main_frame_only {
                        API_UI_USER.with(|g| g.borrow_mut().render_main_frame(ui, pipe, ui_state));
                    } else {
                        API_UI_USER.with(|g| g.borrow_mut().render(ui, pipe, ui_state));
                    }
                },
                &mut UiRenderPipe::new(cur_time, &mut user_data),
                inp.input,
                main_frame_only,
            )
        });

        let platform_output = {
            let graphics = GRAPHICS.with(|g| (*g).clone());
            API_UI.with(|g| {
                render_ui(
                    &mut g.borrow_mut(),
                    full_output,
                    &screen_rect,
                    zoom_level,
                    &graphics.backend_handle,
                    &graphics.texture_handle,
                    &graphics.stream_handle,
                    main_frame_only,
                )
            })
        };

        GRAPHICS_BACKEND.with(|g| g.actual_run_cmds.set(false));
        GRAPHICS.with(|g| {
            g.backend_handle
                .run_backend_buffer(g.stream_handle.stream_data())
        });
        GRAPHICS_BACKEND.with(|g| g.actual_run_cmds.set(true));

        platform_output
    } else {
        Default::default()
    }
}

#[no_mangle]
pub fn ui_has_blur() -> u8 {
    match API_UI_USER.with(|g| g.borrow().has_blur()) {
        true => 1,
        false => 0,
    }
}

#[no_mangle]
pub fn ui_mount() {
    API_UI_USER.with(|g| g.borrow_mut().mount());
}

#[no_mangle]
pub fn ui_unmount() {
    API_UI_USER.with(|g| g.borrow_mut().unmount());
}

#[no_mangle]
pub fn ui_main_frame() {
    let cur_time = read_param_from_host::<Duration>(0);
    let window_props = read_param_from_host::<WindowProps>(1);
    let zoom_level = read_param_from_host::<Option<f32>>(2);

    ui_run_impl(
        cur_time,
        window_props,
        RawInputWrapper {
            input: Default::default(),
        },
        zoom_level,
        true,
        (),
    );
}

#[no_mangle]
pub fn ui_run() {
    let cur_time = read_param_from_host::<Duration>(0);
    let window_props = read_param_from_host::<WindowProps>(1);
    let inp = read_param_from_host::<RawInputWrapper>(2);
    let zoom_level = read_param_from_host::<Option<f32>>(3);

    let output = ui_run_impl(cur_time, window_props, inp, zoom_level, false, ());
    upload_return_val(RawOutputWrapper {
        output,
        zoom_level: API_UI.with(|g| g.borrow().zoom_level.get()),
    });
}
