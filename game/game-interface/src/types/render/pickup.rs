use hiarc::Hiarc;
use math::math::vector::vec2;
use serde::{Deserialize, Serialize};

use crate::types::{game::GameEntityId, pickup::PickupType};

/// The ingame metric is 1 tile = 1.0 float units
#[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize)]
pub struct PickupRenderInfo {
    pub ty: PickupType,
    pub pos: vec2,

    /// If this entity is owned by a character, this should be `Some` and
    /// include the characters id.
    /// In this specific case it _could_ be the nearest character for example.
    /// If unsure leave it to `None`.
    pub owner_id: Option<GameEntityId>,
}
