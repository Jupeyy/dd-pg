use std::net::SocketAddr;

pub struct ServerInfo {
    pub sock_addr: std::sync::Mutex<Option<SocketAddr>>,
}
