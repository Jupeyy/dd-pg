use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use crate::types::game::GameEntityId;

#[derive(Debug, Hiarc, Serialize, Deserialize, Clone, Copy)]
pub struct LeadingCharacter {
    /// id of the character
    pub character_id: GameEntityId,
    /// The score of the player
    pub score: i64,
}

/// Current results for the match.
#[derive(Debug, Hiarc, Serialize, Deserialize, Clone, Copy)]
pub enum MatchStandings {
    Solo {
        /// The top characters in the current match
        leading_characters: [Option<LeadingCharacter>; 2],
    },
    Sided {
        /// Score for the red side
        score_red: i64,
        /// Score for the blue side
        score_blue: i64,
    },
}

/// The side in the current match
#[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchSide {
    Red,
    Blue,
}
