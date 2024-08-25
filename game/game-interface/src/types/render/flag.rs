use hiarc::Hiarc;
use math::math::vector::vec2;
use serde::{Deserialize, Serialize};

use crate::types::{flag::FlagType, game::GameEntityId};

/// The ingame metric is 1 tile = 1.0 float units
#[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize)]
pub struct FlagRenderInfo {
    pub pos: vec2,
    pub ty: FlagType,

    /// If this entity is owned by a character, this should be `Some` and
    /// include the characters id.
    /// In this specific case it could make sense if the flag is currently
    /// owned/carried by a character.
    pub owner_id: Option<GameEntityId>,
}
