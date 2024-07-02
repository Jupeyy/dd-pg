use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use pool::mt_datatypes::PoolVec;

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

pub(crate) type NetworkPacket = PoolVec<u8>;

pub(crate) type NetworkInOrderPackets = HashMap<
    NetworkConnectionID,
    HashMap<NetworkInOrderChannel, Arc<TokioMutex<VecDeque<NetworkPacket>>>>,
>;
