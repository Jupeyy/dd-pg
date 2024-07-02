#![warn(clippy::all)]

use std::{borrow::Borrow, cell::Cell, fmt::Debug, ops::IndexMut, time::Duration};

use crate::map::map_buffered::{MapRenderLayer, MapRenderTextOverlayType};

use super::{
    map_buffered::{
        MapPhysicsRenderInfo, PhysicsTileLayerVisuals, QuadLayerVisuals, TileLayerVisuals,
        TileLayerVisualsBase,
    },
    map_pipeline::{MapGraphics, QuadRenderInfo},
    map_sound::MapSoundProcess,
    map_with_visual::{MapVisual, MapVisualLayerBase},
    render_pipe::{Camera, GameStateRenderInfo, RenderPipeline, RenderPipelineBase},
    render_tools::RenderTools,
};
use client_containers_new::entities::{Entities, EntitiesContainer};
use fixed::traits::{FromFixed, ToFixed};
use game_config::config::ConfigMap;
use game_interface::types::game::GameTickType;
use graphics::handles::{
    backend::backend::GraphicsBackendHandle,
    buffer_object::buffer_object::BufferObject,
    canvas::canvas::GraphicsCanvasHandle,
    stream::stream::{GraphicsStreamHandle, StreamedUniforms},
    texture::texture::{
        TextureContainer, TextureContainer2dArray, TextureType, TextureType2dArray,
    },
};
use hiarc::HiarcTrait;
use hiarc::{hi_closure, Hiarc};
use map::{
    map::{
        animations::{AnimBase, AnimPoint},
        groups::{
            layers::design::{Quad, SoundShape},
            MapGroupAttr,
        },
    },
    skeleton::{
        animations::AnimationsSkeleton, groups::layers::physics::MapLayerPhysicsSkeleton,
        resources::MapResourcesSkeleton,
    },
};
use pool::mixed_pool::Pool;
use serde::de::DeserializeOwned;

use math::math::{
    mix,
    vector::{ffixed, nffixed, nfvec4, ubvec4, vec2},
    PI,
};

use graphics_types::rendering::{BlendType, ColorRGBA, State};
use sound::sound_object::SoundObject;

#[derive(Debug, Clone, Copy)]
pub enum RenderLayerType {
    Background,
    Foreground,
}

