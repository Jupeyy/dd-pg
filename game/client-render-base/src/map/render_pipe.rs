use std::{sync::Arc, time::Duration};

use base_io::io::IO;
use client_containers::entities::EntitiesContainer;
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use shared_base::{datafile::CDatafileWrapper, types::GameTickType};

use base::system::SystemInterface;
use config::config::ConfigMap;

use math::math::vector::vec2;

use super::{client_map_buffered::ClientMapBuffered, map_image::ClientMapImage};

pub struct Camera {
    pub pos: vec2,
    pub zoom: f32,

    /// this is used for syncronized envelopes
    /// usually it should be set to the value the local player
    /// has or the player the local player is spectating
    pub animation_start_tick: GameTickType,
}

pub struct GameStateRenderInfo {
    pub cur_tick: GameTickType,
    pub ticks_per_second: GameTickType,
}

pub struct RenderPipelineBase<'a, B: GraphicsBackendInterface> {
    pub map: &'a CDatafileWrapper,
    pub map_images: &'a Vec<ClientMapImage>,
    pub config: &'a ConfigMap,
    pub graphics: &'a mut GraphicsBase<B>,
    pub sys: &'a dyn SystemInterface,
    pub intra_tick_time: &'a Duration,
    pub game: &'a GameStateRenderInfo,
    pub camera: &'a Camera,

    pub entities_container: &'a mut EntitiesContainer,
    pub io: &'a IO,
    pub runtime_thread_pool: &'a Arc<rayon::ThreadPool>,

    pub force_full_design_render: bool,
}

pub struct RenderPipeline<'a, B: GraphicsBackendInterface> {
    pub base: RenderPipelineBase<'a, B>,
    pub buffered_map: &'a ClientMapBuffered,
}

impl<'a, B: GraphicsBackendInterface> RenderPipeline<'a, B> {
    pub fn new(
        map: &'a CDatafileWrapper,
        map_images: &'a Vec<ClientMapImage>,
        buffered_map: &'a ClientMapBuffered,
        config: &'a ConfigMap,
        graphics: &'a mut GraphicsBase<B>,
        sys: &'a dyn SystemInterface,
        intra_tick_time: &'a Duration,
        game: &'a GameStateRenderInfo,
        camera: &'a Camera,
        entities_container: &'a mut EntitiesContainer,
        io: &'a IO,
        runtime_thread_pool: &'a Arc<rayon::ThreadPool>,
        force_full_design_render: bool,
    ) -> RenderPipeline<'a, B> {
        RenderPipeline {
            base: RenderPipelineBase {
                map,
                map_images,
                config,
                graphics,
                sys,
                intra_tick_time,
                game,
                camera,
                entities_container,
                io,
                runtime_thread_pool,
                force_full_design_render,
            },
            buffered_map,
        }
    }
}
