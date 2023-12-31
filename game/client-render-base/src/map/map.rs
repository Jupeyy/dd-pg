use std::time::Duration;

use crate::map::client_map_buffered::{
    ClientMapBufferedInfo, ClientMapBufferedVisuals, MapRenderLayer, MapRenderTextOverlayType,
};

use super::{
    map_pipeline::{MapGraphics, QuadRenderInfo},
    render_pipe::{GameStateRenderInfo, RenderPipeline, RenderPipelineBase},
    render_tools::RenderTools,
};
use client_containers::entities::Entities;
use graphics::{
    graphics::Graphics,
    handles::{canvas::GraphicsCanvasHandle, stream::GraphicsStreamHandle},
};
use hiarc_macro::Hiarc;
use pool::mt_pool::Pool;
use shared_base::{
    datafile::CDatafileWrapper,
    mapdef::{
        CEnvPoint, CMapItemGroup, CMapItemLayerQuads, CMapItemLayerTilemap, CQuad, LayerFlag,
        MapLayer, MapLayerQuad, MapLayerTile, MapTileLayerDetail,
    },
    state_helper::intra_tick_from_start,
    types::GameTickType,
};

use math::math::{mix, vector::vec2, PI};

use base::system::SystemInterface;

use graphics_types::{
    rendering::{BlendType, ColorRGBA, State},
    textures_handle::TextureIndex,
};

#[derive(Debug, Hiarc)]
pub struct RenderMap {
    #[hiarc]
    map_graphics: MapGraphics,

    #[hiarc]
    canvas_handle: GraphicsCanvasHandle,
    #[hiarc]
    stream_handle: GraphicsStreamHandle,

    index_offset_or_draw_count_pool: Pool<Vec<usize>>,
}