pub enum ForcedTexture<'a> {
    TileLayer(&'a TextureContainer2dArray),
    QuadLayer(&'a TextureContainer),
}

#[derive(Debug, Hiarc)]
pub struct RenderMap {
    map_graphics: MapGraphics,

    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,

    index_offset_or_draw_count_pool: Pool<Vec<usize>>,

    // sound, handled here because it's such an integral part of the map
    pub sound: MapSoundProcess,
}

impl RenderMap {
    pub fn new(
        backend_handle: &GraphicsBackendHandle,
        canvas_handle: &GraphicsCanvasHandle,
        stream_handle: &GraphicsStreamHandle,
    ) -> RenderMap {
        let (index_offset_or_draw_count_pool, sync_point) =
            Pool::with_sized(64 * 2, || Vec::with_capacity(128));
        backend_handle.add_sync_point(sync_point);
        RenderMap {
            map_graphics: MapGraphics::new(backend_handle),

            canvas_handle: canvas_handle.clone(),
            stream_handle: stream_handle.clone(),

            index_offset_or_draw_count_pool,

            sound: MapSoundProcess::new(),
        }
    }

    fn animation_eval<
        F,
        T: DeserializeOwned + Debug + Copy + Default + IndexMut<usize, Output = F>,
    >(
        anim: &AnimBase<AnimPoint<T>>,
        channels: usize,
        game: &GameStateRenderInfo,
        cur_time: &Duration,
        intra_tick_time: &Duration,
        animation_ticks_passed: &GameTickType,
        anim_time_offset: &time::Duration,
    ) -> T
    where
        F: Copy + FromFixed + ToFixed,
    {
        let tick_to_nanoseconds =
            (time::Duration::seconds(1).whole_nanoseconds() / game.ticks_per_second as i128) as i64;

        let mut total_time = time::Duration::try_from(*cur_time).unwrap_or_default();

        if anim.synchronized {
            // get the lerp of the current tick and prev
            let min_tick = animation_ticks_passed.saturating_sub(1);
            let cur_tick = *animation_ticks_passed;
            total_time = time::Duration::nanoseconds(
                (mix::<f64, f64>(
                    &0.0,
                    &((cur_tick - min_tick) as f64),
                    intra_tick_time.as_secs_f64(),
                ) * tick_to_nanoseconds as f64) as i64
                    + min_tick as i64 * tick_to_nanoseconds,
            );
        }
        let anim_time = total_time + *anim_time_offset;

        RenderTools::render_eval_anim(&anim.points, anim_time, channels)
    }

    fn render_tile_layer<AN, AS>(
        &self,
        state: &State,
        texture: TextureType2dArray,
        game: &GameStateRenderInfo,
        cur_time: &Duration,
        animation_start_tick: u64,
        visuals: &TileLayerVisualsBase,
        buffer_object_index: &Option<BufferObject>,
        color_anim: &Option<usize>,
        color_anim_offset: &time::Duration,
        animations: &AnimationsSkeleton<AN, AS>,
        mut color: ColorRGBA,
    ) {
        if let Some(buffer_container_index) = buffer_object_index {
            let (screen_x0, screen_y0, screen_x1, screen_y1) = state.get_canvas_mapping();

            let channels = if let Some(anim) = {
                if let Some(color_anim) = color_anim {
                    animations.color.get(*color_anim)
                } else {
                    None
                }
            } {
                Self::animation_eval(
                    &anim.def,
                    4,
                    game,
                    cur_time,
                    &game.intra_tick_time,
                    &animation_start_tick,
                    color_anim_offset,
                )
            } else {
                nfvec4::new(
                    nffixed::from_num(1),
                    nffixed::from_num(1),
                    nffixed::from_num(1),
                    nffixed::from_num(1),
                )
            };

            let mut draw_border = false;

            let border_y0 = (screen_y0).floor() as i32;
            let border_x0 = (screen_x0).floor() as i32;
            let border_y1 = (screen_y1).ceil() as i32;
            let border_x1 = (screen_x1).ceil() as i32;

            let mut y0 = border_y0;
            let mut x0 = border_x0;
            let mut y1 = border_y1;
            let mut x1 = border_x1;

            let (width, height) = (visuals.width as i32, visuals.height as i32);

            if x0 < 0 {
                x0 = 0;
                draw_border = true;
            }
            if y0 < 0 {
                y0 = 0;
                draw_border = true;
            }
            if x1 > width {
                x1 = width;
                draw_border = true;
            }
            if y1 > height {
                y1 = height;
                draw_border = true;
            }

            let mut draw_layer = true;
            if x1 <= 0 {
                draw_layer = false;
            }
            if y1 <= 0 {
                draw_layer = false;
            }
            if x0 >= width {
                draw_layer = false;
            }
            if y0 >= height {
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

                    if visuals.tiles_of_layer[(y * width + xr) as usize].index_buffer_offset_quad()
                        < visuals.tiles_of_layer[(y * width + x0) as usize]
                            .index_buffer_offset_quad()
                    {
                        panic!("Tile count wrong.");
                    }

                    let num_vertices = ((visuals.tiles_of_layer[(y * width + xr) as usize]
                        .index_buffer_offset_quad()
                        - visuals.tiles_of_layer[(y * width + x0) as usize]
                            .index_buffer_offset_quad())
                        / std::mem::size_of::<u32>())
                        + (if visuals.tiles_of_layer[(y * width + xr) as usize].drawable() {
                            6
                        } else {
                            0
                        });

                    if num_vertices > 0 {
                        index_offsets.push(
                            visuals.tiles_of_layer[(y * width + x0) as usize]
                                .index_buffer_offset_quad(),
                        );
                        draw_counts.push(num_vertices);
                    }
                }

                color.r *= channels.r().to_num::<f32>();
                color.g *= channels.g().to_num::<f32>();
                color.b *= channels.b().to_num::<f32>();
                color.a *= channels.a().to_num::<f32>();

                let draw_count = index_offsets.len();
                if draw_count != 0 {
                    self.map_graphics.render_tile_layer(
                        state,
                        texture.clone(),
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
                    texture.clone(),
                    visuals,
                    buffer_object_index,
                    &color,
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
        texture: TextureType2dArray,
        visuals: &TileLayerVisualsBase,
        buffer_object_index: &Option<BufferObject>,
        color: &ColorRGBA,
        border_x0: i32,
        border_y0: i32,
        border_x1: i32,
        border_y1: i32,
    ) {
        if let Some(buffer_container_index) = &buffer_object_index {
            let mut y0 = border_y0;
            let mut x0 = border_x0;
            let mut y1 = border_y1;
            let mut x1 = border_x1;

            let (width, height) = (visuals.width as i32, visuals.height as i32);

            if x0 < 0 {
                x0 = 0;
            }
            if y0 < 0 {
                y0 = 0;
            }
            if x1 > width {
                x1 = width;
            }
            if y1 > height {
                y1 = height;
            }

            if border_x0 < 0 {
                // Draw corners on left side
                if border_y0 < 0 {
                    if visuals.corner_top_left.drawable() {
                        let mut offset = vec2::default();
                        offset.x = 0.0;
                        offset.y = 0.0;
                        let mut scale = vec2::default();
                        scale.x = border_x0.abs() as f32;
                        scale.y = border_y0.abs() as f32;

                        self.map_graphics.render_border_tiles(
                            state,
                            texture.clone(),
                            buffer_container_index,
                            visuals.buffer_size_all_tiles,
                            color,
                            visuals.corner_top_left.index_buffer_offset_quad(),
                            &offset,
                            &scale,
                            1,
                        );
                    }
                }
                if border_y1 > height {
                    if visuals.corner_bottom_left.drawable() {
                        let mut offset = vec2::default();
                        offset.x = 0.0;
                        offset.y = height as f32;
                        let mut scale = vec2::default();
                        scale.x = border_x0.abs() as f32;
                        scale.y = (border_y1 - height) as f32;

                        self.map_graphics.render_border_tiles(
                            state,
                            texture.clone(),
                            buffer_container_index,
                            visuals.buffer_size_all_tiles,
                            color,
                            visuals.corner_bottom_left.index_buffer_offset_quad(),
                            &offset,
                            &scale,
                            1,
                        );
                    }
                }
            }
            if border_x1 > width {
                // Draw corners on right side
                if border_y0 < 0 {
                    if visuals.corner_top_right.drawable() {
                        let mut offset = vec2::default();
                        offset.x = width as f32;
                        offset.y = 0.0;
                        let mut scale = vec2::default();
                        scale.x = (border_x1 - width) as f32;
                        scale.y = border_y0.abs() as f32;

                        self.map_graphics.render_border_tiles(
                            state,
                            texture.clone(),
                            buffer_container_index,
                            visuals.buffer_size_all_tiles,
                            color,
                            visuals.corner_top_right.index_buffer_offset_quad(),
                            &offset,
                            &scale,
                            1,
                        );
                    }
                }
                if border_y1 > height {
                    if visuals.corner_bottom_right.drawable() {
                        let mut offset = vec2::default();
                        offset.x = width as f32;
                        offset.y = height as f32;
                        let mut scale = vec2::default();
                        scale.x = (border_x1 - width) as f32;
                        scale.y = (border_y1 - height) as f32;

                        self.map_graphics.render_border_tiles(
                            state,
                            texture.clone(),
                            buffer_container_index,
                            visuals.buffer_size_all_tiles,
                            color,
                            visuals.corner_bottom_right.index_buffer_offset_quad(),
                            &offset,
                            &scale,
                            1,
                        );
                    }
                }
            }
            if border_x1 > width {
                // Draw right border
                if y0 < height && y1 > 0 {
                    let yb = y1 - 1;
                    let draw_num = ((visuals.border_right[yb as usize].index_buffer_offset_quad()
                        - visuals.border_right[y0 as usize].index_buffer_offset_quad())
                        / (std::mem::size_of::<u32>() * 6))
                        + (if visuals.border_right[yb as usize].drawable() {
                            1
                        } else {
                            0
                        });
                    let byte_offset = visuals.border_right[y0 as usize].index_buffer_offset_quad();
                    let mut offset = vec2::default();
                    offset.x = width as f32;
                    offset.y = 0.0;
                    let mut scale = vec2::default();
                    scale.x = (border_x1 - width) as f32;
                    scale.y = 1.0;

                    self.map_graphics.render_border_tiles(
                        state,
                        texture.clone(),
                        buffer_container_index,
                        visuals.buffer_size_all_tiles,
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
                if y0 < height && y1 > 0 {
                    let yb = y1 - 1;
                    let draw_num = ((visuals.border_left[yb as usize].index_buffer_offset_quad()
                        - visuals.border_left[y0 as usize].index_buffer_offset_quad())
                        / (std::mem::size_of::<u32>() * 6))
                        + (if visuals.border_left[yb as usize].drawable() {
                            1
                        } else {
                            0
                        });
                    let byte_offset = visuals.border_left[y0 as usize].index_buffer_offset_quad();
                    let mut offset = vec2::default();
                    offset.x = 0.0;
                    offset.y = 0.0;
                    let mut scale = vec2::default();
                    scale.x = (border_x0).abs() as f32;
                    scale.y = 1.0;

                    self.map_graphics.render_border_tiles(
                        state,
                        texture.clone(),
                        buffer_container_index,
                        visuals.buffer_size_all_tiles,
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
                if x0 < width && x1 > 0 {
                    let xr = x1 - 1;
                    let draw_num = ((visuals.border_top[xr as usize].index_buffer_offset_quad()
                        - visuals.border_top[x0 as usize].index_buffer_offset_quad())
                        / (std::mem::size_of::<u32>() * 6))
                        + (if visuals.border_top[xr as usize].drawable() {
                            1
                        } else {
                            0
                        });
                    let byte_offset = visuals.border_top[x0 as usize].index_buffer_offset_quad();
                    let mut offset = vec2::default();
                    offset.x = 0.0;
                    offset.y = 0.0;
                    let mut scale = vec2::default();
                    scale.x = 1.0;
                    scale.y = border_y0.abs() as f32;

                    self.map_graphics.render_border_tiles(
                        state,
                        texture.clone(),
                        buffer_container_index,
                        visuals.buffer_size_all_tiles,
                        color,
                        byte_offset,
                        &offset,
                        &scale,
                        draw_num,
                    );
                }
            }
            if border_y1 > height {
                // Draw bottom border
                if x0 < width && x1 > 0 {
                    let xr = x1 - 1;
                    let draw_num = ((visuals.border_bottom[xr as usize]
                        .index_buffer_offset_quad()
                        - visuals.border_bottom[x0 as usize].index_buffer_offset_quad())
                        / (std::mem::size_of::<u32>() * 6))
                        + (if visuals.border_bottom[xr as usize].drawable() {
                            1
                        } else {
                            0
                        });
                    let byte_offset = visuals.border_bottom[x0 as usize].index_buffer_offset_quad();
                    let mut offset = vec2::default();
                    offset.x = 0.0;
                    offset.y = height as f32;
                    let mut scale = vec2::default();
                    scale.x = 1.0;
                    scale.y = (border_y1 - height) as f32;

                    self.map_graphics.render_border_tiles(
                        state,
                        texture.clone(),
                        buffer_container_index,
                        visuals.buffer_size_all_tiles,
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
        texture: TextureType2dArray,
        visuals: &TileLayerVisuals,
        color: &ColorRGBA,
    ) {
        if let Some(buffer_container_index) = &visuals.buffer_object_index {
            let (canvas_x0, canvas_y0, canvas_x1, canvas_y1) = state.get_canvas_mapping();

            let mut draw_border = false;

            let mut border_y0 = (canvas_y0).floor() as i32;
            let mut border_x0 = (canvas_x0).floor() as i32;
            let mut border_y1 = (canvas_y1).ceil() as i32;
            let mut border_x1 = (canvas_x1).ceil() as i32;

            let (width, height) = (visuals.base.width as i32, visuals.base.height as i32);

            if border_x0 < -201 {
                draw_border = true;
            }
            if border_y0 < -201 {
                draw_border = true;
            }
            if border_x1 > width + 201 {
                draw_border = true;
            }
            if border_y1 > height + 201 {
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
            if border_x1 >= width + 300 {
                border_x1 = width + 299;
            }
            if border_y1 >= height + 300 {
                border_y1 = height + 299;
            }

            if border_x1 < -300 {
                border_x1 = -300;
            }
            if border_y1 < -300 {
                border_y1 = -300;
            }
            if border_x0 >= width + 300 {
                border_x0 = width + 299;
            }
            if border_y0 >= height + 300 {
                border_y0 = height + 299;
            }

            // Draw left kill tile border
            if border_x0 < -201 {
                let mut offset = vec2::default();
                offset.x = border_x0 as f32;
                offset.y = border_y0 as f32;
                let mut scale = vec2::default();
                scale.x = (-201 - border_x0) as f32;
                scale.y = (border_y1 - border_y0) as f32;
                self.map_graphics.render_border_tiles(
                    state,
                    texture.clone(),
                    buffer_container_index,
                    visuals.base.buffer_size_all_tiles,
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
                offset.x = border_x0.max(-201) as f32;
                offset.y = border_y0 as f32;
                let mut scale = vec2::default();
                scale.x = (border_x1.min(width + 201) - border_x0.max(-201)) as f32;
                scale.y = (-201 - border_y0) as f32;
                self.map_graphics.render_border_tiles(
                    state,
                    texture.clone(),
                    buffer_container_index,
                    visuals.base.buffer_size_all_tiles,
                    color,
                    visuals.base.border_kill_tile.index_buffer_offset_quad(),
                    &offset,
                    &scale,
                    1,
                );
            }
            // Draw right kill tile border
            if border_x1 > width + 201 {
                let mut offset = vec2::default();
                offset.x = (width + 201) as f32;
                offset.y = border_y0 as f32;
                let mut scale = vec2::default();
                scale.x = (border_x1 - (width + 201)) as f32;
                scale.y = (border_y1 - border_y0) as f32;
                self.map_graphics.render_border_tiles(
                    state,
                    texture.clone(),
                    buffer_container_index,
                    visuals.base.buffer_size_all_tiles,
                    color,
                    visuals.base.border_kill_tile.index_buffer_offset_quad(),
                    &offset,
                    &scale,
                    1,
                );
            }
            // Draw bottom kill tile border
            if border_y1 > height + 201 {
                let mut offset = vec2::default();
                offset.x = border_x0.max(-201) as f32;
                offset.y = (height + 201) as f32;
                let mut scale = vec2::default();
                scale.x = (border_x1.min(width + 201) - border_x0.max(-201)) as f32;
                scale.y = (border_y1 - (height + 201)) as f32;
                self.map_graphics.render_border_tiles(
                    state,
                    texture,
                    buffer_container_index,
                    visuals.base.buffer_size_all_tiles,
                    color,
                    visuals.base.border_kill_tile.index_buffer_offset_quad(),
                    &offset,
                    &scale,
                    1,
                );
            }
        }
    }

    pub fn prepare_quad_rendering<AN, AS>(
        mut stream_handle: StreamedUniforms<'_, QuadRenderInfo>,
        game: &GameStateRenderInfo,
        cur_time: &Duration,
        camera: &Camera,
        cur_quad_offset: &Cell<usize>,
        animations: &AnimationsSkeleton<AN, AS>,
        quads: &Vec<Quad>,
    ) {
        for i in 0..quads.len() as usize {
            let quad = &quads[i];

            let color = if let Some(anim) = {
                if let Some(color_anim) = quad.color_anim {
                    animations.color.get(color_anim)
                } else {
                    None
                }
            } {
                RenderMap::animation_eval(
                    &anim.def,
                    4,
                    game,
                    cur_time,
                    &game.intra_tick_time,
                    &camera.animation_ticks_passed,
                    &quad.color_anim_offset,
                )
            } else {
                nfvec4::new(
                    nffixed::from_num(1),
                    nffixed::from_num(1),
                    nffixed::from_num(1),
                    nffixed::from_num(1),
                )
            };

            let mut offset_x = 0.0;
            let mut offset_y = 0.0;
            let mut rot = 0.0;

            if let Some(anim) = {
                if let Some(pos_anim) = quad.pos_anim {
                    animations.pos.get(pos_anim)
                } else {
                    None
                }
            } {
                let pos_channels = RenderMap::animation_eval(
                    &anim.def,
                    3,
                    game,
                    cur_time,
                    &game.intra_tick_time,
                    &camera.animation_ticks_passed,
                    &quad.pos_anim_offset,
                );
                offset_x = pos_channels.x.to_num();
                offset_y = pos_channels.y.to_num();
                rot = pos_channels.z.to_num::<f32>() / 180.0 * PI;
            }

            let is_fully_transparent = color.a() <= 0;
            let needs_flush = is_fully_transparent;

            if needs_flush {
                stream_handle.flush();

                cur_quad_offset.set(i);
                if is_fully_transparent {
                    // since this quad is ignored, the offset is the next quad
                    cur_quad_offset.set(cur_quad_offset.get() + 1);
                }
            }

            if !is_fully_transparent {
                stream_handle.add(QuadRenderInfo::new(
                    ColorRGBA {
                        r: color.r().to_num(),
                        g: color.g().to_num(),
                        b: color.b().to_num(),
                        a: color.a().to_num(),
                    },
                    vec2::new(offset_x, offset_y),
                    rot,
                ));
            }
        }
    }

    fn render_quad_layer<AN: HiarcTrait, AS: HiarcTrait>(
        &self,
        state: &State,
        texture: TextureType,
        game: &GameStateRenderInfo,
        cur_time: &Duration,
        camera: &Camera,
        visuals: &QuadLayerVisuals,
        animations: &AnimationsSkeleton<AN, AS>,
        quads: &Vec<Quad>,
    ) {
        if let Some(buffer_container_index) = &visuals.buffer_object_index {
            let map_graphics = &self.map_graphics;
            let texture = &texture;
            let cur_quad_offset_cell = Cell::new(0);
            let cur_quad_offset = &cur_quad_offset_cell;
            self.stream_handle.fill_uniform_instance(
                    hi_closure!(
                        <AN, AS>,
                        [
                            game: &GameStateRenderInfo,
                            cur_time: &Duration,
                            camera: &Camera,
                            cur_quad_offset: &Cell<usize>,
                            animations: &AnimationsSkeleton<AN, AS>,
                            quads: &Vec<Quad>,
                        ],
                    |stream_handle: StreamedUniforms<
                        '_,
                        QuadRenderInfo,
                    >|
                     -> () {
                        RenderMap::prepare_quad_rendering(
                            stream_handle,
                            game,
                            cur_time,
                            camera,
                            cur_quad_offset,
                            animations,
                            quads
                        );
                     }),
                    hi_closure!([map_graphics: &MapGraphics, state: &State, texture: &TextureType, buffer_container_index: &BufferObject, cur_quad_offset: &Cell<usize>], |instance: usize, count: usize| -> () {
                        map_graphics.render_quad_layer(
                            state,
                            texture.clone(),
                            buffer_container_index,
                            instance,
                            count,
                            cur_quad_offset.get(),
                        );
                    }),
                );
        }
    }

    fn get_physics_layer_texture<'a, L>(
        layer: &MapLayerPhysicsSkeleton<L>,
        entities: &'a Entities,
    ) -> &'a TextureContainer2dArray {
        match layer {
            _ => &entities.physics,
        }
    }

    pub fn render_layer<T, Q, AN: HiarcTrait, AS: HiarcTrait, S, A>(
        &self,
        animations: &AnimationsSkeleton<AN, AS>,
        resources: &MapResourcesSkeleton<
            (),
            impl Borrow<TextureContainer>,
            impl Borrow<TextureContainer2dArray>,
            impl Borrow<SoundObject>,
        >,
        config: &ConfigMap,
        camera: &Camera,
        game: &GameStateRenderInfo,
        cur_time: &Duration,
        group_attr: &MapGroupAttr,
        layer: &MapVisualLayerBase<T, Q, S, A>,
        // this can be used to overwrite the layer's texture. only useful for the editor
        forced_texture: Option<ForcedTexture>,
    ) where
        T: Borrow<TileLayerVisuals>,
        Q: Borrow<QuadLayerVisuals>,
    {
        let center = &camera.pos;

        // skip rendering if detail layers if not wanted
        if layer.high_detail() && !config.high_detail {
            return;
        }

        let mut state = State::new();

        // clipping
        if let Some(clipping) = &group_attr.clipping {
            // set clipping
            RenderTools::map_canvas_of_group(
                &self.canvas_handle,
                &mut state,
                center.x,
                center.y,
                None,
                camera.zoom,
            );
            let (canvas_x0, canvas_y0, canvas_x1, canvas_y1) = state.get_canvas_mapping();
            let points: [f32; 4] = [canvas_x0, canvas_y0, canvas_x1, canvas_y1];

            let x0 = (clipping.pos.x.to_num::<f32>() - points[0]) / (points[2] - points[0]);
            let y0 = (clipping.pos.y.to_num::<f32>() - points[1]) / (points[3] - points[1]);
            let x1 = ((clipping.pos.x.to_num::<f32>() + clipping.size.x.to_num::<f32>())
                - points[0])
                / (points[2] - points[0]);
            let y1 = ((clipping.pos.y.to_num::<f32>() + clipping.size.y.to_num::<f32>())
                - points[1])
                / (points[3] - points[1]);

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
                self.canvas_handle.window_width(),
                self.canvas_handle.window_height(),
            );
        }

        RenderTools::map_canvas_of_group(
            &self.canvas_handle,
            &mut state,
            center.x,
            center.y,
            Some(group_attr),
            camera.zoom,
        );

        match layer {
            MapVisualLayerBase::Tile(layer) => {
                let visual = layer.user.borrow();
                let layer = &layer.layer;
                let texture = if let Some(ForcedTexture::TileLayer(forced_texture)) = forced_texture
                {
                    Some(forced_texture)
                } else {
                    layer
                        .attr
                        .image_array
                        .map(|image| resources.image_arrays[image].user.borrow())
                };

                let color = ColorRGBA {
                    r: layer.attr.color.r().to_num::<f32>(),
                    g: layer.attr.color.g().to_num::<f32>(),
                    b: layer.attr.color.b().to_num::<f32>(),
                    a: layer.attr.color.a().to_num::<f32>()
                        * (100 - config.physics_layer_opacity) as f32
                        / 100.0,
                };

                state.blend(BlendType::Alpha);

                self.render_tile_layer(
                    &state,
                    texture.into(),
                    game,
                    cur_time,
                    camera.animation_ticks_passed,
                    &visual.base,
                    &visual.buffer_object_index,
                    &layer.attr.color_anim,
                    &layer.attr.color_anim_offset,
                    animations,
                    color,
                );
            }
            MapVisualLayerBase::Quad(layer) => {
                let visual = layer.user.borrow();
                let layer = &layer.layer;
                let texture = if let Some(ForcedTexture::QuadLayer(forced_texture)) = forced_texture
                {
                    Some(forced_texture)
                } else {
                    layer
                        .attr
                        .image
                        .map(|image| resources.images[image].user.borrow())
                };

                if config.show_quads {
                    state.blend(BlendType::Alpha);
                    self.render_quad_layer(
                        &state,
                        texture.into(),
                        game,
                        cur_time,
                        camera,
                        visual,
                        animations,
                        &layer.quads,
                    );
                }
            }
            MapVisualLayerBase::Sound(layer) => {
                // render sound properties
                // note that this should only be called for e.g. the editor
                for sound in layer.layer.sounds.iter() {
                    match sound.shape {
                        SoundShape::Rect { size } => todo!(),
                        SoundShape::Circle { radius } => {
                            RenderTools::render_circle(
                                &self.stream_handle,
                                &vec2::new(sound.pos.x.to_num(), sound.pos.y.to_num()),
                                radius.to_num(),
                                &ubvec4::new(150, 200, 255, 100),
                                state,
                            );

                            if !sound.falloff.is_zero() {
                                RenderTools::render_circle(
                                    &self.stream_handle,
                                    &vec2::new(sound.pos.x.to_num(), sound.pos.y.to_num()),
                                    (radius * sound.falloff.to_num::<ffixed>()).to_num(),
                                    &ubvec4::new(150, 200, 255, 100),
                                    state,
                                );
                            }
                        }
                    }
                }
            }
            _ => {
                panic!("this layer is not interesting for rendering, fix your map & code");
            }
        }
    }

    pub fn render_physics_layer<AN, AS, L>(
        &self,
        animations: &AnimationsSkeleton<AN, AS>,
        game: &GameStateRenderInfo,
        entities_container: &mut EntitiesContainer,
        layer: &MapLayerPhysicsSkeleton<L>,
        camera: &Camera,
        cur_time: &Duration,
        physics_layer_opacity: u8,
        // force a texture over the one that will be rendered
        // this is usually only useful for the editor
        forced_texture: Option<ForcedTexture>,
    ) where
        L: Borrow<PhysicsTileLayerVisuals>,
    {
        let entities = entities_container.get_or_default(&"TODO".try_into().unwrap());
        let mut state = State::new();

        RenderTools::map_canvas_of_group(
            &self.canvas_handle,
            &mut state,
            camera.pos.x,
            camera.pos.y,
            None,
            camera.zoom,
        );

        let is_main_physics_layer = matches!(layer, MapLayerPhysicsSkeleton::Game(_));

        let color = ColorRGBA {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: physics_layer_opacity as f32 / 100.0,
        };

        state.blend(BlendType::Alpha);

        let texture = Self::get_physics_layer_texture(layer, entities);
        // draw kill tiles outside the entity clipping rectangle
        if is_main_physics_layer {
            // slow blinking to hint that it's not a part of the map
            let seconds = cur_time.as_secs_f64();
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
                texture.into(),
                &layer.user().borrow().base,
                &color_kill,
            );
        }

        self.render_tile_layer(
            &state,
            if let Some(ForcedTexture::TileLayer(forced_texture)) = forced_texture {
                Some(forced_texture)
            } else {
                Some(texture)
            }
            .into(),
            game,
            cur_time,
            camera.animation_ticks_passed,
            &layer.user().borrow().base.base,
            &layer.user().borrow().base.buffer_object_index,
            &None,
            &time::Duration::ZERO,
            animations,
            color,
        );
        for overlay in layer.user().borrow().overlay_buffer_objects.iter() {
            let texture = match overlay.ty {
                MapRenderTextOverlayType::Top => &entities.text_overlay_top,
                MapRenderTextOverlayType::Bottom => &entities.text_overlay_bottom,
                MapRenderTextOverlayType::Center => &entities.text_overlay_center,
            };
            self.render_tile_layer(
                &state,
                texture.into(),
                game,
                cur_time,
                camera.animation_ticks_passed,
                &layer.user().borrow().base.base,
                &overlay.buffer_object,
                &None,
                &time::Duration::ZERO,
                animations,
                color,
            );
        }
    }

    fn render_design_impl<'a>(
        &self,
        map: &MapVisual,
        pipe: &mut RenderPipelineBase,
        render_layers: impl Iterator<Item = &'a MapRenderLayer>,
        layer_ty: RenderLayerType,
    ) {
        if pipe.config.physics_layer_opacity == 100 {
            return;
        }

        for render_layer in render_layers.filter(|render_layer| {
            if let MapRenderLayer::Tile(_) = render_layer {
                if matches!(layer_ty, RenderLayerType::Background)
                    && !pipe.config.background_show_tile_layers
                {
                    return false;
                }
            }
            true
        }) {
            let render_info = render_layer.get_render_info();
            let groups = if matches!(layer_ty, RenderLayerType::Background) {
                &map.groups.background
            } else {
                &map.groups.foreground
            };
            let group = &groups[render_info.group_index];

            self.render_layer(
                &map.animations,
                &map.resources,
                pipe.config,
                pipe.camera,
                pipe.game,
                pipe.cur_time,
                &group.attr,
                &group.layers[render_info.layer_index],
                None,
            );
        }
    }

    pub fn render_physics_layers(
        &self,
        map: &MapVisual,
        pipe: &mut RenderPipelineBase,
        render_infos: &[MapPhysicsRenderInfo],
    ) {
        for render_info in render_infos {
            self.render_physics_layer(
                &map.animations,
                pipe.game,
                pipe.entities_container,
                &map.groups.physics.layers[render_info.layer_index],
                pipe.camera,
                pipe.cur_time,
                pipe.config.physics_layer_opacity,
                None,
            );
        }
    }

    pub fn render_background(&self, map: &MapVisual, pipe: &mut RenderPipeline) {
        self.render_design_impl(
            map,
            &mut pipe.base,
            pipe.buffered_map.render.background_render_layers.iter(),
            RenderLayerType::Background,
        );
        self.sound
            .handle_background(map, pipe.buffered_map, pipe.base.camera);
    }

    pub fn render_foreground(&self, map: &MapVisual, pipe: &mut RenderPipeline) {
        self.render_design_impl(
            map,
            &mut pipe.base,
            pipe.buffered_map.render.foreground_render_layers.iter(),
            RenderLayerType::Foreground,
        );
        self.sound
            .handle_foreground(map, pipe.buffered_map, pipe.base.camera);
    }

    /// render the whole map but only with design layers at full opacity
    pub fn render_full_design(&self, map: &MapVisual, pipe: &mut RenderPipeline) {
        self.render_design_impl(
            map,
            &mut pipe.base,
            pipe.buffered_map.render.background_render_layers.iter(),
            RenderLayerType::Background,
        );
        self.render_design_impl(
            map,
            &mut pipe.base,
            pipe.buffered_map.render.foreground_render_layers.iter(),
            RenderLayerType::Foreground,
        );
    }
}
