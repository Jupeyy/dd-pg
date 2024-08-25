use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use crate::types::game::GameEntityId;

/// Current results for the match.
#[derive(Debug, Hiarc, Serialize, Deserialize, Clone, Copy)]
pub enum MatchStandings {
    Solo {
        /// The top players in the current match
        leading_players: [Option<GameEntityId>; 2],
    },
    Team {
        /// Score for the red team
        score_red: i64,
        /// Score for the blue team
        score_blue: i64,
    },
}
