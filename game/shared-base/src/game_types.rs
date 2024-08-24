use std::time::Duration;

use game_interface::types::game::NonZeroGameTickType;

pub fn time_until_tick(ticks_in_a_second: NonZeroGameTickType) -> Duration {
    Duration::from_micros(1000000 / ticks_in_a_second.get())
}
// half of tick time
pub const TIME_UNTIL_INP: Duration = Duration::from_millis(1000 / 100);

pub fn is_next_tick(
    cur_time: Duration,
    last_tick_time: &mut Duration,
    ticks_in_a_second: NonZeroGameTickType,
) -> bool {
    if cur_time >= *last_tick_time + time_until_tick(ticks_in_a_second) {
        *last_tick_time += time_until_tick(ticks_in_a_second);
        true
    } else {
        false
    }
}

pub fn intra_tick_time(
    cur_time: Duration,
    last_tick_time: Duration,
    ticks_in_a_second: NonZeroGameTickType,
) -> Duration {
    Duration::from_nanos(
        (cur_time.saturating_sub(last_tick_time)).as_nanos() as u64
            % time_until_tick(ticks_in_a_second).as_nanos() as u64,
    )
}

pub fn intra_tick_time_to_ratio(
    intra_tick_time: Duration,
    ticks_in_a_second: NonZeroGameTickType,
) -> f64 {
    intra_tick_time.as_nanos() as f64 / time_until_tick(ticks_in_a_second).as_nanos() as f64
}

pub fn intra_tick_ratio(
    cur_time: Duration,
    last_tick_time: Duration,
    ticks_in_a_second: NonZeroGameTickType,
) -> f64 {
    intra_tick_time_to_ratio(
        intra_tick_time(cur_time, last_tick_time, ticks_in_a_second),
        ticks_in_a_second,
    )
}
