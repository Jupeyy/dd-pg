use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub struct NetworkConnectionID(pub(crate) u64);
pub(crate) const MIN_NETWORK_CON_IDENTIFIER: NetworkConnectionID = NetworkConnectionID(1);
pub(crate) const INVALID_NETWORK_CON_IDENTIFIER: NetworkConnectionID = NetworkConnectionID(0);

impl Default for NetworkConnectionID {
    fn default() -> Self {
        INVALID_NETWORK_CON_IDENTIFIER
    }
}

impl NetworkConnectionID {
    // only for tests
    #[cfg(test)]
    pub(super) fn get_index_unsafe(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStats {
    pub ping: Duration,
    pub packets_lost: u64,
    pub packets_sent: u64,
}

#[derive(Debug)]
pub(crate) struct NetworkConnection<C: Send + Sync> {
    pub(crate) conn: C,
}
