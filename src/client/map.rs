use crate::client_map_buffered::ClientMapBuffered;

use pool::mt_datatypes::PoolVec;
use shared::game::state_wasm_manager::GameStateWasmManager;
use shared_base::{
    datafile::CDatafileWrapper,
    mapdef::{
        CEnvPoint, CMapItemGroup, CMapItemLayerQuads, CMapItemLayerTilemap, CQuad, CSpeedupTile,
        CSwitchTile, CTeleTile, CTile, CTuneTile, LayerFlag, MapLayer, MapLayerTile, MapLayerTypes,
    },
    types::GameTickType,
};

use shared_game::state::state::GameStateInterface;

use graphics_traits::GraphicsSizeQuery;
use math::math::{mix, vector::vec2, PI};

use base::system::SystemInterface;

use super::{
    render_pipe::RenderPipeline,
    render_tools::{LayerRenderFlag, RenderTools, TileRenderFlag},
};

use graphics_types::{
    command_buffer::{BufferContainerIndex, SQuadRenderInfo, GRAPHICS_MAX_QUADS_RENDER_COUNT},
    rendering::{ColorRGBA, State},
};

#[derive(PartialEq, PartialOrd)]
enum RenderMapTypes {
    Background = 0,
    BackgroundForced,
    Foreground,
    FullDesign,
    All = -1,
}

pub struct RenderMap {
    map_type: RenderMapTypes,
}

impl RenderMap {
    pub fn new() -> RenderMap {
        RenderMap {
            map_type: RenderMapTypes::FullDesign,
        }
    }

