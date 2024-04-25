use std::time::Duration;

use client_render_base::map::render_tools::RenderTools;
use graphics::handles::{
    canvas::canvas::GraphicsCanvasHandle,
    stream::stream::{GraphicsStreamHandle, QuadStreamHandle},
    stream_types::StreamedQuad,
};
use graphics_types::rendering::State;
use hiarc::{hi_closure, Hiarc};
use map::map::groups::layers::design::Quad;
use math::math::{
    distance,
    vector::{ffixed, fvec2, nffixed, nfvec4, ubvec4, vec2},
};

use crate::{
    map::{EditorLayer, EditorLayerUnionRef, EditorMap},
    utils::ui_pos_to_world_pos,
};

#[derive(Debug, Hiarc, Clone, Copy)]
pub enum QuadPointerDownPoint {
    Center,
    Corner(usize),
}

pub fn rotate(center: &fvec2, rotation: ffixed, points: &mut [fvec2]) {
    let c = ffixed::from_num(rotation.to_num::<f64>().cos());
    let s = ffixed::from_num(rotation.to_num::<f64>().sin());

    for i in 0..points.len() {
        let x = points[i].x - center.x;
        let y = points[i].y - center.y;
        points[i] = fvec2 {
            x: x * c - y * s + center.x,
            y: x * s + y * c + center.y,
        };
    }
}

pub fn in_box(pos: &fvec2, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
    pos.x.to_num::<f32>() >= x0
        && pos.x.to_num::<f32>() < x1
        && pos.y.to_num::<f32>() >= y0
        && pos.y.to_num::<f32>() < y1
}

pub fn in_radius(pos1: &fvec2, pos2: &vec2, radius: f32) -> bool {
    distance(&vec2::new(pos1.x.to_num(), pos1.y.to_num()), pos2) < radius
}

pub fn get_quad_points_animated(quad: &Quad, map: &EditorMap, time: Duration) -> [fvec2; 5] {
    let mut points = quad.points;
    if let Some(pos_anim) = quad.pos_anim {
        let anim = &map.animations.pos[pos_anim];
        let anim_pos = RenderTools::render_eval_anim(
            anim.def.points.as_slice(),
            time::Duration::try_from(time).unwrap(),
            3,
        );
        let rot = anim_pos.z / ffixed::from_num(360.0) * ffixed::PI * ffixed::from_num(2.0);
        let center = points[4];

        super::shared::rotate(&center, rot, &mut points);

        for point in points.iter_mut() {
            *point += fvec2::new(ffixed::from_num(anim_pos.x), ffixed::from_num(anim_pos.y));
        }
    }
    points
}

pub fn get_quad_points_color_animated(quad: &Quad, map: &EditorMap, time: Duration) -> [nfvec4; 4] {
    let mut color = quad.colors;
    if let Some(color_anim) = quad.color_anim {
        let anim = &map.animations.color[color_anim];
        let anim_color = RenderTools::render_eval_anim(
            anim.def.points.as_slice(),
            time::Duration::try_from(time).unwrap(),
            4,
        );

        for color in color.iter_mut() {
            color.x *= nffixed::from_num(anim_color.x);
            color.y *= nffixed::from_num(anim_color.y);
            color.z *= nffixed::from_num(anim_color.z);
            color.w *= nffixed::from_num(anim_color.w);
        }
    }
    color
}

pub const QUAD_POINT_RADIUS: f32 = 0.75;

pub fn render_quad_points(
    layer: Option<EditorLayerUnionRef>,

    current_pointer_pos: &egui::Pos2,
    stream_handle: &GraphicsStreamHandle,
    canvas_handle: &GraphicsCanvasHandle,
    map: &EditorMap,
) {
    // render quad corner/center points
    if let Some(EditorLayerUnionRef::Design {
        layer: EditorLayer::Quad(layer),
        group,
        ..
    }) = layer
    {
        let (offset, parallax) = (group.attr.offset, group.attr.parallax);

        let pos = current_pointer_pos;

        let pos = vec2::new(pos.x, pos.y);

        let vec2 { x, y } = ui_pos_to_world_pos(
            canvas_handle,
            map.groups.user.zoom,
            vec2::new(pos.x, pos.y),
            map.groups.user.pos.x,
            map.groups.user.pos.y,
            offset.x.to_num::<f32>(),
            offset.y.to_num::<f32>(),
            parallax.x.to_num::<f32>(),
            parallax.y.to_num::<f32>(),
        );
        for quad in &layer.layer.quads {
            let points = super::shared::get_quad_points_animated(quad, map, map.user.time);

            let mut state = State::new();
            RenderTools::map_canvas_of_group(
                canvas_handle,
                &mut state,
                map.groups.user.pos.x,
                map.groups.user.pos.y,
                Some(&group.attr),
                map.groups.user.zoom,
            );
            stream_handle.render_quads(
                    hi_closure!([points: [fvec2; 5], x: f32, y: f32], |mut stream_handle: QuadStreamHandle<'_>| -> () {
                        let point_size = QUAD_POINT_RADIUS * 0.7;
                        for point in &points[0..4] {
                            let color = if super::shared::in_radius(point, &vec2::new(x, y), QUAD_POINT_RADIUS) {
                                ubvec4::new(150, 150, 255, 255)
                            }
                            else {
                                ubvec4::new(0, 0, 255, 255)
                            };
                            stream_handle.add_vertices(
                                StreamedQuad::new().from_pos_and_size(
                                    vec2::new(point.x.to_num::<f32>() - point_size / 2.0, point.y.to_num::<f32>() - point_size / 2.0),
                                    vec2::new(point_size, point_size)
                                )
                                .color(color)
                                .into()
                            );
                        }
                        let color = if super::shared::in_radius(&points[4], &vec2::new(x, y), QUAD_POINT_RADIUS) {
                            ubvec4::new(150, 255, 150, 255)
                        }
                        else {
                            ubvec4::new(0, 255, 0, 255)
                        };
                        stream_handle.add_vertices(
                            StreamedQuad::new().from_pos_and_size(
                                vec2::new(points[4].x.to_num::<f32>() - point_size / 2.0, points[4].y.to_num::<f32>() - point_size / 2.0),
                                vec2::new(point_size, point_size)
                            )
                            .color(color)
                            .into()
                        );
                    }),
                    state,
                );
        }
    }
}
