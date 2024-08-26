use crate::types::game::{GameEntityId, GameTickType, NonZeroGameTickType};
use hiarc::Hiarc;
use math::math::vector::vec2;
use serde::{Deserialize, Serialize};

use crate::types::laser::LaserType;

/// The ingame metric is 1 tile = 1.0 float units
#[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize)]
pub struct LaserRenderInfo {
    pub ty: LaserType,
    pub from: vec2,
    pub pos: vec2,
    /// How long is the laser active and how long should it be active.
    /// So (0, 10) = fresh laser that lasts 10 ticks
    /// Note: that the first value is NOT clamped to the max lifetime.
    /// `None` if the laser was never evaluated
    pub eval_tick_ratio: Option<(GameTickType, NonZeroGameTickType)>,

    /// If this entity is owned by a character, this should be `Some` and
    /// include the characters id.
    pub owner_id: Option<GameEntityId>,
}
