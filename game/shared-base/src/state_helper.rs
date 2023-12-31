use std::time::Duration;

use crate::types::GameTickType;

pub fn intra_tick_from_start(
    ticks_per_second: &GameTickType,
    intra_tick_time: &Duration,
    cur_tick: &GameTickType,
    start_tick: &GameTickType,
) -> f64 {
    // check how much time passed since the start
    // the total passed time since the start - the time passed by amount of ticks gives the current time in the tick
    // now use this time and devide it be the amount of time that is passed per tick
    let time_per_tick = Duration::from_secs(1).as_nanos() as u64 / *ticks_per_second;
    (intra_tick_time.as_nanos() as f64 + ((cur_tick - *start_tick) * time_per_tick) as f64)
        / time_per_tick as f64
}
