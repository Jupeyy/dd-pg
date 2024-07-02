#![allow(clippy::all)]

use game_interface::interface::{
    GameStateCreate, GameStateCreateOptions, GameStateInterface, GameStateStaticInfo,
};
use state::state::GameState;

pub mod collision;
pub mod ctf_controller;
pub mod entities;
pub mod events;
pub mod game_objects;
pub mod match_manager;
pub mod match_state;
pub mod simulation_pipe;
pub mod snapshot;
pub mod spawns;
pub mod stage;
pub mod state;
pub mod types;
pub mod weapons;
pub mod world;

pub use api::*;
pub use api_state::*;

#[no_mangle]
fn mod_state_new(
    map: Vec<u8>,
    options: GameStateCreateOptions,
) -> (Box<dyn GameStateInterface>, GameStateStaticInfo) {
    let (state, info) = GameState::new(map, options);
    (Box::new(state), info)
}
