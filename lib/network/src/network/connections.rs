use std::{
    collections::HashMap,
    future::Future,
    ops::DerefMut,
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use anyhow::anyhow;
use base::system::{SystemTime, SystemTimeInterface};
use base_log::log::{LogLevel, SystemLogGroup, SystemLogInterface};
use pool::{mt_datatypes::PoolVec, mt_pool::Pool};
use quinn::VarInt;
use tokio::sync::Mutex as TokioMutex;

use super::{
    connection::{
        NetworkConnection, NetworkConnectionID, NetworkConnectionPingHandle,
        NetworkConnectionPingHandles, INVALID_NETWORK_CON_IDENTIFIER, MIN_NETWORK_CON_IDENTIFIER,
    },
    event::{NetworkEvent, NetworkStats},
    event_generator::InternalGameEventGenerator,
    network::{NetworkConnectingInterface, NetworkConnectionInterface},
    plugins::{NetworkPluginConnection, NetworkPluginPacket},
    types::{
        InternalNetworkPackets, InternalPingNetworkPackets, NetworkInOrderPackets, NetworkPacket,
    },
};

#[derive(Debug)]
pub(crate) struct NetworkConnectionIDCounter(AtomicU64);
impl NetworkConnectionIDCounter {
    pub(crate) fn new() -> Self {
        Self(AtomicU64::new(MIN_NETWORK_CON_IDENTIFIER.0))
    }

    pub(crate) fn get_next(&self) -> NetworkConnectionID {
        NetworkConnectionID(self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NetworkConnections<C: Send + Sync> {
    pub(crate) connections:
        Arc<TokioMutex<HashMap<NetworkConnectionID, Arc<NetworkConnection<C>>>>>,
    pub(crate) id_gen: Arc<NetworkConnectionIDCounter>,
}

impl<C: NetworkConnectionInterface + Send + Sync + Clone + 'static> NetworkConnections<C> {
    pub(crate) fn new(counter: Arc<NetworkConnectionIDCounter>) -> Self {
        Self {
            connections: Arc::new(TokioMutex::new(HashMap::new())),
            id_gen: counter,
        }
    }

    async fn get_connection_clone_by_id(
        &self,
        id: &NetworkConnectionID,
    ) -> Option<Arc<NetworkConnection<C>>> {
        let connections_guard = self.connections.lock().await;
        let connections = &*connections_guard;
        // check if the connection exists
        connections.get(id).cloned()
    }

    pub async fn get_connection_impl_clone_by_id(&self, id: &NetworkConnectionID) -> Option<C> {
        self.get_connection_clone_by_id(id)
            .await
            .map(|con| con.conn.clone())
    }

    pub(crate) async fn prepare_write_packet(
        id: &NetworkConnectionID,
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

        for packet_plugin in packet_plugins.iter().rev() {
            packet_plugin.prepare_write(id, &mut packet_encoded).await?;
        }

        Ok(packet_encoded)
    }

    async fn disconnect_connection(
        con_id: &NetworkConnectionID,
        connections_clone: &NetworkConnections<C>,
        connection: &C,
        sys: &Arc<SystemTime>,
        game_event_generator: &mut InternalGameEventGenerator,
        reason: String,
        graceful: bool,
        all_packets_in_order: &Arc<TokioMutex<NetworkInOrderPackets>>,
    ) {
        let mut connections = connections_clone.connections.lock().await;
        let _con_rem_res = connections.remove(con_id);
        drop(connections);
        connection.close(VarInt::default(), &[]).await;

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
        connection_async: &Arc<NetworkConnection<C>>,
        mut game_event_generator_clone: &mut InternalGameEventGenerator,
        connection_identifier: &NetworkConnectionID,
        sys: &Arc<SystemTime>,
        logger: &Arc<TokioMutex<SystemLogGroup>>,
        mut recv_stream: Vec<u8>,
        debug_printing: bool,
        pool: &mut Pool<Vec<u8>>,
        packet_plugins: &Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
    ) {
        let timestamp = sys.time_get_nanoseconds();

        for packet_plugin in packet_plugins.iter() {
            if let Err(err) = packet_plugin
                .prepare_read(connection_identifier, &mut recv_stream)
                .await
            {
                logger
                    .lock()
                    .await
                    .log(LogLevel::Warning)
                    .msg("packet plugin failed: ")
                    .msg(&err.to_string());
            }
        }

        let res_packet = bincode::serde::decode_from_slice::<NetworkPacket, _>(
            &recv_stream.as_slice(),
            bincode::config::standard(),
        );
        if let Ok((res_packet, handled_size)) = res_packet {
            let remaining_size = recv_stream.len() - handled_size;
            if remaining_size > 0 && debug_printing {
                logger
                    .lock()
                    .await
                    .log(LogLevel::Warning)
                    .msg("warning: there were remaining bytes (")
                    .msg_var(&remaining_size)
                    .msg(") when processing a valid packet: ")
                    .msg_dbg(&recv_stream[recv_stream.len() - remaining_size..recv_stream.len()]);
            }
            match res_packet {
                NetworkPacket::Internal(internal_packet) => {
                    Self::handle_internal_packet(
                        &sys,
                        &logger,
                        &mut game_event_generator_clone,
                        &connection_identifier,
                        &connection_async,
                        &internal_packet,
                        debug_printing,
                        pool,
                        packet_plugins,
                    )
                    .await;
                }
                NetworkPacket::User(user_packet) => {
                    game_event_generator_clone
                        .generate_from_binary(
                            timestamp,
                            &connection_identifier,
                            user_packet.as_slice(),
                        )
                        .await;
                }
            }
        }
    }

    /// this function must respect the UDP frame size and should never be bigger than ~1.4KB
    async fn send_internal_packet_unreliable(
        id: &NetworkConnectionID,
        connection: &C,
        packet: &NetworkPacket,
        pool: &Pool<Vec<u8>>,
        packet_plugins: &Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
    ) -> anyhow::Result<()> {
        let packet_encoded = Self::prepare_write_packet(id, packet, pool, packet_plugins).await?;
        connection.send_unreliable_unordered(packet_encoded).await?;
        Ok(())
    }

    async fn handle_internal_packet(
        sys: &Arc<SystemTime>,
        logger: &Arc<TokioMutex<SystemLogGroup>>,
        game_event_generator_clone: &mut InternalGameEventGenerator,
        con_id: &NetworkConnectionID,
        connection: &Arc<NetworkConnection<C>>,
        internal_packet: &InternalNetworkPackets,
        debug_printing: bool,
        pool: &mut Pool<Vec<u8>>,
        packet_plugins: &Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
    ) {
        match internal_packet {
            InternalNetworkPackets::PingFamily(identifier, packet) => match packet {
                InternalPingNetworkPackets::Ping => {
                    // add new ping handle for this identifier
                    let cur_time = sys.time_get_nanoseconds();
                    let mut ping_handle = connection.ping_handles.lock().await;
                    let res = ping_handle.inc_ping_handles.try_add(*identifier, cur_time);
                    if let Ok(handle) = res {
                        handle.ping_pong_peng_start_timestamp = cur_time;
                        drop(ping_handle);
                        // also send a pong
                        let con = &connection.conn;
                        Self::send_internal_packet_unreliable(
                            con_id,
                            &con,
                            &NetworkPacket::Internal(InternalNetworkPackets::PingFamily(
                                *identifier,
                                InternalPingNetworkPackets::Pong,
                            )),
                            pool,
                            packet_plugins,
                        )
                        .await
                        .unwrap_or_else(|_| {
                            if debug_printing {
                                logger
                                    .blocking_lock()
                                    .log(LogLevel::Debug)
                                    .msg("error: send diagram for ping failed");
                            }
                        });
                    }
                }
                InternalPingNetworkPackets::Pong => {
                    let cur_time = sys.time_get_nanoseconds();
                    // update the connection ping
                    let mut ping_handle = connection.ping_handles.lock().await;
                    let handle_res = ping_handle.ping_handles.try_remove(identifier, sys);
                    if let Some(handle) = handle_res {
                        let ping = cur_time - handle.ping_pong_peng_start_timestamp;
                        drop(ping_handle);
                        // generate network stats
                        game_event_generator_clone
                            .generate_from_network_event(
                                cur_time,
                                con_id,
                                &NetworkEvent::NetworkStats(NetworkStats { ping: ping }),
                            )
                            .await;
                        // also send a peng
                        let con = &connection.conn;
                        Self::send_internal_packet_unreliable(
                            con_id,
                            &con,
                            &NetworkPacket::Internal(InternalNetworkPackets::PingFamily(
                                *identifier,
                                InternalPingNetworkPackets::Peng,
                            )),
                            pool,
                            packet_plugins,
                        )
                        .await
                        .unwrap_or_else(|_| {
                            if debug_printing {
                                logger
                                    .blocking_lock()
                                    .log(LogLevel::Debug)
                                    .msg("error: send diagram for pong failed");
                            }
                        });
                    }
                }
                InternalPingNetworkPackets::Peng => {
                    let cur_time = sys.time_get_nanoseconds();
                    // update the connection ping
                    let mut ping_handle = connection.ping_handles.lock().await;
                    let handle_res = ping_handle.inc_ping_handles.try_remove(identifier, sys);
                    if let Some(handle) = handle_res {
                        let ping = cur_time - handle.ping_pong_peng_start_timestamp;
                        drop(ping_handle);
                        // generate network stats
                        game_event_generator_clone
                            .generate_from_network_event(
                                cur_time,
                                con_id,
                                &NetworkEvent::NetworkStats(NetworkStats { ping: ping }),
                            )
                            .await;
                    }
                }
            },
        }
    }

    async fn handle_connection_recv_unordered_unreliable(
        connection_async: Arc<NetworkConnection<C>>,
        mut game_event_generator_clone: InternalGameEventGenerator,
        connection_identifier: NetworkConnectionID,
        sys: Arc<SystemTime>,
        logger: Arc<TokioMutex<SystemLogGroup>>,
        debug_printing: bool,
        pool: Pool<Vec<u8>>,
        packet_plugins: Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
    ) -> anyhow::Result<()> {
        'conn_loop: loop {
            let connection = &connection_async.conn;
            let datagram = connection.read_unreliable_unordered().await;
            let mut pool = pool.clone();
            match datagram {
                Ok(recv_stream) => {
                    Self::process_valid_packet(
                        &connection_async,
                        &mut game_event_generator_clone,
                        &connection_identifier,
                        &sys,
                        &logger,
                        recv_stream,
                        debug_printing,
                        &mut pool,
                        &packet_plugins,
                    )
                    .await;
                }
                Err(recv_err) => {
                    logger
                        .lock()
                        .await
                        .log(LogLevel::Debug)
                        .msg("connection stream acception failed ")
                        .msg_var(&recv_err);

                    break 'conn_loop;
                }
            }
        }

        Ok(())
    }

    async fn handle_connection_recv_unordered_reliable(
        connection_async: Arc<NetworkConnection<C>>,
        game_event_generator: InternalGameEventGenerator,
        connection_identifier: NetworkConnectionID,
        sys: Arc<SystemTime>,
        logger: Arc<TokioMutex<SystemLogGroup>>,
        debug_printing: bool,
        pool: Pool<Vec<u8>>,
        packet_plugins: Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
    ) -> anyhow::Result<()> {
        'conn_loop: loop {
            let conn_async_clone = connection_async.clone();
            let mut game_ev_gen_clone = game_event_generator.clone();
            let sys_clone = sys.clone();
            let logger_clone = logger.clone();
            let connection = &connection_async.conn;
            let mut pool = pool.clone();
            let packet_plugins = packet_plugins.clone();
            match connection
                .read_unordered_reliable(move |uni| {
                    tokio::task::spawn(async move {
                        match uni {
                            Ok(data) => {
                                Self::process_valid_packet(
                                    &conn_async_clone,
                                    &mut game_ev_gen_clone,
                                    &connection_identifier,
                                    &sys_clone,
                                    &logger_clone,
                                    data,
                                    debug_printing,
                                    &mut pool,
                                    &packet_plugins,
                                )
                                .await;
                            }
                            Err(err) => {
                                if debug_printing {
                                    logger_clone
                                        .lock()
                                        .await
                                        .log(LogLevel::Debug)
                                        .msg("error: failed to read reliable unordered packet: ")
                                        .msg_var(&err);
                                }
                            }
                        }
                    })
                })
                .await
            {
                Ok(_) => {}
                Err(recv_err) => {
                    logger
                        .lock()
                        .await
                        .log(LogLevel::Debug)
                        .msg("connection stream acception failed ")
                        .msg_var(&recv_err);

                    break 'conn_loop;
                }
            }
        }

        Ok(())
    }

    async fn handle_connection_recv_ordered_reliable(
        connection_async: Arc<NetworkConnection<C>>,
        game_event_generator: InternalGameEventGenerator,
        connection_identifier: NetworkConnectionID,
        sys: Arc<SystemTime>,
        logger: Arc<TokioMutex<SystemLogGroup>>,
        debug_printing: bool,
        pool: Pool<Vec<u8>>,
        packet_plugins: Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
    ) -> anyhow::Result<()> {
        'conn_loop: loop {
            let conn_async_clone = connection_async.clone();
            let game_ev_gen_clone = game_event_generator.clone();
            let sys_clone = sys.clone();
            let logger_clone = logger.clone();
            let packet_plugins = packet_plugins.clone();
            let connection = &connection_async.conn;
            let pool = pool.clone();
            match connection
                .read_ordered_reliable(move |uni| {
                    let conn_async_clone = conn_async_clone.clone();
                    let mut game_ev_gen_clone = game_ev_gen_clone.clone();
                    let sys_clone = sys_clone.clone();
                    let logger_clone = logger_clone.clone();
                    let mut pool = pool.clone();
                    let packet_plugins = packet_plugins.clone();
                    tokio::task::spawn(async move {
                        match uni {
                            Ok(data) => {
                                Self::process_valid_packet(
                                    &conn_async_clone,
                                    &mut game_ev_gen_clone,
                                    &connection_identifier,
                                    &sys_clone,
                                    &logger_clone,
                                    data,
                                    debug_printing,
                                    &mut pool,
                                    &packet_plugins,
                                )
                                .await;
                            }
                            Err(err) => {
                                if debug_printing {
                                    logger_clone
                                        .lock()
                                        .await
                                        .log(LogLevel::Debug)
                                        .msg("error: failed to read reliable ordered packet: ")
                                        .msg_var(&err);
                                }
                            }
                        }
                    })
                })
                .await
            {
                Ok(_) => {}
                Err(recv_err) => {
                    logger
                        .lock()
                        .await
                        .log(LogLevel::Debug)
                        .msg("connection stream acception failed ")
                        .msg_var(&recv_err);

                    break 'conn_loop;
                }
            }
        }

        Ok(())
    }

    async fn ping(
        sys: Arc<SystemTime>,
        logger: Arc<TokioMutex<SystemLogGroup>>,
        connection_async: Arc<NetworkConnection<C>>,
        con_id: &NetworkConnectionID,
        interval: &mut tokio::time::Interval,
        debug_printing: bool,
        pool: Pool<Vec<u8>>,
        packet_plugins: &Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
    ) -> anyhow::Result<()> {
        let mut identifier: u64 = 0;
        loop {
            interval.tick().await;
            let conn = connection_async.clone();
            let sys = sys.clone();
            identifier += 1;
            let identifier_copy = identifier;
            let mut pool = pool.clone();
            let logger = logger.clone();
            let packet_plugins = packet_plugins.clone();
            let id = *con_id;
            // spawn a new ping task
            tokio::spawn(async move {
                // send a normal ping pong peng task
                let connection = &conn.conn;
                let cur_time = sys.time_get_nanoseconds();
                let mut ping_handle = conn.ping_handles.lock().await;
                let handle_res = ping_handle.ping_handles.try_add(identifier_copy, cur_time);

                if let Ok(handle) = handle_res {
                    handle.ping_pong_peng_start_timestamp = cur_time;
                    drop(ping_handle);
                    Self::send_internal_packet_unreliable(
                        &id,
                        &connection,
                        &NetworkPacket::Internal(InternalNetworkPackets::PingFamily(
                            identifier_copy,
                            InternalPingNetworkPackets::Ping,
                        )),
                        &mut pool,
                        &packet_plugins,
                    )
                    .await
                    .unwrap_or_else(|_| {
                        if debug_printing {
                            logger
                                .blocking_lock()
                                .log(LogLevel::Debug)
                                .msg("error: send diagram for interval ping-ing failed");
                        }
                    });
                }
            });
        }

        Ok(())
    }

    pub(crate) async fn handle_connection<
        Z: NetworkConnectingInterface<C>
            + Send
            + Sync
            + 'static
            + Future<Output = Result<C, String>>
            + Unpin,
    >(
        connections: &NetworkConnections<C>,
        game_event_generator: &InternalGameEventGenerator,
        conn: Z,
        pre_defined_id: &NetworkConnectionID,
        sys: Arc<SystemTime>,
        logger: Arc<TokioMutex<SystemLogGroup>>,
        is_server: bool,
        all_packets_in_order: &Arc<TokioMutex<NetworkInOrderPackets>>,
        debug_printing: bool,
        pool: &mut Pool<Vec<u8>>,
        packet_plugins: &Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
        connection_plugins: &Arc<Vec<Arc<dyn NetworkPluginConnection>>>,
    ) -> tokio::task::JoinHandle<()> {
        logger
            .lock()
            .await
            .log(LogLevel::Debug)
            .msg("handling connecting request");

        let mut remote_addr = conn.remote_addr();

        let connections_clone = connections.clone();
        let mut game_event_generator_clone = game_event_generator.clone();
        let all_packets_in_order = all_packets_in_order.clone();

        let pre_def_id = *pre_defined_id;
        let pool = pool.clone();
        let packet_plugins = packet_plugins.clone();
        let connection_plugins = connection_plugins.clone();
        let connecting = conn;
        tokio::spawn(async move {
            let connections = connections_clone;
            let connection;
            let connection_identifier;
            // get connection id
            {
                if pre_def_id != INVALID_NETWORK_CON_IDENTIFIER {
                    connection_identifier = pre_def_id;
                } else {
                    connection_identifier = connections.id_gen.get_next();
                }

                for connection_plugin in connection_plugins.iter() {
                    let res = connection_plugin
                        .on_connect(&connection_identifier, &mut remote_addr)
                        .await;
                    if res.is_err() || res.is_ok_and(|r| !r) {
                        // drop connection
                        logger
                            .lock()
                            .await
                            .log(LogLevel::Debug)
                            .msg("connection was dropped by connection plugin");
                        return;
                    }
                }
            }
            // process connecting
            {
                match connecting.await {
                    Err(err) => {
                        logger
                            .lock()
                            .await
                            .log(LogLevel::Debug)
                            .msg("Connection failed to resolve (connecting failed)");
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
                        connection = Arc::new(NetworkConnection::<C> {
                            conn,

                            ping_handles: TokioMutex::new(NetworkConnectionPingHandles {
                                ping_handles: NetworkConnectionPingHandle::new(),
                                inc_ping_handles: NetworkConnectionPingHandle::new(),
                            }),
                        });
                        connections
                            .connections
                            .lock()
                            .await
                            .insert(connection_identifier, connection.clone());
                        logger
                            .lock()
                            .await
                            .log(LogLevel::Debug)
                            .msg("connecting established");
                        let timestamp = sys.time_get_nanoseconds();
                        game_event_generator_clone
                            .generate_from_network_event(
                                timestamp,
                                &connection_identifier,
                                &NetworkEvent::Connected {
                                    addr: remote_addr,
                                    public_key: Arc::new(con_cert.public_key_data().to_vec()),
                                },
                            )
                            .await
                    }
                }
            }
            let pool = pool.clone();
            let packet_plugins = packet_plugins.clone();
            tokio::spawn(async move {
                let mut ping_interval = tokio::time::interval(if !is_server {
                    Duration::from_secs(1) / 8 // 8 per second from client to server
                } else {
                    Duration::from_secs(1) / 2 // 2 per second from server to client
                });
                let res = tokio::select! {
                    res = Self::handle_connection_recv_unordered_reliable(
                        connection.clone(), game_event_generator_clone.clone(), connection_identifier, sys.clone(),
                        logger.clone(), debug_printing, pool.clone(), packet_plugins.clone()) => {res}
                    res = Self::handle_connection_recv_ordered_reliable(
                        connection.clone(), game_event_generator_clone.clone(), connection_identifier, sys.clone(),
                        logger.clone(), debug_printing, pool.clone(), packet_plugins.clone()) => {res}
                    res = Self::handle_connection_recv_unordered_unreliable(connection.clone(), game_event_generator_clone.clone(),
                    connection_identifier, sys.clone(), logger.clone(), debug_printing, pool.clone(), packet_plugins.clone()) => {res}
                    res = Self::ping(sys.clone(), logger.clone(), connection.clone(), &connection_identifier, &mut ping_interval, debug_printing, pool.clone(), &packet_plugins) => {res}
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
                )
                .await;

                logger
                    .lock()
                    .await
                    .log(LogLevel::Debug)
                    .msg("connection dropped.");
            });
        })
    }
}
