use std::time::Duration;

use hiarc::Hiarc;
use math::math::vector::vec2;
use serde::{Deserialize, Serialize};

pub type SoundID = u128;

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct SoundPlayBaseProps {
    pub pos: vec2,
    pub looped: bool,
    pub volume: f64,
    /// [0-1] where 0.5 is the mid, 0.0 is left, 1.0 is right
    pub panning: f64,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct SoundPlayProps {
    pub base: SoundPlayBaseProps,
    pub start_time_delay: Duration,
    /// Min distance at which the volume is 100%
    pub min_distance: f32,
    /// Max distance at which the volume is 0%
    pub max_distance: f32,
    /// If `None` a linear attenuation is used for the distance volumn interpolation,
    /// otherwise `(1.0 - normalized_distance).pow(val)` creating a monotonic increasing
    /// fading effect (starts slow and speeds up).
    /// Higher values cause the volume to fade faster.
    pub pow_attenuation_value: Option<f64>,
    /// Wether the sound should be spartial (so automatically use panning)
    /// depending on the positions of the listeners and this sound.
    pub spartial: bool,
}

impl SoundPlayProps {
    pub fn new_with_pos(pos: vec2) -> Self {
        Self {
            base: SoundPlayBaseProps {
                pos,
                volume: 1.0,
                looped: false,
                panning: 0.5,
            },
            start_time_delay: Duration::ZERO,
            min_distance: 1.0,
            max_distance: 50.0,
            pow_attenuation_value: Some(0.5),
            spartial: false,
        }
    }
    pub fn with_with_spartial(mut self, spartial: bool) -> Self {
        self.spartial = spartial;
        self
    }
}
