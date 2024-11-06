pub use base::hash::Hash;
pub use ddnet_accounts_types::account_id::AccountId;
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use super::{character_info::NetworkCharacterInfo, network_stats::PlayerNetworkStats};

/// Unique id for accounts, timeout codes etc.
#[derive(Debug, Hiarc, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Serialize, Deserialize)]
pub enum PlayerUniqueId {
    Account(AccountId),
    CertFingerprint(Hash),
}

impl PlayerUniqueId {
    pub fn is_account_then<U, F: FnOnce(AccountId) -> Option<U>>(self, op: F) -> Option<U> {
        match self {
            PlayerUniqueId::Account(id) => op(id),
            PlayerUniqueId::CertFingerprint(_) => None,
        }
    }
}

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
    /// using this id. (like an account)
    /// Or using it as timeout code (to restore the client if
    /// the player dropped).
    pub unique_identifier: PlayerUniqueId,
    /// Initial unreliable network statistic (might be guessed.).
    pub initial_network_stats: PlayerNetworkStats,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum PlayerDropReason {
    /// Graceful disconnect
    Disconnect,
    /// Timeout
    Timeout,
}
