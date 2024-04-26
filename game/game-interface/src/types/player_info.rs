use base::hash::Hash;
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use super::character_info::NetworkCharacterInfo;

/// a player from a client
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct PlayerClientInfo {
    pub info: NetworkCharacterInfo,
    pub is_dummy: bool,
    /// this is a (optional) unique identifier that
    /// allows to identify the client
    /// even after a reconnect.
    /// This is useful to store database entries
    /// using this id. (basically like an account)
    /// Avoid sending it to other clients,
    /// even if they could not use this information
    /// in a harmful way.
    pub unique_identifier: Option<Hash>,
}
