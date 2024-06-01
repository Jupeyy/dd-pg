use hiarc::Hiarc;
use pool::datatypes::{PoolLinkedHashMap, PoolLinkedHashSet};
use serde::{Deserialize, Serialize};

use super::game::GameEntityId;

/// When the server (or client) requests a snapshot it usually requests it for
/// certain players (from the view of these players).
/// Additionally it might want to opt-in into snapping everything etc.
/// For server-side demos, it's possible that no player is requested.
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct SnapshotClientInfo {
    /// A list of players the client requests the snapshot for.
    /// Usually these are the local players (including the dummy).
    pub client_player_ids: PoolLinkedHashSet<GameEntityId>,
    /// A hint that everything should be snapped, regardless of the requested players
    pub snap_everything: bool,
    /// A hint that all stages (a.k.a. ddrace teams) should be snapped
    /// (the client usually renders them with some transparency)
    pub snap_other_stages: bool,
}

/// Information about the local players from the opaque snapshot
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct SnapshotLocalPlayer {
    /// a hint to the client that the local player was added as dummy
    pub is_dummy: bool,
}

/// A parsed snapshot must return this information, which is usually parsed by the client
pub type SnapshotLocalPlayers = PoolLinkedHashMap<GameEntityId, SnapshotLocalPlayer>;
