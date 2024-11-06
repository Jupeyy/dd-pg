use std::collections::HashMap;

use command_parser::parser::CommandArg;
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

/// Commands supported by the server.
#[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
pub struct ChatCommands {
    /// list of commands and their required args
    pub cmds: HashMap<String, Vec<CommandArg>>,
    /// list of prefixes that trigger a chat command (e.g. `/` for slash commands)
    pub prefixes: Vec<char>,
}

/// The command that comes from the client.
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct ClientChatCommand {
    /// the raw unprocessed command string,
    /// excluding the "/"-prefix (or other chosen prefixes)
    pub raw: String,
}
