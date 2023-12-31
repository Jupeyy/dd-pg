use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::game_types::TGameElementID;

#[derive(Debug, Serialize, Deserialize, Decode, Encode, Clone)]
pub enum NetChatMsgPlayerChannel {
    Global,
    GameTeam,
    Whisper(TGameElementID), // sender
}

#[derive(Debug, Serialize, Deserialize, Decode, Encode, Clone)]
pub struct NetChatMsg {
    pub player_id: TGameElementID,
    pub msg: String,
    pub channel: NetChatMsgPlayerChannel,
}

#[derive(Debug, Serialize, Deserialize, Decode, Encode, Clone)]
pub struct NetMsgSystem {
    pub msg: String,
}
