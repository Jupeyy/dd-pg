use std::{collections::VecDeque, time::Duration};

use math::math::Rng;
use prediction_timer::prediction_timing::{PredictionTimer, PredictionTiming};

use super::user_data::SimulationProps;

pub fn simulate(
    history: &mut VecDeque<PredictionTiming>,
    prediction_timer: &mut PredictionTimer,
    props: &SimulationProps,
    rng: &mut Rng,
    cur_time: &Duration,
    last_time: &mut Duration,
) {
    let cur_time = *cur_time * props.time_scale.max(1);
    let time_per_snap = if props.snaps_per_sec == 0 {
        Duration::MAX
    } else {
        Duration::from_nanos(
            (Duration::from_secs(1).as_nanos() / props.snaps_per_sec as u128) as u64,
        )
    };
    if cur_time.saturating_sub(*last_time) > time_per_snap {
        *last_time = cur_time;

        history.push_back(prediction_timer.snapshot());

        let ratio_ping = rng.random_float() as f64 * 2.0;
        prediction_timer.add_ping(
            Duration::from_secs_f64(
                props.rtt_offset.as_secs_f64()
                    + props.half_rtt_jitter_range.as_secs_f64() * ratio_ping,
            ),
            cur_time,
        );

        let ratio_ping = rng.random_float() as f64;
        let off = props.rtt_offset.as_secs_f64() / 2.0
            + props.half_rtt_jitter_range.as_secs_f64() * ratio_ping;
        prediction_timer.add_snap(
            off - (props.rtt_offset.as_secs_f64() / 2.0
                + props.half_rtt_jitter_range.as_secs_f64() / 2.0),
            cur_time,
        );

        while history.len() > 1000 {
            history.pop_front();
        }
    }
}
