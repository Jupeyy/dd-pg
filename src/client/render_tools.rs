use crate::{
    datafile::CDatafileWrapper,
    game::state::GameStateInterface,
    mapdef::{CEnvPoint, CMapItemGroup, CMapItemGroupEx, CQuad, CTile, CurveType, TileFlag},
};

use graphics_base::streaming::{rotate, DrawScopeImpl};
use math::math::{
    fx2f,
    vector::{vec2, vec4},
    PI,
};

use base::system::SystemInterface;

use graphics::graphics::Graphics;

use graphics_types::{
    rendering::{ColorRGBA, GL_SPoint, GL_SVertex, State},
    types::{CQuadItem, Triangle},
};

use super::render_pipe::RenderPipeline;

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
    pub fn render_tile_map<F>(
        pipe: &mut RenderPipeline,
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
        F: Fn(&mut RenderPipeline, i32, i32, &mut ColorRGBA),
    {
        let canvas_width = pipe.graphics.canvas_width();

        let mut channels = ColorRGBA {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        if color_env >= 0 {
            envelop_evaluation_func(pipe, color_env_offset, color_env, &mut channels);
        }

        let tile_buffering_enabled = pipe.graphics.is_tile_buffering_enabled();
        let mut draw_quads = if tile_buffering_enabled {
            pipe.graphics.backend_handle.quads_tex_3d_begin()
        } else {
            pipe.graphics.backend_handle.quads_begin()
        };
        draw_quads.get_draw_scope().set_state(state);

        let mut canvas_x0: f32 = 0.0;
        let mut canvas_y0: f32 = 0.0;
        let mut canvas_x1: f32 = 0.0;
        let mut canvas_y1: f32 = 0.0;
        state.get_canvas_mapping(
            &mut canvas_x0,
            &mut canvas_y0,
            &mut canvas_x1,
            &mut canvas_y1,
        );

        // calculate the final pixelsize for the tiles
        let tile_pixel_size = 1024.0 / 32.0;
        let final_tile_size = scale / (canvas_x1 - canvas_x0) * canvas_width as f32;
        let final_tileset_scale = final_tile_size / tile_pixel_size;

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

        // adjust the texture shift according to mipmap level
        let tex_size = 1024.0;
        let frac = (1.25 / tex_size) * (1.0 / final_tileset_scale);
        let nudge = (0.5 / tex_size) * (1.0 / final_tileset_scale);

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
                    if (flags & TileFlag::OPAQUE as i32) != 0
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
                        let tx = index as i32 % 16;
                        let ty = index as i32 / 16;
                        let px0 = tx * (1024 / 16);
                        let py0 = ty * (1024 / 16);
                        let px1 = px0 + (1024 / 16) - 1;
                        let py1 = py0 + (1024 / 16) - 1;

                        let mut x0 = nudge + px0 as f32 / tex_size + frac;
                        let mut y0 = nudge + py0 as f32 / tex_size + frac;
                        let mut x1 = nudge + px1 as f32 / tex_size - frac;
                        let mut y1 = nudge + py0 as f32 / tex_size + frac;
                        let mut x2 = nudge + px1 as f32 / tex_size - frac;
                        let mut y2 = nudge + py1 as f32 / tex_size - frac;
                        let mut x3 = nudge + px0 as f32 / tex_size + frac;
                        let mut y3 = nudge + py1 as f32 / tex_size - frac;

                        if tile_buffering_enabled {
                            x0 = 0.0;
                            y0 = 0.0;
                            x1 = x0 + 1.0;
                            y1 = y0;
                            x2 = x0 + 1.0;
                            y2 = y0 + 1.0;
                            x3 = x0;
                            y3 = y0 + 1.0;
                        }

                        if (flags & TileFlag::XFLIP as i32) != 0 {
                            x0 = x2;
                            x1 = x3;
                            x2 = x3;
                            x3 = x0;
                        }

                        if (flags & TileFlag::YFLIP as i32) != 0 {
                            y0 = y3;
                            y2 = y1;
                            y3 = y1;
                            y1 = y0;
                        }

                        if (flags & TileFlag::ROTATE as i32) != 0 {
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

                        if tile_buffering_enabled {
                            draw_quads.quads_set_subset_free(x0, y0, x1, y1, x2, y2, x3, y3, index);
                            let _quad_item =
                                CQuadItem::new(x as f32 * scale, y as f32 * scale, scale, scale);
                            //TODO pipe.graphics.QuadsTex3DDrawTL(&QuadItem, 1);
                        } else {
                            draw_quads.quads_set_subset_free(x0, y0, x1, y1, x2, y2, x3, y3, 0);
                            let quad_item =
                                CQuadItem::new(x as f32 * scale, y as f32 * scale, scale, scale);
                            draw_quads.quads_draw_tl(&[quad_item])
                        }
                    }
                }
                x += tiles[c].skip as i32;
                x += 1;
            }
        }

        drop(draw_quads);
        /*if graphics.is_tile_buffering_enabled() {
            pipe.graphics.QuadsTex3DEnd();
        }
        else {
            pipe.graphics.QuadsEnd();
        }*/
        //pipe.graphics.MapCanvas(CanvasX0, CanvasY0, CanvasX1, CanvasY1);
    }

    pub fn render_quads<F>(
        pipe: &mut RenderPipeline,
        state: &State,
        quads: &Vec<CQuad>,
        num_quads: usize,
        render_flags: i32,
        envelop_evaluation_func: F,
        alpha: f32,
    ) where
        F: Fn(
            &CDatafileWrapper,
            &dyn GameStateInterface,
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
            state,
            quads,
            num_quads,
            render_flags,
            envelop_evaluation_func,
            alpha,
        );
    }

    pub fn force_render_quads<F>(
        pipe: &mut RenderPipeline,
        _state: &State,
        pQuads: &Vec<CQuad>,
        NumQuads: usize,
        RenderFlags: i32,
        envelop_evaluation_func: F,
        alpha: f32,
    ) where
        F: Fn(
            &CDatafileWrapper,
            &dyn GameStateInterface,
            &dyn SystemInterface,
            i32,
            i32,
            &mut ColorRGBA,
        ),
    {
        let mut draw_triangles = pipe.graphics.backend_handle.triangles_begin();
        let Conv: f32 = 1.0 / 255.0;
        for i in 0..NumQuads {
            let pQuad = &pQuads[i];

            let mut Color = ColorRGBA {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            };
            if pQuad.color_env >= 0 {
                envelop_evaluation_func(
                    pipe.map,
                    pipe.game,
                    pipe.sys,
                    pQuad.color_env_offset,
                    pQuad.color_env,
                    &mut Color,
                );
            }

            if Color.a <= 0.0 {
                continue;
            }

            let Opaque = false;
            /* TODO: Analyze quadtexture
            if(a < 0.01f || (q->m_aColors[0].a < 0.01f && q->m_aColors[1].a < 0.01f && q->m_aColors[2].a < 0.01f && q->m_aColors[3].a < 0.01f))
                Opaque = true;
            */
            if Opaque && (RenderFlags & LayerRenderFlag::Opaque as i32) == 0 {
                continue;
            }
            if !Opaque && (RenderFlags & LayerRenderFlag::Transparent as i32) == 0 {
                continue;
            }

            let mut OffsetX = 0.0;
            let mut OffsetY = 0.0;
            let mut Rot = 0.0;

            // TODO: fix this
            if pQuad.pos_env >= 0 {
                let mut Channels = ColorRGBA::default();
                envelop_evaluation_func(
                    pipe.map,
                    pipe.game,
                    pipe.sys,
                    pQuad.pos_env_offset,
                    pQuad.pos_env,
                    &mut Channels,
                );
                OffsetX = Channels.r;
                OffsetY = Channels.g;
                Rot = Channels.b / 360.0 * PI * 2.0;
            }

            let Array: [vec4; 4] = [
                vec4::new(
                    pQuad.colors[0].r() as f32 * Conv * Color.r,
                    pQuad.colors[0].g() as f32 * Conv * Color.g,
                    pQuad.colors[0].b() as f32 * Conv * Color.b,
                    pQuad.colors[0].a() as f32 * Conv * Color.a * alpha,
                ),
                vec4::new(
                    pQuad.colors[1].r() as f32 * Conv * Color.r,
                    pQuad.colors[1].g() as f32 * Conv * Color.g,
                    pQuad.colors[1].b() as f32 * Conv * Color.b,
                    pQuad.colors[1].a() as f32 * Conv * Color.a * alpha,
                ),
                vec4::new(
                    pQuad.colors[2].r() as f32 * Conv * Color.r,
                    pQuad.colors[2].g() as f32 * Conv * Color.g,
                    pQuad.colors[2].b() as f32 * Conv * Color.b,
                    pQuad.colors[2].a() as f32 * Conv * Color.a * alpha,
                ),
                vec4::new(
                    pQuad.colors[3].r() as f32 * Conv * Color.r,
                    pQuad.colors[3].g() as f32 * Conv * Color.g,
                    pQuad.colors[3].b() as f32 * Conv * Color.b,
                    pQuad.colors[3].a() as f32 * Conv * Color.a * alpha,
                ),
            ];
            let mut aPoints: [GL_SVertex; 4] = Default::default();
            aPoints.iter_mut().enumerate().for_each(|(index, p)| {
                p.pos = vec2::new(fx2f(pQuad.points[index].x), fx2f(pQuad.points[index].y));
            });

            if Rot != 0.0 {
                let center = vec2::new(fx2f(pQuad.points[4].x), fx2f(pQuad.points[4].y));

                rotate(&center, Rot, &mut aPoints);
            }

            draw_triangles.triangles_set_subset_free(
                fx2f(pQuad.tex_coords[0].x),
                fx2f(pQuad.tex_coords[0].y),
                fx2f(pQuad.tex_coords[1].x),
                fx2f(pQuad.tex_coords[1].y),
                fx2f(pQuad.tex_coords[3].x),
                fx2f(pQuad.tex_coords[3].y),
            );

            draw_triangles.set_colors(&[Array[0], Array[1], Array[3]]);

            let tri = Triangle::new(&[
                vec2::new(aPoints[0].pos.x + OffsetX, aPoints[0].pos.y + OffsetY),
                vec2::new(aPoints[1].pos.x + OffsetX, aPoints[1].pos.y + OffsetY),
                vec2::new(aPoints[3].pos.x + OffsetX, aPoints[3].pos.y + OffsetY),
            ]);

            draw_triangles.triangles_draw_tl(&[tri]);

            draw_triangles.triangles_set_subset_free(
                fx2f(pQuad.tex_coords[0].x),
                fx2f(pQuad.tex_coords[0].y),
                fx2f(pQuad.tex_coords[3].x),
                fx2f(pQuad.tex_coords[3].y),
                fx2f(pQuad.tex_coords[2].x),
                fx2f(pQuad.tex_coords[2].y),
            );

            draw_triangles.set_colors(&[Array[0], Array[3], Array[2]]);

            let tri = Triangle::new(&[
                vec2::new(aPoints[0].pos.x + OffsetX, aPoints[0].pos.y + OffsetY),
                vec2::new(aPoints[3].pos.x + OffsetX, aPoints[3].pos.y + OffsetY),
                vec2::new(aPoints[2].pos.x + OffsetX, aPoints[2].pos.y + OffsetY),
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
        points: &mut [f32; 4],
    ) {
        let mut width = 0.0;
        let mut height = 0.0;
        Self::calc_canvas_params(aspect, zoom, &mut width, &mut height);

        let scale = (parallax_zoom * (zoom - 1.0) + 100.0) / 100.0 / zoom;
        width *= scale;
        height *= scale;

        let center_x = center_x * parallax_x / 100.0;
        let center_y = center_y * parallax_y / 100.0;
        points[0] = offset_x + center_x - width / 2.0;
        points[1] = offset_y + center_y - height / 2.0;
        points[2] = points[0] + width;
        points[3] = points[1] + height;
    }

    pub fn map_canvas_to_group(
        graphics: &Graphics,
        state: &mut State,
        center_x: f32,
        center_y: f32,
        group: &CMapItemGroup,
        _group_ex: Option<&mut CMapItemGroupEx>,
        zoom: f32,
    ) {
        // TODO let ParallaxZoom = GetParallaxZoom(pGroup, pGroupEx);
        let mut points: [f32; 4] = [0.0; 4];
        Self::map_canvas_to_world(
            center_x,
            center_y,
            group.parallax_x as f32,
            group.parallax_y as f32,
            1.0,
            group.offset_x as f32,
            group.offset_y as f32,
            graphics.canvas_aspect(),
            zoom,
            &mut points,
        );
        state.map_canvas(points[0], points[1], points[2], points[3]);
    }

    pub fn render_eval_envelope(
        pPoints: &[CEnvPoint],
        NumPoints: i32,
        Channels: i32,
        TimeNanosParam: std::time::Duration,
        Result: &mut ColorRGBA,
    ) {
        let mut TimeNanos = TimeNanosParam;
        if NumPoints == 0 {
            *Result = ColorRGBA::default();
            return;
        }

        if NumPoints == 1 {
            Result.r = fx2f(pPoints[0].values[0]);
            Result.g = fx2f(pPoints[0].values[1]);
            Result.b = fx2f(pPoints[0].values[2]);
            Result.a = fx2f(pPoints[0].values[3]);
            return;
        }

        let MaxPointTime = pPoints[NumPoints as usize - 1].time as u64
            * std::time::Duration::from_millis(1).as_nanos() as u64;
        if MaxPointTime > 0
        // TODO: remove this check when implementing a IO check for maps(in this case broken envelopes)
        {
            TimeNanos = std::time::Duration::from_nanos(TimeNanos.as_nanos() as u64 % MaxPointTime);
        } else {
            TimeNanos = std::time::Duration::from_nanos(0);
        }

        let TimeMillis =
            (TimeNanos / std::time::Duration::from_millis(1).as_nanos() as u32).as_millis() as u64;
        for i in 0..NumPoints as usize - 1 {
            if TimeMillis >= pPoints[i].time as u64 && TimeMillis <= pPoints[i + 1].time as u64 {
                let Delta = pPoints[i + 1].time - pPoints[i].time;
                let mut a = ((TimeNanos.as_nanos() as f64
                    / std::time::Duration::from_millis(1).as_nanos() as f64)
                    - pPoints[i].time as f64)
                    / Delta as f64;

                if pPoints[i].curve_type == CurveType::CURVETYPE_SMOOTH as i32 {
                    a = -2.0 * a * a * a + 3.0 * a * a; // second hermite basis
                } else if pPoints[i].curve_type == CurveType::CURVETYPE_SLOW as i32 {
                    a = a * a * a;
                } else if pPoints[i].curve_type == CurveType::CURVETYPE_FAST as i32 {
                    a = 1.0 - a;
                    a = 1.0 - a * a * a;
                } else if pPoints[i].curve_type == CurveType::CURVETYPE_STEP as i32 {
                    a = 0.0;
                } else {
                    // linear
                }

                for c in 0..Channels as usize {
                    let v0 = fx2f(pPoints[i].values[c]);
                    let v1 = fx2f(pPoints[i + 1].values[c]);
                    match c {
                        0 => Result.r = (v0 as f64 + (v1 - v0) as f64 * a) as f32,
                        1 => Result.g = (v0 as f64 + (v1 - v0) as f64 * a) as f32,
                        2 => Result.b = (v0 as f64 + (v1 - v0) as f64 * a) as f32,
                        3 => Result.a = (v0 as f64 + (v1 - v0) as f64 * a) as f32,
                        _ => (),
                    }
                }

                return;
            }
        }

        Result.r = fx2f(pPoints[NumPoints as usize - 1].values[0]);
        Result.g = fx2f(pPoints[NumPoints as usize - 1].values[1]);
        Result.b = fx2f(pPoints[NumPoints as usize - 1].values[2]);
        Result.a = fx2f(pPoints[NumPoints as usize - 1].values[3]);
    }
}
