use client_render_base::map::render_tools::RenderTools;
use egui::{vec2, Rect};
use graphics::handles::canvas::canvas::GraphicsCanvasHandle;
use math::math::vector::vec2;

pub type UiCanvasSize = Rect;

pub fn ui_pos_to_world_pos(
    canvas_handle: &GraphicsCanvasHandle,
    ui_canvas: &UiCanvasSize,
    zoom: f32,
    inp: vec2,
    center_x: f32,
    center_y: f32,
    offset_x: f32,
    offset_y: f32,
    parallax_x: f32,
    parallax_y: f32,
) -> vec2 {
    let points = RenderTools::canvas_points_of_group_attr(
        canvas_handle,
        center_x,
        center_y,
        parallax_x,
        parallax_y,
        offset_x,
        offset_y,
        zoom,
    );

    let x = inp.x;
    let y = inp.y;

    let size = ui_canvas
        .size()
        .clamp(vec2(0.01, 0.01), vec2(f32::MAX, f32::MAX));
    let x_ratio = x / size.x;
    let y_ratio = y / size.y;

    let x = points[0] + x_ratio * (points[2] - points[0]);
    let y = points[1] + y_ratio * (points[3] - points[1]);

    vec2::new(x, y)
}
