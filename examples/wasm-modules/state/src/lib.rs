use api::graphics::graphics::Graphics;
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
    options: &GameStateCreateOptions,
) -> Box<dyn GameStateInterface> {
    Box::new(GameState::new(create_pipe, options))
}

#[no_mangle]
fn mod_main(_graphics: &mut Graphics) {}
