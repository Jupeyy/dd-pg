use hiarc::Hiarc;
use pool::datatypes::PoolLinkedHashMap;
use serde::{Deserialize, Serialize};

use crate::types::game::GameEntityId;

use super::{
    character::CharacterRenderInfo, flag::FlagRenderInfo, laser::LaserRenderInfo,
    pickup::PickupRenderInfo, projectiles::ProjectileRenderInfo,
};

/// This represents a single world in the game.
/// A world is always part of a [`Stage`].
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct WorldRenderInfo {
    /// Projectiles that could potentially be rendered
    pub projectiles: PoolLinkedHashMap<GameEntityId, ProjectileRenderInfo>,
    /// Flags that could potentially be rendered
    pub ctf_flags: PoolLinkedHashMap<GameEntityId, FlagRenderInfo>,
    /// Lasers that could potentially be rendered
    pub lasers: PoolLinkedHashMap<GameEntityId, LaserRenderInfo>,
    /// Pickups that could potentially be rendered
    pub pickups: PoolLinkedHashMap<GameEntityId, PickupRenderInfo>,
    /// Contains all information about characters that should be rendered
    pub characters: PoolLinkedHashMap<GameEntityId, CharacterRenderInfo>,
}
