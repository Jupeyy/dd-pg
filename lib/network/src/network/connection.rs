use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub struct NetworkConnectionId(pub(crate) u64);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStats {
    pub ping: Duration,
    pub packets_lost: u64,
    pub packets_sent: u64,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
}

#[derive(Debug)]
pub(crate) struct NetworkConnection<C: Send + Sync> {
    pub(crate) conn: C,
}
