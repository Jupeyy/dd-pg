use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct ClientChatCommand {
    /// the raw unprocessed command string,
    /// excluding the "/"-prefix
    pub raw: String,
}

/// A remote console command that a mod might support.
/// Note that some rcon commands are already processed
/// by the server implementation directly, like
/// changing a map.
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct ClientRconCommand {
    /// the raw unprocessed command string
    pub raw: String,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub enum ClientCommand {
    /// The client requests that his character should respawn
    Kill,
    /// A chat-like command was used (/cmd)
    Chat(ClientChatCommand),
    /// A remote-console-like command was used
    Rcon(ClientRconCommand),
}
