use game_interface::types::id_types::PlayerId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum NetChatMsgPlayerChannel {
    Global,
    GameTeam,
    Whisper(PlayerId), // sender
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetChatMsg {
    pub player_id: PlayerId,
    pub msg: String,
    pub channel: NetChatMsgPlayerChannel,
}
