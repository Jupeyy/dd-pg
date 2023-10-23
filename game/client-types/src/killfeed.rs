use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use shared_base::network::{
    messages::WeaponType,
    types::killfeed::{KillFeedFlags, NetKillfeedMsg, NetKillfeedMsgPlayer},
};

#[derive(Debug, Serialize, Deserialize, Decode, Encode, Clone)]
pub struct KillfeedMsgPlayer {
    #[bincode(with_serde)]
    pub flags: KillFeedFlags, // wallshot or "is dominating the victim" or ended domination
    pub name: String,
}

impl KillfeedMsgPlayer {
    pub fn from_net_msg(msg: NetKillfeedMsgPlayer) -> Self {
        Self {
            flags: msg.flags,
            name: "TODO:".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Decode, Encode, Clone)]
pub struct KillfeedMsg {
    pub attacker: KillfeedMsgPlayer,
    pub assistors: Vec<KillfeedMsgPlayer>,
    pub main_weapon: WeaponType,
    pub victims: Vec<KillfeedMsgPlayer>,
}

impl KillfeedMsg {
    pub fn from_net_msg(msg: NetKillfeedMsg) -> Self {
        Self {
            attacker: KillfeedMsgPlayer::from_net_msg(msg.attacker),
            assistors: msg
                .assistors
                .into_iter()
                .map(|p| KillfeedMsgPlayer::from_net_msg(p))
                .collect(),
            main_weapon: msg.main_weapon,
            victims: msg
                .victims
                .into_iter()
                .map(|p| KillfeedMsgPlayer::from_net_msg(p))
                .collect(),
        }
    }
}
