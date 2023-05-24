use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    marker::PhantomData,
    net::SocketAddr,
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc, Mutex as StdMutex,
    },
    time::Duration,
};

use bytes::Bytes;
use quinn::VarInt;
use rcgen::Certificate;
use tokio::sync::{Mutex as TokioMutex, RwLock as TokioRwLock};

use base::system::{SystemTime, SystemTimeInterface};

pub struct NetworkConnectionIDCounter(AtomicU64);

#[derive(PartialEq, Eq, Copy, Clone, Hash)]
pub struct NetworkConnectionID(u64);
const MIN_NETWORK_CON_IDENTIFIER: NetworkConnectionID = NetworkConnectionID(1);
const INVALID_NETWORK_CON_IDENTIFIER: NetworkConnectionID = NetworkConnectionID(0);

impl Default for NetworkConnectionID {
    fn default() -> Self {
        INVALID_NETWORK_CON_IDENTIFIER
    }
}

impl NetworkConnectionIDCounter {
    pub fn new() -> Self {
        Self(AtomicU64::new(MIN_NETWORK_CON_IDENTIFIER.0))
    }

    pub fn get_next(&self) -> NetworkConnectionID {
        NetworkConnectionID(self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

#[derive(Clone)]
pub struct NetworkStats {
    pub ping: Duration,
}

#[derive(Clone)]
pub enum NetworkGameEvent {
    Connected,
    Disconnected(String),
    ConnectingFailed(String),
    NetworkStats(NetworkStats),
}

pub trait NetworkEventToGameEventGenerator {
    fn generate_from_binary(
        &mut self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        bytes: &[u8],
    );

    fn generate_from_network_event(
        &mut self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        network_event: &NetworkGameEvent,
    );
}

enum NetworkEvent {
    Connect(NetworkConnectionID, String),
    Disconnect(NetworkConnectionID),
    Close(),
    Send((NetworkConnectionID, Vec<u8>)),
}

#[derive(bincode::Encode, bincode::Decode)]
enum InternalPingNetworkPackets {
    // con1 sends packet to con2
    Ping,
    // con2 responds to ping of con1
    Pong,
    // con1 resends a packet to con2, so con2 also has the ping
    Peng,
}

#[derive(bincode::Encode, bincode::Decode)]
enum InternalNetworkPackets {
    // all P*ng carry an unique identifier
    PingFamily(u64, InternalPingNetworkPackets),
}

#[derive(bincode::Encode, bincode::Decode)]
enum NetworkPacket {
    Internal(InternalNetworkPackets),
    User(Vec<u8>),
}

pub struct NetworkEvents {
    events: VecDeque<NetworkEvent>,
}

pub struct NetworkConnectionPingHandleImpl {
    cur_identifier: u64,
    handle_timestamp: Duration,

    ping_pong_peng_start_timestamp: Duration,
}

impl NetworkConnectionPingHandleImpl {
    pub fn new(identifier: u64, add_timestamp: Duration) -> Self {
        Self {
            cur_identifier: identifier,
            handle_timestamp: add_timestamp,

            ping_pong_peng_start_timestamp: Duration::ZERO,
        }
    }
}

pub struct NetworkConnectionPingHandle {
    list: VecDeque<NetworkConnectionPingHandleImpl>,
}

impl NetworkConnectionPingHandle {
    pub fn new() -> Self {
        Self {
            list: VecDeque::new(),
        }
    }

    fn remove_outdated(&mut self, cur_time: Duration) {
        // check if there are outdated ping handles
        while !self.list.is_empty() {
            if cur_time - self.list.front().unwrap().handle_timestamp > Duration::from_secs(2) {
                self.list.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn try_get_mut(
        &mut self,
        identifier: &u64,
        sys: &Arc<SystemTime>,
    ) -> Option<&mut NetworkConnectionPingHandleImpl> {
        let cur_time = sys.time_get_nanoseconds();
        self.remove_outdated(cur_time);

        self.list
            .iter_mut()
            .find(|item| item.cur_identifier == *identifier)
    }

    pub fn try_add(
        &mut self,
        identifier: u64,
        cur_time: Duration,
    ) -> Result<&mut NetworkConnectionPingHandleImpl, ()> {
        self.remove_outdated(cur_time);

        /* TODO: 50 should not be harcoded */
        if self.list.len() < 50 {
            self.list
                .push_back(NetworkConnectionPingHandleImpl::new(identifier, cur_time));
            Ok(self.list.back_mut().unwrap())
        } else {
            Err(())
        }
    }
}

pub struct NetworkConnection<C: Send + Sync, Z: Send + Sync> {
    conn: Option<C>,
    connecting: Option<Z>,

    ping_handles: NetworkConnectionPingHandle,
}

#[async_trait::async_trait]
pub trait NetworkEndpointInterface<Z>
where
    Self: Sized,
{
    fn close(&self, error_code: VarInt, reason: &[u8]);
    fn connect(&self, addr: SocketAddr, server_name: &str) -> anyhow::Result<Z>;
    async fn accept(&self) -> Option<Z>;

    fn make_server_endpoint(
        bind_addr: SocketAddr,
        cert: &Certificate,
    ) -> anyhow::Result<(Self, Vec<u8>)>;

    fn make_client_endpoint(bind_addr: SocketAddr, server_certs: &[&[u8]]) -> anyhow::Result<Self>;
}

pub struct NetworkThread<E, C: Send + Sync, Z: Send + Sync> {
    is_server: bool,
    endpoint: E,
    connections: Arc<
        TokioMutex<(
            Arc<NetworkConnectionIDCounter>,
            HashMap<NetworkConnectionID, Arc<TokioRwLock<NetworkConnection<C, Z>>>>,
        )>,
    >,
    game_event_generator: Arc<TokioMutex<dyn NetworkEventToGameEventGenerator + Send>>,
    sys: Arc<SystemTime>,
}

#[async_trait::async_trait]
pub trait NetworkConnectionSendStreamInterface
where
    Self: Sync + Send,
{
    async fn write(&mut self, buf: &[u8]) -> anyhow::Result<usize>;
    async fn finish(&mut self) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
pub trait NetworkConnectionRecvStreamInterface
where
    Self: Sync + Send,
{
    async fn read_to_end(&mut self, max_size: usize) -> Result<Vec<u8>, String>;
}

#[async_trait::async_trait]
pub trait NetworkConnectionInterface<S, R>
where
    S: NetworkConnectionSendStreamInterface,
    R: NetworkConnectionRecvStreamInterface,
{
    fn close(&self, error_code: VarInt, reason: &[u8]);

    fn send_datagram(&self, data: Bytes) -> anyhow::Result<()>;
    async fn read_datagram(&self) -> Result<Vec<u8>, String>;

    async fn accept_bi(&self) -> Result<(S, R), String>;

    async fn open_bi(&self) -> Result<(S, R), String>;
}

pub struct Network<E, C: Send + Sync, Z: Send + Sync, S, R>
where
    S: NetworkConnectionSendStreamInterface,
    R: NetworkConnectionRecvStreamInterface,
    C: NetworkConnectionInterface<S, R>,
{
    // some attributes are shared with the NetworkThread struct
    // so that the endpoint can be closed without requiring
    // an additional lock
    is_server: bool,
    endpoint: E,
    is_closed: Arc<AtomicBool>,
    thread: Arc<StdMutex<NetworkThread<E, C, Z>>>,
    events: Arc<StdMutex<NetworkEvents>>,
    events_cond: Arc<std::sync::Condvar>,
    run_thread: Option<std::thread::JoinHandle<()>>,

    connection_id_generator: Arc<NetworkConnectionIDCounter>,

    // for the client to remember the last server it connected to
    connecting_connection_id: NetworkConnectionID,
    sys: Arc<SystemTime>,

    s: PhantomData<S>,
    r: PhantomData<R>,
}

impl<E, C, Z, S, R> Network<E, C, Z, S, R>
where
    S: NetworkConnectionSendStreamInterface,
    R: NetworkConnectionRecvStreamInterface,
    C: NetworkConnectionInterface<S, R> + Clone + Send + Sync + 'static,
    Z: Send + Sync + 'static + Future<Output = Result<C, String>> + Unpin,
    E: NetworkEndpointInterface<Z> + Clone + Send + Sync + 'static,
{
    async fn send_datagram(connection: &C, packet: &NetworkPacket) -> Result<(), ()> {
        let packet = bincode::encode_to_vec(packet, bincode::config::standard());
        if let Ok(packet) = packet {
            let pack_bytes = bytes::Bytes::copy_from_slice(&packet[..]);
            let res = connection.send_datagram(pack_bytes);
            if let Err(_) = res {
                return Err(());
            }
            return Ok(());
        }
        Err(())
    }

    async fn handle_internal_packet(
        sys: &Arc<SystemTime>,
        game_event_generator_clone: &Arc<TokioMutex<dyn NetworkEventToGameEventGenerator + Send>>,
        con_id: &NetworkConnectionID,
        connection: &Arc<TokioRwLock<NetworkConnection<C, Z>>>,
        internal_packet: &InternalNetworkPackets,
    ) {
        match internal_packet {
            InternalNetworkPackets::PingFamily(identifier, packet) => match packet {
                InternalPingNetworkPackets::Ping => {
                    // add new ping handle for this identifier
                    let cur_time = sys.time_get_nanoseconds();
                    let mut con_g = connection.write().await;
                    let res = con_g.ping_handles.try_add(*identifier, cur_time);
                    if let Ok(handle) = res {
                        handle.ping_pong_peng_start_timestamp = cur_time;
                        drop(con_g);
                        // also send a pong
                        let con_g = connection.read().await;
                        let con_res = con_g.conn.as_ref();
                        if let Some(con_ref) = con_res {
                            let con = con_ref.clone();
                            drop(con_g);
                            Self::send_datagram(
                                &con,
                                &NetworkPacket::Internal(InternalNetworkPackets::PingFamily(
                                    *identifier,
                                    InternalPingNetworkPackets::Pong,
                                )),
                            )
                            .await;
                        } else {
                            return;
                        }
                    }
                }
                InternalPingNetworkPackets::Pong => {
                    let cur_time = sys.time_get_nanoseconds();
                    // update the connection ping
                    let mut con_g = connection.write().await;
                    let handle_res = con_g.ping_handles.try_get_mut(identifier, sys);
                    if let Some(handle) = handle_res {
                        let ping = cur_time - handle.ping_pong_peng_start_timestamp;
                        drop(con_g);
                        // generate network stats
                        let mut ge_gen = game_event_generator_clone.lock().await;
                        ge_gen.generate_from_network_event(
                            cur_time,
                            con_id,
                            &NetworkGameEvent::NetworkStats(NetworkStats { ping: ping }),
                        );
                        drop(ge_gen);
                        // also send a peng
                        let con_g = connection.read().await;
                        let con_res = con_g.conn.as_ref();
                        if let Some(con_ref) = con_res {
                            let con = con_ref.clone();
                            drop(con_g);
                            Self::send_datagram(
                                &con,
                                &NetworkPacket::Internal(InternalNetworkPackets::PingFamily(
                                    *identifier,
                                    InternalPingNetworkPackets::Peng,
                                )),
                            )
                            .await;
                        } else {
                            return;
                        }
                    }
                }
                InternalPingNetworkPackets::Peng => {
                    let cur_time = sys.time_get_nanoseconds();
                    // update the connection ping
                    let mut con_g = connection.write().await;
                    let handle_res = con_g.ping_handles.try_get_mut(identifier, sys);
                    if let Some(handle) = handle_res {
                        let ping = cur_time - handle.ping_pong_peng_start_timestamp;
                        drop(con_g);
                        // generate network stats
                        let mut ge_gen = game_event_generator_clone.lock().await;
                        ge_gen.generate_from_network_event(
                            cur_time,
                            con_id,
                            &NetworkGameEvent::NetworkStats(NetworkStats { ping: ping }),
                        );
                        drop(ge_gen);
                    }
                }
            },
        }
    }

    async fn handle_connection_recv_datagram(
        connection_async: Arc<TokioRwLock<NetworkConnection<C, Z>>>,
        connections_clone: Arc<
            TokioMutex<(
                Arc<NetworkConnectionIDCounter>,
                HashMap<NetworkConnectionID, Arc<TokioRwLock<NetworkConnection<C, Z>>>>,
            )>,
        >,
        game_event_generator_clone: Arc<TokioMutex<dyn NetworkEventToGameEventGenerator + Send>>,
        connection_identifier: NetworkConnectionID,
        sys: Arc<SystemTime>,
    ) {
        'conn_loop: loop {
            let conn = connection_async.read().await;
            let connection_res = conn.conn.as_ref();
            if let Some(connection_inner) = connection_res {
                let connection = connection_inner.clone();
                // remove read dependency as soon as possible
                drop(conn);
                let datagram = connection.read_datagram().await;
                match datagram {
                    Ok(recv_stream) => {
                        let res = recv_stream;
                        let timestamp = sys.time_get_nanoseconds();
                        let res_packet = bincode::decode_from_slice::<NetworkPacket, _>(
                            &res.as_slice(),
                            bincode::config::standard(),
                        );
                        if let Ok((res_packet, _)) = &res_packet {
                            match res_packet {
                                NetworkPacket::Internal(internal_packet) => {
                                    Self::handle_internal_packet(
                                        &sys,
                                        &game_event_generator_clone,
                                        &connection_identifier,
                                        &connection_async,
                                        internal_packet,
                                    )
                                    .await;
                                }
                                NetworkPacket::User(_user_packet) => {
                                    game_event_generator_clone
                                        .lock()
                                        .await
                                        .generate_from_binary(
                                            timestamp,
                                            &connection_identifier,
                                            res.as_slice(),
                                        );
                                }
                            }
                        }
                    }
                    Err(recv_err) => {
                        println!("connection stream acception failed {}", recv_err);
                        let mut connections = connections_clone.lock().await;
                        let _con_rem_res = connections.1.remove(&connection_identifier);
                        drop(connections);
                        connection.close(VarInt::default(), &[]);
                        let timestamp = sys.time_get_nanoseconds();
                        game_event_generator_clone
                            .lock()
                            .await
                            .generate_from_network_event(
                                timestamp,
                                &connection_identifier,
                                &NetworkGameEvent::Disconnected(recv_err.to_string()),
                            );

                        break 'conn_loop;
                    }
                }
            } else {
                break 'conn_loop;
            }
        }
    }

    async fn handle_connection_recv(
        connection_async: Arc<TokioRwLock<NetworkConnection<C, Z>>>,
        connections_clone: Arc<
            TokioMutex<(
                Arc<NetworkConnectionIDCounter>,
                HashMap<NetworkConnectionID, Arc<TokioRwLock<NetworkConnection<C, Z>>>>,
            )>,
        >,
        game_event_generator_clone: Arc<TokioMutex<dyn NetworkEventToGameEventGenerator + Send>>,
        connection_identifier: NetworkConnectionID,
        sys: Arc<SystemTime>,
    ) {
        'conn_loop: loop {
            let conn = connection_async.read().await;
            let connection_res = conn.conn.as_ref();
            if let Some(connection_inner) = connection_res {
                let connection = connection_inner.clone();
                // remove read dependency as soon as possible
                drop(conn);
                let uni = connection.accept_bi().await;
                match uni {
                    Ok((_, mut recv_stream)) => {
                        let read_res = recv_stream.read_to_end(1024 as usize * 1024 * 1024).await;

                        match read_res {
                            Ok(res) => {
                                let timestamp = sys.time_get_nanoseconds();
                                let res_packet = bincode::decode_from_slice::<NetworkPacket, _>(
                                    &res[..],
                                    bincode::config::standard(),
                                );
                                if let Ok((res_packet, _)) = &res_packet {
                                    match res_packet {
                                        NetworkPacket::Internal(internal_packet) => {
                                            Self::handle_internal_packet(
                                                &sys,
                                                &game_event_generator_clone,
                                                &connection_identifier,
                                                &connection_async,
                                                internal_packet,
                                            )
                                            .await;
                                        }
                                        NetworkPacket::User(user_packet) => {
                                            game_event_generator_clone
                                                .lock()
                                                .await
                                                .generate_from_binary(
                                                    timestamp,
                                                    &connection_identifier,
                                                    user_packet.as_slice(),
                                                );
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                let mut connections = connections_clone.lock().await;
                                let _con_rem_res = connections.1.remove(&connection_identifier);
                                drop(connections);
                                connection.close(VarInt::default(), &[]);
                                let timestamp = sys.time_get_nanoseconds();
                                game_event_generator_clone
                                    .lock()
                                    .await
                                    .generate_from_network_event(
                                        timestamp,
                                        &connection_identifier,
                                        &NetworkGameEvent::Disconnected(err),
                                    );
                                break 'conn_loop;
                            }
                        }
                    }
                    Err(recv_err) => {
                        println!("connection stream acception failed {}", recv_err);
                        let mut connections = connections_clone.lock().await;
                        let _con_rem_res = connections.1.remove(&connection_identifier);
                        drop(connections);
                        connection.close(VarInt::default(), &[]);
                        let timestamp = sys.time_get_nanoseconds();
                        game_event_generator_clone
                            .lock()
                            .await
                            .generate_from_network_event(
                                timestamp,
                                &connection_identifier,
                                &NetworkGameEvent::Disconnected(recv_err.to_string()),
                            );

                        break 'conn_loop;
                    }
                }
            } else {
                break 'conn_loop;
            }
        }
    }

    async fn ping(
        sys: Arc<SystemTime>,
        connection_async: Arc<TokioRwLock<NetworkConnection<C, Z>>>,
        interval: &mut tokio::time::Interval,
    ) {
        let mut identifier: u64 = 0;
        loop {
            interval.tick().await;
            let conn = connection_async.clone();
            let sys = sys.clone();
            identifier += 1;
            let identifier_copy = identifier;
            // spawn a new ping task
            tokio::spawn(async move {
                // send a normal ping pong peng task
                let con_g = conn.read().await;
                let con_res = con_g.conn.as_ref();
                if let Some(con) = con_res {
                    let connection = con.clone();
                    drop(con_g);
                    let cur_time = sys.time_get_nanoseconds();
                    let mut con_g = conn.write().await;
                    let handle_res = con_g.ping_handles.try_add(identifier_copy, cur_time);

                    if let Ok(handle) = handle_res {
                        handle.ping_pong_peng_start_timestamp = cur_time;
                        drop(con_g);
                        Self::send_datagram(
                            &connection,
                            &NetworkPacket::Internal(InternalNetworkPackets::PingFamily(
                                identifier_copy,
                                InternalPingNetworkPackets::Ping,
                            )),
                        )
                        .await;
                    }
                }
            });
        }
    }

    pub fn handle_connection(
        connections: &Arc<
            TokioMutex<(
                Arc<NetworkConnectionIDCounter>,
                HashMap<NetworkConnectionID, Arc<TokioRwLock<NetworkConnection<C, Z>>>>,
            )>,
        >,
        game_event_generator: &Arc<TokioMutex<dyn NetworkEventToGameEventGenerator + Send>>,
        conn: Z,
        pre_defined_id: &NetworkConnectionID,
        sys: Arc<SystemTime>,
        _is_server: bool,
    ) {
        println!("handling connecting request");
        let connection = Arc::new(TokioRwLock::new(NetworkConnection::<C, Z> {
            conn: None,
            connecting: Some(conn),

            ping_handles: NetworkConnectionPingHandle::new(),
        }));
        let connection_async = connection.clone();
        let connections_clone = connections.clone();
        let game_event_generator_clone = game_event_generator.clone();

        let pre_def_id = *pre_defined_id;
        tokio::spawn(async move {
            let mut connection_identifier = INVALID_NETWORK_CON_IDENTIFIER;
            {
                let mut connections = connections_clone.lock().await;
                if pre_def_id != INVALID_NETWORK_CON_IDENTIFIER {
                    connection_identifier = pre_def_id;
                } else {
                    connection_identifier = connections.0.get_next();
                }
                connections.1.insert(connection_identifier, connection);
                drop(connections);
            }
            {
                let mut conn = connection_async.write().await;
                let mut connecting: Option<Z> = None;
                std::mem::swap(&mut connecting, &mut conn.connecting);
                match connecting.unwrap().await {
                    Ok(connection) => {
                        conn.conn = Some(connection);
                        println!("connecting established");
                        let timestamp = sys.time_get_nanoseconds();
                        game_event_generator_clone
                            .lock()
                            .await
                            .generate_from_network_event(
                                timestamp,
                                &connection_identifier,
                                &NetworkGameEvent::Connected,
                            );
                    }
                    Err(err) => {
                        println!("Connection failed to resolve (connecting failed)");
                        let timestamp = sys.time_get_nanoseconds();
                        game_event_generator_clone
                            .lock()
                            .await
                            .generate_from_network_event(
                                timestamp,
                                &connection_identifier,
                                &NetworkGameEvent::ConnectingFailed(err),
                            );
                    }
                }
                drop(conn);
            }
            tokio::spawn(async move {
                let mut ping_interval = tokio::time::interval(Duration::from_secs(1) / 10); // TODO currently 10 times per second
                tokio::select! {
                    _ = Self::handle_connection_recv(connection_async.clone(), connections_clone.clone(), game_event_generator_clone.clone(), connection_identifier, sys.clone()) => {}
                    _ = Self::handle_connection_recv_datagram(connection_async.clone(), connections_clone, game_event_generator_clone, connection_identifier, sys.clone()) => {}
                    _ = Self::ping( sys, connection_async, &mut ping_interval) => {}
                }
                println!("connection dropped.");
            })
        });
    }

    pub fn run(
        thread: &mut NetworkThread<E, C, Z>,
        is_closed: &AtomicBool,
        events_guarded: &StdMutex<NetworkEvents>,
        events_cond: &std::sync::Condvar,
    ) {
        let mut events = events_guarded.lock().unwrap();
        while !is_closed.load(std::sync::atomic::Ordering::Relaxed) {
            if events.events.is_empty() {
                events = events_cond
                    .wait_while(events, |events| events.events.is_empty())
                    .unwrap();
            } else {
                while !events.events.is_empty() {
                    match &events.events[0] {
                        NetworkEvent::Connect(con_id, addr) => {
                            println!("connecting to {}", addr);
                            let conn_res = thread
                                .endpoint
                                .connect(addr.as_str().parse().unwrap(), "localhost");
                            match conn_res {
                                Ok(conn) => {
                                    Self::handle_connection(
                                        &thread.connections,
                                        &thread.game_event_generator,
                                        conn,
                                        con_id,
                                        thread.sys.clone(),
                                        thread.is_server,
                                    );
                                }
                                Err(conn) => {
                                    let game_event_generator_clone =
                                        thread.game_event_generator.clone();
                                    let timestamp = thread.sys.as_ref().time_get_nanoseconds();
                                    tokio::spawn(async move {
                                        game_event_generator_clone
                                            .lock()
                                            .await
                                            .generate_from_network_event(
                                                timestamp,
                                                &INVALID_NETWORK_CON_IDENTIFIER,
                                                &NetworkGameEvent::ConnectingFailed(
                                                    conn.to_string(),
                                                ),
                                            );
                                    });
                                }
                            }
                        }
                        NetworkEvent::Disconnect(connection_id) => {
                            println!("disconnecting");
                            let connections_ = thread.connections.clone();
                            let con_id = *connection_id;
                            tokio::spawn(async move {
                                let mut connections_guard = connections_.lock().await;
                                let (_, connections) = &mut *connections_guard;
                                // remove the connection if it exists
                                let con = connections.remove(&con_id);
                                drop(connections_guard);
                                if let Some(conn) = con {
                                    let mut connection = conn.write().await;
                                    if let Some(connecting) = &mut connection.connecting {
                                        let connecting_res = connecting.await;
                                        if let Ok(connection) = connecting_res {
                                            connection.close(VarInt::default(), &[]);
                                        }
                                    } else if let Some(connection) = &mut connection.conn {
                                        connection.close(VarInt::default(), &[]);
                                    }
                                }
                            });
                        }
                        NetworkEvent::Close() => thread.endpoint.close(VarInt::default(), &[]),
                        NetworkEvent::Send((connection_id, packet)) => {
                            let connections_ = thread.connections.clone();
                            let packet_send = NetworkPacket::User(packet.clone());
                            let con_id = *connection_id;
                            tokio::spawn(async move {
                                let connections_guard = connections_.lock().await;
                                let (_, connections) = &*connections_guard;
                                // if the connection exists
                                let connection = connections.get(&con_id);
                                if let Some(conn) = connection {
                                    let connection = conn.clone();
                                    drop(connections_guard);
                                    let conn_g = connection.read().await;
                                    let con_res = conn_g.conn.as_ref();
                                    if let Some(con) = con_res {
                                        let con_clone = con.clone();
                                        drop(conn_g);
                                        let uni = con_clone.open_bi().await;
                                        if let Ok((mut stream, _)) = uni {
                                            let write_packet_res = bincode::encode_to_vec(
                                                packet_send,
                                                bincode::config::standard(),
                                            );
                                            if let Ok(write_packet) = write_packet_res {
                                                let written_bytes =
                                                    stream.write(&write_packet.as_slice()).await;
                                                if let Err(_written_bytes) = written_bytes {
                                                    println!("packet write failed.");
                                                } else {
                                                    let finish_res = stream.finish().await;
                                                    if let Err(err) = finish_res {
                                                        println!(
                                                            "packet finish failed: {}",
                                                            err.to_string()
                                                        );
                                                    }
                                                }
                                            }
                                        } else if let Err(stream_err) = uni {
                                            println!("sent stream err: {}", stream_err.to_string());
                                        }
                                    }
                                }
                            });
                        }
                        _ => {
                            todo!("error handling")
                        }
                    }
                    events.events.pop_front();
                }
            }
        }
    }

    pub fn init_server(
        addr: &str,
        game_event_generator: Arc<TokioMutex<dyn NetworkEventToGameEventGenerator + Send>>,
        cert: &Certificate,
        sys: Arc<SystemTime>,
        thread_count: Option<usize>,
    ) -> (Self, Vec<u8>) {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(
                thread_count.unwrap_or(
                    std::thread::available_parallelism()
                        .unwrap_or(NonZeroUsize::new(1).unwrap())
                        .into(),
                ),
            )
            .build()
            .unwrap();
        let runtime_guard = runtime.enter();

        let server_addr = addr.parse().unwrap();
        let server = E::make_server_endpoint(server_addr, cert);
        if let Err(err) = &server {
            println!("{}", err);
        }
        let (endpoint, server_cert) = server.unwrap();

        let counter = Arc::new(NetworkConnectionIDCounter::new());

        let endpoint_thread = endpoint.clone();
        let mut res = Network {
            is_server: true,
            is_closed: Arc::new(AtomicBool::new(false)),
            endpoint: endpoint,
            thread: Arc::new(StdMutex::<NetworkThread<E, C, Z>>::new(NetworkThread::<
                E,
                C,
                Z,
            > {
                is_server: true,
                endpoint: endpoint_thread,
                connections: Arc::new(TokioMutex::new((counter.clone(), HashMap::new()))),
                game_event_generator: game_event_generator,
                sys: sys.clone(),
            })),
            events: Arc::new(StdMutex::new(NetworkEvents {
                events: VecDeque::new(),
            })),
            events_cond: Arc::new(Default::default()),
            run_thread: None,
            connection_id_generator: counter,
            connecting_connection_id: INVALID_NETWORK_CON_IDENTIFIER,
            sys: sys,
            r: Default::default(),
            s: Default::default(),
        };
        drop(runtime_guard);
        res.init(runtime);
        (res, server_cert)
    }

    pub fn init_client(
        addr: &str,
        server_cert: &[u8],
        game_event_generator: Arc<TokioMutex<dyn NetworkEventToGameEventGenerator + Send>>,
        sys: Arc<SystemTime>,
    ) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let runtime_guard = runtime.enter();

        let client_addr = addr.parse().unwrap();
        let endpoint = E::make_client_endpoint(client_addr, &[server_cert]).unwrap();

        let counter = Arc::new(NetworkConnectionIDCounter::new());

        let endpoint_thread = endpoint.clone();
        let mut res = Self {
            is_server: false,
            is_closed: Arc::new(AtomicBool::new(false)),
            endpoint: endpoint,
            thread: Arc::new(StdMutex::<NetworkThread<E, C, Z>>::new(NetworkThread::<
                E,
                C,
                Z,
            > {
                is_server: false,
                endpoint: endpoint_thread,
                connections: Arc::new(TokioMutex::new((counter.clone(), HashMap::new()))),
                game_event_generator: game_event_generator,
                sys: sys.clone(),
            })),
            events: Arc::new(StdMutex::new(NetworkEvents {
                events: VecDeque::new(),
            })),
            events_cond: Arc::new(Default::default()),
            run_thread: None,
            connection_id_generator: counter,
            connecting_connection_id: INVALID_NETWORK_CON_IDENTIFIER,
            sys,
            r: Default::default(),
            s: Default::default(),
        };

        drop(runtime_guard);
        res.init(runtime);
        res
    }

    pub fn init(&mut self, runtime: tokio::runtime::Runtime) {
        let network = self.thread.clone();
        let is_closed = self.is_closed.clone();
        let events = self.events.clone();
        let events_cond = self.events_cond.clone();
        self.run_thread = Some(std::thread::spawn(move || {
            let _runtime_guard = runtime.enter();
            let mut network_thread = network.lock().unwrap();
            let endpoint = network_thread.endpoint.clone();
            let connections = network_thread.connections.clone();
            let game_event_generator = network_thread.game_event_generator.clone();
            let sys = network_thread.sys.clone();

            let is_server = network_thread.is_server;
            if is_server {
                tokio::spawn(async move {
                    println!("server: starting to accept connections");
                    while let Some(conn) = endpoint.accept().await {
                        println!("server: accepted a connection");
                        Self::handle_connection(
                            &connections,
                            &game_event_generator,
                            conn,
                            &INVALID_NETWORK_CON_IDENTIFIER,
                            sys.clone(),
                            is_server,
                        );
                    }
                });
            }
            Self::run(&mut network_thread, &is_closed, &*events, &*events_cond);
        }));
    }

    pub fn close(&mut self) {
        let mut writer = self.events.lock().unwrap();
        writer.events.push_back(NetworkEvent::Close());
        writer = self.events_cond.wait(writer).unwrap();
        self.events_cond.notify_all();
        drop(writer);

        let mut run_thread: Option<std::thread::JoinHandle<()>> = None;
        std::mem::swap(&mut run_thread, &mut self.run_thread);
        if let Err(_) = run_thread.unwrap().join() {
            // TODO logging
        }
    }

    pub fn disconnect(&mut self, connection_id: &NetworkConnectionID) {
        let mut writer = self.events.lock().unwrap();
        writer
            .events
            .push_back(NetworkEvent::Disconnect(*connection_id));
        self.connecting_connection_id = INVALID_NETWORK_CON_IDENTIFIER;
        self.events_cond.notify_all();
    }

    pub fn connect(&mut self, connect_addr: &str) -> NetworkConnectionID {
        if self.connecting_connection_id != INVALID_NETWORK_CON_IDENTIFIER {
            self.disconnect(&self.connecting_connection_id.clone());
        }
        self.connecting_connection_id = self.connection_id_generator.get_next();

        let mut writer = self.events.lock().unwrap();
        writer.events.push_back(NetworkEvent::Connect(
            self.connecting_connection_id,
            connect_addr.to_string(),
        ));
        self.events_cond.notify_all();

        self.connecting_connection_id
    }

    pub fn send_to<T>(&mut self, msg: &T, connection_id: &NetworkConnectionID)
    where
        T: bincode::enc::Encode,
    {
        let mut writer = self.events.lock().unwrap();
        let packet = bincode::encode_to_vec(msg, bincode::config::standard()).unwrap();
        writer
            .events
            .push_back(NetworkEvent::Send((*connection_id, packet)));
        self.events_cond.notify_all();
    }

    /**
     * Only use this if `connect` was used
     */
    pub fn send_to_server<T>(&mut self, msg: &T)
    where
        T: bincode::enc::Encode,
    {
        if self.connecting_connection_id != INVALID_NETWORK_CON_IDENTIFIER {
            self.send_to(msg, &self.connecting_connection_id.clone());
        }
    }

    /*
     * Only use this if you also used connect
     */
    pub fn get_current_connect_id(&self) -> NetworkConnectionID {
        self.connecting_connection_id
    }
    // https://ryhl.io/blog/async-what-is-blocking/
    // https://github.com/tokio-rs/tokio/discussions/3858
    // Toktion Runtime::enter
}
