use game_interface::types::game::GameEntityId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum NetChatMsgPlayerChannel {
    Global,
    GameTeam,
    Whisper(GameEntityId), // sender
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetChatMsg {
    pub player_id: GameEntityId,
    pub msg: String,
    pub channel: NetChatMsgPlayerChannel,
}
