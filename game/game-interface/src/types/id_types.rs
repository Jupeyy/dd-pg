use std::fmt::Display;

use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use super::game::GameEntityId;

#[derive(
    Debug, Hiarc, Serialize, Deserialize, PartialEq, Eq, Copy, Clone, Hash, PartialOrd, Ord,
)]
pub struct CharacterId(GameEntityId);

impl From<GameEntityId> for CharacterId {
    fn from(value: GameEntityId) -> Self {
        Self(value)
    }
}

impl Display for CharacterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// A player id is not only type equal to a [`CharacterId`]. It also shares the same
/// value for a player-character pair.
pub type PlayerId = CharacterId;

#[derive(
    Debug, Hiarc, Serialize, Deserialize, PartialEq, Eq, Copy, Clone, Hash, PartialOrd, Ord,
)]
pub struct ProjectileId(GameEntityId);

impl From<GameEntityId> for ProjectileId {
    fn from(value: GameEntityId) -> Self {
        Self(value)
    }
}

#[derive(
    Debug, Hiarc, Serialize, Deserialize, PartialEq, Eq, Copy, Clone, Hash, PartialOrd, Ord,
)]
pub struct LaserId(GameEntityId);

impl From<GameEntityId> for LaserId {
    fn from(value: GameEntityId) -> Self {
        Self(value)
    }
}

#[derive(
    Debug, Hiarc, Serialize, Deserialize, PartialEq, Eq, Copy, Clone, Hash, PartialOrd, Ord,
)]
pub struct PickupId(GameEntityId);

impl From<GameEntityId> for PickupId {
    fn from(value: GameEntityId) -> Self {
        Self(value)
    }
}

#[derive(
    Debug, Hiarc, Serialize, Deserialize, PartialEq, Eq, Copy, Clone, Hash, PartialOrd, Ord,
)]
pub struct CtfFlagId(GameEntityId);

impl From<GameEntityId> for CtfFlagId {
    fn from(value: GameEntityId) -> Self {
        Self(value)
    }
}

#[derive(
    Debug, Hiarc, Serialize, Deserialize, PartialEq, Eq, Copy, Clone, Hash, PartialOrd, Ord,
)]
pub struct StageId(GameEntityId);

impl From<GameEntityId> for StageId {
    fn from(value: GameEntityId) -> Self {
        Self(value)
    }
}
