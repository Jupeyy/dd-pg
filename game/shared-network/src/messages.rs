use std::time::Duration;

use game_interface::{
    events::GameEvents,
    types::game::{GameEntityId, GameTickType},
};
use pool::mt_datatypes::PoolVec;
use serde::{Deserialize, Serialize};
use shared_base::network::messages::{
    MsgClAddLocalPlayer, MsgClChatMsg, MsgClInputs, MsgClReady, MsgSvChatMsg, MsgSvServerInfo,
};

#[derive(Serialize, Deserialize)]
pub enum ServerToClientMessage {
    QueueInfo(String),
    ServerInfo {
        info: MsgSvServerInfo,
    },
    Snapshot {
        /// overhead time: (e.g. if the tick was calculated too late relative to the tick time) + the overhead from the simulation itself etc.
        overhead_time: Duration,
        snapshot: PoolVec<u8>,
        /// diff_id: optional snapshot id to which to apply a binary diff against
        diff_id: Option<u64>,
        /// id of this snapshot
        snap_id: u64,
        /// a strict monotonic tick that is used client side to
        /// make synchronization with the server easier
        /// (for example for sending inputs) and/or
        /// to know the difference between two snapshots, e.g.
        /// for demo replay.
        game_monotonic_tick: GameTickType,
        /// the client should _try_ to store this snap
        /// for snapshot differences.
        as_diff: bool,
    },
    InputAck {
        inp_id: u64,
    },
    Events {
        /// see Snapshot variant
        game_monotonic_tick: GameTickType,
        events: GameEvents,
    },
    // a load event, e.g. because of a map change
    Load(MsgSvServerInfo),
    Chat(MsgSvChatMsg),
}

#[derive(Serialize, Deserialize)]
pub enum ClientToServerPlayerMessage {
    RemLocalPlayer,
    Chat(MsgClChatMsg),
    Kill,
}

#[derive(Serialize, Deserialize)]
pub enum ClientToServerMessage {
    Ready(MsgClReady),
    AddLocalPlayer(MsgClAddLocalPlayer),
    PlayerMsg((GameEntityId, ClientToServerPlayerMessage)),
    Inputs(MsgClInputs),
    SnapshotAck {
        snap_id: u64,
        /// the client stored this snapshot for
        /// snapshot differences
        as_diff: bool,
    },
}

#[derive(Serialize, Deserialize)]
pub struct ClientToServerMessageSignatured {
    pub msg: ClientToServerMessage,
    pub signature: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub enum GameMessage {
    ServerToClient(ServerToClientMessage),
    ClientToServer(ClientToServerMessage),
}
