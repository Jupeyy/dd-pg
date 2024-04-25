use hiarc::Hiarc;
use pool::datatypes::PoolVec;
use serde::{Deserialize, Serialize};

use crate::types::game::GameEntityId;

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct ScoreboardCharacterInfo {
    pub id: GameEntityId,
    pub score: i64,
    pub ping: u64,
    // TODO: add fields to make this more flexible for modding
}

pub type ScoreboardPlayerSpectatorInfo = ScoreboardCharacterInfo;

#[derive(Serialize, Deserialize)]
pub enum ScoreboardGameType {
    /// team = vanilla team
    TeamPlay {
        red_characters: PoolVec<ScoreboardCharacterInfo>,
        blue_characters: PoolVec<ScoreboardCharacterInfo>,
        spectator_players: PoolVec<ScoreboardPlayerSpectatorInfo>,
    },
    SoloPlay {
        characters: PoolVec<ScoreboardCharacterInfo>,
        spectator_players: PoolVec<ScoreboardPlayerSpectatorInfo>,
    },
}
