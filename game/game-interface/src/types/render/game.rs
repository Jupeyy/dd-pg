pub mod game_match;

use game_match::MatchStandings;
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

/// The game information for a single game in a stage.
/// The type of game depends on the game mode (race for ddrace, match for vanilla, etc.)
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameRenderInfo {
    Race {},
    Match { standings: MatchStandings },
}
