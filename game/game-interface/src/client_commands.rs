use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use crate::{
    chat_commands::ClientChatCommand,
    rcon_commands::ClientRconCommand,
    types::{network_string::NetworkString, render::game::game_match::MatchSide},
};

#[derive(Debug, Hiarc, Clone, Copy, Serialize, Deserialize)]
pub enum ClientFreeCamMode {
    /// Go back to a non-free cam mode
    None,
    /// The client wants to join a normal freecam (similar to /pause in ddrace)
    Normal,
    /// The clients wants to join the freecam and make himself invisible (similar to /spec in ddrace)
    Ghost,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub enum ClientCommand {
    /// The client requests that his character should respawn
    Kill,
    /// A chat-like command was used (/cmd)
    Chat(ClientChatCommand),
    /// A remote-console-like command was used
    Rcon(ClientRconCommand),
    /// The client wants to join a stage (a.k.a ddrace-team)
    JoinStage {
        /// The desired name of the stage
        name: NetworkString<24>,
        /// The color of the stage (if the stage doesn't exist yet).
        color: [u8; 3],
    },
    /// The client wants to pick a side (red or blue vanilla team)
    JoinSide(MatchSide),
    /// The client wants to join the spectators
    JoinSpectator,
    /// The client requests to switch to a freecam mode
    SetFreeCamMode(ClientFreeCamMode),
}
