use base::hash::Hash;
use game_interface::{
    interface::GameStateServerOptions,
    types::{
        character_info::NetworkCharacterInfo,
        game::{GameEntityId, GameTickType},
        network_string::{NetworkReducedAsciiString, NetworkString},
    },
    votes::MAX_MAP_NAME_LEN,
};
use math::math::vector::vec2;
use pool::mt_datatypes::{PoolLinkedHashMap, PoolVec};
use serde::{Deserialize, Serialize};

use crate::player_input::PlayerInput;

use super::types::chat::NetChatMsg;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameModification {
    Native,
    Ddnet,
    Wasm {
        /// Name of the game mod to play
        name: NetworkReducedAsciiString<MAX_GAME_MOD_NAME_LEN>,
        /// Since this variant can be downloaded over network,
        /// it must also add the hash to it.
        hash: Hash,
    },
}

const MAX_GAME_MOD_NAME_LEN: usize = 32;
/// All information about the server
/// so that the client can prepare the game.
/// E.g. current map
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgSvServerInfo {
    /// the map that is currently played on
    pub map: NetworkReducedAsciiString<MAX_MAP_NAME_LEN>,
    pub map_blake3_hash: Hash,
    /// The game mod to play, see the config variable to
    /// read about reserved names
    pub game_mod: GameModification,
    /// The serialized optional config for the mod.
    /// The mod must load this and deal with errors automatically.
    /// This is meant to be similar to [`Self::server_options`] just
    /// more flexable and inside the physics mod.
    pub mod_config: Option<Vec<u8>>,
    /// Options of the server the client should know about
    pub server_options: GameStateServerOptions,
    /// - if this is `Some`, it is the port to the fallback resource download server.
    /// - if this is `None`, either resources are downloaded from a official resource
    ///     server or from a resource server stored in the server
    ///     browser information of this server.
    ///
    /// If both cases don't exist, no resources are downloaded, the client might stop connecting.
    /// Note: this is intentionally only a port. If the server contains a resource server in their
    /// server browser info, the client makes sure that the said server relates to this server
    /// (e.g. by a domain + subdomain DNS resolve check)
    pub resource_server_fallback: Option<u16>,
    /// as soon as the client has finished loading it might want to render something to the screen
    /// the server can give a hint what the best camera position is for that
    pub hint_start_camera_pos: vec2,
    /// Whether this server supports spatial chat.
    pub spatial_chat: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MsgSvChatMsg {
    pub msg: NetChatMsg,
}

// # client -> server

#[derive(Serialize, Deserialize)]
pub struct MsgClReady {
    pub player_info: NetworkCharacterInfo,

    /// Optional rcon secret, that should be tried to auth
    /// for rcon access.
    pub rcon_secret: Option<[u8; 32]>,
}

#[derive(Serialize, Deserialize)]
pub struct MsgClAddLocalPlayer {
    pub player_info: NetworkCharacterInfo,

    /// a hint for the server that this local player is a dummy
    pub as_dummy: bool,
}

/// Input that can easily be de-/serialized in a chain, see [`MsgClInputPlayerChain`].
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Default)]
pub struct PlayerInputChainable {
    pub inp: PlayerInput,
    pub for_monotonic_tick: GameTickType,
}

/// The input chain can contain multiple inputs for multiple
/// monotonic ticks.
///
/// # Serialization
/// The first [`MsgClInputPlayer`] uses the player's
/// diff [`MsgClInputPlayer`] (which is the last ack'd input by
/// the server) or [`MsgClInputPlayer::default`] if None such exists.
/// All other inputs in the chain use the previous [`MsgClInputPlayer`].
/// So the second uses the first, the third the second etc.
#[derive(Debug, Serialize, Deserialize)]
pub struct MsgClInputPlayerChain {
    /// The chain of [`PlayerInputChainable`]s (plural)
    pub data: PoolVec<u8>,
    pub diff_id: Option<u64>,
    /// Use this input for this player as diff.
    pub as_diff: bool,
}

pub type MsgClInputs = PoolLinkedHashMap<GameEntityId, MsgClInputPlayerChain>;

/// Acknowledgement from the client that a snapshot arrived.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MsgClSnapshotAck {
    pub snap_id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MsgClChatMsg {
    Global {
        msg: NetworkString<256>,
    },
    GameTeam {
        team: u32, // TODO
        msg: NetworkString<256>,
    },
    Whisper {
        receiver_id: GameEntityId,
        msg: NetworkString<256>,
    },
}

/// Load a list of votes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MsgClLoadVotes {
    Map {
        /// The blake3 hash as if the votes were serialized as json
        cached_votes: Option<Hash>,
    },
    Misc {
        /// The blake3 hash as if the votes were serialized as json
        cached_votes: Option<Hash>,
    },
}
