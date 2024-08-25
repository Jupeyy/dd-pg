use std::{borrow::Borrow, cell::Cell, fmt::Debug, ops::IndexMut, time::Duration};

use crate::map::map_buffered::{MapRenderLayer, MapRenderTextOverlayType};

use super::{
    map_buffered::{
        MapPhysicsRenderInfo, PhysicsTileLayerVisuals, QuadLayerVisuals, TileLayerVisuals,
        TileLayerVisualsBase,
    },
    map_pipeline::{MapGraphics, QuadRenderInfo, TileLayerDrawInfo},
    map_sound::MapSoundProcess,
    map_with_visual::{MapVisual, MapVisualLayerBase},
    render_pipe::{Camera, RenderPipeline, RenderPipelineBase},
    render_tools::RenderTools,
};
use client_containers::{
    container::ContainerKey,
    entities::{Entities, EntitiesContainer},
};
use fixed::traits::{FromFixed, ToFixed};
use game_config::config::ConfigMap;
use game_interface::types::game::{GameTickType, NonZeroGameTickType};
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
            layers::design::{Quad, Sound, SoundShape},
            MapGroupAttr, MapGroupAttrClipping,
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
    vector::{nffixed, nfvec4, ubvec4, uffixed, vec2},
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

    tile_layer_render_info_pool: Pool<Vec<TileLayerDrawInfo>>,

    // sound, handled here because it's such an integral part of the map
    pub sound: MapSoundProcess,
}

impl RenderMap {
    pub fn new(
        backend_handle: &GraphicsBackendHandle,
        canvas_handle: &GraphicsCanvasHandle,
        stream_handle: &GraphicsStreamHandle,
    ) -> RenderMap {
        let (tile_layer_render_info_pool, tile_layer_render_info_sync_point) =
            Pool::with_capacity(64);
        backend_handle.add_sync_point(tile_layer_render_info_sync_point);
        RenderMap {
            map_graphics: MapGraphics::new(backend_handle),

            canvas_handle: canvas_handle.clone(),
            stream_handle: stream_handle.clone(),

            tile_layer_render_info_pool,

            sound: MapSoundProcess::new(),
        }
    }

    pub fn calc_anim_time(
        ticks_per_second: NonZeroGameTickType,
        animation_ticks_passed: GameTickType,
        intra_tick_time: &Duration,
    ) -> Duration {
        let tick_to_nanoseconds = (time::Duration::seconds(1).whole_nanoseconds()
            / ticks_per_second.get() as i128) as u64;
        // get the lerp of the current tick and prev
        let min_tick = animation_ticks_passed.saturating_sub(1);
        let cur_tick = animation_ticks_passed;
        Duration::from_nanos(
            (mix::<f64, f64>(
                &0.0,
                &((cur_tick - min_tick) as f64),
                intra_tick_time.as_secs_f64(),
            ) * tick_to_nanoseconds as f64) as u64
                + min_tick * tick_to_nanoseconds,
        )
    }

    pub(crate) fn animation_eval<
        F,
        T: DeserializeOwned + Debug + Copy + Default + IndexMut<usize, Output = F>,
    >(
        anim: &AnimBase<AnimPoint<T>>,
        channels: usize,
        cur_time: &Duration,
        cur_anim_time: &Duration,
        anim_time_offset: &time::Duration,
    ) -> T
    where
        F: Copy + FromFixed + ToFixed,
    {
        let total_time = if anim.synchronized {
            time::Duration::try_from(*cur_anim_time).unwrap_or_default()
        } else {
            time::Duration::try_from(*cur_time).unwrap_or_default()
        };
        let anim_time = total_time + *anim_time_offset;

        RenderTools::render_eval_anim(&anim.points, anim_time, channels)
    }