impl RenderMap {
    pub fn new(graphics: &mut Graphics) -> RenderMap {
        RenderMap {
            map_graphics: MapGraphics::new(graphics),

            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),

            index_offset_or_draw_count_pool: graphics.index_offset_or_draw_count_pool.clone(),
        }
    }

    fn envelope_eval(
        &self,
        map: &CDatafileWrapper,
        game: &GameStateRenderInfo,
        sys: &dyn SystemInterface,
        intra_tick_time: &Duration,
        animation_start_tick: &GameTickType,
        time_offset_millis: i32,
        env: i32,
        channels: &mut ColorRGBA,
    ) {
        *channels = ColorRGBA::default();

        let mut points: Option<&[CEnvPoint]> = None;

        {
            let num = map.env_point_count();
            if num > 0 {
                points = Some(map.get_env_points()[0].as_slice());
            }
        }

        let num = map.env_count();

        if env as usize >= num {
            return;
        }

        let map_item = map.get_env(env as usize);

        let tick_to_nanoseconds =
            std::time::Duration::from_secs(1).as_nanos() as u64 / game.ticks_per_second as u64;

        let mut total_time = sys.time_get_nanoseconds();

        if map_item.version < 2 || map_item.synchronized > 0 {
            let cur_tick = game.cur_tick;
            // get the lerp of the current tick and prev
            let min_tick = (cur_tick - 1) - animation_start_tick;
            let cur_tick = cur_tick - animation_start_tick;
            total_time = std::time::Duration::from_nanos(
                (mix::<f64, f64>(
                    &0.0,
                    &((cur_tick - min_tick) as f64),
                    intra_tick_from_start(
                        &game.ticks_per_second,
                        intra_tick_time,
                        &cur_tick,
                        animation_start_tick,
                    ),
                ) * tick_to_nanoseconds as f64) as u64
                    + min_tick * tick_to_nanoseconds,
            );
        }
        RenderTools::render_eval_envelope(
            points.unwrap().split_at(map_item.start_point as usize).1,
            map_item.num_points,
            4,
            total_time + std::time::Duration::from_millis(time_offset_millis as u64),
            channels,
        );
    }

    fn render_tile_layer(
        &self,
        state: &State,
        pipe: &mut RenderPipelineBase,
        map_visuals: &ClientMapBufferedVisuals,
        layer_index: usize,
        mut color: ColorRGBA,
        tile_layer: &CMapItemLayerTilemap,
        group: &CMapItemGroup,
    ) {
        let visuals = &map_visuals.tile_layer_visuals[layer_index];
        if let Some(buffer_container_index) = &visuals.buffer_object_index {
            let (screen_x0, screen_y0, screen_x1, screen_y1) = state.get_canvas_mapping();

            let mut channels = ColorRGBA {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            };
            if tile_layer.color_env >= 0 {
                self.envelope_eval(
                    pipe.map,
                    pipe.game,
                    pipe.sys,
                    pipe.intra_tick_time,
                    &pipe.camera.animation_start_tick,
                    tile_layer.color_env_offset,
                    tile_layer.color_env,
                    &mut channels,
                );
            }

            let mut draw_border = false;

            let border_y0 = (screen_y0 / 32.0).floor() as i32;
            let border_x0 = (screen_x0 / 32.0).floor() as i32;
            let border_y1 = (screen_y1 / 32.0).ceil() as i32;
            let border_x1 = (screen_x1 / 32.0).ceil() as i32;

            let mut y0 = border_y0;
            let mut x0 = border_x0;
            let mut y1 = border_y1;
            let mut x1 = border_x1;

            if x0 < 0 {
                x0 = 0;
                draw_border = true;
            }
            if y0 < 0 {
                y0 = 0;
                draw_border = true;
            }
            if x1 > tile_layer.width {
                x1 = tile_layer.width;
                draw_border = true;
            }
            if y1 > tile_layer.height {
                y1 = tile_layer.height;
                draw_border = true;
            }

            let mut draw_layer = true;
            if x1 <= 0 {
                draw_layer = false;
            }
            if y1 <= 0 {
                draw_layer = false;
            }
            if x0 >= tile_layer.width {
                draw_layer = false;
            }
            if y0 >= tile_layer.height {
                draw_layer = false;
            }

            if draw_layer {
                // indices buffers we want to draw
                let mut index_offsets = self.index_offset_or_draw_count_pool.new();
                let mut draw_counts = self.index_offset_or_draw_count_pool.new();

                let reserve: usize = (y1 - y0).abs() as usize + 1;
                index_offsets.reserve(reserve);
                draw_counts.reserve(reserve);

                for y in y0..y1 {
                    if x0 > x1 {
                        continue;
                    }
                    let xr = x1 - 1;

                    if visuals.base.tiles_of_layer[(y * tile_layer.width + xr) as usize]
                        .index_buffer_offset_quad()
                        < visuals.base.tiles_of_layer[(y * tile_layer.width + x0) as usize]
                            .index_buffer_offset_quad()
                    {
                        panic!("Tile count wrong.");
                    }

                    let num_vertices = ((visuals.base.tiles_of_layer
                        [(y * tile_layer.width + xr) as usize]
                        .index_buffer_offset_quad()
                        - visuals.base.tiles_of_layer[(y * tile_layer.width + x0) as usize]
                            .index_buffer_offset_quad())
                        / std::mem::size_of::<u32>())
                        + (if visuals.base.tiles_of_layer[(y * tile_layer.width + xr) as usize]
                            .drawable()
                        {
                            6
                        } else {
                            0
                        });

                    if num_vertices > 0 {
                        index_offsets.push(
                            visuals.base.tiles_of_layer[(y * tile_layer.width + x0) as usize]
                                .index_buffer_offset_quad(),
                        );
                        draw_counts.push(num_vertices);
                    }
                }

                color.r *= channels.r;
                color.g *= channels.g;
                color.b *= channels.b;
                color.a *= channels.a;

                let draw_count = index_offsets.len();
                if draw_count != 0 {
                    self.map_graphics.render_tile_layer(
                        state,
                        buffer_container_index,
                        &color,
                        index_offsets,
                        draw_counts,
                        draw_count,
                    );
                }
            }

            if draw_border {
                self.render_tile_border(
                    state,
                    map_visuals,
                    layer_index,
                    &color,
                    tile_layer,
                    group,
                    border_x0,
                    border_y0,
                    border_x1,
                    border_y1,
                );
            }
        }
    }

    fn render_tile_border(
        &self,
        state: &State,
        map_visuals: &ClientMapBufferedVisuals,
        layer_index: usize,
        color: &ColorRGBA,
        tile_layer: &CMapItemLayerTilemap,
        _group: &CMapItemGroup,
        border_x0: i32,
        border_y0: i32,
        border_x1: i32,
        border_y1: i32,
    ) {
        let visuals = &map_visuals.tile_layer_visuals[layer_index];
        if let Some(buffer_container_index) = &visuals.buffer_object_index {
            let mut y0 = border_y0;
            let mut x0 = border_x0;
            let mut y1 = border_y1;
            let mut x1 = border_x1;

            if x0 < 0 {
                x0 = 0;
            }
            if y0 < 0 {
                y0 = 0;
            }
            if x1 > tile_layer.width {
                x1 = tile_layer.width;
            }
            if y1 > tile_layer.height {
                y1 = tile_layer.height;
            }

            if border_x0 < 0 {
                // Draw corners on left side
                if border_y0 < 0 {
                    if visuals.base.border_top_left.drawable() {
                        let mut offset = vec2::default();
                        offset.x = 0.0;
                        offset.y = 0.0;
                        let mut scale = vec2::default();
                        scale.x = border_x0.abs() as f32;
                        scale.y = border_y0.abs() as f32;

                        self.map_graphics.render_border_tiles(
                            state,
                            buffer_container_index,
                            color,
                            visuals.base.border_top_left.index_buffer_offset_quad(),
                            &offset,
                            &scale,
                            1,
                        );
                    }
                }
                if border_y1 > tile_layer.height {
                    if visuals.base.border_bottom_left.drawable() {
                        let mut offset = vec2::default();
                        offset.x = 0.0;
                        offset.y = tile_layer.height as f32 * 32.0;
                        let mut scale = vec2::default();
                        scale.x = border_x0.abs() as f32;
                        scale.y = (border_y1 - tile_layer.height) as f32;

                        self.map_graphics.render_border_tiles(
                            state,
                            buffer_container_index,
                            color,
                            visuals.base.border_bottom_left.index_buffer_offset_quad(),
                            &offset,
                            &scale,
                            1,
                        );
                    }
                }
            }
            if border_x1 > tile_layer.width {
                // Draw corners on right side
                if border_y0 < 0 {
                    if visuals.base.border_top_right.drawable() {
                        let mut offset = vec2::default();
                        offset.x = tile_layer.width as f32 * 32.0;
                        offset.y = 0.0;
                        let mut scale = vec2::default();
                        scale.x = (border_x1 - tile_layer.width) as f32;
                        scale.y = border_y0.abs() as f32;

                        self.map_graphics.render_border_tiles(
                            state,
                            buffer_container_index,
                            color,
                            visuals.base.border_top_right.index_buffer_offset_quad(),
                            &offset,
                            &scale,
                            1,
                        );
                    }
                }
                if border_y1 > tile_layer.height {
                    if visuals.base.border_bottom_right.drawable() {
                        let mut offset = vec2::default();
                        offset.x = tile_layer.width as f32 * 32.0;
                        offset.y = tile_layer.height as f32 * 32.0;
                        let mut scale = vec2::default();
                        scale.x = (border_x1 - tile_layer.width) as f32;
                        scale.y = (border_y1 - tile_layer.height) as f32;

                        self.map_graphics.render_border_tiles(
                            state,
                            buffer_container_index,
                            color,
                            visuals.base.border_bottom_right.index_buffer_offset_quad(),
                            &offset,
                            &scale,
                            1,
                        );
                    }
                }
            }
            if border_x1 > tile_layer.width {
                // Draw right border
                if y0 < tile_layer.height && y1 > 0 {
                    let yb = y1 - 1;
                    let draw_num = ((visuals.base.border_right[yb as usize]
                        .index_buffer_offset_quad()
                        - visuals.base.border_right[y0 as usize].index_buffer_offset_quad())
                        / (std::mem::size_of::<u32>() * 6))
                        + (if visuals.base.border_right[yb as usize].drawable() {
                            1
                        } else {
                            0
                        });
                    let byte_offset =
                        visuals.base.border_right[y0 as usize].index_buffer_offset_quad();
                    let mut offset = vec2::default();
                    offset.x = tile_layer.width as f32 * 32.0;
                    offset.y = 0.0;
                    let mut scale = vec2::default();
                    scale.x = (border_x1 - tile_layer.width) as f32;
                    scale.y = 1.0;

                    self.map_graphics.render_border_tiles(
                        state,
                        buffer_container_index,
                        color,
                        byte_offset,
                        &offset,
                        &scale,
                        draw_num,
                    );
                }
            }

            if border_x0 < 0 {
                // Draw left border
                if y0 < tile_layer.height && y1 > 0 {
                    let yb = y1 - 1;
                    let draw_num = (((visuals.base.border_left[yb as usize]
                        .index_buffer_offset_quad()
                        - visuals.base.border_left[y0 as usize].index_buffer_offset_quad())
                        / (std::mem::size_of::<u32>() * 6))
                        + (if visuals.base.border_left[yb as usize].drawable() {
                            1
                        } else {
                            0
                        })) as usize;
                    let byte_offset =
                        visuals.base.border_left[y0 as usize].index_buffer_offset_quad();
                    let mut offset = vec2::default();
                    offset.x = 0.0;
                    offset.y = 0.0;
                    let mut scale = vec2::default();
                    scale.x = (border_x0).abs() as f32;
                    scale.y = 1.0;

                    self.map_graphics.render_border_tiles(
                        state,
                        buffer_container_index,
                        color,
                        byte_offset,
                        &offset,
                        &scale,
                        draw_num,
                    );
                }
            }
            if border_y0 < 0 {
                // Draw top border
                if x0 < tile_layer.width && x1 > 0 {
                    let xr = x1 - 1;
                    let draw_num = ((visuals.base.border_top[xr as usize]
                        .index_buffer_offset_quad()
                        - visuals.base.border_top[x0 as usize].index_buffer_offset_quad())
                        / (std::mem::size_of::<u32>() * 6))
                        + (if visuals.base.border_top[xr as usize].drawable() {
                            1
                        } else {
                            0
                        });
                    let byte_offset =
                        visuals.base.border_top[x0 as usize].index_buffer_offset_quad();
                    let mut offset = vec2::default();
                    offset.x = 0.0;
                    offset.y = 0.0;
                    let mut scale = vec2::default();
                    scale.x = 1.0;
                    scale.y = border_y0.abs() as f32;

                    self.map_graphics.render_border_tiles(
                        state,
                        buffer_container_index,
                        color,
                        byte_offset,
                        &offset,
                        &scale,
                        draw_num,
                    );
                }
            }
            if border_y1 > tile_layer.height {
                // Draw bottom border
                if x0 < tile_layer.width && x1 > 0 {
                    let xr = x1 - 1;
                    let draw_num = ((visuals.base.border_bottom[xr as usize]
                        .index_buffer_offset_quad()
                        - visuals.base.border_bottom[x0 as usize].index_buffer_offset_quad())
                        / (std::mem::size_of::<u32>() * 6))
                        + (if visuals.base.border_bottom[xr as usize].drawable() {
                            1
                        } else {
                            0
                        });
                    let byte_offset =
                        visuals.base.border_bottom[x0 as usize].index_buffer_offset_quad();
                    let mut offset = vec2::default();
                    offset.x = 0.0;
                    offset.y = tile_layer.height as f32 * 32.0;
                    let mut scale = vec2::default();
                    scale.x = 1.0;
                    scale.y = (border_y1 - tile_layer.height) as f32;

                    self.map_graphics.render_border_tiles(
                        state,
                        buffer_container_index,
                        color,
                        byte_offset,
                        &offset,
                        &scale,
                        draw_num,
                    );
                }
            }
        }
    }

    fn render_kill_tile_border(
        &self,
        state: &State,
        map_visuals: &ClientMapBufferedVisuals,
        layer_index: usize,
        color: &ColorRGBA,
        tile_layer: &CMapItemLayerTilemap,
        _group: &CMapItemGroup,
    ) {
        let visuals = &map_visuals.tile_layer_visuals[layer_index];
        if let Some(buffer_container_index) = &visuals.buffer_object_index {
            let (canvas_x0, canvas_y0, canvas_x1, canvas_y1) = state.get_canvas_mapping();

            let mut draw_border = false;

            let mut border_y0 = (canvas_y0 / 32.0).floor() as i32;
            let mut border_x0 = (canvas_x0 / 32.0).floor() as i32;
            let mut border_y1 = (canvas_y1 / 32.0).ceil() as i32;
            let mut border_x1 = (canvas_x1 / 32.0).ceil() as i32;

            if border_x0 < -201 {
                draw_border = true;
            }
            if border_y0 < -201 {
                draw_border = true;
            }
            if border_x1 > tile_layer.width + 201 {
                draw_border = true;
            }
            if border_y1 > tile_layer.height + 201 {
                draw_border = true;
            }

            if !draw_border {
                return;
            }
            if !visuals.base.border_kill_tile.drawable() {
                return;
            }

            if border_x0 < -300 {
                border_x0 = -300;
            }
            if border_y0 < -300 {
                border_y0 = -300;
            }
            if border_x1 >= tile_layer.width + 300 {
                border_x1 = tile_layer.width + 299;
            }
            if border_y1 >= tile_layer.height + 300 {
                border_y1 = tile_layer.height + 299;
            }

            if border_x1 < -300 {
                border_x1 = -300;
            }
            if border_y1 < -300 {
                border_y1 = -300;
            }
            if border_x0 >= tile_layer.width + 300 {
                border_x0 = tile_layer.width + 299;
            }
            if border_y0 >= tile_layer.height + 300 {
                border_y0 = tile_layer.height + 299;
            }

            // Draw left kill tile border
            if border_x0 < -201 {
                let mut offset = vec2::default();
                offset.x = border_x0 as f32 * 32.0;
                offset.y = border_y0 as f32 * 32.0;
                let mut scale = vec2::default();
                scale.x = (-201 - border_x0) as f32;
                scale.y = (border_y1 - border_y0) as f32;
                self.map_graphics.render_border_tiles(
                    state,
                    buffer_container_index,
                    color,
                    visuals.base.border_kill_tile.index_buffer_offset_quad(),
                    &offset,
                    &scale,
                    1,
                );
            }
            // Draw top kill tile border
            if border_y0 < -201 {
                let mut offset = vec2::default();
                offset.x = border_x0.max(-201) as f32 * 32.0;
                offset.y = border_y0 as f32 * 32.0;
                let mut scale = vec2::default();
                scale.x = (border_x1.min(tile_layer.width + 201) - border_x0.max(-201)) as f32;
                scale.y = (-201 - border_y0) as f32;
                self.map_graphics.render_border_tiles(
                    state,
                    buffer_container_index,
                    color,
                    visuals.base.border_kill_tile.index_buffer_offset_quad(),
                    &offset,
                    &scale,
                    1,
                );
            }
            // Draw right kill tile border
            if border_x1 > tile_layer.width + 201 {
                let mut offset = vec2::default();
                offset.x = 32.0 * (tile_layer.width + 201) as f32;
                offset.y = border_y0 as f32 * 32.0;
                let mut scale = vec2::default();
                scale.x = (border_x1 - (tile_layer.width + 201)) as f32;
                scale.y = (border_y1 - border_y0) as f32;
                self.map_graphics.render_border_tiles(
                    state,
                    buffer_container_index,
                    color,
                    visuals.base.border_kill_tile.index_buffer_offset_quad(),
                    &offset,
                    &scale,
                    1,
                );
            }
            // Draw bottom kill tile border
            if border_y1 > tile_layer.height + 201 {
                let mut offset = vec2::default();
                offset.x = border_x0.max(-201) as f32 * 32.0;
                offset.y = (tile_layer.height + 201) as f32 * 32.0;
                let mut scale = vec2::default();
                scale.x = (border_x1.min(tile_layer.width + 201) - border_x0.max(-201)) as f32;
                scale.y = (border_y1 - (tile_layer.height + 201)) as f32;
                self.map_graphics.render_border_tiles(
                    state,
                    buffer_container_index,
                    color,
                    visuals.base.border_kill_tile.index_buffer_offset_quad(),
                    &offset,
                    &scale,
                    1,
                );
            }
        }
    }

    fn render_quad_layer(
        &self,
        state: &State,
        pipe: &mut RenderPipelineBase,
        map_visuals: &ClientMapBufferedVisuals,
        layer_index: usize,
        quad_layer: &CMapItemLayerQuads,
        quads: &Vec<CQuad>,
        _group: &CMapItemGroup,
    ) {
        let visuals = &map_visuals.quad_layer_visuals[layer_index];
        if let Some(buffer_container_index) = &visuals.buffer_object_index {
            let mut quad_render_infos = self.stream_handle.get_uniform_instance::<QuadRenderInfo>();
            let (mut quads_infos, mut used_count, mut instance) = quad_render_infos.get();

            let mut cur_quad_offset = 0;
            for i in 0..quad_layer.num_quads as usize {
                let quad = &quads[i];

                let mut color = ColorRGBA {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                };
                if quad.color_env >= 0 {
                    self.envelope_eval(
                        pipe.map,
                        pipe.game,
                        pipe.sys,
                        pipe.intra_tick_time,
                        &pipe.camera.animation_start_tick,
                        quad.color_env_offset,
                        quad.color_env,
                        &mut color,
                    );
                }

                let mut offset_x = 0.0;
                let mut offset_y = 0.0;
                let mut rot = 0.0;

                if quad.pos_env >= 0 {
                    let mut color_channels = ColorRGBA::default();
                    self.envelope_eval(
                        pipe.map,
                        pipe.game,
                        pipe.sys,
                        pipe.intra_tick_time,
                        &pipe.camera.animation_start_tick,
                        quad.pos_env_offset,
                        quad.pos_env,
                        &mut color_channels,
                    );
                    offset_x = color_channels.r;
                    offset_y = color_channels.g;
                    rot = color_channels.b / 180.0 * PI;
                }

                let is_fully_transparent = color.a <= 0.0;
                let needs_flush = *used_count == quads_infos.len() || is_fully_transparent;

                if needs_flush {
                    // render quads of the current offset directly(cancel batching)
                    let quad_count = *used_count;
                    drop(quad_render_infos);
                    self.map_graphics.render_quad_layer(
                        &state,
                        buffer_container_index,
                        instance,
                        quad_count,
                        cur_quad_offset,
                    );

                    quad_render_infos = self.stream_handle.get_uniform_instance::<QuadRenderInfo>();
                    (quads_infos, used_count, instance) = quad_render_infos.get();

                    cur_quad_offset = i;
                    if is_fully_transparent {
                        // since this quad is ignored, the offset is the next quad
                        cur_quad_offset += 1;
                    }
                }

                if !is_fully_transparent {
                    let quad_info = &mut quads_infos[*used_count];
                    *used_count += 1;
                    quad_info.color = color;
                    quad_info.offsets.x = offset_x;
                    quad_info.offsets.y = offset_y;
                    quad_info.rotation = rot;
                }
            }
            let quad_count = *used_count;
            drop(quad_render_infos);
            self.map_graphics.render_quad_layer(
                &state,
                buffer_container_index,
                instance,
                quad_count,
                cur_quad_offset,
            );
        }
    }

    fn get_physics_layer_texture<'a>(
        layer_detail: &MapTileLayerDetail,
        entities: &'a Entities,
    ) -> &'a TextureIndex {
        match layer_detail {
            _ => &entities.vanilla,
        }
    }

    fn render_layer(
        &self,
        pipe: &mut RenderPipelineBase,
        map_visuals: &ClientMapBufferedVisuals,
        map_info: &ClientMapBufferedInfo,
        render_layer: &MapRenderLayer,
        is_background: bool,
        center: &vec2,
    ) {
        let entities = pipe.entities_container.get_or_default("TODO:");
        let render_info = render_layer.get_render_info();
        if render_info.is_physics_layer && pipe.force_full_design_render {
            return;
        }

        if let MapRenderLayer::Tile(_) = render_layer {
            if is_background && !pipe.config.background_show_tile_layers {
                return;
            }
        }

        let group = pipe.map.get_group(render_info.group_index);
        let group_ex = None; // TODO: pipe.map.GetGroupEx(g);
        let layer = pipe
            .map
            .get_layer(group.start_layer as usize + render_info.layer_index);

        let is_main_physics_layer = render_info.group_index
            == map_info.main_physics_layer_group_index
            && render_info.layer_index == map_info.main_physics_layer_layer_index;

        // skip rendering if detail layers if not wanted
        if (layer.get_tile_layer_base().flags & LayerFlag::Detail as i32) != 0
            && (!pipe.config.high_detail)
            && !render_info.is_physics_layer
        {
            return;
        }

        let physics_opacity_val = if pipe.force_full_design_render {
            0
        } else {
            pipe.config.physics_layer_opacity
        };

        // skip rendering if either full physics layer rendering or full design rendering
        if (physics_opacity_val == 100 && !render_info.is_physics_layer)
            || (physics_opacity_val == 0 && render_info.is_physics_layer)
        {
            return;
        }

        let mut state = State::new();

        // clipping
        if group.use_clipping > 0 {
            // set clipping
            RenderTools::map_canvas_to_group(
                &self.canvas_handle,
                &mut state,
                center.x,
                center.y,
                pipe.map.get_game_group(),
                None, // TODO: pipe.map.GameGroupEx(),
                pipe.camera.zoom,
            );
            let (canvas_x0, canvas_y0, canvas_x1, canvas_y1) = state.get_canvas_mapping();
            let points: [f32; 4] = [canvas_x0, canvas_y0, canvas_x1, canvas_y1];

            let x0 = (group.clip_x as f32 - points[0]) / (points[2] - points[0]);
            let y0 = (group.clip_y as f32 - points[1]) / (points[3] - points[1]);
            let x1 = ((group.clip_x + group.clip_w) as f32 - points[0]) / (points[2] - points[0]);
            let y1 = ((group.clip_y + group.clip_h) as f32 - points[1]) / (points[3] - points[1]);

            if x1 < 0.0 || x0 > 1.0 || y1 < 0.0 || y0 > 1.0 {
                //check tile layer count of this group
                return;
            }

            let (x, y, w, h) = State::auto_round_clipping(
                x0 * self.canvas_handle.window_width() as f32,
                y0 * self.canvas_handle.window_height() as f32,
                (x1 - x0) * self.canvas_handle.window_width() as f32,
                (y1 - y0) * self.canvas_handle.window_height() as f32,
            );

            state.clip_clamped(
                x,
                y,
                w,
                h,
                self.canvas_handle.window_width() as u32,
                self.canvas_handle.window_height() as u32,
            );
        }

        RenderTools::map_canvas_to_group(
            &self.canvas_handle,
            &mut state,
            center.x,
            center.y,
            group,
            group_ex,
            pipe.camera.zoom,
        );

        if let MapLayer::Tile(MapLayerTile(tile_layer, tile_layer_detail, _)) = layer {
            if tile_layer.image == -1 {
                if render_info.is_physics_layer {
                    if let Some(text_overlay) = render_info.cur_text_overlay {
                        state.set_texture(match text_overlay {
                            MapRenderTextOverlayType::Top => &entities.text_overlay_top,
                            MapRenderTextOverlayType::Bottom => &entities.text_overlay_bottom,
                            MapRenderTextOverlayType::Center => &entities.text_overlay_center,
                        });
                    } else {
                        state.set_texture(Self::get_physics_layer_texture(
                            tile_layer_detail,
                            entities,
                        ));
                    }
                }
            } else {
                state.set_texture(
                    pipe.map_images[tile_layer.image as usize]
                        .texture_index_3d
                        .as_ref()
                        .unwrap(),
                );
            }

            let color = if render_info.is_physics_layer {
                ColorRGBA {
                    r: tile_layer.color.r() as f32 / 255.0,
                    g: tile_layer.color.g() as f32 / 255.0,
                    b: tile_layer.color.b() as f32 / 255.0,
                    a: tile_layer.color.a() as f32 / 255.0 * physics_opacity_val as f32 / 100.0,
                }
            } else {
                ColorRGBA {
                    r: tile_layer.color.r() as f32 / 255.0,
                    g: tile_layer.color.g() as f32 / 255.0,
                    b: tile_layer.color.b() as f32 / 255.0,
                    a: tile_layer.color.a() as f32 / 255.0 * (100 - physics_opacity_val) as f32
                        / 100.0,
                }
            };

            state.blend(BlendType::Alpha);
            // draw kill tiles outside the entity clipping rectangle
            if is_main_physics_layer {
                // slow blinking to hint that it's not a part of the map
                let seconds = pipe.sys.time_get_nanoseconds().as_secs_f64();
                let color_hint = ColorRGBA {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.3 + 0.7 * (1.0 + (2.0 * PI as f64 * seconds / 3.0).sin() as f32) / 2.0,
                };

                let color_kill = ColorRGBA {
                    r: color.r * color_hint.r,
                    g: color.g * color_hint.g,
                    b: color.b * color_hint.b,
                    a: color.a * color_hint.a,
                };
                self.render_kill_tile_border(
                    &state,
                    map_visuals,
                    render_info.visual_index,
                    &color_kill,
                    tile_layer,
                    group,
                );
            }
            self.render_tile_layer(
                &state,
                pipe,
                map_visuals,
                render_info.visual_index,
                color,
                tile_layer,
                group,
            );
        } else if let MapLayer::Quads(MapLayerQuad(quad_layer, quads)) = layer {
            if quad_layer.image != -1 {
                state.set_texture(
                    pipe.map_images[quad_layer.image as usize]
                        .texture_index
                        .as_ref()
                        .unwrap(),
                );
            }

            if pipe.config.show_quads || pipe.force_full_design_render {
                state.blend(BlendType::Alpha);
                self.render_quad_layer(
                    &state,
                    pipe,
                    map_visuals,
                    render_info.visual_index,
                    quad_layer,
                    quads,
                    group,
                );
            }
        }
    }

    fn render_impl<'a>(
        &self,
        map_visuals: &ClientMapBufferedVisuals,
        map_info: &ClientMapBufferedInfo,
        pipe: &mut RenderPipelineBase,
        render_layers: impl Iterator<Item = &'a MapRenderLayer>,
        is_background: bool,
    ) {
        // TODO if m_OnlineOnly && Client().State() != IClient::STATE_ONLINE && Client().State() != IClient::STATE_DEMOPLAYBACK)
        //	return;
        let center = pipe.camera.pos;

        for render_layer in render_layers {
            self.render_layer(
                pipe,
                map_visuals,
                map_info,
                render_layer,
                is_background,
                &center,
            );
        }
    }

    pub fn render_background(&self, pipe: &mut RenderPipeline) {
        self.render_impl(
            &pipe.buffered_map.visuals,
            &pipe.buffered_map.info,
            &mut pipe.base,
            pipe.buffered_map.render.background_render_layers.iter(),
            true,
        );
    }

    pub fn render_foreground(&self, pipe: &mut RenderPipeline) {
        self.render_impl(
            &pipe.buffered_map.visuals,
            &pipe.buffered_map.info,
            &mut pipe.base,
            pipe.buffered_map.render.foreground_render_layers.iter(),
            true,
        );
    }

    pub fn render_full(&self, pipe: &mut RenderPipeline) {
        self.render_impl(
            &pipe.buffered_map.visuals,
            &pipe.buffered_map.info,
            &mut pipe.base,
            pipe.buffered_map.render.background_render_layers.iter(),
            true,
        );
        self.render_impl(
            &pipe.buffered_map.visuals,
            &pipe.buffered_map.info,
            &mut pipe.base,
            pipe.buffered_map.render.foreground_render_layers.iter(),
            true,
        );
    }
}
