use std::{sync::Arc, time::Duration};

use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use graphics_backend::types::Graphics;
use shared::game::state_wasm_manager::GameStateWasmManager;
use shared_base::{datafile::CDatafileWrapper, types::GameTickType};

use base::system::SystemInterface;
use config::config::ConfigMap;

use math::math::vector::vec2;

use crate::containers::entities::EntitiesContainer;

use super::{client_map::ClientMapImage, client_map_buffered::ClientMapBuffered};

pub struct Camera {
    pub pos: vec2,
    pub zoom: f32,

    /// this is used for syncronized envelopes
    /// usually it should be set to the value the local player
    /// has or the player the local player is spectating
    pub animation_start_tick: GameTickType,
}

pub struct RenderPipelineBase<'a> {
    pub map: &'a CDatafileWrapper,
    pub map_images: &'a Vec<ClientMapImage>,
    pub config: &'a ConfigMap,
    pub graphics: &'a mut Graphics,
    pub sys: &'a dyn SystemInterface,
    pub intra_tick_time: &'a Duration,
    pub game: &'a GameStateWasmManager,
    pub camera: &'a Camera,

    pub entities_container: &'a mut EntitiesContainer,
    pub fs: &'a Arc<FileSystem>,
    pub io_batcher: &'a TokIOBatcher,
    pub runtime_thread_pool: &'a Arc<rayon::ThreadPool>,

    pub force_full_design_render: bool,
}

pub struct RenderPipeline<'a> {
    pub base: RenderPipelineBase<'a>,
    pub buffered_map: &'a ClientMapBuffered,
}

impl<'a> RenderPipeline<'a> {
    pub fn new(
        map: &'a CDatafileWrapper,
        map_images: &'a Vec<ClientMapImage>,
        buffered_map: &'a ClientMapBuffered,
        config: &'a ConfigMap,
        graphics: &'a mut Graphics,
        sys: &'a dyn SystemInterface,
        intra_tick_time: &'a Duration,
        game: &'a GameStateWasmManager,
        camera: &'a Camera,
        entities_container: &'a mut EntitiesContainer,
        fs: &'a Arc<FileSystem>,
        io_batcher: &'a TokIOBatcher,
        runtime_thread_pool: &'a Arc<rayon::ThreadPool>,
        force_full_design_render: bool,
    ) -> RenderPipeline<'a> {
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
                fs,
                io_batcher,
                runtime_thread_pool,
                force_full_design_render,
            },
            buffered_map,
        }
    }
}
