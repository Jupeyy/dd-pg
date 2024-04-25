use hiarc::Hiarc;
use math::math::vector::vec2;
use serde::{Deserialize, Serialize};

use crate::types::flag::FlagType;

/// The ingame metric is 1 tile = 1.0 float units
#[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize)]
pub struct FlagRenderInfo {
    pub pos: vec2,
    pub ty: FlagType,
}
