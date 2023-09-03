use std::sync::Arc;

use base_log::log::SystemLog;
use shared_base::types::GameTickType;
use state::state::{GameState, GameStateCreateOptions, GameStateCreatePipe, GameStateInterface};

pub mod collision;
pub mod entities;
pub mod player;
pub mod simulation_pipe;
pub mod snapshot;
pub mod spawns;
pub mod stage;
pub mod state;
pub mod weapons;
pub mod world;

pub use api::*;
pub use api_state::*;

#[no_mangle]
fn mod_state_new(
    create_pipe: &GameStateCreatePipe,
    log: &Arc<SystemLog>,
    options: &GameStateCreateOptions,
) -> (Box<dyn GameStateInterface>, GameTickType) {
    let state = GameState::new(create_pipe, log, options);
    let game_tick_speed = state.game_tick_speed();
    (Box::new(state), game_tick_speed)
}
