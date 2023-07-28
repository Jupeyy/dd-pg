use std::time::Duration;

use bincode::{Decode, Encode};
use pool::mt_datatypes::PoolVec;
use serde::{Deserialize, Serialize};
use shared_base::network::messages::{MsgClInput, MsgClReady, MsgSvPlayerInfo, MsgSvServerInfo};
use shared_game::snapshot::snapshot::Snapshot;

#[derive(Serialize, Deserialize, Decode, Encode)]
pub enum ServerToClientMessage {
    QueueInfo(String),
    ServerInfo(MsgSvServerInfo),
    // overhead time (e.g. if the tick was calculated too late) & snapshot
    Snapshot((Duration, Snapshot)),
    PlayerInfo(MsgSvPlayerInfo),
    PlayerInfos(PoolVec<MsgSvPlayerInfo>),
    // a load event, e.g. because of a map change
    Load(MsgSvServerInfo),
}

#[derive(Decode, Encode)]
pub enum ClientToServerMessage {
    Ready(MsgClReady),
    Input(MsgClInput),
}

#[derive(Decode, Encode)]
pub enum GameMessage {
    ServerToClient(ServerToClientMessage),
    ClientToServer(ClientToServerMessage),
}
