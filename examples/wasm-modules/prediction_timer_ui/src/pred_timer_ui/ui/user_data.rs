use std::{collections::VecDeque, time::Duration};

use math::math::Rng;
use prediction_timer::prediction_timing::{PredictionTimer, PredictionTiming};

#[derive(Debug)]
pub struct SimulationProps {
    pub rtt_offset: Duration,
    pub half_rtt_jitter_range: Duration,
    pub ratio_ping: f64,
    pub snaps_per_sec: u32,
    pub time_scale: u32,
}

impl Default for SimulationProps {
    fn default() -> Self {
        Self {
            rtt_offset: Default::default(),
            half_rtt_jitter_range: Default::default(),
            ratio_ping: Default::default(),
            snaps_per_sec: 25,
            time_scale: 1,
        }
    }
}

pub struct UserData<'a> {
    pub prediction_timer: &'a mut PredictionTimer,
    pub history: &'a mut VecDeque<PredictionTiming>,
    pub props: &'a mut SimulationProps,
    pub rng: &'a mut Rng,
    pub last_time: &'a mut Duration,
}
