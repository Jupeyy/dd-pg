use graphics::{
    handles::{
        backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle, texture::texture::GraphicsTextureHandle,
    },
    utils::{render_blur, render_swapped_frame, DEFAULT_BLUR_MIX_LENGTH, DEFAULT_BLUR_RADIUS},
};
use math::math::vector::vec4;
use ui_base::{types::UIPipe, ui::UI, ui_render::render_ui_2};
use ui_traits::traits::UIRenderCallbackFunc;

fn render_impl<U, C1: 'static, C2: 'static>(
    backend_handle: &GraphicsBackendHandle,
    texture_handle: &GraphicsTextureHandle,
    stream_handle: &GraphicsStreamHandle,
    canvas_handle: &GraphicsCanvasHandle,
    ui: &mut UI,
    ui_impl: &mut dyn UIRenderCallbackFunc<U>,

    custom_callback_type1: &mut C1,
    custom_callback_type2: &mut C2,

    pipe: &mut UIPipe<U>,
    inp: egui::RawInput,
    main_frame_only: bool,
) -> egui::PlatformOutput {
    let window_width = canvas_handle.window_width();
    let window_height = canvas_handle.window_height();
    let window_pixels_per_point = canvas_handle.window_pixels_per_point();

    let (screen_rect, full_output, zoom_level) = ui.render(
        window_width,
        window_height,
        window_pixels_per_point,
        |ui, inner_pipe, ui_state| {
            if main_frame_only {
                ui_impl.render_main_frame(ui, inner_pipe, ui_state)
            } else {
                ui_impl.render(ui, inner_pipe, ui_state)
            }
        },
        pipe,
        inp,
        main_frame_only,
    );
    render_ui_2(
        ui,
        custom_callback_type1,
        custom_callback_type2,
        full_output,
        &screen_rect,
        zoom_level,
        backend_handle,
        texture_handle,
        stream_handle,
        main_frame_only,
    )
}

pub fn render<U, C1: 'static, C2: 'static>(
    backend_handle: &GraphicsBackendHandle,
    texture_handle: &GraphicsTextureHandle,
    stream_handle: &GraphicsStreamHandle,
    canvas_handle: &GraphicsCanvasHandle,
    ui: &mut UI,
    ui_impl: &mut dyn UIRenderCallbackFunc<U>,

    custom_callback_type1: &mut C1,
    custom_callback_type2: &mut C2,

    pipe: &mut UIPipe<U>,

    dummy_inp: egui::RawInput,
    inp: egui::RawInput,
) -> egui::PlatformOutput {
    if ui_impl.has_blur() {
        backend_handle.next_switch_pass();
        render_impl(
            backend_handle,
            texture_handle,
            stream_handle,
            canvas_handle,
            ui,
            ui_impl,
            custom_callback_type1,
            custom_callback_type2,
            pipe,
            dummy_inp,
            true,
        );
        render_blur(
            backend_handle,
            stream_handle,
            canvas_handle,
            true,
            DEFAULT_BLUR_RADIUS,
            DEFAULT_BLUR_MIX_LENGTH,
            &vec4::new(1.0, 1.0, 1.0, 0.15),
        );
        render_swapped_frame(canvas_handle, stream_handle);
    }
    render_impl(
        backend_handle,
        texture_handle,
        stream_handle,
        canvas_handle,
        ui,
        ui_impl,
        custom_callback_type1,
        custom_callback_type2,
        pipe,
        inp,
        false,
    )
}
