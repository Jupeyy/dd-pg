use std::time::Duration;

use bincode::{Decode, Encode};
use pool::mt_datatypes::PoolVec;
use serde::{Deserialize, Serialize};
use shared_base::{
    game_types::TGameElementID,
    network::messages::{
        MsgClAddLocalPlayer, MsgClChatMsg, MsgClInput, MsgClReady, MsgSvChatMsg, MsgSvKillfeedMsg,
        MsgSvPlayerInfo, MsgSvServerInfo, MsgSvSystemMsg,
    },
};
use shared_game::snapshot::snapshot::Snapshot;

#[derive(Serialize, Deserialize, Decode, Encode)]
pub enum ServerToClientMessage {
    QueueInfo(String),
    ServerInfo(MsgSvServerInfo),
    /// overhead time: (e.g. if the tick was calculated too late relative to the tick time) + the overhead from the simulation itself etc.
    Snapshot {
        overhead_time: Duration,
        snapshot: Snapshot,
    },
    PlayerInfo(MsgSvPlayerInfo),
    PlayerInfos(PoolVec<MsgSvPlayerInfo>),
    // a load event, e.g. because of a map change
    Load(MsgSvServerInfo),
    Chat(MsgSvChatMsg),
    System(MsgSvSystemMsg),
    Killfeed(MsgSvKillfeedMsg),
}

#[derive(Decode, Encode)]
pub enum ClientToServerPlayerMessage {
    Input(MsgClInput),
    RemLocalPlayer,
    Chat(MsgClChatMsg),
}

#[derive(Decode, Encode)]
pub enum ClientToServerMessage {
    Ready(MsgClReady),
    AddLocalPlayer(MsgClAddLocalPlayer),
    PlayerMsg((TGameElementID, ClientToServerPlayerMessage)),
}

#[derive(Decode, Encode)]
pub enum GameMessage {
    ServerToClient(ServerToClientMessage),
    ClientToServer(ClientToServerMessage),
}
