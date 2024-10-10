use game_database::types::UnixUtcTimestamp;
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use crate::types::network_string::NetworkReducedAsciiString;

/// Account information that the client can interpret by default.
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    /// The name of the account on this game server
    pub name: NetworkReducedAsciiString<32>,
    /// The date when the account was first registered
    /// on this game server.
    pub creation_date: UnixUtcTimestamp,
}
