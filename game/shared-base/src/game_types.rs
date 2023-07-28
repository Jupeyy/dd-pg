use std::time::Duration;

use crate::id_gen::{IDGeneratorIDType, ID_GENERATOR_ID_FIRST, ID_GENERATOR_ID_INVALID};

/// The game element id is a unique identifier to help
/// creating a connecting from a network object and the actual game object
/// it should be unique per type
pub type TGameElementID = IDGeneratorIDType;
pub const INVALID_GAME_ELEMENT_ID: IDGeneratorIDType = ID_GENERATOR_ID_INVALID;
pub const FIRST_GAME_ELEMENT_ID: IDGeneratorIDType = ID_GENERATOR_ID_FIRST;

pub const TIME_UNTIL_TICK: Duration = Duration::from_millis(1000 / 50);
// half of tick time
pub const TIME_UNTIL_INP: Duration = Duration::from_millis(1000 / 100);

pub fn is_next_tick(cur_time: Duration, last_tick_time: &mut Duration) -> bool {
    if (cur_time - *last_tick_time) > TIME_UNTIL_TICK {
        *last_tick_time += TIME_UNTIL_TICK;
        true
    } else {
        false
    }
}

pub fn is_next_inp_tick(
    cur_time: Duration,
    last_game_tick_time: Duration,
    last_tick_time: &mut Duration,
) -> bool {
    if *last_tick_time < last_game_tick_time + TIME_UNTIL_INP {
        *last_tick_time = last_game_tick_time;
    }
    if cur_time > *last_tick_time && (cur_time - *last_tick_time) > TIME_UNTIL_INP {
        *last_tick_time = cur_time + TIME_UNTIL_TICK;
        true
    } else {
        false
    }
}

pub fn intra_tick_time(cur_time: Duration, last_tick_time: Duration) -> Duration {
    Duration::from_nanos(
        ((cur_time - last_tick_time).as_nanos() / TIME_UNTIL_TICK.as_nanos()) as u64
            + (cur_time - last_tick_time).as_nanos() as u64 % TIME_UNTIL_TICK.as_nanos() as u64,
    )
}

pub fn intra_tick_time_to_ratio(intra_tick_time: Duration) -> f64 {
    intra_tick_time.as_nanos() as f64 / TIME_UNTIL_TICK.as_nanos() as f64
}

pub fn intra_tick_ratio(cur_time: Duration, last_tick_time: Duration) -> f64 {
    intra_tick_time_to_ratio(intra_tick_time(cur_time, last_tick_time))
}
