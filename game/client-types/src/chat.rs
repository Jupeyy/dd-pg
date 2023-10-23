use bincode::{Decode, Encode};
use pool::datatypes::{PoolString, StringPool};
use serde::{Deserialize, Serialize};
use shared_base::network::types::chat::{NetChatMsg, NetChatMsgPlayerChannel, NetMsgSystem};

#[derive(Debug, Serialize, Deserialize, Decode, Encode)]
pub enum ChatMsgPlayerChannel {
    Global,
    GameTeam,
    Whisper(PoolString), // sender name
}

impl ChatMsgPlayerChannel {
    pub fn from_net_msg(msg: NetChatMsgPlayerChannel, pool: &StringPool) -> Self {
        match msg {
            NetChatMsgPlayerChannel::Global => Self::Global,
            NetChatMsgPlayerChannel::GameTeam => Self::GameTeam,
            NetChatMsgPlayerChannel::Whisper(_) => Self::Whisper(pool.new()), // TODO
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Decode, Encode)]
pub struct ChatMsg {
    pub msg: String,
    pub channel: ChatMsgPlayerChannel,
}

impl ChatMsg {
    pub fn from_net_msg(msg: NetChatMsg, pool: &StringPool) -> Self {
        Self {
            msg: msg.msg,
            channel: ChatMsgPlayerChannel::from_net_msg(msg.channel, pool),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Decode, Encode)]
pub struct MsgSystem {
    pub msg: String,
}

impl MsgSystem {
    pub fn from_net_msg(msg: NetMsgSystem, _pool: &StringPool) -> Self {
        Self { msg: msg.msg }
    }
}

pub enum ServerMsg {
    Chat(ChatMsg),
    System(MsgSystem),
}