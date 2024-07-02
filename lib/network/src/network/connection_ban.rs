use std::net::{IpAddr, SocketAddr};

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::{connection::NetworkConnectionID, plugins::NetworkPluginConnection};

/// plugin to disallow/ban certain connections
#[derive(Debug)]
pub struct ConnectionBans {
    ipv4_bans: Mutex<iprange::IpRange<ipnet::Ipv4Net>>,
    ipv6_bans: Mutex<iprange::IpRange<ipnet::Ipv6Net>>,
}

#[async_trait]
impl NetworkPluginConnection for ConnectionBans {
    async fn on_incoming(&self, remote_addr: &SocketAddr) -> anyhow::Result<bool> {
        let should_drop_connection = match remote_addr.ip() {
            IpAddr::V4(ip) => self
                .ipv4_bans
                .lock()
                .await
                .contains(&ipnet::Ipv4Net::new(ip, 32)?),
            IpAddr::V6(ip) => self
                .ipv6_bans
                .lock()
                .await
                .contains(&ipnet::Ipv6Net::new(ip, 128)?),
        };

        Ok(!should_drop_connection)
    }
    async fn on_connect(&self, _id: &NetworkConnectionID, _remote_addr: &SocketAddr) {}
}
