use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use pool::mt_datatypes::PoolVec;
use serde::{Deserialize, Serialize};

use tokio::sync::Mutex as TokioMutex;

use super::connection::NetworkConnectionID;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NetworkInOrderChannel {
    Global,
    Custom(usize),
}

pub(crate) enum NetworkEventSendType {
    // packet loss possible, out of order possible
    UnreliableUnordered,
    // packet loss **not** possible, out of order possible
    ReliableUnordered,
    // packet loss **not** possible, **in-order**
    ReliableOrdered(NetworkInOrderChannel),
}

pub(crate) enum NetworkLogicEvent {
    Connect(NetworkConnectionID, String),
    Disconnect(NetworkConnectionID),
    Close(),
    Send((NetworkConnectionID, PoolVec<u8>, NetworkEventSendType)),
}

#[derive(Serialize, Deserialize)]
pub(crate) enum InternalPingNetworkPackets {
    // con1 sends packet to con2
    Ping,
    // con2 responds to ping of con1
    Pong,
    // con1 resends a packet to con2, so con2 also has the ping
    Peng,
}

#[derive(Serialize, Deserialize)]
pub(crate) enum InternalNetworkPackets {
    // all P*ng carry an unique identifier
    PingFamily(u64, InternalPingNetworkPackets),
}

#[derive(Serialize, Deserialize)]
pub(crate) enum NetworkPacket {
    Internal(InternalNetworkPackets),
    User(PoolVec<u8>),
}

pub(crate) type NetworkInOrderPackets = HashMap<
    NetworkConnectionID,
    HashMap<NetworkInOrderChannel, Arc<TokioMutex<VecDeque<NetworkPacket>>>>,
>;
