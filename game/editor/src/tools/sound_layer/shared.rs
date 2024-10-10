use client_render_base::map::render_tools::RenderTools;
use graphics::handles::{
    canvas::canvas::GraphicsCanvasHandle,
    stream::stream::{GraphicsStreamHandle, QuadStreamHandle},
    stream_types::StreamedQuad,
};
use graphics_types::rendering::State;
use hiarc::{hi_closure, Hiarc};
use map::map::groups::layers::design::Sound;
use math::math::vector::{ffixed, fvec2, ubvec4, vec2};
use std::time::Duration;

use crate::{
    map::{EditorLayer, EditorLayerUnionRef, EditorMap},
    tools::shared::in_radius,
    utils::{ui_pos_to_world_pos, UiCanvasSize},
};

#[derive(Debug, Hiarc, Clone, Copy)]
pub enum SoundPointerDownPoint {
    Center,
}

pub fn get_sound_point_animated(snd: &Sound, map: &EditorMap, time: Duration) -> fvec2 {
    let mut point = snd.pos;
    if let Some(pos_anim) = snd.pos_anim {
        let anim = &map.animations.pos[pos_anim];
        let anim_pos = RenderTools::render_eval_anim(
            anim.def.points.as_slice(),
            time::Duration::try_from(time).unwrap(),
            3,
        );

        point += fvec2::new(ffixed::from_num(anim_pos.x), ffixed::from_num(anim_pos.y));
    }
    point
}

pub const SOUND_POINT_RADIUS: f32 = 0.75;

pub fn render_sound_points(
    ui_canvas: &UiCanvasSize,
    layer: Option<EditorLayerUnionRef>,

    current_pointer_pos: &egui::Pos2,
    stream_handle: &GraphicsStreamHandle,
    canvas_handle: &GraphicsCanvasHandle,
    map: &EditorMap,
) {
    // render sound corner point
    if let Some(EditorLayerUnionRef::Design {
        layer: EditorLayer::Sound(layer),
        group,
        ..
    }) = layer
    {
        let (offset, parallax) = (group.attr.offset, group.attr.parallax);

        let pos = current_pointer_pos;

        let pos = vec2::new(pos.x, pos.y);

        let vec2 { x, y } = ui_pos_to_world_pos(
            canvas_handle,
            ui_canvas,
            map.groups.user.zoom,
            vec2::new(pos.x, pos.y),
            map.groups.user.pos.x,
            map.groups.user.pos.y,
            offset.x.to_num::<f32>(),
            offset.y.to_num::<f32>(),
            parallax.x.to_num::<f32>(),
            parallax.y.to_num::<f32>(),
        );
        for sound in &layer.layer.sounds {
            let point = get_sound_point_animated(sound, map, map.user.time);

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
                    hi_closure!([point: fvec2, x: f32, y: f32], |mut stream_handle: QuadStreamHandle<'_>| -> () {
                        let point_size = SOUND_POINT_RADIUS * 0.7;
                        let color = if in_radius(&point, &vec2::new(x, y), SOUND_POINT_RADIUS) {
                            ubvec4::new(150, 255, 150, 255)
                        }
                        else {
                            ubvec4::new(0, 255, 0, 255)
                        };
                        stream_handle.add_vertices(
                            StreamedQuad::default().from_pos_and_size(
                                vec2::new(point.x.to_num::<f32>() - point_size / 2.0, point.y.to_num::<f32>() - point_size / 2.0),
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
