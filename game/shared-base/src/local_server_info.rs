use std::{
    net::SocketAddr,
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};

use base::join_thread::JoinThread;
use game_interface::types::game::GameTickType;
use hashlink::LinkedHashMap;

use crate::server_browser::ServerBrowserInfo;

#[derive(Debug)]
pub struct ServerDbgGame {
    pub time: Instant,
    pub tick_time: Duration,
    pub players: String,
    pub projectiles: String,
    pub inputs: String,
    pub caller: String,
}

#[derive(Debug)]
pub struct LocalServerThread {
    pub server_is_open: Arc<AtomicBool>,
    // must be the last entry
    pub _thread: JoinThread<()>,
}

impl Drop for LocalServerThread {
    fn drop(&mut self) {
        self.server_is_open
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub struct LocalServerConnectInfo {
    pub sock_addr: SocketAddr,
    pub dbg_games: LinkedHashMap<GameTickType, ServerDbgGame>,
    pub rcon_secret: [u8; 32],
    pub server_cert_hash: [u8; 32],
}

#[derive(Debug, Default)]
pub enum LocalServerState {
    #[default]
    None,
    Starting {
        server_cert_hash: [u8; 32],
        // must be last
        thread: LocalServerThread,
    },
    Ready {
        connect_info: LocalServerConnectInfo,
        // must be last
        thread: LocalServerThread,
        browser_info: Option<ServerBrowserInfo>,
    },
}

#[derive(Debug, Default)]
pub struct LocalServerInfo {
    pub state: std::sync::Mutex<LocalServerState>,
    /// client internal server,
    /// this server should only be reachable in LAN configurations
    pub is_internal_server: bool,
}

impl LocalServerInfo {
    pub fn new(is_internal_server: bool) -> Self {
        Self {
            is_internal_server,
            ..Default::default()
        }
    }
}
