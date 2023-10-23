use std::time::Duration;

use crate::map::client_map_buffered::{
    ClientMapBufferedInfo, ClientMapBufferedVisuals, MapRenderLayer, MapRenderTextOverlayType,
};

use super::{
    render_pipe::{GameStateRenderInfo, RenderPipeline, RenderPipelineBase},
    render_tools::RenderTools,
};
use client_containers::entities::Entities;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use graphics_base::buffer_object_handle::BufferObjectIndex;
use pool::mt_datatypes::PoolVec;
use shared_base::{
    datafile::CDatafileWrapper,
    mapdef::{
        CEnvPoint, CMapItemGroup, CMapItemLayerQuads, CMapItemLayerTilemap, CQuad, LayerFlag,
        MapLayer, MapLayerQuad, MapLayerTile, MapTileLayerDetail,
    },
    state_helper::intra_tick_from_start,
    types::GameTickType,
};

use graphics_base_traits::traits::GraphicsSizeQuery;
use math::math::{mix, vector::vec2, PI};

use base::system::SystemInterface;

use graphics_types::{
    command_buffer::{SQuadRenderInfo, GRAPHICS_MAX_QUADS_RENDER_COUNT},
    rendering::{ColorRGBA, State},
    textures_handle::TextureIndex,
};

pub struct RenderMap {}

