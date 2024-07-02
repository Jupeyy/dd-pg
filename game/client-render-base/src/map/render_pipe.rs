use std::time::Duration;

use client_containers_new::entities::EntitiesContainer;
use game_config::config::ConfigMap;
use game_interface::types::game::GameTickType;
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use math::math::vector::vec2;

use super::{map_buffered::ClientMapBuffered, map_with_visual::MapVisual};

#[derive(Debug, Hiarc)]
pub struct Camera {
    pub pos: vec2,
    pub zoom: f32,

    /// This is used for syncronized envelopes
    /// usually it should be set to the value the local player
    /// has or the player the local player is spectating.
    /// It's the amount of ticks that passed since the animation
    /// started
    pub animation_ticks_passed: GameTickType,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct GameStateRenderInfo {
    pub ticks_per_second: GameTickType,
    pub intra_tick_time: Duration,
}

#[derive(Debug)]
pub struct RenderPipelineBase<'a> {
    pub map: &'a MapVisual,
    pub config: &'a ConfigMap,
    pub cur_time: &'a Duration,
    pub game: &'a GameStateRenderInfo,
    pub camera: &'a Camera,

    pub entities_container: &'a mut EntitiesContainer,
}

pub struct RenderPipeline<'a> {
    pub base: RenderPipelineBase<'a>,
    pub buffered_map: &'a ClientMapBuffered,
}

impl<'a> RenderPipeline<'a> {
    pub fn new(
        map: &'a MapVisual,
        buffered_map: &'a ClientMapBuffered,
        config: &'a ConfigMap,
        cur_time: &'a Duration,
        game: &'a GameStateRenderInfo,
        camera: &'a Camera,
        entities_container: &'a mut EntitiesContainer,
    ) -> RenderPipeline<'a> {
        RenderPipeline {
            base: RenderPipelineBase {
                map,
                config,
                cur_time,
                game,
                camera,
                entities_container,
            },
            buffered_map,
        }
    }
}
