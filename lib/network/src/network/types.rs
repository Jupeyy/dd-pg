use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use pool::mt_datatypes::PoolVec;

use tokio::sync::Mutex as TokioMutex;

use super::connection::NetworkConnectionId;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NetworkInOrderChannel {
    Global,
    Custom(usize),
}

pub(crate) enum NetworkEventSendType {
    /// packet loss possible, out of order possible
    UnreliableUnordered,
    /// packet loss **not** possible, out of order possible
    ReliableUnordered,
    /// Tries to send as unrealible first, if unsupported
    /// or packet too big for a single packet, falls back
    /// to reliable.
    UnorderedAuto,
    /// packet loss **not** possible, **in-order**
    ReliableOrdered(NetworkInOrderChannel),
}

pub(crate) enum NetworkLogicEvent {
    Connect(NetworkConnectionId, String),
    Disconnect(NetworkConnectionId),
    Send((NetworkConnectionId, PoolVec<u8>, NetworkEventSendType)),
    Kick(NetworkConnectionId),
}

pub(crate) type NetworkPacket = PoolVec<u8>;

pub(crate) type NetworkInOrderPackets = HashMap<
    NetworkConnectionId,
    HashMap<NetworkInOrderChannel, Arc<TokioMutex<VecDeque<NetworkPacket>>>>,
>;
