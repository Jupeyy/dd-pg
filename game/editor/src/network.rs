use std::{collections::HashSet, sync::Arc, time::Duration};

use base::system::System;
use network::network::{
    connection::NetworkConnectionID,
    event::NetworkEvent,
    network::{
        NetworkClientCertCheckMode, NetworkClientInitOptions, NetworkServerCertMode,
        NetworkServerInitOptions,
    },
    quinn_network::QuinnNetwork,
    quinnminimal::create_certificate,
    types::NetworkInOrderChannel,
};

use crate::event::{EditorEvent, EditorEventGenerator};

/// small wrapper around network for needs of editor
pub struct EditorNetwork {
    network: QuinnNetwork,

    is_server: bool,

    connections: HashSet<NetworkConnectionID>,
}

impl EditorNetwork {
    pub fn new_server(
        sys: &System,
        event_generator: Arc<EditorEventGenerator>,
        cert: Option<NetworkServerCertMode>,
        port: Option<u16>,
    ) -> (Self, Vec<u8>, u16) {
        let mut cert_dummy = None;

        let (network, server_cert, addr, _) = QuinnNetwork::init_server(
            &format!("0.0.0.0:{}", port.unwrap_or_default()),
            event_generator.clone(),
            cert.unwrap_or_else(|| {
                cert_dummy = Some(create_certificate());
                NetworkServerCertMode::FromCert {
                    cert: cert_dummy.as_ref().unwrap(),
                }
            }),
            sys,
            NetworkServerInitOptions::new()
                .with_max_thread_count(6)
                .with_timeout(Duration::from_secs(120)),
            Default::default(),
            Default::default(),
            None,
        );
        let port = addr.port();
        (
            Self {
                network,
                is_server: true,
                connections: Default::default(),
            },
            server_cert,
            port,
        )
    }

    pub fn new_client(
        sys: &System,
        event_generator: Arc<EditorEventGenerator>,
        server_addr: &str,
        server_info: NetworkClientCertCheckMode,
    ) -> Self {
        let mut network = QuinnNetwork::init_client(
            &format!("0.0.0.0:{}", 0),
            event_generator.clone(),
            sys,
            NetworkClientInitOptions::new(server_info).with_timeout(Duration::from_secs(120)),
            Default::default(),
            Default::default(),
            None,
        )
        .0;
        network.connect(server_addr);

        Self {
            network,
            is_server: false,
            connections: Default::default(),
        }
    }

    pub fn send(&mut self, ev: EditorEvent) {
        if self.is_server {
            for connection in &self.connections {
                self.network
                    .send_in_order_to(&ev, connection, NetworkInOrderChannel::Global);
            }
        } else {
            self.network
                .send_in_order_to_server(&ev, NetworkInOrderChannel::Global);
        }
    }

    pub fn send_to(&mut self, id: &NetworkConnectionID, ev: EditorEvent) {
        if self.is_server {
            self.network
                .send_in_order_to(&ev, id, NetworkInOrderChannel::Global);
        } else {
            self.network
                .send_in_order_to_server(&ev, NetworkInOrderChannel::Global);
        }
    }

    pub fn handle_network_ev(&mut self, id: NetworkConnectionID, ev: NetworkEvent) {
        match ev {
            NetworkEvent::Connected(_) => {
                self.connections.insert(id);
            }
            NetworkEvent::Disconnected(_) => {
                self.connections.remove(&id);
            }
            NetworkEvent::ConnectingFailed(_) => {}
            NetworkEvent::NetworkStats(_) => {}
        }
    }

    pub fn disconnect(&mut self) {
        self.network
            .disconnect(&self.network.get_current_connect_id());
    }
}
