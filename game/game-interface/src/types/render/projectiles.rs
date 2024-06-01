use hiarc::Hiarc;
use math::math::vector::vec2;
use serde::{Deserialize, Serialize};

use crate::types::weapons::WeaponType;

/// The ingame metric is 1 tile = 1.0 float units
#[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize)]
pub struct ProjectileRenderInfo {
    pub ty: WeaponType,
    pub pos: vec2,
    pub vel: vec2,
}
