use std::time::Duration;

use crate::id_gen::{IDGeneratorIDType, ID_GENERATOR_ID_FIRST, ID_GENERATOR_ID_INVALID};

/// The game element id is a unique identifier to help
/// creating a connecting from a network object and the actual game object
/// it should be unique per type
pub type TGameElementID = IDGeneratorIDType;
pub const INVALID_GAME_ELEMENT_ID: IDGeneratorIDType = ID_GENERATOR_ID_INVALID;
pub const FIRST_GAME_ELEMENT_ID: IDGeneratorIDType = ID_GENERATOR_ID_FIRST;

pub fn time_until_tick(ticks_in_a_second: u64) -> Duration {
    Duration::from_micros(1000000 / ticks_in_a_second)
}
// half of tick time
pub const TIME_UNTIL_INP: Duration = Duration::from_millis(1000 / 100);

pub fn is_next_tick(
    cur_time: Duration,
    last_tick_time: &mut Duration,
    ticks_in_a_second: u64,
) -> bool {
    if (cur_time - *last_tick_time) > time_until_tick(ticks_in_a_second) {
        *last_tick_time += time_until_tick(ticks_in_a_second);
        true
    } else {
        false
    }
}

pub fn is_next_inp_tick(
    cur_time: Duration,
    last_game_tick_time: Duration,
    last_tick_time: &mut Duration,
    ticks_in_a_second: u64,
) -> bool {
    if *last_tick_time < last_game_tick_time + TIME_UNTIL_INP {
        *last_tick_time = last_game_tick_time;
    }
    if cur_time > *last_tick_time && (cur_time - *last_tick_time) > TIME_UNTIL_INP {
        *last_tick_time = cur_time + time_until_tick(ticks_in_a_second);
        true
    } else {
        false
    }
}

pub fn intra_tick_time(
    cur_time: Duration,
    last_tick_time: Duration,
    ticks_in_a_second: u64,
) -> Duration {
    Duration::from_nanos(
        ((cur_time - last_tick_time).as_nanos() / time_until_tick(ticks_in_a_second).as_nanos())
            as u64
            + (cur_time - last_tick_time).as_nanos() as u64
                % time_until_tick(ticks_in_a_second).as_nanos() as u64,
    )
}

pub fn intra_tick_time_to_ratio(intra_tick_time: Duration, ticks_in_a_second: u64) -> f64 {
    intra_tick_time.as_nanos() as f64 / time_until_tick(ticks_in_a_second).as_nanos() as f64
}

pub fn intra_tick_ratio(
    cur_time: Duration,
    last_tick_time: Duration,
    ticks_in_a_second: u64,
) -> f64 {
    intra_tick_time_to_ratio(
        intra_tick_time(cur_time, last_tick_time, ticks_in_a_second),
        ticks_in_a_second,
    )
}
