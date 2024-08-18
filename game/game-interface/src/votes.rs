use std::time::Duration;

use base::hash::Hash;
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use crate::types::{game::GameEntityId, network_string::NetworkReducedAsciiString};

pub const MAX_MAP_NAME_LEN: usize = 64;
/// Information of a map for a vote on the client.
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct MapVote {
    pub name: NetworkReducedAsciiString<MAX_MAP_NAME_LEN>,
    /// The hash is optional. If the hash is `None`, then
    /// a preview of the map is not possible.
    pub hash: Option<Hash>,
    /// Has a thumbnail resource that can be downlaoded
    /// from the game server's resource server.
    pub thumbnail_resource: bool,
}

/// Which kinds of votes are supported.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoteType {
    /// A map is identified by its name + hash.
    Map(MapVote),
    VoteKickPlayer {
        voted_player_id: GameEntityId,
    },
    VoteSpecPlayer {
        voted_player_id: GameEntityId,
    },
    Misc(),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Voted {
    Yes,
    No,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteState {
    pub vote: VoteType,
    /// The remaining time of the vote
    /// on the server.
    /// Since there isn't any prediction, this
    /// should simply be subtracted with the avg ping.
    pub remaining_time: Duration,

    pub yes_votes: usize,
    pub no_votes: usize,
    /// Number of clients that are allowed to participate in this vote.
    pub allowed_to_vote_count: usize,
}
