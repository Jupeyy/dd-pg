use hiarc::Hiarc;
use pool::datatypes::PoolLinkedHashMap;
use serde::{Deserialize, Serialize};

use crate::types::id_types::{CharacterId, CtfFlagId, LaserId, PickupId, ProjectileId};

use super::{
    character::CharacterRenderInfo, flag::FlagRenderInfo, laser::LaserRenderInfo,
    pickup::PickupRenderInfo, projectiles::ProjectileRenderInfo,
};

/// This represents a single world in the game.
/// A world is always part of a [`Stage`].
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct WorldRenderInfo {
    /// Projectiles that could potentially be rendered
    pub projectiles: PoolLinkedHashMap<ProjectileId, ProjectileRenderInfo>,
    /// Flags that could potentially be rendered
    pub ctf_flags: PoolLinkedHashMap<CtfFlagId, FlagRenderInfo>,
    /// Lasers that could potentially be rendered
    pub lasers: PoolLinkedHashMap<LaserId, LaserRenderInfo>,
    /// Pickups that could potentially be rendered
    pub pickups: PoolLinkedHashMap<PickupId, PickupRenderInfo>,
    /// Contains all information about characters that should be rendered
    pub characters: PoolLinkedHashMap<CharacterId, CharacterRenderInfo>,
}
