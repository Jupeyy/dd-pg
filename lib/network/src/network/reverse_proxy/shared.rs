use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReverseProxyPacketHeader {
    pub remote_addr: SocketAddr,
}
