use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use crate::{chat_commands::ClientChatCommand, rcon_commands::ClientRconCommand};

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub enum ClientCommand {
    /// The client requests that his character should respawn
    Kill,
    /// A chat-like command was used (/cmd)
    Chat(ClientChatCommand),
    /// A remote-console-like command was used
    Rcon(ClientRconCommand),
}
