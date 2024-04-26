use hiarc::Hiarc;
use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};

// TODO: move this somewhere better
#[derive(
    Debug,
    Hiarc,
    Default,
    Copy,
    Clone,
    PartialEq,
    Eq,
    FromPrimitive,
    Serialize,
    Deserialize,
    Hash,
    PartialOrd,
    Ord,
)]
pub enum WeaponType {
    #[default]
    Hammer = 0,
    Gun,
    Shotgun,
    Grenade,
    Laser,
}
pub const NUM_WEAPONS: usize = WeaponType::Laser as usize + 1;
