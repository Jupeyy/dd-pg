use game_interface::types::{character_info::NetworkSkinInfo, resource_key::ResourceKey};
use serde::{Deserialize, Serialize};
use shared_base::network::types::chat::NetChatMsgPlayerChannel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatMsgPlayerChannel {
    Global,
    GameTeam,
    Whisper(String), // sender name
}

impl ChatMsgPlayerChannel {
    pub fn from_net_msg(msg: NetChatMsgPlayerChannel) -> Self {
        match msg {
            NetChatMsgPlayerChannel::Global => Self::Global,
            NetChatMsgPlayerChannel::GameTeam => Self::GameTeam,
            NetChatMsgPlayerChannel::Whisper(_) => Self::Whisper(String::new()), // TODO: not implemented + should use PoolString
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMsg {
    pub player: String,
    pub clan: String,
    pub skin_name: ResourceKey,
    pub skin_info: NetworkSkinInfo,
    pub msg: String,
    pub channel: ChatMsgPlayerChannel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgSystem {
    pub msg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMsg {
    Chat(ChatMsg),
    System(MsgSystem),
}
