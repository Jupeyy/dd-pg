pub mod core;
pub mod player;
pub mod pos {
    pub use ::shared_game::entities::character::pos::*;
}
pub mod hook {
    pub use ::shared_game::entities::character::hook::*;
}

use api_macros::character_mod;

#[character_mod("../../../")]
pub mod character {}
