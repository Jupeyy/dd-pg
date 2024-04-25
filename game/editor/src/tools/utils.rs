use client_render_base::map::render_tools::RenderTools;
use graphics::handles::{
    canvas::canvas::GraphicsCanvasHandle,
    stream::stream::{GraphicsStreamHandle, LinesStreamHandle, QuadStreamHandle},
    stream_types::{StreamedLine, StreamedQuad},
};
use graphics_types::rendering::{ColorMaskMode, State, StencilMode};
use hiarc::hi_closure;
use math::math::vector::{ubvec4, vec2};

use crate::map::EditorMap;

pub fn render_rect_from_state(
    stream_handle: &GraphicsStreamHandle,
    state: State,
    rect: egui::Rect,
    color: ubvec4,
) {
    stream_handle.render_lines(
        hi_closure!([rect: egui::Rect, color: ubvec4], |mut stream_handle: LinesStreamHandle<'_>| -> () {
            let mut line = StreamedLine::new().with_color(color);

            line = line.from_pos(
                [vec2::new(rect.min.x, rect.min.y), vec2::new(rect.max.x, rect.min.y)]
            );
            stream_handle.add_vertices(line.into());
            line = line.from_pos(
                [vec2::new(rect.min.x, rect.min.y), vec2::new(rect.min.x, rect.max.y)]
            );
            stream_handle.add_vertices(line.into());
            line = line.from_pos(
                [vec2::new(rect.max.x, rect.min.y), vec2::new(rect.max.x, rect.max.y)]
            );
            stream_handle.add_vertices(line.into());
            line = line.from_pos(
                [vec2::new(rect.min.x, rect.max.y), vec2::new(rect.max.x, rect.max.y)]
            );
            stream_handle.add_vertices(line.into());
        }),
        state,
    );
}

pub fn render_rect(
    canvas_handle: &GraphicsCanvasHandle,
    stream_handle: &GraphicsStreamHandle,
    map: &EditorMap,
    rect: egui::Rect,
    color: ubvec4,
    parallax: &vec2,
    offset: &vec2,
) {
    let mut state = State::new();
    let points: [f32; 4] = RenderTools::map_canvas_to_world(
        map.groups.user.pos.x,
        map.groups.user.pos.y,
        parallax.x,
        parallax.y,
        100.0,
        offset.x,
        offset.y,
        canvas_handle.canvas_aspect(),
        map.groups.user.zoom,
    );
    state.map_canvas(points[0], points[1], points[2], points[3]);

    render_rect_from_state(stream_handle, state, rect, color)
}

pub fn render_filled_rect(
    canvas_handle: &GraphicsCanvasHandle,
    stream_handle: &GraphicsStreamHandle,
    map: &EditorMap,
    rect: egui::Rect,
    color: ubvec4,
    parallax: &vec2,
    offset: &vec2,
    as_stencil: bool,
) {
    let mut state = State::new();
    let points: [f32; 4] = RenderTools::map_canvas_to_world(
        map.groups.user.pos.x,
        map.groups.user.pos.y,
        parallax.x,
        parallax.y,
        100.0,
        offset.x,
        offset.y,
        canvas_handle.canvas_aspect(),
        map.groups.user.zoom,
    );
    state.map_canvas(points[0], points[1], points[2], points[3]);

    state.set_stencil_mode(if as_stencil {
        StencilMode::FillStencil
    } else {
        StencilMode::None
    });
    state.set_color_mask(if as_stencil {
        ColorMaskMode::WriteAlphaOnly
    } else {
        ColorMaskMode::WriteAll
    });

    stream_handle.render_quads(
        hi_closure!([rect: egui::Rect, color: ubvec4], |mut stream_quads: QuadStreamHandle<'_>| -> () {
            let pos = rect.min;
            let size = rect.size();
            stream_quads.add_vertices(
                StreamedQuad::new()
                    .from_pos_and_size(vec2::new(pos.x, pos.y), vec2::new(size.x, size.y))
                    .tex_free_form(
                        vec2::new(0.0, 0.0),
                        vec2::new(1.0, 0.0),
                        vec2::new(1.0, 1.0),
                        vec2::new(0.0, 1.0),
                    )
                    .color(color)
                    .into()
            );
        }),
        state,
    );
}