    fn render_tile_layer<AN, AS>(
        &self,
        state: &State,
        texture: TextureType2dArray,
        cur_time: &Duration,
        cur_anim_time: &Duration,
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
                Self::animation_eval(&anim.def, 4, cur_time, cur_anim_time, color_anim_offset)
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
                let mut draws = self.tile_layer_render_info_pool.new();

                let reserve: usize = (y1 - y0).unsigned_abs() as usize + 1;
                draws.reserve(reserve);

                for y in y0..y1 {
                    if x0 > x1 {
                        continue;
                    }
                    let xr = x1 - 1;

                    if visuals.tiles_of_layer[(y * width + xr) as usize].quad_offset()
                        < visuals.tiles_of_layer[(y * width + x0) as usize].quad_offset()
                    {
                        panic!("Tile count wrong.");
                    }

                    let num_quads = (visuals.tiles_of_layer[(y * width + xr) as usize]
                        .quad_offset()
                        - visuals.tiles_of_layer[(y * width + x0) as usize].quad_offset())
                        + (if visuals.tiles_of_layer[(y * width + xr) as usize].drawable() {
                            1
                        } else {
                            0
                        });

                    if num_quads > 0 {
                        draws.push(TileLayerDrawInfo {
                            quad_offset: visuals.tiles_of_layer[(y * width + x0) as usize]
                                .quad_offset(),
                            quad_count: num_quads,
                        });
                    }
                }

                color.r *= channels.r().to_num::<f32>();
                color.g *= channels.g().to_num::<f32>();
                color.b *= channels.b().to_num::<f32>();
                color.a *= channels.a().to_num::<f32>();

                let draw_count = draws.len();
                if draw_count != 0 {
                    self.map_graphics.render_tile_layer(
                        state,
                        texture.clone(),
                        buffer_container_index,
                        &color,
                        draws,
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
                if border_y0 < 0 && visuals.corner_top_left.drawable() {
                    let offset = vec2::new(0.0, 0.0);
                    let scale = vec2::new(border_x0.abs() as f32, border_y0.abs() as f32);

                    self.map_graphics.render_border_tiles(
                        state,
                        texture.clone(),
                        buffer_container_index,
                        visuals.buffer_size_all_tiles,
                        color,
                        &offset,
                        &scale,
                        visuals.corner_top_left.quad_offset(),
                        1,
                    );
                }
                if border_y1 > height && visuals.corner_bottom_left.drawable() {
                    let offset = vec2::new(0.0, height as f32);
                    let scale = vec2::new(border_x0.abs() as f32, (border_y1 - height) as f32);

                    self.map_graphics.render_border_tiles(
                        state,
                        texture.clone(),
                        buffer_container_index,
                        visuals.buffer_size_all_tiles,
                        color,
                        &offset,
                        &scale,
                        visuals.corner_bottom_left.quad_offset(),
                        1,
                    );
                }
            }
            if border_x1 > width {
                // Draw corners on right side
                if border_y0 < 0 && visuals.corner_top_right.drawable() {
                    let offset = vec2::new(width as f32, 0.0);
                    let scale = vec2::new((border_x1 - width) as f32, border_y0.abs() as f32);

                    self.map_graphics.render_border_tiles(
                        state,
                        texture.clone(),
                        buffer_container_index,
                        visuals.buffer_size_all_tiles,
                        color,
                        &offset,
                        &scale,
                        visuals.corner_top_right.quad_offset(),
                        1,
                    );
                }
                if border_y1 > height && visuals.corner_bottom_right.drawable() {
                    let offset = vec2::new(width as f32, height as f32);
                    let scale = vec2::new((border_x1 - width) as f32, (border_y1 - height) as f32);

                    self.map_graphics.render_border_tiles(
                        state,
                        texture.clone(),
                        buffer_container_index,
                        visuals.buffer_size_all_tiles,
                        color,
                        &offset,
                        &scale,
                        visuals.corner_bottom_right.quad_offset(),
                        1,
                    );
                }
            }
            if border_x1 > width {
                // Draw right border
                if y0 < height && y1 > 0 {
                    let yb = y1 - 1;
                    let quad_count = (visuals.border_right[yb as usize].quad_offset()
                        - visuals.border_right[y0 as usize].quad_offset())
                        + (if visuals.border_right[yb as usize].drawable() {
                            1
                        } else {
                            0
                        });
                    let quad_offset = visuals.border_right[y0 as usize].quad_offset();
                    let offset = vec2::new(width as f32, 0.0);
                    let scale = vec2::new((border_x1 - width) as f32, 1.0);

                    self.map_graphics.render_border_tiles(
                        state,
                        texture.clone(),
                        buffer_container_index,
                        visuals.buffer_size_all_tiles,
                        color,
                        &offset,
                        &scale,
                        quad_offset,
                        quad_count,
                    );
                }
            }

            if border_x0 < 0 {
                // Draw left border
                if y0 < height && y1 > 0 {
                    let yb = y1 - 1;
                    let quad_count = (visuals.border_left[yb as usize].quad_offset()
                        - visuals.border_left[y0 as usize].quad_offset())
                        + (if visuals.border_left[yb as usize].drawable() {
                            1
                        } else {
                            0
                        });
                    let quad_offset = visuals.border_left[y0 as usize].quad_offset();
                    let offset = vec2::new(0.0, 0.0);
                    let scale = vec2::new((border_x0).abs() as f32, 1.0);

                    self.map_graphics.render_border_tiles(
                        state,
                        texture.clone(),
                        buffer_container_index,
                        visuals.buffer_size_all_tiles,
                        color,
                        &offset,
                        &scale,
                        quad_offset,
                        quad_count,
                    );
                }
            }
            if border_y0 < 0 {
                // Draw top border
                if x0 < width && x1 > 0 {
                    let xr = x1 - 1;
                    let quad_count = (visuals.border_top[xr as usize].quad_offset()
                        - visuals.border_top[x0 as usize].quad_offset())
                        + (if visuals.border_top[xr as usize].drawable() {
                            1
                        } else {
                            0
                        });
                    let quad_offset = visuals.border_top[x0 as usize].quad_offset();
                    let offset = vec2::new(0.0, 0.0);
                    let scale = vec2::new(1.0, border_y0.abs() as f32);

                    self.map_graphics.render_border_tiles(
                        state,
                        texture.clone(),
                        buffer_container_index,
                        visuals.buffer_size_all_tiles,
                        color,
                        &offset,
                        &scale,
                        quad_offset,
                        quad_count,
                    );
                }
            }
            if border_y1 > height {
                // Draw bottom border
                if x0 < width && x1 > 0 {
                    let xr = x1 - 1;
                    let quad_count = (visuals.border_bottom[xr as usize].quad_offset()
                        - visuals.border_bottom[x0 as usize].quad_offset())
                        + (if visuals.border_bottom[xr as usize].drawable() {
                            1
                        } else {
                            0
                        });
                    let quad_offset = visuals.border_bottom[x0 as usize].quad_offset();
                    let offset = vec2::new(0.0, height as f32);
                    let scale = vec2::new(1.0, (border_y1 - height) as f32);

                    self.map_graphics.render_border_tiles(
                        state,
                        texture.clone(),
                        buffer_container_index,
                        visuals.buffer_size_all_tiles,
                        color,
                        &offset,
                        &scale,
                        quad_offset,
                        quad_count,
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
        if let Some(buffer_container_index) = &visuals.buffer_object {
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
                let offset = vec2::new(border_x0 as f32, border_y0 as f32);
                let scale = vec2::new((-201 - border_x0) as f32, (border_y1 - border_y0) as f32);
                self.map_graphics.render_border_tiles(
                    state,
                    texture.clone(),
                    buffer_container_index,
                    visuals.base.buffer_size_all_tiles,
                    color,
                    &offset,
                    &scale,
                    visuals.base.border_kill_tile.quad_offset(),
                    1,
                );
            }
            // Draw top kill tile border
            if border_y0 < -201 {
                let offset = vec2::new(border_x0.max(-201) as f32, border_y0 as f32);
                let scale = vec2::new(
                    (border_x1.min(width + 201) - border_x0.max(-201)) as f32,
                    (-201 - border_y0) as f32,
                );
                self.map_graphics.render_border_tiles(
                    state,
                    texture.clone(),
                    buffer_container_index,
                    visuals.base.buffer_size_all_tiles,
                    color,
                    &offset,
                    &scale,
                    visuals.base.border_kill_tile.quad_offset(),
                    1,
                );
            }
            // Draw right kill tile border
            if border_x1 > width + 201 {
                let offset = vec2::new((width + 201) as f32, border_y0 as f32);
                let scale = vec2::new(
                    (border_x1 - (width + 201)) as f32,
                    (border_y1 - border_y0) as f32,
                );
                self.map_graphics.render_border_tiles(
                    state,
                    texture.clone(),
                    buffer_container_index,
                    visuals.base.buffer_size_all_tiles,
                    color,
                    &offset,
                    &scale,
                    visuals.base.border_kill_tile.quad_offset(),
                    1,
                );
            }
            // Draw bottom kill tile border
            if border_y1 > height + 201 {
                let offset = vec2::new(border_x0.max(-201) as f32, (height + 201) as f32);
                let scale = vec2::new(
                    (border_x1.min(width + 201) - border_x0.max(-201)) as f32,
                    (border_y1 - (height + 201)) as f32,
                );
                self.map_graphics.render_border_tiles(
                    state,
                    texture,
                    buffer_container_index,
                    visuals.base.buffer_size_all_tiles,
                    color,
                    &offset,
                    &scale,
                    visuals.base.border_kill_tile.quad_offset(),
                    1,
                );
            }
        }
    }

    pub fn prepare_quad_rendering<AN, AS>(
        mut stream_handle: StreamedUniforms<'_, QuadRenderInfo>,
        cur_time: &Duration,
        cur_anim_time: &Duration,
        cur_quad_offset: &Cell<usize>,
        animations: &AnimationsSkeleton<AN, AS>,
        quads: &[Quad],
    ) {
        for (i, quad) in quads.iter().enumerate() {
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
                    cur_time,
                    cur_anim_time,
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
                    cur_time,
                    cur_anim_time,
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
        cur_time: &Duration,
        cur_anim_time: &Duration,
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
                            cur_time: &Duration,
                            cur_anim_time: &Duration,
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
                            cur_time,
                            cur_anim_time,
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

    pub fn set_group_clipping(
        &self,
        state: &mut State,
        center: &vec2,
        zoom: f32,
        clipping: &MapGroupAttrClipping,
    ) {
        RenderTools::map_canvas_of_group(
            &self.canvas_handle,
            state,
            center.x,
            center.y,
            None,
            zoom,
        );
        let (canvas_x0, canvas_y0, canvas_x1, canvas_y1) = state.get_canvas_mapping();
        let points: [f32; 4] = [canvas_x0, canvas_y0, canvas_x1, canvas_y1];

        let x0 = (clipping.pos.x.to_num::<f32>() - points[0]) / (points[2] - points[0]);
        let y0 = (clipping.pos.y.to_num::<f32>() - points[1]) / (points[3] - points[1]);
        let x1 = ((clipping.pos.x.to_num::<f32>() + clipping.size.x.to_num::<f32>()) - points[0])
            / (points[2] - points[0]);
        let y1 = ((clipping.pos.y.to_num::<f32>() + clipping.size.y.to_num::<f32>()) - points[1])
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

    pub fn render_sounds<'a, AN, AS>(
        stream_handle: &GraphicsStreamHandle,
        animations: &AnimationsSkeleton<AN, AS>,
        cur_time: &Duration,
        cur_anim_time: &Duration,
        sounds: impl Iterator<Item = &'a Sound>,
        state: State,
    ) {
        for sound in sounds {
            let mut pos = sound.pos;
            let mut rot = 0.0;
            if let Some(anim) = {
                if let Some(pos_anim) = sound.pos_anim {
                    animations.pos.get(pos_anim)
                } else {
                    None
                }
            } {
                let pos_channels = RenderMap::animation_eval(
                    &anim.def,
                    3,
                    cur_time,
                    cur_anim_time,
                    &sound.pos_anim_offset,
                );
                pos.x += pos_channels.x;
                pos.y += pos_channels.y;
                rot = pos_channels.z.to_num::<f32>() / 180.0 * PI;
            }
            match sound.shape {
                SoundShape::Rect { size } => {
                    RenderTools::render_rect(
                        stream_handle,
                        &vec2::new(pos.x.to_num(), pos.y.to_num()),
                        &vec2::new(size.x.to_num(), size.y.to_num()),
                        &ubvec4::new(150, 200, 255, 100),
                        state,
                    );

                    if !sound.falloff.is_zero() {
                        RenderTools::render_rect(
                            stream_handle,
                            &vec2::new(pos.x.to_num(), pos.y.to_num()),
                            &vec2::new(
                                (size.x * sound.falloff.to_num::<uffixed>()).to_num(),
                                (size.y * sound.falloff.to_num::<uffixed>()).to_num(),
                            ),
                            &ubvec4::new(150, 200, 255, 100),
                            state,
                        );
                    }
                }
                SoundShape::Circle { radius } => {
                    RenderTools::render_circle(
                        stream_handle,
                        &vec2::new(pos.x.to_num(), pos.y.to_num()),
                        radius.to_num(),
                        &ubvec4::new(150, 200, 255, 100),
                        state,
                    );

                    if !sound.falloff.is_zero() {
                        RenderTools::render_circle(
                            stream_handle,
                            &vec2::new(pos.x.to_num(), pos.y.to_num()),
                            (radius * sound.falloff.to_num::<uffixed>()).to_num(),
                            &ubvec4::new(150, 200, 255, 100),
                            state,
                        );
                    }
                }
            }
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
        cur_time: &Duration,
        cur_anim_time: &Duration,
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
            self.set_group_clipping(&mut state, center, camera.zoom, clipping);
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
                    cur_time,
                    cur_anim_time,
                    &visual.base,
                    &visual.buffer_object,
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
                        cur_time,
                        cur_anim_time,
                        visual,
                        animations,
                        &layer.quads,
                    );
                }
            }
            MapVisualLayerBase::Sound(layer) => {
                // render sound properties
                // note that this should only be called for e.g. the editor
                Self::render_sounds(
                    &self.stream_handle,
                    animations,
                    cur_time,
                    cur_anim_time,
                    layer.layer.sounds.iter(),
                    state,
                );
            }
            _ => {
                panic!("this layer is not interesting for rendering, fix your map & code");
            }
        }
    }

    pub fn render_physics_layer<AN, AS, L>(
        &self,
        animations: &AnimationsSkeleton<AN, AS>,
        entities_container: &mut EntitiesContainer,
        entities_key: Option<&ContainerKey>,
        layer: &MapLayerPhysicsSkeleton<L>,
        camera: &Camera,
        cur_time: &Duration,
        cur_anim_time: &Duration,
        physics_layer_opacity: u8,
        // force a texture over the one that will be rendered
        // this is usually only useful for the editor
        forced_texture: Option<ForcedTexture>,
    ) where
        L: Borrow<PhysicsTileLayerVisuals>,
    {
        let entities = entities_container.get_or_default_opt(entities_key);
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
            cur_time,
            cur_anim_time,
            &layer.user().borrow().base.base,
            &layer.user().borrow().base.buffer_object,
            &None,
            &time::Duration::ZERO,
            animations,
            color,
        );
        for overlay in layer.user().borrow().overlays.iter() {
            let texture = match overlay.ty {
                MapRenderTextOverlayType::Top => &entities.text_overlay_top,
                MapRenderTextOverlayType::Bottom => &entities.text_overlay_bottom,
                MapRenderTextOverlayType::Center => &entities.text_overlay_center,
            };
            self.render_tile_layer(
                &state,
                texture.into(),
                cur_time,
                cur_anim_time,
                &overlay.visuals.base,
                &overlay.visuals.buffer_object,
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
                pipe.cur_time,
                pipe.cur_anim_time,
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
                pipe.entities_container,
                pipe.entities_key,
                &map.groups.physics.layers[render_info.layer_index],
                pipe.camera,
                pipe.cur_time,
                pipe.cur_anim_time,
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
        self.sound.handle_background(
            pipe.base.cur_time,
            pipe.base.cur_anim_time,
            map,
            pipe.buffered_map,
            pipe.base.camera,
        );
    }

    pub fn render_foreground(&self, map: &MapVisual, pipe: &mut RenderPipeline) {
        self.render_design_impl(
            map,
            &mut pipe.base,
            pipe.buffered_map.render.foreground_render_layers.iter(),
            RenderLayerType::Foreground,
        );
        self.sound.handle_foreground(
            pipe.base.cur_time,
            pipe.base.cur_anim_time,
            map,
            pipe.buffered_map,
            pipe.base.camera,
        );
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
