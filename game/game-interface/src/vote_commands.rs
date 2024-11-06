use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use crate::types::id_types::PlayerId;

/// Represents a vote command that usually
/// results from a completed vote.
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub enum VoteCommand {
    /// A player is about to be kicked from the game as a result of
    /// ban/kick vote.
    /// The actual client drop can happen at an unspecified later time.
    AboutToBeKicked(PlayerId),
    /// A player was moved to spec
    JoinSpectator(PlayerId),
    /// A rcon command as a result of a vote
    Misc(String),
}
