use bincode::{Decode, Encode};
use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use crate::{game_types::TGameElementID, network::messages::WeaponType};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
    pub struct KillFeedFlags: i32 {
         const WALLSHOT = (1 << 0);
    }
}

#[derive(Debug, Serialize, Deserialize, Decode, Encode, Clone)]
pub struct NetKillfeedMsgPlayer {
    #[bincode(with_serde)]
    pub flags: KillFeedFlags, // wallshot or "is dominating the victim" or ended domination
    pub player_id: TGameElementID,
}

#[derive(Debug, Serialize, Deserialize, Decode, Encode, Clone)]
pub struct NetKillfeedMsg {
    pub attacker: NetKillfeedMsgPlayer,
    pub assistors: Vec<NetKillfeedMsgPlayer>,
    pub main_weapon: WeaponType,
    pub victims: Vec<NetKillfeedMsgPlayer>,
}
