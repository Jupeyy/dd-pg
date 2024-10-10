use hiarc::Hiarc;
use pool::datatypes::{PoolLinkedHashMap, PoolLinkedHashSet};
use serde::{Deserialize, Serialize};

use super::{game::GameEntityId, render::character::PlayerCameraMode};

/// When the server (or client) requests a snapshot it usually requests it for
/// certain players (from the view of these players).
///
/// Additionally it might want to opt-in into snapping everything etc.
/// For server-side demos, it's possible that no player is requested.
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub enum SnapshotClientInfo {
    /// A list of players the client requests the snapshot for.
    /// Usually these are the local players (including the dummy).
    ForPlayerIds(PoolLinkedHashSet<GameEntityId>),
    /// All stages (a.k.a. ddrace teams) should be snapped
    /// (the client usually renders them with some transparency)
    OtherStagesForPlayerIds(PoolLinkedHashSet<GameEntityId>),
    /// Everything should be snapped
    Everything,
}

/// Information about the local players from the opaque snapshot
#[derive(Debug, Hiarc, Clone, Copy, Serialize, Deserialize)]
pub struct SnapshotLocalPlayer {
    /// a hint to the client that the local player was added as dummy
    pub is_dummy: bool,
    /// What camera mode the player currently uses during input
    /// handling.
    pub input_cam_mode: PlayerCameraMode,
}

/// A parsed snapshot must return this information, which is usually parsed by the client
pub type SnapshotLocalPlayers = PoolLinkedHashMap<GameEntityId, SnapshotLocalPlayer>;

/// Mode for building the game state from a snapshot
#[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
pub enum FromSnapshotBuildMode {
    /// Just build the snapshot as is
    #[default]
    Default,
    /// Most entities should not be predicted.
    /// The implementation keeps the character entities and
    /// connected entities (e.g. projectiles etc.) of the given
    /// id list untouched (predicted), but resets all other entities.
    Unpredicted {
        keep_characters: PoolLinkedHashSet<GameEntityId>,
    },
}
