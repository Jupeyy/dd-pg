use std::time::Duration;

use game_interface::{
    events::GameEvents,
    types::game::{GameEntityId, GameTickType},
    votes::{MapVote, VoteState, VoteType, Voted},
};
use pool::mt_datatypes::PoolCow;
use serde::{Deserialize, Serialize};
use shared_base::network::messages::{
    MsgClAddLocalPlayer, MsgClChatMsg, MsgClInputs, MsgClLoadVotes, MsgClReady, MsgClSnapshotAck,
    MsgSvChatMsg, MsgSvServerInfo,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MsgSvInputAck {
    pub id: u64,
    /// Logic overhead here means that the server does not
    /// directly ack an input and how ever long it took
    /// for the input packet from arriving to ack'ing, that
    /// is the overhead time here.
    pub logic_overhead: Duration,
}

/// List of votes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MsgSvLoadVotes {
    Map { votes: Vec<MapVote> },
    Misc {},
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerToClientMessage<'a> {
    QueueInfo(String),
    ServerInfo {
        info: MsgSvServerInfo,
        /// To make the first ping estimation better the server adds
        /// the overhead from network to when this msg is sent.
        overhead: Duration,
    },
    Snapshot {
        /// overhead time: (e.g. if the tick was calculated too late relative to the tick time) + the overhead from the simulation itself etc.
        overhead_time: Duration,
        snapshot: PoolCow<'a, [u8]>,
        /// diff_id: optional snapshot id to which to apply a binary diff against
        diff_id: Option<u64>,
        /// id of this snapshot
        /// if `diff_id` is `Some`, this value must be added to the diff id
        /// to get the real `snap_id`
        snap_id_diffed: u64,
        /// a strict monotonic tick that is used client side to
        /// make synchronization with the server easier
        /// (for example for sending inputs) and/or
        /// to know the difference between two snapshots, e.g.
        /// for demo replay.
        /// if `diff_id` is `Some`, this value must be added to the
        /// monotonic tick of the related diff
        /// to get the real `game_monotonic_tick`.
        game_monotonic_tick_diff: GameTickType,
        /// the client should _try_ to store this snap
        /// for snapshot differences.
        as_diff: bool,
        /// An input is ack'd by the server,
        /// Note that the server doesn't care if the input packet
        /// actually contained player inputs.
        input_ack: PoolCow<'a, [MsgSvInputAck]>,
    },
    Events {
        /// see Snapshot variant
        game_monotonic_tick: GameTickType,
        events: GameEvents,
    },
    // a load event, e.g. because of a map change
    Load(MsgSvServerInfo),
    Chat(MsgSvChatMsg),
    /// A value of `None` must be interpreted as no vote active.
    Vote(Option<VoteState>),
    LoadVote(MsgSvLoadVotes),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientToServerPlayerMessage {
    RemLocalPlayer,
    Chat(MsgClChatMsg),
    Kill,
    StartVote(VoteType),
    Voted(Voted),
}

#[derive(Serialize, Deserialize)]
pub enum ClientToServerMessage<'a> {
    Ready(MsgClReady),
    AddLocalPlayer(MsgClAddLocalPlayer),
    PlayerMsg((GameEntityId, ClientToServerPlayerMessage)),
    Inputs {
        /// unique id that identifies this packet (for acks)
        id: u64,
        inputs: MsgClInputs,
        snap_ack: PoolCow<'a, [MsgClSnapshotAck]>,
    },
    LoadVotes(MsgClLoadVotes),
}

#[derive(Serialize, Deserialize)]
pub enum GameMessage<'a> {
    ServerToClient(ServerToClientMessage<'a>),
    ClientToServer(ClientToServerMessage<'a>),
}
