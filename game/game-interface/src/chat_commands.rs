use std::collections::HashMap;

use command_parser::parser::CommandArg;
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
pub struct ChatCommands {
    /// list of commands and their required args
    pub cmds: HashMap<String, Vec<CommandArg>>,
    /// list of prefixes that trigger a chat command (e.g. `/` for slash commands)
    pub prefixes: Vec<char>,
}
