use base::hash::Hash;
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use super::character_info::NetworkCharacterInfo;

/// a player from a client
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct PlayerClientInfo {
    pub info: NetworkCharacterInfo,
    pub is_dummy: bool,
    /// a hint which player
    /// (of the many players a client can theoretically have)
    /// of the client was added here.
    /// Useful to restore after timeout in adition to
    /// the unique identifier.
    pub player_index: usize,
    /// this is an (optional) unique identifier that
    /// allows to identify the client
    /// even after a reconnect.
    /// This is useful to store database entries
    /// using this id. (basically like an account)
    /// Or using it as timeout code (to restore the client if
    /// the player dropped).
    /// Avoid sending it to other clients,
    /// even if they could not use this information
    /// in a harmful way.
    pub unique_identifier: Option<Hash>,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum PlayerDropReason {
    /// Graceful disconnect
    Disconnect,
    /// Timeout
    Timeout,
}
