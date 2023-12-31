use graphics::{
    graphics::Graphics,
    utils::{render_blur, render_swapped_frame, DEFAULT_BLUR_MIX_LENGTH, DEFAULT_BLUR_RADIUS},
};
use math::math::vector::vec4;
use ui_base::{
    types::{UINativePipe, UIPipe},
    ui::UI,
    ui_render::render_ui_2,
};
use ui_traits::traits::UIRenderCallbackFunc;

fn render_impl<U, T, C1: 'static, C2: 'static>(
    graphics: &mut Graphics,
    ui: &mut UI<T>,
    ui_impl: &mut dyn UIRenderCallbackFunc<U>,

    custom_callback_type1: &mut C1,
    custom_callback_type2: &mut C2,

    pipe: &mut UIPipe<U>,
    native_pipe: &mut UINativePipe<T>,
    main_frame_only: bool,
) {
    let window_width = graphics.canvas_handle.window_width();
    let window_height = graphics.canvas_handle.window_height();
    let window_pixels_per_point = graphics.canvas_handle.window_pixels_per_point();

    let (screen_rect, full_output, zoom_level) = ui.render(
        window_width,
        window_height,
        window_pixels_per_point,
        |ui, inner_pipe, ui_state| {
            if main_frame_only {
                ui_impl.render_main_frame(ui, inner_pipe, ui_state, graphics)
            } else {
                ui_impl.render(ui, inner_pipe, ui_state, graphics)
            }
        },
        pipe,
        native_pipe,
        main_frame_only,
    );
    render_ui_2(
        ui,
        native_pipe,
        custom_callback_type1,
        custom_callback_type2,
        full_output,
        &screen_rect,
        zoom_level,
        graphics,
        main_frame_only,
    );
}

pub fn render<U, T, C1: 'static, C2: 'static>(
    graphics: &mut Graphics,
    ui: &mut UI<T>,
    ui_impl: &mut dyn UIRenderCallbackFunc<U>,

    custom_callback_type1: &mut C1,
    custom_callback_type2: &mut C2,

    pipe: &mut UIPipe<U>,

    dummy_native_pipe: &mut UINativePipe<T>,
    native_pipe: &mut UINativePipe<T>,
) {
    if ui_impl.has_blur() {
        graphics.next_switch_pass();
        render_impl(
            graphics,
            ui,
            ui_impl,
            custom_callback_type1,
            custom_callback_type2,
            pipe,
            dummy_native_pipe,
            true,
        );
        render_blur(
            graphics,
            true,
            DEFAULT_BLUR_RADIUS,
            DEFAULT_BLUR_MIX_LENGTH,
            &vec4::new(1.0, 1.0, 1.0, 0.15),
        );
        render_swapped_frame(graphics);
    }
    render_impl(
        graphics,
        ui,
        ui_impl,
        custom_callback_type1,
        custom_callback_type2,
        pipe,
        native_pipe,
        false,
    );
}
