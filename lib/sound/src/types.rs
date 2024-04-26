use std::time::Duration;

use hiarc::Hiarc;
use math::math::vector::vec2;
use serde::{Deserialize, Serialize};

pub type SoundID = u128;

#[derive(Debug, Hiarc, Default, Serialize, Deserialize)]
pub struct SoundPlayProps {
    pub pos: vec2,
    pub time_offset: Duration,
    pub looped: bool,
}
