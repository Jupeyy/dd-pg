pub mod game_match;

use game_match::MatchStandings;
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use crate::types::game::NonZeroGameTickType;

/// The game information for a single game in a stage.
/// The type of game depends on the game mode (race for ddrace, match for vanilla, etc.)
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameRenderInfo {
    Race {},
    Match {
        standings: MatchStandings,
        /// If the game has a game round countdown for this character,
        /// this should be set to `Some(cooldown)`.
        /// Else it should be set to `None`.
        /// This is usually a round timer e.g. for competitive games.
        game_round_ticks: Option<NonZeroGameTickType>,
    },
}
