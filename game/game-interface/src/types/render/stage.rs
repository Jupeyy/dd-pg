use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use super::{game::GameRenderInfo, world::WorldRenderInfo};

/// The stage in which the world is
/// and matches/races are happening.
/// In ddrace language this is a "team".
/// A game can generally have multiple stages.
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct StageRenderInfo {
    /// Contains the render information for all entities in the world.
    pub world: WorldRenderInfo,
    /// Contains the game related rendering information.
    pub game: GameRenderInfo,
}
