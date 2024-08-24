use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

use game_interface::types::game::GameTickType;
use hashlink::LinkedHashMap;

#[derive(Debug)]
pub struct ServerDbgGame {
    pub time: Instant,
    pub tick_time: Duration,
    pub players: String,
    pub inputs: String,
    pub caller: String,
}

#[derive(Debug, Default)]
pub struct ServerInfo {
    pub sock_addr: std::sync::Mutex<Option<SocketAddr>>,
    pub dbg_game: std::sync::Mutex<LinkedHashMap<GameTickType, ServerDbgGame>>,
    pub rcon_secret: std::sync::Mutex<Option<[u8; 32]>>,
    /// client internal server,
    /// this server should only be reachable in LAN configurations
    pub is_internal_server: bool,
}

impl ServerInfo {
    pub fn new(is_internal_server: bool) -> Self {
        Self {
            is_internal_server,
            ..Default::default()
        }
    }
}
