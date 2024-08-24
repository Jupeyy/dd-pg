use hiarc::Hiarc;
use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};
use strum::EnumCount;

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
    EnumCount,
)]
pub enum WeaponType {
    #[default]
    Hammer = 0,
    Gun,
    Shotgun,
    Grenade,
    Laser,
}
