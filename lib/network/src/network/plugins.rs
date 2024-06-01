use std::net::SocketAddr;

use async_trait::async_trait;

use super::connection::NetworkConnectionID;

/// Plugin system interface for packets:
/// - modify a raw buffer before being sent
/// - modify a raw buffer before being read
/// Respects the order in which plugins are passed, the first plugin will always modify a write
/// buffer as last, and modify a read packet as first
#[async_trait]
pub trait NetworkPluginPacket: Sync + Send + 'static {
    async fn prepare_write(
        &self,
        id: &NetworkConnectionID,
        buffer: &mut Vec<u8>,
    ) -> anyhow::Result<()>;
    async fn prepare_read(
        &self,
        id: &NetworkConnectionID,
        buffer: &mut Vec<u8>,
    ) -> anyhow::Result<()>;
}

/// Plugin system interface for connection related events:
/// - can listen for on_connect events (e.g. to drop connections by IP, or modify the socket addr to emulate a proxy)
/// Respects the order in which plugins are passed, the first plugin will always listen for on_connect events first
#[async_trait]
pub trait NetworkPluginConnection: Sync + Send + 'static {
    /// returns true if the connection should be dropped
    async fn on_connect(
        &self,
        id: &NetworkConnectionID,
        remote_addr: &mut SocketAddr,
    ) -> anyhow::Result<bool>;
}
