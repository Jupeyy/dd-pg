use std::time::Duration;

use game_interface::{events::KillFeedFlags, types::weapons::WeaponType};

#[derive(Debug)]
pub struct ActionFeedPlayer {
    pub name: String,
}

#[derive(Debug)]
pub enum ActionFeedKillWeapon {
    Weapon {
        weapon: WeaponType,
    },
    Ninja,
    /// Kill tiles or world border
    World,
}

#[derive(Debug)]
pub struct ActionFeedKill {
    pub killer: Option<ActionFeedPlayer>,
    /// assists to the killer
    pub assists: Vec<ActionFeedPlayer>,
    pub victims: Vec<ActionFeedPlayer>,
    pub weapon: ActionFeedKillWeapon,
    pub flags: KillFeedFlags,
}

#[derive(Debug)]
pub enum ActionFeed {
    Kill(ActionFeedKill),
    RaceFinish {
        players: Vec<ActionFeedPlayer>,
        finish_time: Duration,
    },
    Custom(String),
}
