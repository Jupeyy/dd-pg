use std::time::Duration;

use client_containers::{container::ContainerKey, entities::EntitiesContainer};
use game_config::config::ConfigMap;
use game_interface::types::game::NonZeroGameTickType;
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use math::math::vector::vec2;

use super::{map_buffered::ClientMapBuffered, map_with_visual::MapVisual};

#[derive(Debug, Hiarc)]
pub struct Camera {
    pub pos: vec2,
    pub zoom: f32,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct GameTimeInfo {
    pub ticks_per_second: NonZeroGameTickType,
    pub intra_tick_time: Duration,
}

#[derive(Debug)]
pub struct RenderPipelineBase<'a> {
    pub map: &'a MapVisual,
    pub config: &'a ConfigMap,
    pub cur_time: &'a Duration,
    pub cur_anim_time: &'a Duration,
    pub camera: &'a Camera,

    pub entities_container: &'a mut EntitiesContainer,
    pub entities_key: Option<&'a ContainerKey>,
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
        cur_anim_time: &'a Duration,
        camera: &'a Camera,
        entities_container: &'a mut EntitiesContainer,
        entities_key: Option<&'a ContainerKey>,
    ) -> RenderPipeline<'a> {
        RenderPipeline {
            base: RenderPipelineBase {
                map,
                config,
                cur_time,
                cur_anim_time,
                camera,
                entities_container,
                entities_key,
            },
            buffered_map,
        }
    }
}
