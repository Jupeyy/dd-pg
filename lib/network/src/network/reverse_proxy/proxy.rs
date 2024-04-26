use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use anyhow::anyhow;
use async_trait::async_trait;
use base::system::System;
use ed25519_dalek::Signature;
use pool::mt_pool::Pool;
use tokio::sync::{Mutex, RwLock};

use crate::network::{
    connection::NetworkConnectionID,
    event::NetworkEvent,
    event_generator::NetworkEventToGameEventGenerator,
    network::{NetworkClientCertCheckMode, NetworkClientInitOptions, NetworkServerCertMode},
    notifier::NetworkEventNotifier,
    quinn_network::QuinnNetwork,
    quinnminimal::create_certificate,
    types::NetworkInOrderChannel,
};

use super::{client::ReverseProxyClient, shared::ReverseProxyPacketHeader};

pub enum ProxyEvent {
    Network(NetworkEvent),
    User(Vec<u8>),
}

pub struct ProxyEventGenerator {
    events: Arc<Mutex<VecDeque<(NetworkConnectionID, Duration, ProxyEvent)>>>,
}

#[async_trait]
impl NetworkEventToGameEventGenerator for ProxyEventGenerator {
    async fn generate_from_binary(
        &self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        bytes: &[u8],
        _signature: Option<Signature>,
    ) {
        self.events
            .lock()
            .await
            .push_back((*con_id, timestamp, ProxyEvent::User(bytes.to_vec())));
    }

    async fn generate_from_network_event(
        &self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        network_event: &NetworkEvent,
    ) -> bool {
        self.events.lock().await.push_back((
            *con_id,
            timestamp,
            ProxyEvent::Network(network_event.clone()),
        ));
        true
    }
}

struct Networks {
    // proxy -> real server
    to_server_network: Mutex<QuinnNetwork>,
    to_server_network_ntfy: NetworkEventNotifier,
    // real client -> proxy
    from_clients_network: Mutex<QuinnNetwork>,
    from_clients_network_ntfy: NetworkEventNotifier,
}

/// the actual proxy instance
pub struct ReverseProxy {
    clients: Arc<RwLock<HashMap<NetworkConnectionID, SocketAddr>>>,

    networks: Networks,

    client_events: Arc<ProxyEventGenerator>,
    server_events: Arc<ProxyEventGenerator>,

    helper_pool: Pool<Vec<u8>>,
}

impl ReverseProxy {
    pub fn new(server_cert: Vec<u8>, sys: System) -> Self {
        let client_events = Arc::new(ProxyEventGenerator {
            events: Default::default(),
        });
        let server_events = Arc::new(ProxyEventGenerator {
            events: Default::default(),
        });

        let clients: Arc<RwLock<HashMap<NetworkConnectionID, SocketAddr>>> = Default::default();

        let helper_pool = Pool::with_capacity(64);
        let to_server_plugin = Arc::new(ReverseProxyClient::new(
            clients.clone(),
            helper_pool.clone(),
        ));

        let (to_server_network, to_server_network_ntfy) = QuinnNetwork::init_client(
            "0.0.0.0:0",
            server_events.clone(),
            &sys,
            NetworkClientInitOptions::new(NetworkClientCertCheckMode::CheckByCert {
                cert: &server_cert,
            }),
            Arc::new(vec![to_server_plugin.clone()]),
            Arc::new(vec![to_server_plugin.clone()]),
            None,
        );

        let cert = create_certificate();

        let (from_clients_network, _, _, from_clients_network_ntfy) = QuinnNetwork::init_server(
            "0.0.0.0:0",
            client_events.clone(),
            NetworkServerCertMode::FromCert { cert: &cert },
            &sys,
            Default::default(),
            Default::default(),
            Default::default(),
            None,
        );

        Self {
            clients,
            networks: Networks {
                from_clients_network: Mutex::new(from_clients_network),
                from_clients_network_ntfy,
                to_server_network: Mutex::new(to_server_network),
                to_server_network_ntfy,
            },

            client_events,
            server_events,

            helper_pool,
        }
    }

    pub fn run(&self) {
        loop {
            let client_events = std::mem::take(&mut *self.client_events.events.blocking_lock());
            for (id, _, ev) in client_events {
                match ev {
                    ProxyEvent::Network(ev) => match ev {
                        NetworkEvent::Connected(remote_addr) => {
                            self.clients.blocking_write().insert(id, remote_addr);
                        }
                        NetworkEvent::Disconnected(_) => {
                            self.clients.blocking_write().remove(&id);
                        }
                        NetworkEvent::ConnectingFailed(_) => {}
                        NetworkEvent::NetworkStats(_) => {}
                    },
                    ProxyEvent::User(buffer) => {
                        // rewrite the packet
                        if let Ok(remote_addr) = self
                            .clients
                            .blocking_read()
                            .get(&id)
                            .cloned()
                            .ok_or_else(|| anyhow!("client with that id not found."))
                        {
                            let mut packet = self.helper_pool.new();
                            let packet: &mut Vec<_> = &mut packet;

                            let header = ReverseProxyPacketHeader { remote_addr };

                            if let Ok(_) = bincode::serde::encode_into_std_write(
                                &header,
                                packet,
                                bincode::config::standard(),
                            ) {
                                packet.extend(buffer.iter());

                                self.networks
                                    .to_server_network
                                    .blocking_lock()
                                    .send_in_order_to_server(packet, NetworkInOrderChannel::Global);
                            }
                        }
                    }
                }
            }

            let server_events = std::mem::take(&mut *self.server_events.events.blocking_lock());
            for (_, _, ev) in server_events {
                match ev {
                    ProxyEvent::Network(ev) => match ev {
                        NetworkEvent::Connected(_) => {}
                        NetworkEvent::Disconnected(_) => {
                            // TODO: try to reconnect
                        }
                        NetworkEvent::ConnectingFailed(_) => {
                            // TODO: try to reconnect
                        }
                        NetworkEvent::NetworkStats(_) => {}
                    },
                    ProxyEvent::User(_) => {
                        //self.clients.blocking_read().
                    }
                }
            }

            self.networks
                .from_clients_network_ntfy
                .join(&self.networks.to_server_network_ntfy)
                .wait_for_event(None);
        }
    }
}
