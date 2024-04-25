use std::{net::SocketAddr, time::Duration};

#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub ping: Duration,
}

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// [`SocketAddr`] represents:
    /// - client: the ip of the server
    /// - server: the ip of the client
    Connected(SocketAddr),
    Disconnected(String),
    ConnectingFailed(String),
    NetworkStats(NetworkStats),
}
