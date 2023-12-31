use std::time::Duration;

use graphics::handles::{canvas::GraphicsCanvasHandle, stream::GraphicsStreamHandle};
use shared_base::{
    datafile::CDatafileWrapper,
    mapdef::{CEnvPoint, CMapItemGroup, CMapItemGroupEx, CQuad, CTile, CurveType, TileFlag},
};

use graphics::streaming::{rotate, DrawScopeImpl};

use math::math::{
    fx2f,
    vector::{vec2, vec4},
    PI,
};

use base::system::SystemInterface;

use graphics_types::{
    rendering::{ColorRGBA, GlVertex, State},
    types::{StreamedQuad, Triangle},
};

use super::render_pipe::{GameStateRenderInfo, RenderPipelineBase};

/*/ enum
{
    SPRITE_FLAG_FLIP_Y = 1,
    SPRITE_FLAG_FLIP_X = 2,
};*/

pub enum LayerRenderFlag {
    Opaque = 1,
    Transparent = 2,
}

pub enum TileRenderFlag {
    Extend = 4,
}

pub struct RenderTools {}

impl RenderTools {
    pub fn _render_tile_map<F>(
        pipe: &mut RenderPipelineBase,
        stream_handle: &mut GraphicsStreamHandle,
        state: &State,
        tiles: &[CTile],
        w: i32,
        h: i32,
        scale: f32,
        color: &ColorRGBA,
        render_flags: i32,
        envelop_evaluation_func: F,
        color_env: i32,
        color_env_offset: i32,
    ) where
        F: Fn(&mut RenderPipelineBase, i32, i32, &mut ColorRGBA),
    {
        let mut channels = ColorRGBA {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        if color_env >= 0 {
            envelop_evaluation_func(pipe, color_env_offset, color_env, &mut channels);
        }

        let mut draw_quads = stream_handle.quads_tex_3d_begin();
        draw_quads.get_draw_scope().set_state(state);

        let (canvas_x0, canvas_y0, canvas_x1, canvas_y1) = state.get_canvas_mapping();

        draw_quads.set_colors_from_single(
            color.r * channels.r,
            color.g * channels.g,
            color.b * channels.b,
            color.a * channels.a,
        );

        let start_y = (canvas_y0 / scale) as i32 - 1;
        let start_x = (canvas_x0 / scale) as i32 - 1;
        let end_y = (canvas_y1 / scale) as i32 + 1;
        let end_x = (canvas_x1 / scale) as i32 + 1;

        for y in start_y..end_y {
            let mut x = start_x;
            while x < end_x {
                let mut mx = x;
                let mut my = y;

                if (render_flags & TileRenderFlag::Extend as i32) != 0 {
                    if mx < 0 {
                        mx = 0;
                    }
                    if mx >= w {
                        mx = w - 1;
                    }
                    if my < 0 {
                        my = 0;
                    }
                    if my >= h {
                        my = h - 1;
                    }
                } else {
                    if mx < 0 {
                        continue; // mx = 0;
                    }
                    if mx >= w {
                        continue; // mx = w-1;
                    }
                    if my < 0 {
                        continue; // my = 0;
                    }
                    if my >= h {
                        continue; // my = h-1;
                    }
                }

                let c = (mx + my * w) as usize;

                let index = tiles[c].index;
                if index > 0 {
                    let flags = tiles[c].flags as i32;

                    let mut render = false;
                    if (flags & TileFlag::OPAQUE.bits()) != 0
                        && color.a * channels.a > 254.0 / 255.0
                    {
                        if (render_flags & LayerRenderFlag::Opaque as i32) != 0 {
                            render = true;
                        }
                    } else {
                        if (render_flags & LayerRenderFlag::Transparent as i32) != 0 {
                            render = true;
                        }
                    }

                    if render {
                        let mut x0 = 0.0;
                        let mut y0 = 0.0;
                        let mut x1 = x0 + 1.0;
                        let mut y1 = y0;
                        let mut x2 = x0 + 1.0;
                        let mut y2 = y0 + 1.0;
                        let mut x3 = x0;
                        let mut y3 = y0 + 1.0;

                        if (flags & TileFlag::XFLIP.bits()) != 0 {
                            x0 = x2;
                            x1 = x3;
                            x2 = x3;
                            x3 = x0;
                        }

                        if (flags & TileFlag::YFLIP.bits()) != 0 {
                            y0 = y3;
                            y2 = y1;
                            y3 = y1;
                            y1 = y0;
                        }

                        if (flags & TileFlag::ROTATE.bits()) != 0 {
                            let mut tmp = x0;
                            x0 = x3;
                            x3 = x2;
                            x2 = x1;
                            x1 = tmp;
                            tmp = y0;
                            y0 = y3;
                            y3 = y2;
                            y2 = y1;
                            y1 = tmp;
                        }

                        draw_quads.quads_set_subset_free(x0, y0, x1, y1, x2, y2, x3, y3, index);
                        let _quad_item = StreamedQuad::from_pos_and_size(
                            x as f32 * scale,
                            y as f32 * scale,
                            scale,
                            scale,
                        );
                        //TODO pipe.graphics.QuadsTex3DDrawTL(&QuadItem, 1);
                    }
                }
                x += tiles[c].skip as i32;
                x += 1;
            }
        }

        drop(draw_quads);
        /* TODO: if graphics.is_tile_buffering_enabled() {
            pipe.graphics.QuadsTex3DEnd();
        }
        else {
            pipe.graphics.QuadsEnd();
        }*/
        //pipe.graphics.MapCanvas(CanvasX0, CanvasY0, CanvasX1, CanvasY1);
    }

    pub fn render_quads<F>(
        pipe: &mut RenderPipelineBase,
        stream_handle: &mut GraphicsStreamHandle,
        state: &State,
        quads: &Vec<CQuad>,
        num_quads: usize,
        render_flags: i32,
        envelop_evaluation_func: F,
        alpha: f32,
    ) where
        F: Fn(
            &CDatafileWrapper,
            &GameStateRenderInfo,
            &dyn SystemInterface,
            i32,
            i32,
            &mut ColorRGBA,
        ),
    {
        /* TODO: if(!g_Config.m_ClShowQuads || g_Config.m_ClOverlayEntities == 100)
        return;
        let alpha = (100 - g_Config.m_ClOverlayEntities) / 100.0f;
        */

        Self::force_render_quads(
            pipe,
            stream_handle,
            state,
            quads,
            num_quads,
            render_flags,
            envelop_evaluation_func,
            alpha,
        );
    }

    pub fn force_render_quads<F>(
        pipe: &mut RenderPipelineBase,
        stream_handle: &mut GraphicsStreamHandle,
        _state: &State,
        quads: &Vec<CQuad>,
        num_quads: usize,
        render_flags: i32,
        envelop_evaluation_func: F,
        alpha: f32,
    ) where
        F: Fn(
            &CDatafileWrapper,
            &GameStateRenderInfo,
            &dyn SystemInterface,
            i32,
            i32,
            &mut ColorRGBA,
        ),
    {
        let mut draw_triangles = stream_handle.triangles_begin();
        let conv: f32 = 1.0 / 255.0;
        for i in 0..num_quads {
            let quad = &quads[i];

            let mut color = ColorRGBA {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            };
            if quad.color_env >= 0 {
                envelop_evaluation_func(
                    pipe.map,
                    pipe.game,
                    pipe.sys,
                    quad.color_env_offset,
                    quad.color_env,
                    &mut color,
                );
            }

            if color.a <= 0.0 {
                continue;
            }

            let opaque = false;
            /* TODO: Analyze quadtexture
            if(a < 0.01f || (q->m_aColors[0].a < 0.01f && q->m_aColors[1].a < 0.01f && q->m_aColors[2].a < 0.01f && q->m_aColors[3].a < 0.01f))
                Opaque = true;
            */
            if opaque && (render_flags & LayerRenderFlag::Opaque as i32) == 0 {
                continue;
            }
            if !opaque && (render_flags & LayerRenderFlag::Transparent as i32) == 0 {
                continue;
            }

            let mut offset_x = 0.0;
            let mut offset_y = 0.0;
            let mut rot = 0.0;

            // TODO: fix this
            if quad.pos_env >= 0 {
                let mut color_channels = ColorRGBA::default();
                envelop_evaluation_func(
                    pipe.map,
                    pipe.game,
                    pipe.sys,
                    quad.pos_env_offset,
                    quad.pos_env,
                    &mut color_channels,
                );
                offset_x = color_channels.r;
                offset_y = color_channels.g;
                rot = color_channels.b / 360.0 * PI * 2.0;
            }

            let array_colors: [vec4; 4] = [
                vec4::new(
                    quad.colors[0].r() as f32 * conv * color.r,
                    quad.colors[0].g() as f32 * conv * color.g,
                    quad.colors[0].b() as f32 * conv * color.b,
                    quad.colors[0].a() as f32 * conv * color.a * alpha,
                ),
                vec4::new(
                    quad.colors[1].r() as f32 * conv * color.r,
                    quad.colors[1].g() as f32 * conv * color.g,
                    quad.colors[1].b() as f32 * conv * color.b,
                    quad.colors[1].a() as f32 * conv * color.a * alpha,
                ),
                vec4::new(
                    quad.colors[2].r() as f32 * conv * color.r,
                    quad.colors[2].g() as f32 * conv * color.g,
                    quad.colors[2].b() as f32 * conv * color.b,
                    quad.colors[2].a() as f32 * conv * color.a * alpha,
                ),
                vec4::new(
                    quad.colors[3].r() as f32 * conv * color.r,
                    quad.colors[3].g() as f32 * conv * color.g,
                    quad.colors[3].b() as f32 * conv * color.b,
                    quad.colors[3].a() as f32 * conv * color.a * alpha,
                ),
            ];
            let mut points: [GlVertex; 4] = Default::default();
            points.iter_mut().enumerate().for_each(|(index, p)| {
                p.pos = vec2::new(fx2f(quad.points[index].x), fx2f(quad.points[index].y));
            });

            if rot != 0.0 {
                let center = vec2::new(fx2f(quad.points[4].x), fx2f(quad.points[4].y));

                rotate(&center, rot, &mut points);
            }

            draw_triangles.triangles_set_subset_free(
                fx2f(quad.tex_coords[0].x),
                fx2f(quad.tex_coords[0].y),
                fx2f(quad.tex_coords[1].x),
                fx2f(quad.tex_coords[1].y),
                fx2f(quad.tex_coords[3].x),
                fx2f(quad.tex_coords[3].y),
            );

            draw_triangles.set_colors(&[array_colors[0], array_colors[1], array_colors[3]]);

            let tri = Triangle::new(&[
                vec2::new(points[0].pos.x + offset_x, points[0].pos.y + offset_y),
                vec2::new(points[1].pos.x + offset_x, points[1].pos.y + offset_y),
                vec2::new(points[3].pos.x + offset_x, points[3].pos.y + offset_y),
            ]);

            draw_triangles.triangles_draw_tl(&[tri]);

            draw_triangles.triangles_set_subset_free(
                fx2f(quad.tex_coords[0].x),
                fx2f(quad.tex_coords[0].y),
                fx2f(quad.tex_coords[3].x),
                fx2f(quad.tex_coords[3].y),
                fx2f(quad.tex_coords[2].x),
                fx2f(quad.tex_coords[2].y),
            );

            draw_triangles.set_colors(&[array_colors[0], array_colors[3], array_colors[2]]);

            let tri = Triangle::new(&[
                vec2::new(points[0].pos.x + offset_x, points[0].pos.y + offset_y),
                vec2::new(points[3].pos.x + offset_x, points[3].pos.y + offset_y),
                vec2::new(points[2].pos.x + offset_x, points[2].pos.y + offset_y),
            ]);

            draw_triangles.triangles_draw_tl(&[tri]);
        }
    }

    pub fn calc_canvas_params(aspect: f32, zoom: f32, width: &mut f32, height: &mut f32) {
        const AMOUNT: f32 = 1150.0 * 1000.0;
        const WIDTH_MAX: f32 = 1500.0;
        const HEIGHT_MAX: f32 = 1050.0;

        let f = AMOUNT.sqrt() / aspect.sqrt();
        *width = f * aspect;
        *height = f;

        // limit the view
        if *width > WIDTH_MAX {
            *width = WIDTH_MAX;
            *height = *width / aspect;
        }

        if *height > HEIGHT_MAX {
            *height = HEIGHT_MAX;
            *width = *height * aspect;
        }

        *width *= zoom;
        *height *= zoom;
    }

    pub fn map_canvas_to_world(
        center_x: f32,
        center_y: f32,
        parallax_x: f32,
        parallax_y: f32,
        parallax_zoom: f32,
        offset_x: f32,
        offset_y: f32,
        aspect: f32,
        zoom: f32,
    ) -> [f32; 4] {
        let mut width = 0.0;
        let mut height = 0.0;
        Self::calc_canvas_params(aspect, zoom, &mut width, &mut height);

        let scale = (parallax_zoom * (zoom - 1.0) + 100.0) / 100.0 / zoom;
        width *= scale;
        height *= scale;

        let center_x = center_x * parallax_x / 100.0;
        let center_y = center_y * parallax_y / 100.0;
        let mut points: [f32; 4] = [0.0; 4];
        points[0] = offset_x + center_x - width / 2.0;
        points[1] = offset_y + center_y - height / 2.0;
        points[2] = points[0] + width;
        points[3] = points[1] + height;
        points
    }

    pub fn map_canvas_to_group(
        canvas_handle: &GraphicsCanvasHandle,
        state: &mut State,
        center_x: f32,
        center_y: f32,
        group: &CMapItemGroup,
        _group_ex: Option<&mut CMapItemGroupEx>,
        zoom: f32,
    ) {
        // TODO let ParallaxZoom = GetParallaxZoom(pGroup, pGroupEx);
        let points: [f32; 4] = Self::map_canvas_to_world(
            center_x,
            center_y,
            group.parallax_x as f32,
            group.parallax_y as f32,
            100.0,
            group.offset_x as f32,
            group.offset_y as f32,
            canvas_handle.canvas_aspect(),
            zoom,
        );
        state.map_canvas(points[0], points[1], points[2], points[3]);
    }

    pub fn render_eval_envelope(
        points: &[CEnvPoint],
        num_points: i32,
        channels: i32,
        time_nanos_param: std::time::Duration,
        resulting_color: &mut ColorRGBA,
    ) {
        let mut time_nanos = time_nanos_param;
        if num_points == 0 {
            *resulting_color = ColorRGBA::default();
            return;
        }

        if num_points == 1 {
            resulting_color.r = fx2f(points[0].values[0]);
            resulting_color.g = fx2f(points[0].values[1]);
            resulting_color.b = fx2f(points[0].values[2]);
            resulting_color.a = fx2f(points[0].values[3]);
            return;
        }

        let max_point_time = points[num_points as usize - 1].time as u64
            * std::time::Duration::from_millis(1).as_nanos() as u64;
        if max_point_time > 0
        // TODO: remove this check when implementing a IO check for maps(in this case broken envelopes)
        {
            time_nanos =
                std::time::Duration::from_nanos(time_nanos.as_nanos() as u64 % max_point_time);
        } else {
            time_nanos = std::time::Duration::from_nanos(0);
        }

        for i in 0..num_points as usize - 1 {
            if time_nanos >= Duration::from_millis(points[i].time as u64)
                && time_nanos <= Duration::from_millis(points[i + 1].time as u64)
            {
                let delta = points[i + 1].time - points[i].time;
                let mut a = ((time_nanos.as_nanos() as f64
                    / std::time::Duration::from_millis(1).as_nanos() as f64)
                    - points[i].time as f64)
                    / delta as f64;

                if points[i].curve_type == CurveType::Smooth as i32 {
                    a = -2.0 * a * a * a + 3.0 * a * a; // second hermite basis
                } else if points[i].curve_type == CurveType::Slow as i32 {
                    a = a * a * a;
                } else if points[i].curve_type == CurveType::Fast as i32 {
                    a = 1.0 - a;
                    a = 1.0 - a * a * a;
                } else if points[i].curve_type == CurveType::Step as i32 {
                    a = 0.0;
                } else {
                    // linear
                }

                for c in 0..channels as usize {
                    let v0 = fx2f(points[i].values[c]);
                    let v1 = fx2f(points[i + 1].values[c]);
                    match c {
                        0 => resulting_color.r = (v0 as f64 + (v1 - v0) as f64 * a) as f32,
                        1 => resulting_color.g = (v0 as f64 + (v1 - v0) as f64 * a) as f32,
                        2 => resulting_color.b = (v0 as f64 + (v1 - v0) as f64 * a) as f32,
                        3 => resulting_color.a = (v0 as f64 + (v1 - v0) as f64 * a) as f32,
                        _ => (),
                    }
                }

                return;
            }
        }

        resulting_color.r = fx2f(points[num_points as usize - 1].values[0]);
        resulting_color.g = fx2f(points[num_points as usize - 1].values[1]);
        resulting_color.b = fx2f(points[num_points as usize - 1].values[2]);
        resulting_color.a = fx2f(points[num_points as usize - 1].values[3]);
    }
}
