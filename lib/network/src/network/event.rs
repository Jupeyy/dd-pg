use std::{net::SocketAddr, sync::Arc, time::Duration};

#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub ping: Duration,
}

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// [`SocketAddr`] represents:
    /// - client: the ip of the server
    /// - server: the ip of the client
    Connected {
        addr: SocketAddr,
        public_key: Arc<Vec<u8>>,
    },
    Disconnected {
        reason: String,
        /// if false, then the disconnect happened because of an error
        graceful: bool,
    },
    ConnectingFailed(String),
    NetworkStats(NetworkStats),
}
