#![allow(clippy::too_many_arguments)]

#[cfg(feature = "recorder")]
pub mod recorder;

pub mod utils;

use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use base::{hash::Hash, reduced_ascii_str::ReducedAsciiString};
use game_interface::{
    events::GameEvents,
    interface::GameStateCreateOptions,
    types::{game::NonZeroGameTickType, network_string::NetworkReducedAsciiString},
};
use serde::{Deserialize, Serialize};
use shared_base::network::{messages::GameModification, types::chat::NetChatMsg};

pub type DemoGameModification = GameModification;

/// The demo header, of const size.
/// A broken demo can be detected if [`DemoHeader::len`] or
/// [`DemoHeader::size_chunks`] is zero.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct DemoHeader {
    /// Length of the full demo
    pub len: Duration,
    /// Size to read for the whole [`DemoHeaderExt`] struct.
    pub size_ext: u64,
    /// Size to read for all chunks.
    pub size_chunks: u64,
}

/// The tail of the demo is written last,
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DemoTail {
    /// the key is the monotonic tick, while the value is the
    /// file offset relative to the beginning of the chunk.
    pub snapshots_index: BTreeMap<u64, u64>,
    /// the key is the monotonic tick, while the value is the
    /// file offset relative to the beginning of the chunk.
    pub events_index: BTreeMap<u64, u64>,
}

/// A more flexible header, that can contain dynamic sized elements.
/// Here header simply means, never changing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoHeaderExt {
    /// optional server name, address or whatever - can be left empty
    pub server: String,
    pub physics_mod: DemoGameModification,
    pub render_mod: DemoGameModification,
    /// resources the game **has** to load before
    /// the game/demo starts (e.g. because the game mod requires
    /// them for gameplay).
    pub required_resources: HashMap<String, String>,
    pub map: ReducedAsciiString,
    pub map_hash: Hash,
    pub ticks_per_second: NonZeroGameTickType,
    pub game_options: GameStateCreateOptions,
    pub physics_group_name: NetworkReducedAsciiString<24>,
}

/// When a chunk of snapshots or events ([`DemoRecorderChunk`]) is serialized, this header
/// is written.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkHeader {
    pub monotonic_tick: u64,
    pub size: u64,
}

pub type DemoSnapshot = Vec<u8>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DemoEvent {
    Game(GameEvents),
    Chat(NetChatMsg),
}

pub type DemoEvents = Vec<DemoEvent>;

// 50 here is the assumed snap send rate
// so it writes up to 30 seconds full of chunks
/// number of chunks to write at once
const DATA_PER_CHUNK_TO_WRITE: u64 = 30 * 50;
/// time offset so that even late packets have a chance
/// to be considered in the demo.
const SECONDS_UNTIL_WRITE: u64 = 3;
