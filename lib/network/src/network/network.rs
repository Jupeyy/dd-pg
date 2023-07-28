use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    net::SocketAddr,
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc, Mutex as StdMutex, Weak,
    },
    time::Duration,
};

use anyhow::anyhow;
use async_trait::async_trait;
use hashlink::LinkedHashMap;
use pool::{mt_datatypes::PoolVec, mt_pool::Pool};
use quinn::VarInt;
use rcgen::Certificate;
use tokio::{
    sync::{Mutex as TokioMutex, Notify, RwLock as TokioRwLock},
    task::JoinHandle,
};

use base::system::{
    LogLevel, System, SystemLogGroup, SystemLogInterface, SystemTime, SystemTimeInterface,
};

struct NetworkConnectionIDCounter(AtomicU64);

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub struct NetworkConnectionID(u64);
const MIN_NETWORK_CON_IDENTIFIER: NetworkConnectionID = NetworkConnectionID(1);
const INVALID_NETWORK_CON_IDENTIFIER: NetworkConnectionID = NetworkConnectionID(0);

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

impl NetworkConnectionIDCounter {
    fn new() -> Self {
        Self(AtomicU64::new(MIN_NETWORK_CON_IDENTIFIER.0))
    }

    fn get_next(&self) -> NetworkConnectionID {
        NetworkConnectionID(self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub ping: Duration,
}

#[derive(Debug, Clone)]
pub enum NetworkGameEvent {
    Connected,
    Disconnected(String),
    ConnectingFailed(String),
    NetworkStats(NetworkStats),
}

#[derive(Debug, Clone)]
pub struct NetworkEventNotifier {
    rt: Weak<tokio::runtime::Runtime>,
    notify: Arc<Notify>,
}

impl NetworkEventNotifier {
    /// returns false if timeout was exceeded, others always returns true
    pub fn wait_for_event(&self, timeout: Option<Duration>) -> bool {
        let _g = self.rt.upgrade().unwrap().enter();
        tokio::task::block_in_place(|| {
            self.rt.upgrade().unwrap().block_on(async {
                match timeout {
                    Some(timeout) => {
                        match tokio::time::timeout(timeout, self.notify.notified()).await {
                            Ok(_) => true,
                            Err(_) => false,
                        }
                    }
                    None => {
                        self.notify.notified().await;
                        true
                    }
                }
            })
        })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NetworkInOrderChannel {
    Global,
    Custom(usize),
}

#[async_trait]
pub trait NetworkEventToGameEventGenerator {
    async fn generate_from_binary(
        &self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        bytes: &[u8],
    );

    /**
     * Returns true if the network notifier should be notified
     * Returning false can make sense if the notifier should not
     * notify about events with less priority, such as a network stat
     * event.
     * Important: You should be careful returning false, it might fill up
     * your event queue, if you use something like that. E.g. network stats are sent
     * quite regularly
     */
    async fn generate_from_network_event(
        &self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        network_event: &NetworkGameEvent,
    ) -> bool;
}

enum NetworkEventSendType {
    // packet loss possible, out of order possible
    UnreliableUnordered,
    // packet loss **not** possible, out of order possible
    ReliableUnordered,
    // packet loss **not** possible, **in-order**
    ReliableOrdered(NetworkInOrderChannel),
}

enum NetworkEvent {
    Connect(NetworkConnectionID, String),
    Disconnect(NetworkConnectionID),
    Close(),
    Send((NetworkConnectionID, PoolVec<u8>, NetworkEventSendType)),
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
    User(PoolVec<u8>),
}

pub struct NetworkSharedInitOptions {
    pub debug_printing: bool,
    pub timeout: Duration,
}

impl NetworkSharedInitOptions {
    pub fn new() -> Self {
        Self {
            debug_printing: false,
            timeout: Duration::ZERO,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_debug_priting(mut self, debug_printing: bool) -> Self {
        self.debug_printing = debug_printing;
        self
    }
}

pub struct NetworkServerInitOptions {
    pub base: NetworkSharedInitOptions,
    pub max_thread_count: usize,
    // disallow QUICs 0.5-RTT fast connection
    pub disallow_05_rtt: bool,
}

impl NetworkServerInitOptions {
    pub fn new() -> Self {
        Self {
            base: NetworkSharedInitOptions::new(),
            max_thread_count: 0,
            disallow_05_rtt: false,
        }
    }

    pub fn with_max_thread_count(mut self, max_thread_count: usize) -> Self {
        self.max_thread_count = max_thread_count;
        self
    }

    pub fn with_disallow_05_rtt(mut self, disallow_05_rtt: bool) -> Self {
        self.disallow_05_rtt = disallow_05_rtt;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.base = self.base.with_timeout(timeout);
        self
    }

    pub fn with_debug_priting(mut self, debug_printing: bool) -> Self {
        self.base = self.base.with_debug_priting(debug_printing);
        self
    }
}

pub struct NetworkClientInitOptions {
    pub base: NetworkSharedInitOptions,
    // not recommended, only useful for debugging
    pub skip_cert_check: bool,
}

impl NetworkClientInitOptions {
    pub fn new() -> Self {
        Self {
            base: NetworkSharedInitOptions::new(),
            skip_cert_check: false,
        }
    }

    pub fn with_skip_cert_check(mut self, skip_cert_check: bool) -> Self {
        self.skip_cert_check = skip_cert_check;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.base = self.base.with_timeout(timeout);
        self
    }

    pub fn with_debug_priting(mut self, debug_printing: bool) -> Self {
        self.base = self.base.with_debug_priting(debug_printing);
        self
    }
}

struct NetworkEvents {
    events: VecDeque<NetworkEvent>,
}

struct NetworkConnectionPingHandleImpl {
    handle_timestamp: Duration,

    ping_pong_peng_start_timestamp: Duration,
}

impl NetworkConnectionPingHandleImpl {
    fn new(add_timestamp: Duration) -> Self {
        Self {
            handle_timestamp: add_timestamp,

            ping_pong_peng_start_timestamp: Duration::ZERO,
        }
    }
}

struct NetworkConnectionPingHandle {
    list: LinkedHashMap<u64, NetworkConnectionPingHandleImpl>,
}

impl NetworkConnectionPingHandle {
    fn new() -> Self {
        Self {
            list: Default::default(),
        }
    }

    fn remove_outdated(&mut self, cur_time: Duration) {
        // check if there are outdated ping handles
        while !self.list.is_empty() {
            if cur_time - self.list.values().next().unwrap().handle_timestamp
                > Duration::from_secs(2)
            {
                self.list.pop_front();
            } else {
                break;
            }
        }
    }

    fn try_remove(
        &mut self,
        identifier: &u64,
        sys: &Arc<SystemTime>,
    ) -> Option<NetworkConnectionPingHandleImpl> {
        let cur_time = sys.time_get_nanoseconds();
        self.remove_outdated(cur_time);

        self.list.remove(identifier)
    }

    fn try_add(
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

pub struct NetworkConnection<C: Send + Sync, Z: Send + Sync> {
    conn: Option<C>,
    connecting: Option<Z>,

    ping_handles: NetworkConnectionPingHandle,
    inc_ping_handles: NetworkConnectionPingHandle,
}

#[async_trait::async_trait]
pub trait NetworkEndpointInterface<Z>
where
    Self: Sized,
{
    fn close(&self, error_code: VarInt, reason: &[u8]);
    fn connect(&self, addr: SocketAddr, server_name: &str) -> anyhow::Result<Z>;
    async fn accept(&self) -> Option<Z>;
    fn sock_addr(&self) -> anyhow::Result<SocketAddr>;

    fn make_server_endpoint(
        bind_addr: SocketAddr,
        cert: &Certificate,
        options: &Option<NetworkServerInitOptions>,
    ) -> anyhow::Result<(Self, Vec<u8>)>;

    fn make_client_endpoint(
        bind_addr: SocketAddr,
        server_certs: &[&[u8]],
        options: &Option<NetworkClientInitOptions>,
    ) -> anyhow::Result<Self>;
}

struct NetworkConnections<C: Send + Sync, Z: Send + Sync>(
    Arc<
        TokioMutex<(
            Arc<NetworkConnectionIDCounter>,
            HashMap<NetworkConnectionID, Arc<TokioRwLock<NetworkConnection<C, Z>>>>,
        )>,
    >,
);

impl<C: Send + Sync, Z: Send + Sync> Clone for NetworkConnections<C, Z> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<C: Send + Sync + Clone, Z: Send + Sync> NetworkConnections<C, Z> {
    fn new(counter: Arc<NetworkConnectionIDCounter>) -> Self {
        Self {
            0: Arc::new(TokioMutex::new((counter, HashMap::new()))),
        }
    }

    async fn get_connection_clone_by_id(
        &self,
        id: &NetworkConnectionID,
    ) -> Option<Arc<TokioRwLock<NetworkConnection<C, Z>>>> {
        let connections_guard = self.0.lock().await;
        let (_, connections) = &*connections_guard;
        // check if the connection exists
        let connection = connections.get(id);
        if let Some(conn) = connection {
            let connection = conn.clone();
            drop(connections_guard);
            Some(connection)
        } else {
            None
        }
    }

    pub async fn get_connection_impl_clone_by_id(&self, id: &NetworkConnectionID) -> Option<C> {
        let connection = self.get_connection_clone_by_id(id).await;
        if let Some(connection) = connection {
            let conn_g = connection.read().await;
            let con_res = conn_g.conn.as_ref();
            if let Some(con) = con_res {
                let con_clone = con.clone();
                drop(conn_g);
                Some(con_clone)
            } else {
                None
            }
        } else {
            None
        }
    }
}

type NetworkInOrderPackets = HashMap<
    NetworkConnectionID,
    HashMap<NetworkInOrderChannel, Arc<TokioMutex<VecDeque<NetworkPacket>>>>,
>;

#[derive(Clone)]
pub struct InternalGameEventGenerator {
    game_event_generator: Arc<dyn NetworkEventToGameEventGenerator + Sync + Send>,
    game_event_notifier: NetworkEventNotifier,
}

impl InternalGameEventGenerator {
    async fn generate_from_binary(
        &mut self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        bytes: &[u8],
    ) {
        self.game_event_generator
            .generate_from_binary(timestamp, con_id, bytes)
            .await;
        self.game_event_notifier.notify.notify_one();
    }

    async fn generate_from_network_event(
        &mut self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        network_event: &NetworkGameEvent,
    ) {
        if self
            .game_event_generator
            .generate_from_network_event(timestamp, con_id, network_event)
            .await
        {
            self.game_event_notifier.notify.notify_one();
        }
    }
}

struct NetworkThread<E, C: Send + Sync, Z: Send + Sync> {
    is_server: bool,
    endpoint: E,
    connections: NetworkConnections<C, Z>,
    all_in_order_packets: Arc<TokioMutex<NetworkInOrderPackets>>,
    game_event_generator: InternalGameEventGenerator,
    sys: Arc<SystemTime>,
    logger: Arc<TokioMutex<SystemLogGroup>>,
    is_debug: bool,
    packet_pool: Pool<Vec<u8>>,
}

#[async_trait::async_trait]
pub trait NetworkConnectionInterface {
    fn close(&self, error_code: VarInt, reason: &[u8]);

    async fn send_unreliable_unordered(&self, data: PoolVec<u8>) -> anyhow::Result<()>;
    async fn read_unreliable_unordered(&self) -> anyhow::Result<Vec<u8>>;

    async fn send_unordered_reliable(&self, data: PoolVec<u8>) -> anyhow::Result<()>;
    async fn read_unordered_reliable<
        F: FnOnce(anyhow::Result<Vec<u8>>) -> JoinHandle<()> + Send + 'static,
    >(
        &self,
        on_data: F,
    ) -> anyhow::Result<()>;

    // this function guarantees that the packet was given to the implementation
    // in order. it should not block the network implementation more than necessary
    async fn push_ordered_reliable_packet_in_order(
        &self,
        data: PoolVec<u8>,
        channel: NetworkInOrderChannel,
    );
    async fn send_one_ordered_reliable(&self, channel: NetworkInOrderChannel)
        -> anyhow::Result<()>;
    async fn read_ordered_reliable<
        F: Fn(anyhow::Result<Vec<u8>>) -> JoinHandle<()> + Send + Sync + 'static,
    >(
        &self,
        on_data: F,
    ) -> anyhow::Result<()>;
}

pub trait NetworkConnectingInterface<C>
where
    Self: Sized,
{
    fn try_fast_unwrap(self) -> Result<C, Self>;
}

pub struct Network<E, C: Send + Sync, Z: Send + Sync>
where
    C: NetworkConnectionInterface,
    Z: NetworkConnectingInterface<C>,
{
    // some attributes are shared with the NetworkThread struct
    // so that the endpoint can be closed without requiring
    // an additional lock
    _is_server: bool,
    _endpoint: E,
    is_closed: Arc<AtomicBool>,
    thread: Arc<StdMutex<NetworkThread<E, C, Z>>>,
    events: Arc<StdMutex<NetworkEvents>>,
    events_cond: Arc<std::sync::Condvar>,
    run_thread: Option<std::thread::JoinHandle<()>>,

    connection_id_generator: Arc<NetworkConnectionIDCounter>,

    // for the client to remember the last server it connected to
    connecting_connection_id: NetworkConnectionID,
    packet_pool: Pool<Vec<u8>>,
    _sys: Arc<SystemTime>,
    _is_debug: bool,
}

impl<E, C, Z> Network<E, C, Z>
where
    C: NetworkConnectionInterface + Clone + Send + Sync + 'static,
    Z: NetworkConnectingInterface<C>
        + Send
        + Sync
        + 'static
        + Future<Output = Result<C, String>>
        + Unpin,
    E: NetworkEndpointInterface<Z> + Clone + Send + Sync + 'static,
{
    /// this function must respect the UDP frame size and should never be bigger than ~1.4KB
    async fn send_internal_packet_unreliable(
        connection: &C,
        packet: &NetworkPacket,
        pool: &mut Pool<Vec<u8>>,
    ) -> anyhow::Result<()> {
        let mut packet_encoded = pool.new();
        let res = bincode::encode_into_std_write(
            packet,
            &mut std::io::BufWriter::<&mut Vec<u8>>::new(&mut packet_encoded),
            bincode::config::standard(),
        );
        if let Ok(_) = res {
            let res = connection.send_unreliable_unordered(packet_encoded).await;
            match res {
                Ok(_) => Ok(()),
                Err(err) => Err(anyhow!(err.to_string())),
            }
        } else {
            Err(anyhow!("packet was invalid and could not be encoded.",))
        }
    }

    async fn disconnect_connection(
        con_id: &NetworkConnectionID,
        connections_clone: &NetworkConnections<C, Z>,
        connection: &C,
        sys: &Arc<SystemTime>,
        game_event_generator: &mut InternalGameEventGenerator,
        reason: String,
        all_packets_in_order: &Arc<TokioMutex<NetworkInOrderPackets>>,
    ) {
        let mut connections = connections_clone.0.lock().await;
        let _con_rem_res = connections.1.remove(con_id);
        drop(connections);
        connection.close(VarInt::default(), &[]);

        let timestamp = sys.time_get_nanoseconds();
        game_event_generator
            .generate_from_network_event(timestamp, con_id, &NetworkGameEvent::Disconnected(reason))
            .await;
        all_packets_in_order.lock().await.remove(con_id);
    }

    async fn process_valid_packet(
        connection_async: &Arc<TokioRwLock<NetworkConnection<C, Z>>>,
        mut game_event_generator_clone: &mut InternalGameEventGenerator,
        connection_identifier: &NetworkConnectionID,
        sys: &Arc<SystemTime>,
        logger: &Arc<TokioMutex<SystemLogGroup>>,
        recv_stream: Vec<u8>,
        debug_printing: bool,
        pool: &mut Pool<Vec<u8>>,
    ) {
        let timestamp = sys.time_get_nanoseconds();
        let res_packet = bincode::decode_from_slice::<NetworkPacket, _>(
            &recv_stream.as_slice(),
            bincode::config::standard(),
        );
        if let Ok((res_packet, handled_size)) = &res_packet {
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
                        internal_packet,
                        debug_printing,
                        pool,
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

    async fn handle_internal_packet(
        sys: &Arc<SystemTime>,
        logger: &Arc<TokioMutex<SystemLogGroup>>,
        game_event_generator_clone: &mut InternalGameEventGenerator,
        con_id: &NetworkConnectionID,
        connection: &Arc<TokioRwLock<NetworkConnection<C, Z>>>,
        internal_packet: &InternalNetworkPackets,
        debug_printing: bool,
        pool: &mut Pool<Vec<u8>>,
    ) {
        match internal_packet {
            InternalNetworkPackets::PingFamily(identifier, packet) => match packet {
                InternalPingNetworkPackets::Ping => {
                    // add new ping handle for this identifier
                    let cur_time = sys.time_get_nanoseconds();
                    let mut con_g = connection.write().await;
                    let res = con_g.inc_ping_handles.try_add(*identifier, cur_time);
                    if let Ok(handle) = res {
                        handle.ping_pong_peng_start_timestamp = cur_time;
                        drop(con_g);
                        // also send a pong
                        let con_g = connection.read().await;
                        let con_res = con_g.conn.as_ref();
                        if let Some(con_ref) = con_res {
                            let con = con_ref.clone();
                            drop(con_g);
                            Self::send_internal_packet_unreliable(
                                &con,
                                &NetworkPacket::Internal(InternalNetworkPackets::PingFamily(
                                    *identifier,
                                    InternalPingNetworkPackets::Pong,
                                )),
                                pool,
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
                        } else {
                            return;
                        }
                    }
                }
                InternalPingNetworkPackets::Pong => {
                    let cur_time = sys.time_get_nanoseconds();
                    // update the connection ping
                    let mut con_g = connection.write().await;
                    let handle_res = con_g.ping_handles.try_remove(identifier, sys);
                    if let Some(handle) = handle_res {
                        let ping = cur_time - handle.ping_pong_peng_start_timestamp;
                        drop(con_g);
                        // generate network stats
                        game_event_generator_clone
                            .generate_from_network_event(
                                cur_time,
                                con_id,
                                &NetworkGameEvent::NetworkStats(NetworkStats { ping: ping }),
                            )
                            .await;
                        // also send a peng
                        let con_g = connection.read().await;
                        let con_res = con_g.conn.as_ref();
                        if let Some(con_ref) = con_res {
                            let con = con_ref.clone();
                            drop(con_g);
                            Self::send_internal_packet_unreliable(
                                &con,
                                &NetworkPacket::Internal(InternalNetworkPackets::PingFamily(
                                    *identifier,
                                    InternalPingNetworkPackets::Peng,
                                )),
                                pool,
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
                        } else {
                            return;
                        }
                    }
                }
                InternalPingNetworkPackets::Peng => {
                    let cur_time = sys.time_get_nanoseconds();
                    // update the connection ping
                    let mut con_g = connection.write().await;
                    let handle_res = con_g.inc_ping_handles.try_remove(identifier, sys);
                    if let Some(handle) = handle_res {
                        let ping = cur_time - handle.ping_pong_peng_start_timestamp;
                        drop(con_g);
                        // generate network stats
                        game_event_generator_clone
                            .generate_from_network_event(
                                cur_time,
                                con_id,
                                &NetworkGameEvent::NetworkStats(NetworkStats { ping: ping }),
                            )
                            .await;
                    }
                }
            },
        }
    }

    async fn handle_connection_recv_unordered_unreliable(
        connection_async: Arc<TokioRwLock<NetworkConnection<C, Z>>>,
        connections_clone: NetworkConnections<C, Z>,
        mut game_event_generator_clone: InternalGameEventGenerator,
        connection_identifier: NetworkConnectionID,
        sys: Arc<SystemTime>,
        logger: Arc<TokioMutex<SystemLogGroup>>,
        all_packets_in_order: Arc<TokioMutex<NetworkInOrderPackets>>,
        debug_printing: bool,
        pool: Pool<Vec<u8>>,
    ) {
        'conn_loop: loop {
            let conn = connection_async.read().await;
            let connection_res = conn.conn.as_ref();
            if let Some(connection_inner) = connection_res {
                let connection = connection_inner.clone();
                // remove read dependency as soon as possible
                drop(conn);
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

                        Self::disconnect_connection(
                            &connection_identifier,
                            &connections_clone,
                            &connection,
                            &sys,
                            &mut game_event_generator_clone,
                            recv_err.to_string(),
                            &all_packets_in_order,
                        )
                        .await;

                        break 'conn_loop;
                    }
                }
            } else {
                break 'conn_loop;
            }
        }
    }

    async fn handle_connection_recv_unordered_reliable(
        connection_async: Arc<TokioRwLock<NetworkConnection<C, Z>>>,
        connections_clone: NetworkConnections<C, Z>,
        mut game_event_generator: InternalGameEventGenerator,
        connection_identifier: NetworkConnectionID,
        sys: Arc<SystemTime>,
        logger: Arc<TokioMutex<SystemLogGroup>>,
        all_packets_in_order: Arc<TokioMutex<NetworkInOrderPackets>>,
        debug_printing: bool,
        pool: Pool<Vec<u8>>,
    ) {
        'conn_loop: loop {
            let conn_async_clone = connection_async.clone();
            let mut game_ev_gen_clone = game_event_generator.clone();
            let sys_clone = sys.clone();
            let logger_clone = logger.clone();
            let conn = connection_async.read().await;
            let connection_res = conn.conn.as_ref();
            if let Some(connection_inner) = connection_res {
                let connection = connection_inner.clone();
                // remove read dependency as soon as possible
                let mut pool = pool.clone();
                drop(conn);
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
                                    )
                                    .await;
                                }
                                Err(err) => {
                                    if debug_printing {
                                        logger_clone
                                            .lock()
                                            .await
                                            .log(LogLevel::Debug)
                                            .msg(
                                                "error: failed to read reliable unordered packet: ",
                                            )
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

                        Self::disconnect_connection(
                            &connection_identifier,
                            &connections_clone,
                            &connection,
                            &sys,
                            &mut game_event_generator,
                            recv_err.to_string(),
                            &all_packets_in_order,
                        )
                        .await;

                        break 'conn_loop;
                    }
                }
            } else {
                break 'conn_loop;
            }
        }
    }

    async fn handle_connection_recv_ordered_reliable(
        connection_async: Arc<TokioRwLock<NetworkConnection<C, Z>>>,
        connections_clone: NetworkConnections<C, Z>,
        mut game_event_generator: InternalGameEventGenerator,
        connection_identifier: NetworkConnectionID,
        sys: Arc<SystemTime>,
        logger: Arc<TokioMutex<SystemLogGroup>>,
        all_packets_in_order: Arc<TokioMutex<NetworkInOrderPackets>>,
        debug_printing: bool,
        pool: Pool<Vec<u8>>,
    ) {
        'conn_loop: loop {
            let conn_async_clone = connection_async.clone();
            let game_ev_gen_clone = game_event_generator.clone();
            let sys_clone = sys.clone();
            let logger_clone = logger.clone();
            let conn = connection_async.read().await;
            let connection_res = conn.conn.as_ref();
            if let Some(connection_inner) = connection_res {
                let connection = connection_inner.clone();
                let pool = pool.clone();
                // remove read dependency as soon as possible
                drop(conn);
                match connection
                    .read_ordered_reliable(move |uni| {
                        let conn_async_clone = conn_async_clone.clone();
                        let mut game_ev_gen_clone = game_ev_gen_clone.clone();
                        let sys_clone = sys_clone.clone();
                        let logger_clone = logger_clone.clone();
                        let mut pool = pool.clone();
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

                        Self::disconnect_connection(
                            &connection_identifier,
                            &connections_clone,
                            &connection,
                            &sys,
                            &mut game_event_generator,
                            recv_err.to_string(),
                            &all_packets_in_order,
                        )
                        .await;

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
        logger: Arc<TokioMutex<SystemLogGroup>>,
        connection_async: Arc<TokioRwLock<NetworkConnection<C, Z>>>,
        interval: &mut tokio::time::Interval,
        debug_printing: bool,
        pool: Pool<Vec<u8>>,
    ) {
        let mut identifier: u64 = 0;
        loop {
            interval.tick().await;
            let conn = connection_async.clone();
            let sys = sys.clone();
            identifier += 1;
            let identifier_copy = identifier;
            let mut pool = pool.clone();
            let logger = logger.clone();
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
                        Self::send_internal_packet_unreliable(
                            &connection,
                            &NetworkPacket::Internal(InternalNetworkPackets::PingFamily(
                                identifier_copy,
                                InternalPingNetworkPackets::Ping,
                            )),
                            &mut pool,
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
                }
            });
        }
    }

    async fn handle_connection(
        connections: &NetworkConnections<C, Z>,
        game_event_generator: &InternalGameEventGenerator,
        conn: Z,
        pre_defined_id: &NetworkConnectionID,
        sys: Arc<SystemTime>,
        logger: Arc<TokioMutex<SystemLogGroup>>,
        is_server: bool,
        all_packets_in_order: &Arc<TokioMutex<NetworkInOrderPackets>>,
        debug_printing: bool,
        pool: &mut Pool<Vec<u8>>,
    ) {
        logger
            .lock()
            .await
            .log(LogLevel::Debug)
            .msg("handling connecting request");
        let connection = Arc::new(TokioRwLock::new(NetworkConnection::<C, Z> {
            conn: None,
            connecting: Some(conn),

            ping_handles: NetworkConnectionPingHandle::new(),
            inc_ping_handles: NetworkConnectionPingHandle::new(),
        }));
        let connection_async = connection.clone();
        let connections_clone = connections.clone();
        let mut game_event_generator_clone = game_event_generator.clone();
        let all_packets_in_order = all_packets_in_order.clone();

        let pre_def_id = *pre_defined_id;
        let pool = pool.clone();
        tokio::spawn(async move {
            let connection_identifier;
            {
                let mut connections = connections_clone.0.lock().await;
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
                match connecting.unwrap().try_fast_unwrap() {
                    Ok(connection) => {
                        conn.conn = Some(connection);
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
                                &NetworkGameEvent::Connected,
                            )
                            .await
                    }
                    Err(connecting) => match connecting.await {
                        Ok(connection) => {
                            conn.conn = Some(connection);
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
                                    &NetworkGameEvent::Connected,
                                )
                                .await;
                        }
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
                                    &NetworkGameEvent::ConnectingFailed(err),
                                )
                                .await;
                        }
                    },
                }
                drop(conn);
            }
            let pool = pool.clone();
            tokio::spawn(async move {
                let mut ping_interval = tokio::time::interval(if !is_server {
                    Duration::from_secs(1) / 8 // 8 per second from client to server
                } else {
                    Duration::from_secs(1) / 2 // 2 per second from server to client
                });
                tokio::select! {
                    _ = Self::handle_connection_recv_unordered_reliable(connection_async.clone(), connections_clone.clone(), game_event_generator_clone.clone(), connection_identifier, sys.clone(), logger.clone(), all_packets_in_order.clone(), debug_printing, pool.clone()) => {}
                    _ = Self::handle_connection_recv_ordered_reliable(connection_async.clone(), connections_clone.clone(), game_event_generator_clone.clone(), connection_identifier, sys.clone(), logger.clone(), all_packets_in_order.clone(), debug_printing, pool.clone()) => {}
                    _ = Self::handle_connection_recv_unordered_unreliable(connection_async.clone(), connections_clone, game_event_generator_clone, connection_identifier, sys.clone(), logger.clone(), all_packets_in_order.clone(), debug_printing, pool.clone()) => {}
                    _ = Self::ping(sys, logger.clone(), connection_async, &mut ping_interval, debug_printing, pool.clone()) => {}
                }
                logger
                    .lock()
                    .await
                    .log(LogLevel::Debug)
                    .msg("connection dropped.");
            })
        });
    }

    fn run(
        thread: &mut NetworkThread<E, C, Z>,
        is_closed: &AtomicBool,
        events_guarded: &StdMutex<NetworkEvents>,
        events_cond: &std::sync::Condvar,
    ) {
        let pool = thread.packet_pool.clone();
        let mut events = events_guarded.lock().unwrap();
        while !is_closed.load(std::sync::atomic::Ordering::SeqCst) {
            if events.events.is_empty() {
                // in case someone is waiting for this to finish
                events_cond.notify_all();
                events = events_cond
                    .wait_while(events, |events| {
                        events.events.is_empty()
                            && !is_closed.load(std::sync::atomic::Ordering::SeqCst)
                    })
                    .unwrap();
            }
            for event in events.events.drain(..) {
                match event {
                    NetworkEvent::Connect(con_id, addr) => {
                        thread
                            .logger
                            .blocking_lock()
                            .log(LogLevel::Debug)
                            .msg("connecting to ")
                            .msg_var(&addr);
                        let conn_res = thread
                            .endpoint
                            .connect(addr.as_str().parse().unwrap(), "localhost");
                        match conn_res {
                            Ok(conn) => {
                                let mut pool = pool.clone();
                                let connections = thread.connections.clone();
                                let game_event_generator = thread.game_event_generator.clone();
                                let sys = thread.sys.clone();
                                let logger = thread.logger.clone();
                                let is_server = thread.is_server;
                                let all_in_order_packets = thread.all_in_order_packets.clone();
                                let is_debug = thread.is_debug;
                                tokio::spawn(async move {
                                    Self::handle_connection(
                                        &connections,
                                        &game_event_generator,
                                        conn,
                                        &con_id,
                                        sys,
                                        logger,
                                        is_server,
                                        &all_in_order_packets,
                                        is_debug,
                                        &mut pool,
                                    )
                                    .await;
                                });
                            }
                            Err(conn) => {
                                let mut game_event_generator_clone =
                                    thread.game_event_generator.clone();
                                let timestamp = thread.sys.as_ref().time_get_nanoseconds();
                                tokio::spawn(async move {
                                    game_event_generator_clone
                                        .generate_from_network_event(
                                            timestamp,
                                            &INVALID_NETWORK_CON_IDENTIFIER,
                                            &NetworkGameEvent::ConnectingFailed(conn.to_string()),
                                        )
                                        .await;
                                });
                            }
                        }
                    }
                    NetworkEvent::Disconnect(connection_id) => {
                        thread
                            .logger
                            .blocking_lock()
                            .log(LogLevel::Debug)
                            .msg("disconnecting");
                        let connections_ = thread.connections.clone();
                        let con_id = connection_id;
                        tokio::spawn(async move {
                            let mut connections_guard = connections_.0.lock().await;
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
                    NetworkEvent::Send((connection_id, packet, packet_order)) => {
                        let packet_send = NetworkPacket::User(packet);
                        let connections_ = thread.connections.clone();
                        let con_id = connection_id;
                        let debug_printing = thread.is_debug;
                        let logger = thread.logger.clone();
                        match packet_order {
                            NetworkEventSendType::ReliableOrdered(channel) => {
                                let mut in_order = thread.all_in_order_packets.blocking_lock();
                                if !in_order.contains_key(&con_id) {
                                    in_order.insert(con_id.clone(), Default::default());
                                }
                                let channels = in_order.get_mut(&con_id).unwrap();
                                if !channels.contains_key(&channel) {
                                    channels.insert(channel, Default::default());
                                }
                                let channel_packets = channels.get(&channel).unwrap().clone();
                                drop(in_order);
                                channel_packets.blocking_lock().push_back(packet_send);

                                let channel = channel;

                                let pool = pool.clone();

                                tokio::spawn(async move {
                                    let connection =
                                        connections_.get_connection_impl_clone_by_id(&con_id).await;
                                    let mut in_order = channel_packets.lock().await;
                                    let packet_to_send = in_order.pop_front();
                                    if let Some(con_clone) = connection {
                                        if let Some(packet) = packet_to_send {
                                            let mut write_packet = pool.new();
                                            let res = bincode::encode_into_std_write(
                                                packet,
                                                &mut std::io::BufWriter::<&mut Vec<u8>>::new(
                                                    &mut write_packet,
                                                ),
                                                bincode::config::standard(),
                                            );
                                            if let Ok(_) = res {
                                                con_clone
                                                    .push_ordered_reliable_packet_in_order(
                                                        write_packet,
                                                        channel,
                                                    )
                                                    .await;
                                                drop(in_order);
                                                match con_clone
                                                    .send_one_ordered_reliable(channel)
                                                    .await
                                                {
                                                    Ok(_) => {}
                                                    Err(err) => {
                                                        if debug_printing {
                                                            logger
                                                                .lock().await
                                                                .log(LogLevel::Debug)
                                                                .msg("error: send ordered packet failed: ").msg_var(&err);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                            NetworkEventSendType::UnreliableUnordered => {
                                let pool = pool.clone();
                                tokio::spawn(async move {
                                    let connection =
                                        connections_.get_connection_impl_clone_by_id(&con_id).await;
                                    if let Some(con_clone) = connection {
                                        let mut write_packet = pool.new();
                                        let res = bincode::encode_into_std_write(
                                            packet_send,
                                            &mut std::io::BufWriter::<&mut Vec<u8>>::new(
                                                &mut write_packet,
                                            ),
                                            bincode::config::standard(),
                                        );
                                        if let Ok(_) = res {
                                            match con_clone
                                                .send_unreliable_unordered(write_packet)
                                                .await
                                            {
                                                Ok(_) => {}
                                                Err(err) => {
                                                    if debug_printing {
                                                        logger
                                                            .lock().await
                                                            .log(LogLevel::Debug)
                                                            .msg("error: send unreliable unordered packet failed: ").msg_var(&err);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                            NetworkEventSendType::ReliableUnordered => {
                                let pool = pool.clone();
                                tokio::spawn(async move {
                                    let connection =
                                        connections_.get_connection_impl_clone_by_id(&con_id).await;
                                    if let Some(con_clone) = connection {
                                        let mut write_packet = pool.new();
                                        let res = bincode::encode_into_std_write(
                                            packet_send,
                                            &mut std::io::BufWriter::<&mut Vec<u8>>::new(
                                                &mut write_packet,
                                            ),
                                            bincode::config::standard(),
                                        );
                                        if let Ok(_) = res {
                                            match con_clone
                                                .send_unordered_reliable(write_packet)
                                                .await
                                            {
                                                Ok(_) => {}
                                                Err(err) => {
                                                    if debug_printing {
                                                        logger
                                                            .lock().await
                                                            .log(LogLevel::Debug)
                                                            .msg("error: send reliable unordered packet failed: ").msg_var(&err);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }
        is_closed.store(false, std::sync::atomic::Ordering::SeqCst);
        events_cond.notify_all();
    }

    pub fn init_server(
        addr: &str,
        game_event_generator: Arc<dyn NetworkEventToGameEventGenerator + Send + Sync>,
        cert: &Certificate,
        sys: &System,
        options: Option<NetworkServerInitOptions>,
    ) -> (Self, Vec<u8>, SocketAddr, NetworkEventNotifier) {
        let thread_count = if options.is_some() && options.as_ref().unwrap().max_thread_count > 0 {
            options.as_ref().unwrap().max_thread_count
        } else {
            std::thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(2).unwrap()) // at least two
                .into()
        }
        .max(2); // at least two
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(thread_count)
                .max_blocking_threads((thread_count / 2).max(2)) // at least two
                .build()
                .unwrap(),
        );
        let runtime_guard = runtime.enter();

        let event_notifier = NetworkEventNotifier {
            rt: Arc::downgrade(&runtime),
            notify: Default::default(),
        };

        let logger = Arc::new(TokioMutex::new(sys.log.logger("network_server")));

        let server_addr = addr.parse().unwrap();
        let server = E::make_server_endpoint(server_addr, cert, &options);
        if let Err(err) = &server {
            logger.blocking_lock().log(LogLevel::Info).msg_var(err);
        }
        let (endpoint, server_cert) = server.unwrap();

        let sock_addr = endpoint.sock_addr().unwrap();

        let counter = Arc::new(NetworkConnectionIDCounter::new());

        let debug_priting = if let Some(options) = options {
            options.base.debug_printing
        } else {
            false
        };

        let endpoint_thread = endpoint.clone();
        let pool = Pool::with_sized(2048, || Vec::with_capacity(4096));
        let mut res = Network {
            _is_server: true,
            is_closed: Arc::new(AtomicBool::new(false)),
            _endpoint: endpoint,
            thread: Arc::new(StdMutex::<NetworkThread<E, C, Z>>::new(NetworkThread::<
                E,
                C,
                Z,
            > {
                is_server: true,
                endpoint: endpoint_thread,
                connections: NetworkConnections::new(counter.clone()),
                all_in_order_packets: Default::default(),
                game_event_generator: InternalGameEventGenerator {
                    game_event_generator: game_event_generator,
                    game_event_notifier: event_notifier.clone(),
                },
                sys: sys.time.clone(),
                logger,
                is_debug: debug_priting,
                packet_pool: pool.clone(),
            })),
            events: Arc::new(StdMutex::new(NetworkEvents {
                events: VecDeque::new(),
            })),
            events_cond: Arc::new(Default::default()),
            run_thread: None,
            connection_id_generator: counter,
            connecting_connection_id: INVALID_NETWORK_CON_IDENTIFIER,
            packet_pool: pool,
            _sys: sys.time.clone(),
            _is_debug: debug_priting,
        };
        drop(runtime_guard);
        res.init(runtime);
        (res, server_cert, sock_addr, event_notifier)
    }

    pub fn init_client(
        addr: &str,
        server_cert: &[u8],
        game_event_generator: Arc<dyn NetworkEventToGameEventGenerator + Send + Sync>,
        sys: &System,
        options: Option<NetworkClientInitOptions>,
    ) -> (Self, NetworkEventNotifier) {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2) // at least 2
                .max_blocking_threads(2) // at least 2
                .enable_all()
                .build()
                .unwrap(),
        );
        let runtime_guard = runtime.enter();

        let event_notifier = NetworkEventNotifier {
            rt: Arc::downgrade(&runtime),
            notify: Default::default(),
        };

        let client_addr = addr.parse().unwrap();
        let endpoint = E::make_client_endpoint(client_addr, &[server_cert], &options).unwrap();

        let counter = Arc::new(NetworkConnectionIDCounter::new());

        let debug_priting = if let Some(options) = options {
            options.base.debug_printing
        } else {
            false
        };

        let endpoint_thread = endpoint.clone();
        let pool = Pool::with_sized(512, || Vec::with_capacity(4096));
        let mut res = Self {
            _is_server: false,
            is_closed: Arc::new(AtomicBool::new(false)),
            _endpoint: endpoint,
            thread: Arc::new(StdMutex::<NetworkThread<E, C, Z>>::new(NetworkThread::<
                E,
                C,
                Z,
            > {
                is_server: false,
                endpoint: endpoint_thread,
                connections: NetworkConnections::new(counter.clone()),
                all_in_order_packets: Default::default(),
                game_event_generator: InternalGameEventGenerator {
                    game_event_generator: game_event_generator,
                    game_event_notifier: event_notifier.clone(),
                },
                sys: sys.time.clone(),
                logger: Arc::new(TokioMutex::new(sys.log.logger("network_client"))),
                is_debug: debug_priting,
                packet_pool: pool.clone(),
            })),
            events: Arc::new(StdMutex::new(NetworkEvents {
                events: VecDeque::new(),
            })),
            events_cond: Arc::new(Default::default()),
            run_thread: None,
            connection_id_generator: counter,
            connecting_connection_id: INVALID_NETWORK_CON_IDENTIFIER,
            packet_pool: pool,
            _sys: sys.time.clone(),
            _is_debug: debug_priting,
        };

        drop(runtime_guard);
        res.init(runtime);
        (res, event_notifier)
    }

    fn init(&mut self, runtime: Arc<tokio::runtime::Runtime>) {
        let network = self.thread.clone();
        let is_closed = self.is_closed.clone();
        let events = self.events.clone();
        let events_cond = self.events_cond.clone();
        let mut pool = self.packet_pool.clone();
        self.run_thread = Some(std::thread::spawn(move || {
            let _runtime_guard = runtime.enter();
            let mut network_thread = network.lock().unwrap();
            let endpoint = network_thread.endpoint.clone();
            let connections = network_thread.connections.clone();
            let game_event_generator = network_thread.game_event_generator.clone();
            let sys = network_thread.sys.clone();
            let logger = network_thread.logger.clone();
            let all_packets_in_order = network_thread.all_in_order_packets.clone();

            let is_server = network_thread.is_server;
            let is_debug = network_thread.is_debug;
            if is_server {
                Some(tokio::spawn(async move {
                    logger
                        .lock()
                        .await
                        .log(LogLevel::Debug)
                        .msg("server: starting to accept connections");
                    while let Some(conn) = endpoint.accept().await {
                        logger
                            .lock()
                            .await
                            .log(LogLevel::Debug)
                            .msg("server: accepted a connection");
                        Self::handle_connection(
                            &connections,
                            &game_event_generator,
                            conn,
                            &INVALID_NETWORK_CON_IDENTIFIER,
                            sys.clone(),
                            logger.clone(),
                            is_server,
                            &all_packets_in_order,
                            is_debug,
                            &mut pool,
                        )
                        .await;
                    }
                }));
            }
            Self::run(&mut network_thread, &is_closed, &*events, &*events_cond);
            Arc::try_unwrap(runtime)
                .unwrap()
                .shutdown_timeout(Duration::from_secs(2));
        }));
    }

    pub fn close(&mut self) {
        let mut writer = self.events.lock().unwrap();
        writer.events.push_back(NetworkEvent::Close());
        self.is_closed
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.events_cond.notify_all();
        writer = self
            .events_cond
            .wait_while(writer, |_| {
                !self.is_closed.load(std::sync::atomic::Ordering::SeqCst)
            })
            .unwrap();
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

    fn send_to_impl<T>(
        &self,
        msg: &T,
        connection_id: &NetworkConnectionID,
        send_type: NetworkEventSendType,
    ) where
        T: bincode::enc::Encode,
    {
        let mut packet = self.packet_pool.new();
        let mut writer: std::io::BufWriter<&mut Vec<u8>> = std::io::BufWriter::new(&mut packet);
        bincode::encode_into_std_write(msg, &mut writer, bincode::config::standard()).unwrap();
        drop(writer);
        let mut writer = self.events.lock().unwrap();
        if writer.events.len() > 1024 {
            writer = self
                .events_cond
                .wait_while(writer, |g| g.events.len() > 4096)
                .unwrap();
        }
        writer
            .events
            .push_back(NetworkEvent::Send((*connection_id, packet, send_type)));
        self.events_cond.notify_all();
    }

    pub fn send_unordered_to<T>(&self, msg: &T, connection_id: &NetworkConnectionID)
    where
        T: bincode::enc::Encode,
    {
        self.send_to_impl(msg, connection_id, NetworkEventSendType::ReliableUnordered);
    }

    pub fn send_in_order_to<T>(
        &self,
        msg: &T,
        connection_id: &NetworkConnectionID,
        channel: NetworkInOrderChannel,
    ) where
        T: bincode::enc::Encode,
    {
        self.send_to_impl(
            msg,
            connection_id,
            NetworkEventSendType::ReliableOrdered(channel),
        );
    }

    pub fn send_unreliable_to<T>(&self, msg: &T, connection_id: &NetworkConnectionID)
    where
        T: bincode::enc::Encode,
    {
        self.send_to_impl(
            msg,
            connection_id,
            NetworkEventSendType::UnreliableUnordered,
        );
    }

    /**
     * Only use this if `connect` was used
     */
    pub fn send_unordered_to_server<T>(&self, msg: &T)
    where
        T: bincode::enc::Encode,
    {
        if self.connecting_connection_id != INVALID_NETWORK_CON_IDENTIFIER {
            self.send_unordered_to(msg, &self.connecting_connection_id.clone());
        }
    }

    /**
     * Only use this if `connect` was used
     */
    pub fn send_in_order_to_server<T>(&self, msg: &T, channel: NetworkInOrderChannel)
    where
        T: bincode::enc::Encode,
    {
        if self.connecting_connection_id != INVALID_NETWORK_CON_IDENTIFIER {
            self.send_in_order_to(msg, &self.connecting_connection_id.clone(), channel);
        }
    }

    /**
     * Only use this if `connect` was used
     */
    pub fn send_unreliable_to_server<T>(&self, msg: &T)
    where
        T: bincode::enc::Encode,
    {
        if self.connecting_connection_id != INVALID_NETWORK_CON_IDENTIFIER {
            self.send_unreliable_to(msg, &self.connecting_connection_id.clone());
        }
    }

    /*
     * Only use this if you also used connect
     */
    pub fn get_current_connect_id(&self) -> NetworkConnectionID {
        self.connecting_connection_id
    }
}
