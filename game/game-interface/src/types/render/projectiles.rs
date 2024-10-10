use hiarc::Hiarc;
use math::math::vector::vec2;
use serde::{Deserialize, Serialize};

use crate::types::{game::GameEntityId, weapons::WeaponType};

/// The ingame metric is 1 tile = 1.0 float units
#[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize)]
pub struct ProjectileRenderInfo {
    pub ty: WeaponType,
    pub pos: vec2,
    pub vel: vec2,

    /// If this entity is owned by a character, this should be `Some` and
    /// include the characters id.
    pub owner_id: Option<GameEntityId>,
}
