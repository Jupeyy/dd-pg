use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use async_trait::async_trait;
use pool::mt_pool::Pool;
use tokio::sync::RwLock;

use crate::network::{
    connection::NetworkConnectionID,
    plugins::{NetworkPluginConnection, NetworkPluginPacket},
};

/// Client here is the proxy that is connected to the "main" system (the server).
/// The client wraps the real remote address of the connected
/// client into a structure that the server can interpret.
/// # Important
/// The instance of this struct must be passed as both connection plugin & packet plugin
/// to a server network (the point the "real" clients connect to).
/// It's recommended to pass it as first connection plugin, but as a packet plugin
/// that happens before compression (rewrite packet -> compress, decompress -> recreate real packet).
pub struct ReverseProxyClient {
    clients: Arc<RwLock<HashMap<NetworkConnectionID, SocketAddr>>>,
    /// this network is used to connect to the "real" server
    helper_pool: Pool<Vec<u8>>,
}

impl ReverseProxyClient {
    pub fn new(
        clients: Arc<RwLock<HashMap<NetworkConnectionID, SocketAddr>>>,
        helper_pool: Pool<Vec<u8>>,
    ) -> Self {
        Self {
            clients,
            helper_pool,
        }
    }
}

#[async_trait]
impl NetworkPluginConnection for ReverseProxyClient {
    async fn on_connect(
        &self,
        id: &NetworkConnectionID,
        remote_addr: &mut SocketAddr,
    ) -> anyhow::Result<bool> {
        self.clients.write().await.insert(*id, *remote_addr);

        Ok(false)
    }
}

#[async_trait]
impl NetworkPluginPacket for ReverseProxyClient {
    async fn prepare_write(
        &self,
        id: &NetworkConnectionID,
        buffer: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    async fn prepare_read(
        &self,
        _id: &NetworkConnectionID,
        _buffer: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
