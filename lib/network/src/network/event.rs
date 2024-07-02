use std::{net::SocketAddr, sync::Arc};

use super::connection::ConnectionStats;

pub type NetworkStats = ConnectionStats;

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// [`SocketAddr`] represents:
    /// - client: the ip of the server
    /// - server: the ip of the client
    Connected {
        addr: SocketAddr,
        cert: Arc<x509_cert::Certificate>,
    },
    Disconnected {
        reason: String,
        /// if false, then the disconnect happened because of an error
        graceful: bool,
    },
    ConnectingFailed(String),
    NetworkStats(NetworkStats),
}