    fn envelope_eval(
        &self,
        map: &CDatafileWrapper,
        game: &GameStateWasmManager,
        sys: &dyn SystemInterface,
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
            std::time::Duration::from_secs(1).as_nanos() as u64 / game.game_tick_speed() as u64;

        // TODO: these were static before
        let mut total_time = std::time::Duration::from_nanos(0);
        let mut last_local_time = sys.time_get_nanoseconds();

        if map_item.version < 2 || map_item.synchronized > 0 {
            let cur_tick = game.cur_monotonic_tick();
            // get the lerp of the current tick and prev
            let min_tick = (cur_tick - 1) - animation_start_tick;
            let cur_tick = cur_tick - animation_start_tick;
            total_time = std::time::Duration::from_nanos(
                (mix::<f64, f64>(
                    &0.0,
                    &((cur_tick - min_tick) as f64),
                    game.intra_tick(sys, &cur_tick, animation_start_tick),
                ) * tick_to_nanoseconds as f64) as u64
                    + min_tick * tick_to_nanoseconds,
            );
        } else {
            let cur_time = sys.time_get_nanoseconds();
            total_time += cur_time - last_local_time;
            last_local_time = cur_time;
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
        buffered_map: &ClientMapBuffered,
        state: &State,
        pipe: &mut RenderPipeline,
        layer_index: usize,
        mut color: ColorRGBA,
        tile_layer: &CMapItemLayerTilemap,
        group: &CMapItemGroup,
    ) {
        let visuals = &buffered_map.tile_layer_visuals[layer_index];
        if let Some(buffer_container_index) = &visuals.buffer_container_index {
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
                    &pipe.camera.animation_start_tick,
                    tile_layer.color_env_offset,
                    tile_layer.color_env,
                    &mut channels,
                );
            }

            let border_x0;
            let border_y0;
            let border_x1;
            let border_y1;
            let mut draw_border = false;

            border_y0 = (screen_y0 / 32.0).floor() as i32;
            border_x0 = (screen_x0 / 32.0).floor() as i32;
            border_y1 = (screen_y1 / 32.0).floor() as i32;
            border_x1 = (screen_x1 / 32.0).floor() as i32;

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
                        .index_buffer_byte_offset()
                        < visuals.base.tiles_of_layer[(y * tile_layer.width + x0) as usize]
                            .index_buffer_byte_offset()
                    {
                        panic!("Tile count wrong.");
                    }

                    let num_vertices = ((visuals.base.tiles_of_layer
                        [(y * tile_layer.width + x1) as usize]
                        .index_buffer_byte_offset()
                        - visuals.base.tiles_of_layer[(y * tile_layer.width + x0) as usize]
                            .index_buffer_byte_offset())
                        / std::mem::size_of::<u32>())
                        + (if visuals.base.tiles_of_layer[(y * tile_layer.width + x1) as usize]
                            .do_draw()
                        {
                            6
                        } else {
                            0
                        });

                    if num_vertices > 0 {
                        index_offsets.push(
                            visuals.base.tiles_of_layer[(y * tile_layer.width + x0) as usize]
                                .index_buffer_byte_offset(),
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
                    buffered_map,
                    state,
                    pipe,
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

    fn render_tile_border_corner_tiles(
        &self,
        _buffered_map: &ClientMapBuffered,
        state: &State,
        pipe: &mut RenderPipeline,
        width_offset_to_origin: i32,
        height_offset_to_origin: i32,
        tile_count_width: i32,
        tile_count_height: i32,
        buffer_container_index: &BufferContainerIndex,
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
            buffer_container_index,
            color,
            index_buffer_offset,
            offset,
            dir,
            count_x,
            count,
        );
    }

    fn render_tile_border(
        &self,
        buffered_map: &ClientMapBuffered,
        state: &State,
        pipe: &mut RenderPipeline,
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
        let visuals = &buffered_map.tile_layer_visuals[layer_index];
        if let Some(buffer_container_index) = &visuals.buffer_container_index {
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
                    if visuals.base.border_top_left.do_draw() {
                        let mut offset = vec2::default();
                        offset.x = border_x0 as f32 * 32.0;
                        offset.y = border_y0 as f32 * 32.0;
                        let mut dir = vec2::default();
                        dir.x = 32.0;
                        dir.y = 32.0;

                        self.render_tile_border_corner_tiles(
                            buffered_map,
                            state,
                            pipe,
                            (border_x0).abs() + 1,
                            (border_y0).abs() + 1,
                            count_width,
                            count_height,
                            buffer_container_index,
                            color,
                            visuals.base.border_top_left.index_buffer_byte_offset(),
                            &offset,
                            &dir,
                        );
                    }
                }
                if border_y1 >= tile_layer.height - 1 {
                    if visuals.base.border_bottom_left.do_draw() {
                        let mut offset = vec2::default();
                        offset.x = border_x0 as f32 * 32.0;
                        offset.y = (border_y1 - (tile_layer.height - 1)) as f32 * 32.0;
                        let mut dir = vec2::default();
                        dir.x = 32.0;
                        dir.y = -32.0;

                        self.render_tile_border_corner_tiles(
                            buffered_map,
                            state,
                            pipe,
                            (border_x0).abs() + 1,
                            (border_y1 - (tile_layer.height - 1)) + 1,
                            count_width,
                            count_height,
                            buffer_container_index,
                            color,
                            visuals.base.border_bottom_left.index_buffer_byte_offset(),
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
                        .index_buffer_byte_offset()
                        - visuals.base.border_left[(y0 - 1) as usize].index_buffer_byte_offset())
                        / std::mem::size_of::<u32>())
                        + (if visuals.base.border_left[(y1 - 1) as usize].do_draw() {
                            6
                        } else {
                            0
                        })) as usize;
                    let byte_offset =
                        visuals.base.border_left[(y0 - 1) as usize].index_buffer_byte_offset();
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
                    if visuals.base.border_top_right.do_draw() {
                        let mut offset = vec2::default();
                        offset.x = (border_x1 - (tile_layer.width - 1)) as f32 * 32.0;
                        offset.y = border_y0 as f32 * 32.0;
                        let mut dir = vec2::default();
                        dir.x = -32.0;
                        dir.y = 32.0;

                        self.render_tile_border_corner_tiles(
                            buffered_map,
                            state,
                            pipe,
                            (border_x1 - (tile_layer.width - 1)) + 1,
                            (border_y0.abs()) + 1,
                            count_width,
                            count_height,
                            buffer_container_index,
                            color,
                            visuals.base.border_top_right.index_buffer_byte_offset(),
                            &offset,
                            &dir,
                        );
                    }
                }
                if border_y1 >= tile_layer.height - 1 {
                    if visuals.base.border_bottom_right.do_draw() {
                        let mut offset = vec2::default();
                        offset.x = (border_x1 - (tile_layer.width - 1)) as f32 * 32.0;
                        offset.y = (border_y1 - (tile_layer.height - 1)) as f32 * 32.0;
                        let mut dir = vec2::default();
                        dir.x = -32.0;
                        dir.y = -32.0;

                        self.render_tile_border_corner_tiles(
                            buffered_map,
                            state,
                            pipe,
                            (border_x1 - (tile_layer.width - 1)) + 1,
                            (border_y1 - (tile_layer.height - 1)) + 1,
                            count_width,
                            count_height,
                            buffer_container_index,
                            color,
                            visuals.base.border_bottom_right.index_buffer_byte_offset(),
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
                        .index_buffer_byte_offset()
                        - visuals.base.border_right[(y0 - 1) as usize].index_buffer_byte_offset())
                        / std::mem::size_of::<u32>())
                        + (if visuals.base.border_right[(y1 - 1) as usize].do_draw() {
                            6
                        } else {
                            0
                        });
                    let byte_offset =
                        visuals.base.border_right[(y0 - 1) as usize].index_buffer_byte_offset();
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
                        .index_buffer_byte_offset()
                        - visuals.base.border_top[(x0 - 1) as usize].index_buffer_byte_offset())
                        / std::mem::size_of::<u32>())
                        + (if visuals.base.border_top[(x1 - 1) as usize].do_draw() {
                            6
                        } else {
                            0
                        });
                    let byte_offset =
                        visuals.base.border_top[(x0 - 1) as usize].index_buffer_byte_offset();
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
                        .index_buffer_byte_offset()
                        - visuals.base.border_bottom[(x0 - 1) as usize]
                            .index_buffer_byte_offset())
                        / std::mem::size_of::<u32>())
                        + (if visuals.base.border_bottom[(x1 - 1) as usize].do_draw() {
                            6
                        } else {
                            0
                        });
                    let byte_offset =
                        visuals.base.border_bottom[(x0 - 1) as usize].index_buffer_byte_offset();
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

    fn render_kill_tile_border(
        &self,
        buffered_map: &ClientMapBuffered,
        state: &State,
        pipe: &mut RenderPipeline,
        layer_index: usize,
        color: &ColorRGBA,
        tile_layer: &CMapItemLayerTilemap,
        _group: &CMapItemGroup,
    ) {
        let visuals = &buffered_map.tile_layer_visuals[layer_index];
        if let Some(buffer_container_index) = &visuals.buffer_container_index {
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
            if !visuals.base.border_kill_tile.do_draw() {
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
                    visuals.base.border_kill_tile.index_buffer_byte_offset(),
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
                    visuals.base.border_kill_tile.index_buffer_byte_offset(),
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
                    visuals.base.border_kill_tile.index_buffer_byte_offset(),
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
                    visuals.base.border_kill_tile.index_buffer_byte_offset(),
                    &offset,
                    &dir,
                    off_x1 - off_x0,
                    count as usize,
                );
            }
        }
    }

    fn render_quad_layer(
        &self,
        buffered_map: &ClientMapBuffered,
        state: &State,
        pipe: &mut RenderPipeline,
        layer_index: usize,
        quad_layer: &CMapItemLayerQuads,
        quads: &Vec<CQuad>,
        _group: &CMapItemGroup,
        force: bool,
    ) {
        let visuals = &buffered_map.quad_layer_visuals[layer_index];
        if let Some(buffer_container_index) = &visuals.buffer_container_index {
            if !force
        // TODO: && (!g_Config.m_ClShowQuads || g_Config.m_ClOverlayEntities == 100)) {
            && false
            {
                return;
            }

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

    fn layers_of_group_count(
        &self,
        pipe: &mut RenderPipeline,
        group: &CMapItemGroup,
        tile_layer_count: &mut usize,
        quad_layer_count: &mut usize,
        passed_game_layer: &mut bool,
    ) {
        let mut tile_layer_counter = 0;
        let mut quad_layer_counter = 0;
        for l in 0..group.num_layers as usize {
            let layer_index = group.start_layer as usize + l;
            let layer = pipe.map.get_layer(layer_index);
            let mut is_front_layer = false;
            let mut is_switch_layer = false;
            let mut is_tele_layer = false;
            let mut is_speedup_layer = false;
            let mut is_tune_layer = false;

            if pipe.map.is_game_layer(layer_index) {
                *passed_game_layer = true;
            }

            if pipe.map.is_front_layer(layer_index) {
                is_front_layer = true;
            }

            if pipe.map.is_switch_layer(layer_index) {
                is_switch_layer = true;
            }

            if pipe.map.is_tele_layer(layer_index) {
                is_tele_layer = true;
            }

            if pipe.map.is_speedup_layer(layer_index) {
                is_speedup_layer = true;
            }

            if pipe.map.is_tune_layer(layer_index) {
                is_tune_layer = true;
            }

            /*if(m_Type <= TYPE_BACKGROUND_FORCE)
            {
                if(PassedGameLayer)
                    break;
            }
            else if(m_Type == TYPE_FOREGROUND)
            {
                if(!PassedGameLayer)
                    continue;
            }*/

            if let MapLayer::Tile(_) = layer {
                let tile_layer_and_overlay_count;
                if is_front_layer {
                    tile_layer_and_overlay_count = 1;
                } else if is_switch_layer {
                    tile_layer_and_overlay_count = 3;
                } else if is_tele_layer {
                    tile_layer_and_overlay_count = 2;
                } else if is_speedup_layer {
                    tile_layer_and_overlay_count = 3;
                } else if is_tune_layer {
                    tile_layer_and_overlay_count = 1;
                } else {
                    tile_layer_and_overlay_count = 1;
                }

                tile_layer_counter += tile_layer_and_overlay_count;
            } else if let MapLayer::Quads(_) = layer {
                quad_layer_counter += 1;
            }
        }

        *tile_layer_count += tile_layer_counter;
        *quad_layer_count += quad_layer_counter;
    }

    pub fn render(&self, pipe: &mut RenderPipeline) {
        // TODO if m_OnlineOnly && Client().State() != IClient::STATE_ONLINE && Client().State() != IClient::STATE_DEMOPLAYBACK)
        //	return;

        let center: vec2 = pipe.camera.pos;

        let mut passed_game_layer = false;
        let mut tile_layer_counter: usize = 0;
        let mut quad_layer_counter: usize = 0;

        let mut state = State::new();

        for g in 0..pipe.map.num_groups() as usize {
            let group = &pipe.map.get_group(g);
            let group_ex = None; //pipe.map.GetGroupEx(g);

            /* TODO filter group before? if pGroup.is_null()
            {
                dbg_msg("maplayers", "error group was null, group number = %d, total groups = %d", g, map.NumGroups());
                dbg_msg("maplayers", "this is here to prevent a crash but the source of this is unknown, please report this for it to get fixed");
                dbg_msg("maplayers", "we need mapname and crc and the map that caused this if possible, and anymore info you think is relevant");
                continue;
            }*/

            if (!pipe.config.gfx_no_clip || self.map_type == RenderMapTypes::FullDesign)
                && (*group).version >= 2
                && (*group).use_clipping > 0
            {
                // set clipping
                RenderTools::map_canvas_to_group(
                    &pipe.graphics,
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
                let x1 =
                    ((group.clip_x + group.clip_w) as f32 - points[0]) / (points[2] - points[0]);
                let y1 =
                    ((group.clip_y + group.clip_h) as f32 - points[1]) / (points[3] - points[1]);

                if x1 < 0.0 || x0 > 1.0 || y1 < 0.0 || y0 > 1.0 {
                    //check tile layer count of this group
                    self.layers_of_group_count(
                        pipe,
                        group,
                        &mut tile_layer_counter,
                        &mut quad_layer_counter,
                        &mut passed_game_layer,
                    );
                    continue;
                }

                let x = (x0 * pipe.graphics.canvas_width() as f32) as i32;
                let y = (y0 * pipe.graphics.canvas_height() as f32) as i32;
                let w = ((x1 - x0) * pipe.graphics.canvas_width() as f32) as u32;
                let h = ((y1 - y0) * pipe.graphics.canvas_height() as f32) as u32;

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
                &pipe.graphics,
                &mut state,
                center.x,
                center.y,
                group,
                group_ex,
                1.0, /* TODO Zoom */
            );

            for l in 0..group.num_layers as usize {
                let layer_index = group.start_layer as usize + l;
                let layer = pipe.map.get_layer(layer_index);
                let mut do_render = false;
                let mut is_game_layer = false;
                let mut is_front_layer = false;
                let mut is_switch_layer = false;
                let mut is_tele_layer = false;
                let mut is_speedup_layer = false;
                let mut is_tune_layer = false;
                let mut is_entity_layer = false;

                if pipe.map.is_game_layer(layer_index) {
                    is_entity_layer = true;
                    is_game_layer = true;
                    passed_game_layer = true;
                }

                if pipe.map.is_front_layer(layer_index) {
                    is_entity_layer = true;
                    is_front_layer = true;
                }

                if pipe.map.is_switch_layer(layer_index) {
                    is_entity_layer = true;
                    is_switch_layer = true;
                }

                if pipe.map.is_tele_layer(layer_index) {
                    is_entity_layer = true;
                    is_tele_layer = true;
                }

                if pipe.map.is_speedup_layer(layer_index) {
                    is_entity_layer = true;
                    is_speedup_layer = true;
                }

                if pipe.map.is_tune_layer(layer_index) {
                    is_entity_layer = true;
                    is_tune_layer = true;
                }

                if self.map_type == RenderMapTypes::All {
                    do_render = true;
                } else if self.map_type <= RenderMapTypes::BackgroundForced {
                    if passed_game_layer {
                        return;
                    }
                    do_render = true;

                    if self.map_type == RenderMapTypes::BackgroundForced {
                        if layer.get_tile_layer_base().item_layer == MapLayerTypes::Tiles as i32
                            && !pipe.config.cl_background_show_tile_layers
                        {
                            continue;
                        }
                    }
                } else if self.map_type == RenderMapTypes::Foreground {
                    if passed_game_layer && !is_game_layer {
                        do_render = true;
                    }
                } else if self.map_type == RenderMapTypes::FullDesign {
                    if !is_game_layer {
                        do_render = true;
                    }
                }

                /*
                if Render && pLayer.m_Type == LAYERTYPE_TILES && Input().ModifierIsPressed() && Input().ShiftIsPressed() && Input().KeyPress(KEY_KP_0))
                {
                    CMapItemLayerTilemap *pTMap = (CMapItemLayerTilemap *)pLayer;
                    CTile *pTiles = (CTile *)m_pLayers.Map().GetData(pTMap.m_Data);
                    CServerInfo CurrentServerInfo;
                    Client().GetServerInfo(&CurrentServerInfo);
                    char aFilename[IO_MAX_PATH_LENGTH];
                    str_format(aFilename, sizeof(aFilename), "dumps/tilelayer_dump_%s-%d-%d-%dx%d.txt", CurrentServerInfo.m_aMap, g, l, pTMap.m_Width, pTMap.m_Height);
                    IOHANDLE File = Storage().OpenFile(aFilename, IOFLAG_WRITE, IStorage::TYPE_SAVE);
                    if File)
                    {
                        for(int y = 0; y < pTMap.m_Height; y++)
                        {
                            for(int x = 0; x < pTMap.m_Width; x++)
                                io_write(File, &(pTiles[y * pTMap.m_Width + x].m_Index), sizeof(pTiles[y * pTMap.m_Width + x].m_Index));
                            io_write_newline(File);
                        }
                        io_close(File);
                    }
                }*/

                if let MapLayer::Tile(MapLayerTile(tile_layer, _, _)) = layer {
                    if do_render || is_game_layer {
                        let mut data_index: i32 = 0;
                        let mut tile_size: u32 = 0;
                        let mut tile_layer_and_overlay_count: usize = 0;
                        if is_front_layer {
                            data_index = tile_layer.front;
                            tile_size = std::mem::size_of::<CTile>() as u32;
                            tile_layer_and_overlay_count = 1;
                        } else if is_switch_layer {
                            data_index = tile_layer.switch;
                            tile_size = std::mem::size_of::<CSwitchTile>() as u32;
                            tile_layer_and_overlay_count = 3;
                        } else if is_tele_layer {
                            data_index = tile_layer.tele;
                            tile_size = std::mem::size_of::<CTeleTile>() as u32;
                            tile_layer_and_overlay_count = 2;
                        } else if is_speedup_layer {
                            data_index = tile_layer.speedup;
                            tile_size = std::mem::size_of::<CSpeedupTile>() as u32;
                            tile_layer_and_overlay_count = 3;
                        } else if is_tune_layer {
                            data_index = tile_layer.tune;
                            tile_size = std::mem::size_of::<CTuneTile>() as u32;
                            tile_layer_and_overlay_count = 1;
                        } else {
                            data_index = tile_layer.data;
                            tile_size = std::mem::size_of::<CTile>() as u32;
                            tile_layer_and_overlay_count = 1;
                        }

                        tile_layer_counter += tile_layer_and_overlay_count;
                    }
                } else if do_render
                    && layer.get_tile_layer_base().item_layer == MapLayerTypes::Quads as i32
                {
                    quad_layer_counter += 1;
                }

                // skip rendering if detail layers if not wanted, or is entity layer and we are a background map
                if ((layer.get_tile_layer_base().flags & LayerFlag::Detail as i32) != 0
                    && (!pipe.config.gfx_high_detail
                        && !(self.map_type == RenderMapTypes::FullDesign))
                    && !is_game_layer)
                    || (self.map_type == RenderMapTypes::BackgroundForced && is_entity_layer)
                    || (self.map_type == RenderMapTypes::FullDesign && is_entity_layer)
                {
                    continue;
                }

                let mut entity_overlay_val = pipe.config.cl_overlay_entities;
                if self.map_type == RenderMapTypes::FullDesign {
                    entity_overlay_val = 0;
                }

                if (do_render
                    && entity_overlay_val < 100
                    && !is_game_layer
                    && !is_front_layer
                    && !is_switch_layer
                    && !is_tele_layer
                    && !is_speedup_layer
                    && !is_tune_layer)
                    || (entity_overlay_val > 0 && is_game_layer)
                    || (self.map_type == RenderMapTypes::BackgroundForced)
                {
                    if let MapLayer::Tile(MapLayerTile(tile_layer, _, tiles)) = layer {
                        if tile_layer.image == -1 {
                            if !is_game_layer {
                                state.clear_texture();
                            } else {
                                // TODO pipe.graphics.texture_set(m_pImages.GetEntities(MAP_IMAGE_ENTITY_LAYER_TYPE_GAME));
                            }
                        } else {
                            state.set_texture(
                                pipe.map_images[tile_layer.image as usize]
                                    .texture_index_3d
                                    .as_ref()
                                    .unwrap(),
                            );
                        }
                        let tiles = tiles;
                        let mut color = ColorRGBA {
                            r: tile_layer.color.r() as f32 / 255.0,
                            g: tile_layer.color.g() as f32 / 255.0,
                            b: tile_layer.color.b() as f32 / 255.0,
                            a: tile_layer.color.a() as f32 / 255.0,
                        };
                        if is_game_layer && entity_overlay_val > 0 {
                            color = ColorRGBA {
                                r: tile_layer.color.r() as f32 / 255.0,
                                g: tile_layer.color.g() as f32 / 255.0,
                                b: tile_layer.color.b() as f32 / 255.0,
                                a: tile_layer.color.a() as f32 / 255.0 * entity_overlay_val as f32
                                    / 100.0,
                            };
                        } else if !is_game_layer
                            && entity_overlay_val > 0
                            && !(self.map_type == RenderMapTypes::BackgroundForced)
                        {
                            color = ColorRGBA {
                                r: tile_layer.color.r() as f32 / 255.0,
                                g: tile_layer.color.g() as f32 / 255.0,
                                b: tile_layer.color.b() as f32 / 255.0,
                                a: tile_layer.color.a() as f32 / 255.0
                                    * (100 - entity_overlay_val) as f32
                                    / 100.0,
                            };
                        }
                        if let Some(buffered_map) = pipe.buffered_map {
                            state.blend_normal();
                            // draw kill tiles outside the entity clipping rectangle
                            if is_game_layer {
                                // slow blinking to hint that it's not a part of the map
                                let seconds = pipe.sys.time_get_nanoseconds().as_secs_f64();
                                let color_hint = ColorRGBA {
                                    r: 1.0,
                                    g: 1.0,
                                    b: 1.0,
                                    a: 0.3
                                        + 0.7
                                            * (1.0
                                                + (2.0 * PI as f64 * seconds / 3.0).sin() as f32)
                                            / 2.0,
                                };

                                let color_kill = ColorRGBA {
                                    r: color.r * color_hint.r,
                                    g: color.g * color_hint.g,
                                    b: color.b * color_hint.b,
                                    a: color.a * color_hint.a,
                                };
                                self.render_kill_tile_border(
                                    buffered_map,
                                    &state,
                                    pipe,
                                    tile_layer_counter - 1,
                                    &color_kill,
                                    tile_layer,
                                    group,
                                );
                            }
                            self.render_tile_layer(
                                buffered_map,
                                &state,
                                pipe,
                                tile_layer_counter - 1,
                                color,
                                tile_layer,
                                group,
                            );
                        } else {
                            state.blend_normal();

                            // draw kill tiles outside the entity clipping rectangle
                            /*TODO if IsGameLayer
                            {
                                // slow blinking to hint that it's not a part of the map
                                double Seconds = time_get() / (double)time_freq();
                                ColorRGBA ColorHint = ColorRGBA(1.0, 1.0, 1.0, 0.3 + 0.7 * (1 + sin(2 * (double)pi * Seconds / 3)) / 2);

                                RenderTools().RenderTileRectangle(-201, -201, pTMap.m_Width + 402, pTMap.m_Height + 402,
                                    0, TILE_DEATH, // display air inside, death outside
                                    32.0, Color.v4() * ColorHint.v4(), TileRenderFlag::Extend | LAYERRENDERFLAG_TRANSPARENT,
                                    EnvelopeEval, this, pTMap.m_ColorEnv, pTMap.m_ColorEnvOffset);
                            }*/

                            RenderTools::render_tile_map(
                                pipe,
                                &state,
                                tiles,
                                tile_layer.width,
                                tile_layer.height,
                                32.0,
                                &color,
                                TileRenderFlag::Extend as i32 | LayerRenderFlag::Transparent as i32,
                                |pipe, time_offset_millis, env, channels| {
                                    self.envelope_eval(
                                        pipe.map,
                                        pipe.game,
                                        pipe.sys,
                                        &pipe.camera.animation_start_tick,
                                        time_offset_millis,
                                        env,
                                        channels,
                                    )
                                },
                                tile_layer.color_env,
                                tile_layer.color_env_offset,
                            );
                        }
                    } else if let MapLayer::Quads(quad_layer) = layer {
                        if quad_layer.0.image == -1 {
                            state.clear_texture();
                        } else {
                            state.set_texture(
                                pipe.map_images[quad_layer.0.image as usize]
                                    .texture_index
                                    .as_ref()
                                    .unwrap(),
                            );
                        }

                        let quads = &quad_layer.1;
                        if false
                        /*TODO: self.map_type == TYPE_BACKGROUND_FORCE
                        || self.map_type == TYPE_FULL_DESIGN*/
                        {
                            if false
                            /* TODO: g_Config.m_ClShowQuads || self.map_type == TYPE_FULL_DESIGN*/
                            {
                                if let Some(buffered_map) = pipe.buffered_map {
                                    state.blend_normal();
                                    self.render_quad_layer(
                                        buffered_map,
                                        &state,
                                        pipe,
                                        quad_layer_counter - 1,
                                        &quad_layer.0,
                                        &quad_layer.1,
                                        group,
                                        true,
                                    );
                                } else {
                                    state.blend_normal();
                                    RenderTools::force_render_quads(
                                        pipe,
                                        &state,
                                        quads,
                                        quad_layer.0.num_quads as usize,
                                        LayerRenderFlag::Transparent as i32,
                                        |map, game, sys, time_offset_millis, env, channels| {
                                            self.envelope_eval(
                                                map,
                                                game,
                                                sys,
                                                &pipe.camera.animation_start_tick,
                                                time_offset_millis,
                                                env,
                                                channels,
                                            )
                                        },
                                        1.0,
                                    );
                                }
                            }
                        } else {
                            if let Some(buffered_map) = pipe.buffered_map {
                                state.blend_normal();
                                self.render_quad_layer(
                                    buffered_map,
                                    &state,
                                    pipe,
                                    quad_layer_counter - 1,
                                    &quad_layer.0,
                                    &quad_layer.1,
                                    group,
                                    false,
                                );
                            } else {
                                state.blend_normal();
                                RenderTools::render_quads(
                                    pipe,
                                    &state,
                                    quads,
                                    quad_layer.0.num_quads as usize,
                                    LayerRenderFlag::Transparent as i32,
                                    |map, game, sys, time_offset_millis, env, channels| {
                                        self.envelope_eval(
                                            map,
                                            game,
                                            sys,
                                            &pipe.camera.animation_start_tick,
                                            time_offset_millis,
                                            env,
                                            channels,
                                        )
                                    },
                                    1.0, // TODO
                                );
                            }
                        }
                    }
                }
                /*else if Render && EntityOverlayVal && IsFrontLayer)
                   {
                       CMapItemLayerTilemap *pTMap = (CMapItemLayerTilemap *)pLayer;
                       Graphics().TextureSet(m_pImages.GetEntities(MAP_IMAGE_ENTITY_LAYER_TYPE_FRONT));

                       CTile *pFrontTiles = (CTile *)m_pLayers.Map().GetData(pTMap.m_Front);
                       u32 Size = m_pLayers.Map().GetDataSize(pTMap.m_Front);

                       if Size >= (size_t)pTMap.m_Width * pTMap.m_Height * sizeof(CTile))
                       {
                           ColorRGBA Color = ColorRGBA(pTMap.m_Color.r() / 255.0, pTMap.m_Color.g() / 255.0, pTMap.m_Color.b() / 255.0, pTMap.m_Color.a() / 255.0 * EntityOverlayVal / 100.0);
                           if !Graphics().IsTileBufferingEnabled())
                           {
                               Graphics().BlendNone();
                               RenderTools().RenderTilemap(pFrontTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LayerRenderFlag::Opaque,
                                   EnvelopeEval, this, pTMap.m_ColorEnv, pTMap.m_ColorEnvOffset);
                               Graphics().BlendNormal();
                               RenderTools().RenderTilemap(pFrontTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LAYERRENDERFLAG_TRANSPARENT,
                                   EnvelopeEval, this, pTMap.m_ColorEnv, pTMap.m_ColorEnvOffset);
                           }
                           else
                           {
                               Graphics().BlendNormal();
                               RenderTileLayer(TileLayerCounter - 1, Color, pTMap, pGroup);
                           }
                       }
                   }
                   else if Render && EntityOverlayVal && IsSwitchLayer)
                   {
                       CMapItemLayerTilemap *pTMap = (CMapItemLayerTilemap *)pLayer;
                       Graphics().TextureSet(m_pImages.GetEntities(MAP_IMAGE_ENTITY_LAYER_TYPE_SWITCH));

                       CSwitchTile *pSwitchTiles = (CSwitchTile *)m_pLayers.Map().GetData(pTMap.m_Switch);
                       u32 Size = m_pLayers.Map().GetDataSize(pTMap.m_Switch);

                       if Size >= (size_t)pTMap.m_Width * pTMap.m_Height * sizeof(CSwitchTile))
                       {
                           ColorRGBA Color = ColorRGBA(pTMap.m_Color.r() / 255.0, pTMap.m_Color.g() / 255.0, pTMap.m_Color.b() / 255.0, pTMap.m_Color.a() / 255.0 * EntityOverlayVal / 100.0);
                           if !Graphics().IsTileBufferingEnabled())
                           {
                               Graphics().BlendNone();
                               RenderTools().RenderSwitchmap(pSwitchTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LayerRenderFlag::Opaque);
                               Graphics().BlendNormal();
                               RenderTools().RenderSwitchmap(pSwitchTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LAYERRENDERFLAG_TRANSPARENT);
                               RenderTools().RenderSwitchOverlay(pSwitchTiles, pTMap.m_Width, pTMap.m_Height, 32.0, EntityOverlayVal / 100.0);
                           }
                           else
                           {
                               Graphics().BlendNormal();
                               RenderTileLayer(TileLayerCounter - 3, Color, pTMap, pGroup);
                               if g_Config.m_ClTextEntities)
                               {
                                   Graphics().TextureSet(m_pImages.GetOverlayBottom());
                                   RenderTileLayer(TileLayerCounter - 2, Color, pTMap, pGroup);
                                   Graphics().TextureSet(m_pImages.GetOverlayTop());
                                   RenderTileLayer(TileLayerCounter - 1, Color, pTMap, pGroup);
                               }
                           }
                       }
                   }
                   else if Render && EntityOverlayVal && IsTeleLayer)
                   {
                       CMapItemLayerTilemap *pTMap = (CMapItemLayerTilemap *)pLayer;
                       Graphics().TextureSet(m_pImages.GetEntities(MAP_IMAGE_ENTITY_LAYER_TYPE_TELE));

                       CTeleTile *pTeleTiles = (CTeleTile *)m_pLayers.Map().GetData(pTMap.m_Tele);
                       u32 Size = m_pLayers.Map().GetDataSize(pTMap.m_Tele);

                       if Size >= (size_t)pTMap.m_Width * pTMap.m_Height * sizeof(CTeleTile))
                       {
                           ColorRGBA Color = ColorRGBA(pTMap.m_Color.r() / 255.0, pTMap.m_Color.g() / 255.0, pTMap.m_Color.b() / 255.0, pTMap.m_Color.a() / 255.0 * EntityOverlayVal / 100.0);
                           if !Graphics().IsTileBufferingEnabled())
                           {
                               Graphics().BlendNone();
                               RenderTools().RenderTelemap(pTeleTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LayerRenderFlag::Opaque);
                               Graphics().BlendNormal();
                               RenderTools().RenderTelemap(pTeleTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LAYERRENDERFLAG_TRANSPARENT);
                               RenderTools().RenderTeleOverlay(pTeleTiles, pTMap.m_Width, pTMap.m_Height, 32.0, EntityOverlayVal / 100.0);
                           }
                           else
                           {
                               Graphics().BlendNormal();
                               RenderTileLayer(TileLayerCounter - 2, Color, pTMap, pGroup);
                               if g_Config.m_ClTextEntities)
                               {
                                   Graphics().TextureSet(m_pImages.GetOverlayCenter());
                                   RenderTileLayer(TileLayerCounter - 1, Color, pTMap, pGroup);
                               }
                           }
                       }
                   }
                   else if Render && EntityOverlayVal && IsSpeedupLayer)
                   {
                       CMapItemLayerTilemap *pTMap = (CMapItemLayerTilemap *)pLayer;
                       Graphics().TextureSet(m_pImages.GetEntities(MAP_IMAGE_ENTITY_LAYER_TYPE_SPEEDUP));

                       CSpeedupTile *pSpeedupTiles = (CSpeedupTile *)m_pLayers.Map().GetData(pTMap.m_Speedup);
                       u32 Size = m_pLayers.Map().GetDataSize(pTMap.m_Speedup);

                       if Size >= (size_t)pTMap.m_Width * pTMap.m_Height * sizeof(CSpeedupTile))
                       {
                           ColorRGBA Color = ColorRGBA(pTMap.m_Color.r() / 255.0, pTMap.m_Color.g() / 255.0, pTMap.m_Color.b() / 255.0, pTMap.m_Color.a() / 255.0 * EntityOverlayVal / 100.0);
                           if !Graphics().IsTileBufferingEnabled())
                           {
                               Graphics().BlendNone();
                               RenderTools().RenderSpeedupmap(pSpeedupTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LayerRenderFlag::Opaque);
                               Graphics().BlendNormal();
                               RenderTools().RenderSpeedupmap(pSpeedupTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LAYERRENDERFLAG_TRANSPARENT);
                               RenderTools().RenderSpeedupOverlay(pSpeedupTiles, pTMap.m_Width, pTMap.m_Height, 32.0, EntityOverlayVal / 100.0);
                           }
                           else
                           {
                               Graphics().BlendNormal();

                               // draw arrow -- clamp to the edge of the arrow image
                               Graphics().WrapClamp();
                               Graphics().TextureSet(m_pImages.GetSpeedupArrow());
                               RenderTileLayer(TileLayerCounter - 3, Color, pTMap, pGroup);
                               Graphics().WrapNormal();
                               if g_Config.m_ClTextEntities)
                               {
                                   Graphics().TextureSet(m_pImages.GetOverlayBottom());
                                   RenderTileLayer(TileLayerCounter - 2, Color, pTMap, pGroup);
                                   Graphics().TextureSet(m_pImages.GetOverlayTop());
                                   RenderTileLayer(TileLayerCounter - 1, Color, pTMap, pGroup);
                               }
                           }
                       }
                   }
                   else if Render && EntityOverlayVal && IsTuneLayer)
                   {
                       CMapItemLayerTilemap *pTMap = (CMapItemLayerTilemap *)pLayer;
                       Graphics().TextureSet(m_pImages.GetEntities(MAP_IMAGE_ENTITY_LAYER_TYPE_TUNE));

                       CTuneTile *pTuneTiles = (CTuneTile *)m_pLayers.Map().GetData(pTMap.m_Tune);
                       u32 Size = m_pLayers.Map().GetDataSize(pTMap.m_Tune);

                       if Size >= (size_t)pTMap.m_Width * pTMap.m_Height * sizeof(CTuneTile))
                       {
                           ColorRGBA Color = ColorRGBA(pTMap.m_Color.r() / 255.0, pTMap.m_Color.g() / 255.0, pTMap.m_Color.b() / 255.0, pTMap.m_Color.a() / 255.0 * EntityOverlayVal / 100.0);
                           if !Graphics().IsTileBufferingEnabled())
                           {
                               Graphics().BlendNone();
                               RenderTools().RenderTunemap(pTuneTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LayerRenderFlag::Opaque);
                               Graphics().BlendNormal();
                               RenderTools().RenderTunemap(pTuneTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LAYERRENDERFLAG_TRANSPARENT);
                               //RenderTools().RenderTuneOverlay(pTuneTiles, pTMap.m_Width, pTMap.m_Height, 32.0, EntityOverlayVal/100.0);
                           }
                           else
                           {
                               Graphics().BlendNormal();
                               RenderTileLayer(TileLayerCounter - 1, Color, pTMap, pGroup);
                           }
                       }
                   }
                */
            }

            /*
            if !g_Config.m_GfxNoclip || self.map_type == TYPE_FULL_DESIGN)
                Graphics().ClipDisable();
             */
        }
        /*
        if !g_Config.m_GfxNoclip || self.map_type == TYPE_FULL_DESIGN)
            Graphics().ClipDisable();*/
    }
}
