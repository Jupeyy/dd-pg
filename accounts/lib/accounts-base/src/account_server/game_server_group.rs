use serde::{Deserialize, Serialize};

use crate::client::game_server_data::GameServerKeyPair;

/// On success the account server sends the key-pair
/// to client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameServerKeyPairResponseSuccess {
    /// The key-pair for a game server group,
    /// which was stored on the account server.
    pub key_pair: GameServerKeyPair,
}

/// The response for a game server group key-pair
/// from the account server for the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameServerKeyPairResponse {
    /// A key-pair was found.
    Success(Box<GameServerKeyPairResponseSuccess>),
    /// No key-pair for this game server group was
    /// found on the account server.
    NotFound,
    /// The user was not logged in.
    /// Or the user cannot create keys (not verified).
    InvalidAuth,
}

/// The response to store a game server group key-pair
/// on the account server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StoreGameServerKeyPairResponse {
    /// The key-pair was saved.
    Success,
    /// No game server group was found matching the
    /// requested id.
    GameServerGroupNotFound,
    /// The user was not logged in.
    /// Or the user can simply not create game server keys.
    /// (not verified or no permission as game server)
    InvalidAuth,
}
