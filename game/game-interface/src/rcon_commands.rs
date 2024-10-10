use std::collections::HashMap;

use command_parser::parser::CommandArg;
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

/// Commands supported by the server.
#[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
pub struct RconCommands {
    /// list of commands and their required args
    pub cmds: HashMap<String, Vec<CommandArg>>,
}

#[derive(Debug, Hiarc, Default, Clone, Copy, Serialize, Deserialize)]
pub enum AuthLevel {
    #[default]
    None,
    Moderator,
    Admin,
}

/// A remote console command that a mod might support.
/// Note that some rcon commands are already processed
/// by the server implementation directly, like
/// changing a map.
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct ClientRconCommand {
    /// the raw unprocessed command string.
    pub raw: String,
    /// The auth level the client has for this command.
    pub auth_level: AuthLevel,
}