impl RenderMap {
    pub fn new() -> RenderMap {
        RenderMap {}
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

    fn render_tile_layer<B: GraphicsBackendInterface>(
        &self,
        state: &State,
        pipe: &mut RenderPipelineBase<B>,
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
            let border_y1 = (screen_y1 / 32.0).floor() as i32;
            let border_x1 = (screen_x1 / 32.0).floor() as i32;

            let mut y0 = border_y0;
            let mut x0 = border_x0;
            let mut y1 = border_y1;
            let mut x1 = border_x1;

            if x0 <= 0 {
                x0 = 0;
                draw_border = true;
            }
            if y0 <= 0 {
                y0 = 0;
                draw_border = true;
            }
            if x1 >= tile_layer.width - 1 {
                x1 = tile_layer.width - 1;
                draw_border = true;
            }
            if y1 >= tile_layer.height - 1 {
                y1 = tile_layer.height - 1;
                draw_border = true;
            }

            let mut draw_layer = true;
            if x1 < 0 {
                draw_layer = false;
            }
            if y1 < 0 {
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
                let mut index_offsets = pipe.graphics.index_offset_or_draw_count_pool.new();
                let mut draw_counts = pipe.graphics.index_offset_or_draw_count_pool.new();

                let reserve: usize = (y1 - y0).abs() as usize + 1;
                index_offsets.reserve(reserve);
                draw_counts.reserve(reserve);

                for y in y0..=y1 {
                    if x0 > x1 {
                        continue;
                    }

                    if visuals.base.tiles_of_layer[(y * tile_layer.width + x1) as usize]
                        .index_buffer_offset_quad()
                        < visuals.base.tiles_of_layer[(y * tile_layer.width + x0) as usize]
                            .index_buffer_offset_quad()
                    {
                        panic!("Tile count wrong.");
                    }

                    let num_vertices = ((visuals.base.tiles_of_layer
                        [(y * tile_layer.width + x1) as usize]
                        .index_buffer_offset_quad()
                        - visuals.base.tiles_of_layer[(y * tile_layer.width + x0) as usize]
                            .index_buffer_offset_quad())
                        / std::mem::size_of::<u32>())
                        + (if visuals.base.tiles_of_layer[(y * tile_layer.width + x1) as usize]
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
                    pipe.graphics.render_tile_layer(
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
                    pipe,
                    map_visuals,
                    layer_index,
                    &color,
                    tile_layer,
                    group,
                    border_x0,
                    border_y0,
                    border_x1,
                    border_y1,
                    (-((-screen_x1) / 32.0).floor()) as i32 - border_x0,
                    (-((-screen_y1) / 32.0).floor()) as i32 - border_y0,
                );
            }
        }
    }

    fn render_tile_border_corner_tiles<B: GraphicsBackendInterface>(
        &self,
        state: &State,
        pipe: &mut RenderPipelineBase<B>,
        width_offset_to_origin: i32,
        height_offset_to_origin: i32,
        tile_count_width: i32,
        tile_count_height: i32,
        buffer_object_index: &BufferObjectIndex,
        color: &ColorRGBA,
        index_buffer_offset: usize,
        offset: &vec2,
        dir: &vec2,
    ) {
        // if border is still in range of the original corner, it doesn't needs to be redrawn
        let corner_visible = (width_offset_to_origin - 1 < tile_count_width)
            && (height_offset_to_origin - 1 < tile_count_height);

        let count_x = width_offset_to_origin.min(tile_count_width);
        let count_y = height_offset_to_origin.min(tile_count_height);

        let count = (count_x * count_y) as usize - (if corner_visible { 1 } else { 0 }); // Don't draw the corner again

        pipe.graphics.render_border_tiles(
            state,
            buffer_object_index,
            color,
            index_buffer_offset,
            offset,
            dir,
            count_x,
            count,
        );
    }

    fn render_tile_border<B: GraphicsBackendInterface>(
        &self,
        state: &State,
        pipe: &mut RenderPipelineBase<B>,
        map_visuals: &ClientMapBufferedVisuals,
        layer_index: usize,
        color: &ColorRGBA,
        tile_layer: &CMapItemLayerTilemap,
        _group: &CMapItemGroup,
        border_x0: i32,
        border_y0: i32,
        border_x1: i32,
        border_y1: i32,
        canvas_width_tile_count: i32,
        canvas_height_tile_count: i32,
    ) {
        let visuals = &map_visuals.tile_layer_visuals[layer_index];
        if let Some(buffer_container_index) = &visuals.buffer_object_index {
            let mut y0 = border_y0;
            let mut x0 = border_x0;
            let mut y1 = border_y1;
            let mut x1 = border_x1;

            let count_width = canvas_width_tile_count;
            let count_height = canvas_height_tile_count;

            if x0 < 1 {
                x0 = 1;
            }
            if y0 < 1 {
                y0 = 1;
            }
            if x1 >= tile_layer.width - 1 {
                x1 = tile_layer.width - 2;
            }
            if y1 >= tile_layer.height - 1 {
                y1 = tile_layer.height - 2;
            }

            if border_x0 <= 0 {
                // Draw corners on left side
                if border_y0 <= 0 {
                    if visuals.base.border_top_left.drawable() {
                        let mut offset = vec2::default();
                        offset.x = border_x0 as f32 * 32.0;
                        offset.y = border_y0 as f32 * 32.0;
                        let mut dir = vec2::default();
                        dir.x = 32.0;
                        dir.y = 32.0;

                        self.render_tile_border_corner_tiles(
                            state,
                            pipe,
                            (border_x0).abs() + 1,
                            (border_y0).abs() + 1,
                            count_width,
                            count_height,
                            buffer_container_index,
                            color,
                            visuals.base.border_top_left.index_buffer_offset_quad(),
                            &offset,
                            &dir,
                        );
                    }
                }
                if border_y1 >= tile_layer.height - 1 {
                    if visuals.base.border_bottom_left.drawable() {
                        let mut offset = vec2::default();
                        offset.x = border_x0 as f32 * 32.0;
                        offset.y = (border_y1 - (tile_layer.height - 1)) as f32 * 32.0;
                        let mut dir = vec2::default();
                        dir.x = 32.0;
                        dir.y = -32.0;

                        self.render_tile_border_corner_tiles(
                            state,
                            pipe,
                            (border_x0).abs() + 1,
                            (border_y1 - (tile_layer.height - 1)) + 1,
                            count_width,
                            count_height,
                            buffer_container_index,
                            color,
                            visuals.base.border_bottom_left.index_buffer_offset_quad(),
                            &offset,
                            &dir,
                        );
                    }
                }
            }
            if border_x0 < 0 {
                // Draw left border
                if y0 < tile_layer.height - 1 && y1 > 0 {
                    let draw_num = (((visuals.base.border_left[(y1 - 1) as usize]
                        .index_buffer_offset_quad()
                        - visuals.base.border_left[(y0 - 1) as usize].index_buffer_offset_quad())
                        / std::mem::size_of::<u32>())
                        + (if visuals.base.border_left[(y1 - 1) as usize].drawable() {
                            6
                        } else {
                            0
                        })) as usize;
                    let byte_offset =
                        visuals.base.border_left[(y0 - 1) as usize].index_buffer_offset_quad();
                    let mut offset = vec2::default();
                    offset.x = 32.0 * border_x0 as f32;
                    offset.y = 0.0;
                    let mut dir = vec2::default();
                    dir.x = 32.0;
                    dir.y = 0.0;
                    pipe.graphics.render_border_tile_lines(
                        state,
                        buffer_container_index,
                        color,
                        byte_offset,
                        &offset,
                        &dir,
                        draw_num,
                        border_x0.abs().min(count_width) as usize,
                    );
                }
            }

            if border_x1 >= tile_layer.width - 1 {
                // Draw corners on right side
                if border_y0 <= 0 {
                    if visuals.base.border_top_right.drawable() {
                        let mut offset = vec2::default();
                        offset.x = (border_x1 - (tile_layer.width - 1)) as f32 * 32.0;
                        offset.y = border_y0 as f32 * 32.0;
                        let mut dir = vec2::default();
                        dir.x = -32.0;
                        dir.y = 32.0;

                        self.render_tile_border_corner_tiles(
                            state,
                            pipe,
                            (border_x1 - (tile_layer.width - 1)) + 1,
                            (border_y0.abs()) + 1,
                            count_width,
                            count_height,
                            buffer_container_index,
                            color,
                            visuals.base.border_top_right.index_buffer_offset_quad(),
                            &offset,
                            &dir,
                        );
                    }
                }
                if border_y1 >= tile_layer.height - 1 {
                    if visuals.base.border_bottom_right.drawable() {
                        let mut offset = vec2::default();
                        offset.x = (border_x1 - (tile_layer.width - 1)) as f32 * 32.0;
                        offset.y = (border_y1 - (tile_layer.height - 1)) as f32 * 32.0;
                        let mut dir = vec2::default();
                        dir.x = -32.0;
                        dir.y = -32.0;

                        self.render_tile_border_corner_tiles(
                            state,
                            pipe,
                            (border_x1 - (tile_layer.width - 1)) + 1,
                            (border_y1 - (tile_layer.height - 1)) + 1,
                            count_width,
                            count_height,
                            buffer_container_index,
                            color,
                            visuals.base.border_bottom_right.index_buffer_offset_quad(),
                            &offset,
                            &dir,
                        );
                    }
                }
            }
            if border_x1 > tile_layer.width - 1 {
                // Draw right border
                if y0 < tile_layer.height - 1 && y1 > 0 {
                    let draw_num = ((visuals.base.border_right[(y1 - 1) as usize]
                        .index_buffer_offset_quad()
                        - visuals.base.border_right[(y0 - 1) as usize].index_buffer_offset_quad())
                        / std::mem::size_of::<u32>())
                        + (if visuals.base.border_right[(y1 - 1) as usize].drawable() {
                            6
                        } else {
                            0
                        });
                    let byte_offset =
                        visuals.base.border_right[(y0 - 1) as usize].index_buffer_offset_quad();
                    let mut offset = vec2::default();
                    offset.x = 32.0 * (border_x1 - (tile_layer.width - 1)) as f32;
                    offset.y = 0.0;
                    let mut dir = vec2::default();
                    dir.x = -32.0;
                    dir.y = 0.0;
                    pipe.graphics.render_border_tile_lines(
                        state,
                        buffer_container_index,
                        color,
                        byte_offset,
                        &offset,
                        &dir,
                        draw_num,
                        (border_x1 - (tile_layer.width - 1)).min(count_width) as usize,
                    );
                }
            }
            if border_y0 < 0 {
                // Draw top border
                if x0 < tile_layer.width - 1 && x1 > 0 {
                    let draw_num = ((visuals.base.border_top[(x1 - 1) as usize]
                        .index_buffer_offset_quad()
                        - visuals.base.border_top[(x0 - 1) as usize].index_buffer_offset_quad())
                        / std::mem::size_of::<u32>())
                        + (if visuals.base.border_top[(x1 - 1) as usize].drawable() {
                            6
                        } else {
                            0
                        });
                    let byte_offset =
                        visuals.base.border_top[(x0 - 1) as usize].index_buffer_offset_quad();
                    let mut offset = vec2::default();
                    offset.x = 0.0;
                    offset.y = 32.0 * border_y0 as f32;
                    let mut dir = vec2::default();
                    dir.x = 0.0;
                    dir.y = 32.0;
                    pipe.graphics.render_border_tile_lines(
                        state,
                        buffer_container_index,
                        color,
                        byte_offset,
                        &offset,
                        &dir,
                        draw_num,
                        (border_y0).abs().min(count_height) as usize,
                    );
                }
            }
            if border_y1 >= tile_layer.height {
                // Draw bottom border
                if x0 < tile_layer.width - 1 && x1 > 0 {
                    let draw_num = ((visuals.base.border_bottom[(x1 - 1) as usize]
                        .index_buffer_offset_quad()
                        - visuals.base.border_bottom[(x0 - 1) as usize]
                            .index_buffer_offset_quad())
                        / std::mem::size_of::<u32>())
                        + (if visuals.base.border_bottom[(x1 - 1) as usize].drawable() {
                            6
                        } else {
                            0
                        });
                    let byte_offset =
                        visuals.base.border_bottom[(x0 - 1) as usize].index_buffer_offset_quad();
                    let mut offset = vec2::default();
                    offset.x = 0.0;
                    offset.y = 32.0 * (border_y1 - (tile_layer.height - 1)) as f32;
                    let mut dir = vec2::default();
                    dir.x = 0.0;
                    dir.y = -32.0;
                    pipe.graphics.render_border_tile_lines(
                        state,
                        buffer_container_index,
                        color,
                        byte_offset,
                        &offset,
                        &dir,
                        draw_num,
                        (border_y1 - (tile_layer.height - 1)).min(count_height) as usize,
                    );
                }
            }
        }
    }

    fn render_kill_tile_border<B: GraphicsBackendInterface>(
        &self,
        state: &State,
        pipe: &mut RenderPipelineBase<B>,
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

            let mut border_y0 = (canvas_y0 / 32.0) as i32 - 1;
            let mut border_x0 = (canvas_x0 / 32.0) as i32 - 1;
            let mut border_y1 = (canvas_y1 / 32.0) as i32 + 1;
            let mut border_x1 = (canvas_x1 / 32.0) as i32 + 1;

            if border_x0 < -201 {
                draw_border = true;
            }
            if border_y0 < -201 {
                draw_border = true;
            }
            if border_x1 >= tile_layer.width + 201 {
                draw_border = true;
            }
            if border_y1 >= tile_layer.height + 201 {
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
                let mut dir = vec2::default();
                dir.x = 32.0;
                dir.y = 32.0;

                let count = ((border_x0).abs() - 201) * (border_y1 - border_y0);

                pipe.graphics.render_border_tiles(
                    state,
                    buffer_container_index,
                    color,
                    visuals.base.border_kill_tile.index_buffer_offset_quad(),
                    &offset,
                    &dir,
                    (border_x0).abs() - 201,
                    count as usize,
                );
            }
            // Draw top kill tile border
            if border_y0 < -201 {
                let mut offset = vec2::default();
                let mut off_x0 = if border_x0 < -201 { -201 } else { border_x0 };
                let mut off_x1 = if border_x1 >= tile_layer.width + 201 {
                    tile_layer.width + 201
                } else {
                    border_x1
                };
                off_x0 = off_x0.clamp(-201, tile_layer.width + 201);
                off_x1 = off_x1.clamp(-201, tile_layer.width + 201);
                offset.x = off_x0 as f32 * 32.0;
                offset.y = border_y0 as f32 * 32.0;
                let mut dir = vec2::default();
                dir.x = 32.0;
                dir.y = 32.0;

                let count = (off_x1 - off_x0) * ((border_y0).abs() - 201);

                pipe.graphics.render_border_tiles(
                    state,
                    buffer_container_index,
                    color,
                    visuals.base.border_kill_tile.index_buffer_offset_quad(),
                    &offset,
                    &dir,
                    off_x1 - off_x0,
                    count as usize,
                );
            }
            if border_x1 >= tile_layer.width + 201 {
                let mut offset = vec2::default();
                offset.x = (tile_layer.width + 201) as f32 * 32.0;
                offset.y = border_y0 as f32 * 32.0;
                let mut dir = vec2::default();
                dir.x = 32.0;
                dir.y = 32.0;

                let count = (border_x1 - (tile_layer.width + 201)) * (border_y1 - border_y0);

                pipe.graphics.render_border_tiles(
                    state,
                    buffer_container_index,
                    color,
                    visuals.base.border_kill_tile.index_buffer_offset_quad(),
                    &offset,
                    &dir,
                    border_x1 - (tile_layer.width + 201),
                    count as usize,
                );
            }
            if border_y1 >= tile_layer.height + 201 {
                let mut offset = vec2::default();
                let mut off_x0 = if border_x0 < -201 { -201 } else { border_x0 };
                let mut off_x1 = if border_x1 >= tile_layer.width + 201 {
                    tile_layer.width + 201
                } else {
                    border_x1
                };
                off_x0 = off_x0.clamp(-201, tile_layer.width + 201);
                off_x1 = off_x1.clamp(-201, tile_layer.width + 201);
                offset.x = off_x0 as f32 * 32.0;
                offset.y = (tile_layer.height + 201) as f32 * 32.0;
                let mut dir = vec2::default();
                dir.x = 32.0;
                dir.y = 32.0;

                let count = (off_x1 - off_x0) * (border_y1 - (tile_layer.height + 201));

                pipe.graphics.render_border_tiles(
                    state,
                    buffer_container_index,
                    color,
                    visuals.base.border_kill_tile.index_buffer_offset_quad(),
                    &offset,
                    &dir,
                    off_x1 - off_x0,
                    count as usize,
                );
            }
        }
    }

    fn render_quad_layer<B: GraphicsBackendInterface>(
        &self,
        state: &State,
        pipe: &mut RenderPipelineBase<B>,
        map_visuals: &ClientMapBufferedVisuals,
        layer_index: usize,
        quad_layer: &CMapItemLayerQuads,
        quads: &Vec<CQuad>,
        _group: &CMapItemGroup,
    ) {
        let visuals = &map_visuals.quad_layer_visuals[layer_index];
        if let Some(buffer_container_index) = &visuals.buffer_object_index {
            let mut quad_render_infos: PoolVec<SQuadRenderInfo> =
                pipe.graphics.quad_render_info_pool.new();

            quad_render_infos.resize(quad_layer.num_quads as usize, Default::default());
            let mut quad_render_count = 0;
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
                let needs_flush =
                    quad_render_count == GRAPHICS_MAX_QUADS_RENDER_COUNT || is_fully_transparent;

                if needs_flush {
                    // render quads of the current offset directly(cancel batching)
                    quad_render_infos.resize(quad_render_count, Default::default());
                    pipe.graphics.render_quad_layer(
                        &state,
                        buffer_container_index,
                        quad_render_infos,
                        quad_render_count,
                        cur_quad_offset,
                    );
                    quad_render_infos = pipe.graphics.quad_render_info_pool.new();
                    quad_render_infos.resize(quad_layer.num_quads as usize, Default::default());
                    quad_render_count = 0;
                    cur_quad_offset = i;
                    if is_fully_transparent {
                        // since this quad is ignored, the offset is the next quad
                        cur_quad_offset += 1;
                    }
                }

                if !is_fully_transparent {
                    let quad_info = &mut quad_render_infos[quad_render_count];
                    quad_render_count += 1;
                    quad_info.color = color;
                    quad_info.offsets.x = offset_x;
                    quad_info.offsets.y = offset_y;
                    quad_info.rotation = rot;
                }
            }
            quad_render_infos.resize(quad_render_count, Default::default());
            pipe.graphics.render_quad_layer(
                &state,
                buffer_container_index,
                quad_render_infos,
                quad_render_count,
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

    fn render_layer<B: GraphicsBackendInterface>(
        &self,
        pipe: &mut RenderPipelineBase<B>,
        map_visuals: &ClientMapBufferedVisuals,
        map_info: &ClientMapBufferedInfo,
        render_layer: &MapRenderLayer,
        is_background: bool,
        center: &vec2,
    ) {
        let entities = pipe
            .entities_container
            .get_or_default("TODO:", pipe.graphics);
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
                pipe.graphics,
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
                x0 * pipe.graphics.canvas_width() as f32,
                y0 * pipe.graphics.canvas_height() as f32,
                (x1 - x0) * pipe.graphics.canvas_width() as f32,
                (y1 - y0) * pipe.graphics.canvas_height() as f32,
            );

            state.clip_clamped(
                x,
                y,
                w,
                h,
                pipe.graphics.canvas_width(),
                pipe.graphics.canvas_height(),
            );
        }

        RenderTools::map_canvas_to_group(
            pipe.graphics,
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

            state.blend_normal();
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
                    pipe,
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
                state.blend_normal();
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

    fn render_impl<'a, B: GraphicsBackendInterface>(
        &self,
        map_visuals: &ClientMapBufferedVisuals,
        map_info: &ClientMapBufferedInfo,
        pipe: &mut RenderPipelineBase<B>,
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

    pub fn render_background<B: GraphicsBackendInterface>(&self, pipe: &mut RenderPipeline<B>) {
        self.render_impl(
            &pipe.buffered_map.visuals,
            &pipe.buffered_map.info,
            &mut pipe.base,
            pipe.buffered_map.render.background_render_layers.iter(),
            true,
        );
    }

    pub fn render_foreground<B: GraphicsBackendInterface>(&self, pipe: &mut RenderPipeline<B>) {
        self.render_impl(
            &pipe.buffered_map.visuals,
            &pipe.buffered_map.info,
            &mut pipe.base,
            pipe.buffered_map.render.foreground_render_layers.iter(),
            true,
        );
    }

    pub fn render_full<B: GraphicsBackendInterface>(&self, pipe: &mut RenderPipeline<B>) {
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
