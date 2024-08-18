use std::{
    collections::HashMap,
    ops::DerefMut,
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use anyhow::anyhow;
use base::system::{SystemTime, SystemTimeInterface};
use pool::{mt_datatypes::PoolVec, mt_pool::Pool};
use quinn::VarInt;
use tokio::sync::Mutex as TokioMutex;

use super::{
    connection::{NetworkConnection, NetworkConnectionId},
    event::NetworkEvent,
    event_generator::InternalGameEventGenerator,
    network::{NetworkConnectingInterface, NetworkConnectionInterface},
    plugins::{NetworkPluginConnection, NetworkPluginPacket},
    types::{NetworkInOrderPackets, NetworkPacket},
};

#[derive(Debug)]
pub struct NetworkConnectionIdCounter(AtomicU64);
impl NetworkConnectionIdCounter {
    pub fn get_next(&self) -> NetworkConnectionId {
        NetworkConnectionId(self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

impl Default for NetworkConnectionIdCounter {
    fn default() -> Self {
        Self(AtomicU64::new(0))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NetworkConnections<C: Send + Sync> {
    pub(crate) connections:
        Arc<TokioMutex<HashMap<NetworkConnectionId, Arc<NetworkConnection<C>>>>>,
    pub(crate) id_gen: Arc<NetworkConnectionIdCounter>,
}

impl<C: NetworkConnectionInterface + Send + Sync + Clone + 'static> NetworkConnections<C> {
    pub(crate) fn new(counter: Arc<NetworkConnectionIdCounter>) -> Self {
        Self {
            connections: Arc::new(TokioMutex::new(HashMap::new())),
            id_gen: counter,
        }
    }

    async fn get_connection_clone_by_id(
        &self,
        id: &NetworkConnectionId,
    ) -> Option<Arc<NetworkConnection<C>>> {
        let connections_guard = self.connections.lock().await;
        let connections = &*connections_guard;
        // check if the connection exists
        connections.get(id).cloned()
    }

    pub async fn get_connection_impl_clone_by_id(&self, id: &NetworkConnectionId) -> Option<C> {
        self.get_connection_clone_by_id(id)
            .await
            .map(|con| con.conn.clone())
    }

    pub(crate) async fn prepare_write_packet(
        id: &NetworkConnectionId,
        packet: &NetworkPacket,
        pool: &Pool<Vec<u8>>,
        packet_plugins: &Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
    ) -> anyhow::Result<PoolVec<u8>> {
        let mut packet_encoded = pool.new();
        bincode::serde::encode_into_std_write(
            packet,
            packet_encoded.deref_mut(),
            bincode::config::standard(),
        )
        .map_err(|err| anyhow!("packet was invalid and could not be encoded: {err}"))?;

        for packet_plugin in packet_plugins.iter() {
            packet_plugin.prepare_write(id, &mut packet_encoded).await?;
        }

        Ok(packet_encoded)
    }

    async fn disconnect_connection(
        con_id: &NetworkConnectionId,
        connections_clone: &NetworkConnections<C>,
        connection: &C,
        sys: &Arc<SystemTime>,
        game_event_generator: &mut InternalGameEventGenerator,
        reason: String,
        graceful: bool,
        all_packets_in_order: &Arc<TokioMutex<NetworkInOrderPackets>>,
        connection_plugins: &Arc<Vec<Arc<dyn NetworkPluginConnection>>>,
    ) {
        let mut connections = connections_clone.connections.lock().await;
        let _con_rem_res = connections.remove(con_id);
        drop(connections);
        connection.close(VarInt::default(), &[]).await;

        for plugin in connection_plugins.iter() {
            plugin
                .on_disconnect(con_id, &connection.remote_addr())
                .await;
        }

        let timestamp = sys.time_get_nanoseconds();
        game_event_generator
            .generate_from_network_event(
                timestamp,
                con_id,
                &NetworkEvent::Disconnected { reason, graceful },
            )
            .await;
        all_packets_in_order.lock().await.remove(con_id);
    }

    async fn process_valid_packet(
        game_event_generator_clone: &InternalGameEventGenerator,
        connection_identifier: &NetworkConnectionId,
        sys: &Arc<SystemTime>,
        mut recv_stream: Vec<u8>,
        debug_printing: bool,
        packet_plugins: &Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
    ) {
        let timestamp = sys.time_get_nanoseconds();

        for packet_plugin in packet_plugins.iter().rev() {
            if let Err(err) = packet_plugin
                .prepare_read(connection_identifier, &mut recv_stream)
                .await
            {
                log::warn!("packet plugin failed: {err}");
            }
        }

        let res_packet = bincode::serde::decode_from_slice::<NetworkPacket, _>(
            recv_stream.as_slice(),
            bincode::config::standard(),
        );
        if let Ok((res_packet, handled_size)) = res_packet {
            let remaining_size = recv_stream.len() - handled_size;
            if remaining_size > 0 && debug_printing {
                log::warn!(
                    "warning: there were remaining bytes ({}) when processing a valid packet: {:?}",
                    remaining_size,
                    &recv_stream[recv_stream.len() - remaining_size..recv_stream.len()]
                );
            }
            game_event_generator_clone
                .generate_from_binary(timestamp, connection_identifier, res_packet.as_slice())
                .await;
        }
    }

    async fn handle_connection_recv_unordered_unreliable(
        connection_async: Arc<NetworkConnection<C>>,
        game_event_generator_clone: InternalGameEventGenerator,
        connection_identifier: NetworkConnectionId,
        sys: Arc<SystemTime>,
        debug_printing: bool,
        packet_plugins: Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
    ) -> anyhow::Result<()> {
        'conn_loop: loop {
            let connection = &connection_async.conn;
            let datagram = connection.read_unreliable_unordered().await;
            match datagram {
                Ok(recv_stream) => {
                    Self::process_valid_packet(
                        &game_event_generator_clone,
                        &connection_identifier,
                        &sys,
                        recv_stream,
                        debug_printing,
                        &packet_plugins,
                    )
                    .await;
                }
                Err(recv_err) => {
                    log::debug!("connection stream acception failed {recv_err}");

                    break 'conn_loop;
                }
            }
        }

        Ok(())
    }

    async fn handle_connection_recv_unordered_reliable(
        connection_async: Arc<NetworkConnection<C>>,
        game_event_generator: InternalGameEventGenerator,
        connection_identifier: NetworkConnectionId,
        sys: Arc<SystemTime>,
        debug_printing: bool,
        packet_plugins: Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
    ) -> anyhow::Result<()> {
        'conn_loop: loop {
            let game_ev_gen_clone = game_event_generator.clone();
            let sys_clone = sys.clone();
            let connection = &connection_async.conn;
            let packet_plugins = packet_plugins.clone();
            match connection
                .read_unordered_reliable(move |uni| {
                    tokio::task::spawn(async move {
                        match uni {
                            Ok(data) => {
                                Self::process_valid_packet(
                                    &game_ev_gen_clone,
                                    &connection_identifier,
                                    &sys_clone,
                                    data,
                                    debug_printing,
                                    &packet_plugins,
                                )
                                .await;
                            }
                            Err(err) => {
                                if debug_printing {
                                    log::debug!(
                                        "error: failed to read reliable unordered packet: {err}"
                                    );
                                }
                            }
                        }
                    })
                })
                .await
            {
                Ok(_) => {}
                Err(recv_err) => {
                    log::debug!("connection stream acception failed {recv_err}");

                    break 'conn_loop;
                }
            }
        }

        Ok(())
    }

    async fn handle_connection_recv_ordered_reliable(
        connection_async: Arc<NetworkConnection<C>>,
        game_event_generator: InternalGameEventGenerator,
        connection_identifier: NetworkConnectionId,
        sys: Arc<SystemTime>,
        debug_printing: bool,
        packet_plugins: Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
    ) -> anyhow::Result<()> {
        'conn_loop: loop {
            let game_ev_gen_clone = game_event_generator.clone();
            let sys_clone = sys.clone();
            let packet_plugins = packet_plugins.clone();
            let connection = &connection_async.conn;
            match connection
                .read_ordered_reliable(move |uni| {
                    let game_ev_gen_clone = game_ev_gen_clone.clone();
                    let sys_clone = sys_clone.clone();
                    let packet_plugins = packet_plugins.clone();
                    tokio::task::spawn(async move {
                        match uni {
                            Ok(data) => {
                                Self::process_valid_packet(
                                    &game_ev_gen_clone,
                                    &connection_identifier,
                                    &sys_clone,
                                    data,
                                    debug_printing,
                                    &packet_plugins,
                                )
                                .await;
                            }
                            Err(err) => {
                                if debug_printing {
                                    log::debug!(
                                        "error: failed to read reliable ordered packet: {err}"
                                    );
                                }
                            }
                        }
                    })
                })
                .await
            {
                Ok(_) => {}
                Err(recv_err) => {
                    log::debug!("connection stream acception failed {recv_err}");

                    break 'conn_loop;
                }
            }
        }

        Ok(())
    }

    async fn ping(
        sys: Arc<SystemTime>,
        game_event_generator: InternalGameEventGenerator,
        connection: Arc<NetworkConnection<C>>,
        con_id: &NetworkConnectionId,
        interval: &mut tokio::time::Interval,
    ) -> anyhow::Result<()> {
        loop {
            interval.tick().await;
            // spawn a new ping task
            let game_event_generator_clone = game_event_generator.clone();
            let connection_async = connection.clone();
            let con_id = *con_id;
            let sys = sys.clone();
            tokio::spawn(async move {
                // send a normal ping pong peng task
                game_event_generator_clone
                    .generate_from_network_event(
                        sys.time_get_nanoseconds(),
                        &con_id,
                        &NetworkEvent::NetworkStats(connection_async.conn.stats()),
                    )
                    .await;
            });
        }
    }

    pub(crate) async fn handle_connection<Z: NetworkConnectingInterface<C>>(
        connections: &NetworkConnections<C>,
        game_event_generator: &InternalGameEventGenerator,
        conn: Z,
        pre_defined_id: Option<&NetworkConnectionId>,
        sys: Arc<SystemTime>,
        is_server: bool,
        all_packets_in_order: &Arc<TokioMutex<NetworkInOrderPackets>>,
        debug_printing: bool,
        packet_plugins: &Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
        connection_plugins: &Arc<Vec<Arc<dyn NetworkPluginConnection>>>,
    ) -> tokio::task::JoinHandle<()> {
        let remote_addr = conn.remote_addr();
        log::debug!("handling connecting request for {:?}", remote_addr);

        let connections_clone = connections.clone();
        let mut game_event_generator_clone = game_event_generator.clone();
        let all_packets_in_order = all_packets_in_order.clone();

        let pre_def_id = pre_defined_id.copied();
        let packet_plugins = packet_plugins.clone();
        let connection_plugins = connection_plugins.clone();
        let connecting = conn;
        tokio::spawn(async move {
            let connections = connections_clone;
            let connection;
            let connection_identifier;
            // get connection id
            {
                if let Some(pre_def_id) = pre_def_id {
                    connection_identifier = pre_def_id;
                } else {
                    connection_identifier = connections.id_gen.get_next();
                }
            }
            // process connecting
            {
                match connecting.await {
                    Err(err) => {
                        log::debug!("Connection failed to resolve (connecting failed)");
                        let timestamp = sys.time_get_nanoseconds();
                        game_event_generator_clone
                            .generate_from_network_event(
                                timestamp,
                                &connection_identifier,
                                &NetworkEvent::ConnectingFailed(err),
                            )
                            .await;
                        return;
                    }
                    Ok(conn) => {
                        let con_cert = conn.peer_identity();
                        // insert connection
                        connection = Arc::new(NetworkConnection::<C> { conn });
                        connections
                            .connections
                            .lock()
                            .await
                            .insert(connection_identifier, connection.clone());
                        log::debug!("connecting established");
                        let timestamp = sys.time_get_nanoseconds();
                        for plugin in connection_plugins.iter() {
                            plugin
                                .on_connect(&connection_identifier, &remote_addr)
                                .await;
                        }
                        game_event_generator_clone
                            .generate_from_network_event(
                                timestamp,
                                &connection_identifier,
                                &NetworkEvent::Connected {
                                    addr: remote_addr,
                                    cert: Arc::new(con_cert),
                                    initial_network_stats: connection.conn.stats(),
                                },
                            )
                            .await
                    }
                }
            }
            let packet_plugins = packet_plugins.clone();
            tokio::spawn(async move {
                let mut ping_interval = tokio::time::interval(Duration::from_secs(1));
                let res = tokio::select! {
                    res = Self::handle_connection_recv_unordered_reliable(
                        connection.clone(), game_event_generator_clone.clone(), connection_identifier, sys.clone(),
                        debug_printing,  packet_plugins.clone()) => {res}
                    res = Self::handle_connection_recv_ordered_reliable(
                        connection.clone(), game_event_generator_clone.clone(), connection_identifier, sys.clone(),
                        debug_printing, packet_plugins.clone()) => {res}
                    res = Self::handle_connection_recv_unordered_unreliable(connection.clone(), game_event_generator_clone.clone(),
                    connection_identifier, sys.clone(), debug_printing, packet_plugins.clone()) => {res}
                    res = Self::ping(sys.clone(), game_event_generator_clone.clone(), connection.clone(), &connection_identifier, &mut ping_interval) => {res}
                };

                let (reason, graceful) = match res {
                    Ok(_) => ("".to_string(), true),
                    Err(err) => (err.to_string(), false),
                };
                Self::disconnect_connection(
                    &connection_identifier,
                    &connections,
                    &connection.conn,
                    &sys,
                    &mut game_event_generator_clone,
                    reason,
                    graceful,
                    &all_packets_in_order,
                    &connection_plugins,
                )
                .await;

                log::debug!("connection dropped.");
            });
        })
    }
}
