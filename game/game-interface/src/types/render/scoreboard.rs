use hiarc::Hiarc;
use pool::datatypes::{PoolString, PoolVec};
use serde::{Deserialize, Serialize};

use crate::types::{game::GameEntityId, network_stats::PlayerNetworkStats};

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum ScoreboardConnectionType {
    /// "Normal" network connection
    Network(PlayerNetworkStats),
    /// A local/server-side bot
    Bot,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct ScoreboardCharacterInfo {
    pub id: GameEntityId,
    pub score: i64,
    pub ping: ScoreboardConnectionType,
    // TODO: add fields to make this more flexible for modding
}

pub type ScoreboardPlayerSpectatorInfo = ScoreboardCharacterInfo;

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum ScoreboardGameType {
    /// team = vanilla team
    TeamPlay {
        red_characters: PoolVec<ScoreboardCharacterInfo>,
        blue_characters: PoolVec<ScoreboardCharacterInfo>,
        spectator_players: PoolVec<ScoreboardPlayerSpectatorInfo>,

        red_team_name: PoolString,
        blue_team_name: PoolString,
    },
    SoloPlay {
        characters: PoolVec<ScoreboardCharacterInfo>,
        spectator_players: PoolVec<ScoreboardPlayerSpectatorInfo>,
    },
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct ScoreboardGameOptions {
    pub score_limit: u64,
    pub map_name: PoolString,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct Scoreboard {
    pub game: ScoreboardGameType,
    pub options: ScoreboardGameOptions,
}
