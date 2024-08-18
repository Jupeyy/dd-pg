use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr},
};

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::{connection::NetworkConnectionId, plugins::NetworkPluginConnection};

#[derive(Debug, Default)]
pub struct BanState {
    ipv4_bans: iprange::IpRange<ipnet::Ipv4Net>,
    ipv6_bans: iprange::IpRange<ipnet::Ipv6Net>,
    active_connections: HashMap<IpAddr, HashSet<NetworkConnectionId>>,
}

/// plugin to disallow/ban certain connections
#[derive(Debug, Default)]
pub struct ConnectionBans {
    state: Mutex<BanState>,
}

#[async_trait]
impl NetworkPluginConnection for ConnectionBans {
    async fn on_incoming(&self, remote_addr: &SocketAddr) -> anyhow::Result<bool> {
        let should_drop_connection = match remote_addr.ip() {
            IpAddr::V4(ip) => self
                .state
                .lock()
                .await
                .ipv4_bans
                .contains(&ipnet::Ipv4Net::new(ip, 32)?),
            IpAddr::V6(ip) => self
                .state
                .lock()
                .await
                .ipv6_bans
                .contains(&ipnet::Ipv6Net::new(ip, 128)?),
        };

        Ok(!should_drop_connection)
    }
    async fn on_connect(&self, id: &NetworkConnectionId, remote_addr: &SocketAddr) {
        self.state
            .lock()
            .await
            .active_connections
            .entry(remote_addr.ip())
            .or_default()
            .insert(*id);
    }
    async fn on_disconnect(&self, id: &NetworkConnectionId, remote_addr: &SocketAddr) {
        let mut state = self.state.lock().await;
        if let Some(connections) = state.active_connections.get_mut(&remote_addr.ip()) {
            connections.remove(id);
            if connections.is_empty() {
                state.active_connections.remove(&remote_addr.ip());
            }
        }
    }
}

impl ConnectionBans {
    /// Returns all network ids for that ip.
    #[must_use]
    pub fn ban_ip(&self, ip: IpAddr) -> HashSet<NetworkConnectionId> {
        let mut state = self.state.blocking_lock();
        let ids = state
            .active_connections
            .get(&ip)
            .cloned()
            .unwrap_or_default();

        match ip {
            IpAddr::V4(ip) => {
                state.ipv4_bans.add(ip.into());
            }
            IpAddr::V6(ip) => {
                state.ipv6_bans.add(ip.into());
            }
        }

        ids
    }
}
