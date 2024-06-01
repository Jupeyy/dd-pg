use ed25519_dalek::VerifyingKey;

/// Represents a unique identifier to identify a group of game servers.
pub type GameServerGroupId = [u8; 32];

/// Get the game server group id from a public key send by a game server.
pub fn game_server_group_id_from_pub_key(public_key: VerifyingKey) -> GameServerGroupId {
    public_key.to_bytes()
}
