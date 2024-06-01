use std::{sync::Arc, time::Duration};

use base::system::SystemTime;
use base::system::SystemTimeInterface;
use hashlink::LinkedHashMap;
use tokio::sync::Mutex;

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

#[derive(Debug)]
pub(crate) struct NetworkConnectionPingHandleImpl {
    pub(crate) handle_timestamp: Duration,

    pub(crate) ping_pong_peng_start_timestamp: Duration,
}

impl NetworkConnectionPingHandleImpl {
    fn new(add_timestamp: Duration) -> Self {
        Self {
            handle_timestamp: add_timestamp,

            ping_pong_peng_start_timestamp: Duration::ZERO,
        }
    }
}

#[derive(Debug)]
pub(crate) struct NetworkConnectionPingHandle {
    pub(crate) list: LinkedHashMap<u64, NetworkConnectionPingHandleImpl>,
}

impl NetworkConnectionPingHandle {
    pub(crate) fn new() -> Self {
        Self {
            list: Default::default(),
        }
    }

    pub(crate) fn remove_outdated(&mut self, cur_time: Duration) {
        // check if there are outdated ping handles
        while !self.list.is_empty() {
            if cur_time.saturating_sub(self.list.values().next().unwrap().handle_timestamp)
                > Duration::from_secs(2)
            {
                self.list.pop_front();
            } else {
                break;
            }
        }
    }

    pub(crate) fn try_remove(
        &mut self,
        identifier: &u64,
        sys: &Arc<SystemTime>,
    ) -> Option<NetworkConnectionPingHandleImpl> {
        let cur_time = sys.time_get_nanoseconds();
        self.remove_outdated(cur_time);

        self.list.remove(identifier)
    }

    pub(crate) fn try_add(
        &mut self,
        identifier: u64,
        cur_time: Duration,
    ) -> Result<&mut NetworkConnectionPingHandleImpl, ()> {
        self.remove_outdated(cur_time);

        /* TODO: 50 should not be harcoded */
        if self.list.len() < 50 {
            self.list
                .insert(identifier, NetworkConnectionPingHandleImpl::new(cur_time));
            Ok(self.list.values_mut().last().unwrap())
        } else {
            Err(())
        }
    }
}

#[derive(Debug)]
pub(crate) struct NetworkConnectionPingHandles {
    pub(crate) ping_handles: NetworkConnectionPingHandle,
    pub(crate) inc_ping_handles: NetworkConnectionPingHandle,
}

#[derive(Debug)]
pub(crate) struct NetworkConnection<C: Send + Sync> {
    pub(crate) conn: C,

    pub(crate) ping_handles: Mutex<NetworkConnectionPingHandles>,
}
