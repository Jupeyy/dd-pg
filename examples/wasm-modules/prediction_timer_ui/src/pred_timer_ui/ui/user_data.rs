use std::{collections::VecDeque, time::Duration};

use client_containers::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use game_interface::{
    types::{character_info::NetworkSkinInfo, resource_key::ResourceKey},
    votes::{MapVote, VoteState, Voted},
};
use graphics::handles::{
    canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle,
};
use math::math::Rng;
use prediction_timer::prediction_timing::{PredictionTimer, PredictionTiming};

#[derive(Debug, Default)]
pub struct SimulationProps {
    pub rtt_offset: Duration,
    pub half_rtt_jitter_range: Duration,
    pub ratio_ping: f64,
    pub snaps_per_sec: u32,
}

pub struct UserData<'a> {
    pub prediction_timer: &'a mut PredictionTimer,
    pub history: &'a mut VecDeque<PredictionTiming>,
    pub props: &'a mut SimulationProps,
    pub rng: &'a mut Rng,
    pub last_time: &'a mut Duration,
}
