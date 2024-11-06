use hiarc::Hiarc;
use math::math::vector::ubvec4;
use pool::datatypes::{PoolLinkedHashMap, PoolString, PoolVec};
use serde::{Deserialize, Serialize};

use crate::types::{
    id_types::{CharacterId, StageId},
    network_stats::PlayerNetworkStats,
};

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum ScoreboardConnectionType {
    /// "Normal" network connection
    Network(PlayerNetworkStats),
    /// A local/server-side bot
    Bot,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct ScoreboardCharacterInfo {
    pub id: CharacterId,
    pub score: i64,
    pub ping: ScoreboardConnectionType,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct ScoreboardStageInfo {
    pub characters: PoolVec<ScoreboardCharacterInfo>,
    pub name: PoolString,
    pub max_size: usize,
    pub color: ubvec4,
}

pub type ScoreboardPlayerSpectatorInfo = ScoreboardCharacterInfo;

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum ScoreboardGameType {
    /// side = vanilla team/side
    /// stage = ddrace team
    SidedPlay {
        red_stages: PoolLinkedHashMap<StageId, ScoreboardStageInfo>,
        blue_stages: PoolLinkedHashMap<StageId, ScoreboardStageInfo>,
        spectator_players: PoolVec<ScoreboardPlayerSpectatorInfo>,

        /// This stage is going to be ignored in scoreboard rendering
        /// E.g. team 0 in ddrace has no background color
        ignore_stage: StageId,

        red_side_name: PoolString,
        blue_side_name: PoolString,
    },
    SoloPlay {
        stages: PoolLinkedHashMap<StageId, ScoreboardStageInfo>,

        /// This stage is going to be ignored in scoreboard rendering
        /// E.g. team 0 in ddrace has no background color
        ignore_stage: StageId,

        spectator_players: PoolVec<ScoreboardPlayerSpectatorInfo>,
    },
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct ScoreboardGameOptions {
    pub score_limit: u64,
    pub map_name: PoolString,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct Scoreboard {
    pub game: ScoreboardGameType,
    pub options: ScoreboardGameOptions,
}
