use std::{
    collections::VecDeque,
    future::Future,
    marker::PhantomData,
    net::SocketAddr,
    num::NonZeroUsize,
    ops::DerefMut,
    sync::{atomic::AtomicBool, Arc, Mutex as StdMutex},
    time::Duration,
};

use base::{
    hash::Hash,
    system::{System, SystemTime, SystemTimeInterface},
};
use base_log::log::{LogLevel, SystemLogGroup, SystemLogInterface};
use pool::{mt_datatypes::PoolVec, mt_pool::Pool};
use quinn::VarInt;
use rcgen::{Certificate, CertifiedKey};
use serde::Serialize;
use tokio::{sync::Mutex as TokioMutex, task::JoinHandle};
use x509_certificate::X509Certificate;

use super::{
    connection::{NetworkConnectionID, INVALID_NETWORK_CON_IDENTIFIER},
    connections::{NetworkConnectionIDCounter, NetworkConnections},
    event::NetworkEvent,
    event_generator::{InternalGameEventGenerator, NetworkEventToGameEventGenerator},
    notifier::NetworkEventNotifier,
    plugins::{NetworkPluginConnection, NetworkPluginPacket},
    types::{
        NetworkEventSendType, NetworkInOrderChannel, NetworkInOrderPackets, NetworkLogicEvent,
        NetworkPacket,
    },
};

#[derive(Debug, Default)]
pub struct NetworkSharedInitOptions {
    pub debug_printing: Option<bool>,
    pub timeout: Option<Duration>,
}

impl NetworkSharedInitOptions {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn with_debug_priting(mut self, debug_printing: bool) -> Self {
        self.debug_printing = Some(debug_printing);
        self
    }
}

#[derive(Debug, Default)]
pub struct NetworkServerInitOptions {
    pub base: NetworkSharedInitOptions,
    pub max_thread_count: Option<usize>,
    /// disallow QUICs 0.5-RTT fast connection
    pub disallow_05_rtt: Option<bool>,
    /// disable that the connecting clients have
    /// to prove their connection.
    /// enabling this config makes connecting to
    /// the server faster, but might give more
    /// attack surface for DoS attacks
    pub disable_retry_on_connect: bool,
}

impl NetworkServerInitOptions {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_max_thread_count(mut self, max_thread_count: usize) -> Self {
        self.max_thread_count = Some(max_thread_count);
        self
    }

    pub fn with_disallow_05_rtt(mut self, disallow_05_rtt: bool) -> Self {
        self.disallow_05_rtt = Some(disallow_05_rtt);
        self
    }

    pub fn with_disable_retry_on_connect(mut self, disable_retry_on_connect: bool) -> Self {
        self.disable_retry_on_connect = disable_retry_on_connect;
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

#[derive(Debug)]
pub enum NetworkClientCertCheckMode<'a> {
    CheckByCert { cert: &'a [u8] },
    CheckByPubKeyHash { hash: &'a Hash },
    // not recommended, only useful for debugging
    DisableCheck,
}

pub enum NetworkClientCertMode {
    FromCertifiedKeyPair { cert: CertifiedKey },
}

pub enum NetworkServerCertModeResult {
    Cert { cert: Certificate },
    PubKeyHash { hash: Hash },
}

pub enum NetworkServerCertMode {
    FromCertifiedKeyPair { cert: CertifiedKey },
}

pub struct NetworkClientInitOptions<'a> {
    pub base: NetworkSharedInitOptions,
    pub cert_check: NetworkClientCertCheckMode<'a>,
    pub cert: NetworkClientCertMode,
}

impl<'a> NetworkClientInitOptions<'a> {
    pub fn new(cert_check: NetworkClientCertCheckMode<'a>, cert: NetworkClientCertMode) -> Self {
        Self {
            base: Default::default(),
            cert_check,
            cert,
        }
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
    events: VecDeque<NetworkLogicEvent>,
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
        cert_mode: NetworkServerCertMode,
        options: &NetworkServerInitOptions,
    ) -> anyhow::Result<(Self, NetworkServerCertModeResult)>;

    fn make_client_endpoint(
        bind_addr: SocketAddr,
        options: &NetworkClientInitOptions,
    ) -> anyhow::Result<Self>;
}

struct NetworkThread<E, C: Send + Sync> {
    is_server: bool,
    endpoint: E,
    connections: NetworkConnections<C>,
    all_in_order_packets: Arc<TokioMutex<NetworkInOrderPackets>>,
    game_event_generator: InternalGameEventGenerator,
    sys: Arc<SystemTime>,
    logger: Arc<TokioMutex<SystemLogGroup>>,
    is_debug: bool,
    packet_pool: Pool<Vec<u8>>,

    // plugins
    packet_plugins: Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
    connection_plugins: Arc<Vec<Arc<dyn NetworkPluginConnection>>>,
}

/// the interface for connections. This includes sending receiving etc.
/// If a function returns an error, this usually results into a drop of the connection
#[async_trait::async_trait]
pub trait NetworkConnectionInterface {
    async fn close(&self, error_code: VarInt, reason: &[u8]);

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

    fn remote_addr(&self) -> SocketAddr;
    fn peer_identity(&self) -> X509Certificate;
}

pub trait NetworkConnectingInterface<C>
where
    Self: Sized,
{
    fn remote_addr(&self) -> SocketAddr;
}

pub struct Network<E, C, Z>
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
    // some attributes are shared with the NetworkThread struct
    // so that the endpoint can be closed without requiring
    // an additional lock
    _is_server: bool,
    _endpoint: E,
    is_closed: Arc<AtomicBool>,
    thread: Arc<StdMutex<NetworkThread<E, C>>>,
    events: Arc<StdMutex<NetworkEvents>>,
    events_cond: Arc<std::sync::Condvar>,
    run_thread: Option<std::thread::JoinHandle<()>>,

    connection_id_generator: Arc<NetworkConnectionIDCounter>,

    // for the client to remember the last server it connected to
    connecting_connection_id: NetworkConnectionID,
    packet_pool: Pool<Vec<u8>>,
    _sys: Arc<SystemTime>,
    _is_debug: bool,

    _connecting: PhantomData<Z>,
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
    fn run(
        thread: &mut NetworkThread<E, C>,
        is_closed: &AtomicBool,
        events_guarded: &StdMutex<NetworkEvents>,
        events_cond: &std::sync::Condvar,
        runtime: &Arc<tokio::runtime::Runtime>,
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
            while let Some(event) = events.events.pop_front() {
                match event {
                    NetworkLogicEvent::Connect(con_id, addr) => {
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
                                let packet_plugins = thread.packet_plugins.clone();
                                let connection_plugins = thread.connection_plugins.clone();
                                // handle the connect sync (since it's client side only), however don't block the events queue
                                // this allows sending packages directly after connect
                                drop(events);
                                if let Err(err) = runtime.block_on(tokio::spawn(async move {
                                    NetworkConnections::handle_connection(
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
                                        &packet_plugins,
                                        &connection_plugins,
                                    )
                                    .await
                                    .await
                                })) {
                                    let mut game_event_generator_clone =
                                        thread.game_event_generator.clone();
                                    let timestamp = thread.sys.as_ref().time_get_nanoseconds();
                                    tokio::spawn(async move {
                                        game_event_generator_clone
                                            .generate_from_network_event(
                                                timestamp,
                                                &INVALID_NETWORK_CON_IDENTIFIER,
                                                &NetworkEvent::ConnectingFailed(err.to_string()),
                                            )
                                            .await;
                                    });
                                }
                                events = events_guarded.lock().unwrap();
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
                                            &NetworkEvent::ConnectingFailed(conn.to_string()),
                                        )
                                        .await;
                                });
                            }
                        }
                    }
                    NetworkLogicEvent::Disconnect(connection_id) => {
                        thread
                            .logger
                            .blocking_lock()
                            .log(LogLevel::Debug)
                            .msg("disconnecting");
                        let connections_ = thread.connections.clone();
                        let con_id = connection_id;
                        // handle the disconnect sync (since it's client side only), however don't block the events queue
                        // this allows using the network object further as desired
                        drop(events);
                        // ignore error here, nobody cares about it anyway
                        let _ = runtime.block_on(tokio::spawn(async move {
                            let mut connections_guard = connections_.connections.lock().await;
                            let connections = &mut *connections_guard;
                            // remove the connection if it exists
                            let con = connections.remove(&con_id);
                            drop(connections_guard);
                            if let Some(conn) = con {
                                conn.conn.close(VarInt::default(), &[]).await;
                            }
                        }));
                        events = events_guarded.lock().unwrap();
                    }
                    NetworkLogicEvent::Close() => thread.endpoint.close(VarInt::default(), &[]),
                    NetworkLogicEvent::Send((connection_id, packet, packet_order)) => {
                        let packet_send = NetworkPacket::User(packet);
                        let connections_ = thread.connections.clone();
                        let con_id = connection_id;
                        let debug_printing = thread.is_debug;
                        let logger = thread.logger.clone();
                        let packet_plugins = thread.packet_plugins.clone();
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
                                let packet_plugins = packet_plugins.clone();

                                tokio::spawn(async move {
                                    let connection =
                                        connections_.get_connection_impl_clone_by_id(&con_id).await;
                                    let mut in_order = channel_packets.lock().await;
                                    let packet_to_send = in_order.pop_front();
                                    if let Some(con_clone) = connection {
                                        if let Some(packet) = packet_to_send {
                                            let write_packet =
                                                NetworkConnections::<C>::prepare_write_packet(
                                                    &con_id,
                                                    &packet,
                                                    &pool,
                                                    &packet_plugins,
                                                )
                                                .await;
                                            if let Ok(write_packet) = write_packet {
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
                                let packet_plugins = thread.packet_plugins.clone();
                                tokio::spawn(async move {
                                    let connection =
                                        connections_.get_connection_impl_clone_by_id(&con_id).await;
                                    if let Some(con_clone) = connection {
                                        let write_packet =
                                            NetworkConnections::<C>::prepare_write_packet(
                                                &con_id,
                                                &packet_send,
                                                &pool,
                                                &packet_plugins,
                                            )
                                            .await;
                                        if let Ok(write_packet) = write_packet {
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
                                let packet_plugins = thread.packet_plugins.clone();
                                tokio::spawn(async move {
                                    let connection =
                                        connections_.get_connection_impl_clone_by_id(&con_id).await;
                                    if let Some(con_clone) = connection {
                                        let write_packet =
                                            NetworkConnections::<C>::prepare_write_packet(
                                                &con_id,
                                                &packet_send,
                                                &pool,
                                                &packet_plugins,
                                            )
                                            .await;
                                        if let Ok(write_packet) = write_packet {
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

    /// returns a tuple of:
    /// Self, server_cert, server_addr, net_event_notifier
    pub fn init_server(
        addr: &str,
        game_event_generator: Arc<dyn NetworkEventToGameEventGenerator + Send + Sync>,
        cert_mode: NetworkServerCertMode,
        sys: &System,
        options: NetworkServerInitOptions,
        packet_plugins: Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
        connection_plugins: Arc<Vec<Arc<dyn NetworkPluginConnection>>>,
    ) -> (
        Self,
        NetworkServerCertModeResult,
        SocketAddr,
        NetworkEventNotifier,
    ) {
        let thread_count = options
            .max_thread_count
            .unwrap_or(
                std::thread::available_parallelism()
                    .unwrap_or(NonZeroUsize::new(2).unwrap()) // at least two
                    .into(),
            )
            .max(2); // at least two
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("network-server")
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
        let server = E::make_server_endpoint(server_addr, cert_mode, &options);
        if let Err(err) = &server {
            logger.blocking_lock().log(LogLevel::Info).msg_var(err);
        }
        let (endpoint, server_cert) = server.unwrap();

        let sock_addr = endpoint.sock_addr().unwrap();

        let counter = Arc::new(NetworkConnectionIDCounter::new());

        let debug_priting = options.base.debug_printing.unwrap_or(false);

        let endpoint_thread = endpoint.clone();
        let pool = Pool::with_sized(2048, || Vec::with_capacity(4096));
        let mut res = Network {
            _is_server: true,
            is_closed: Arc::new(AtomicBool::new(false)),
            _endpoint: endpoint,
            thread: Arc::new(StdMutex::<NetworkThread<E, C>>::new(
                NetworkThread::<E, C> {
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
                    packet_plugins,
                    connection_plugins,
                },
            )),
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
            _connecting: Default::default(),
        };
        drop(runtime_guard);
        res.init(runtime);
        (res, server_cert, sock_addr, event_notifier)
    }

    pub fn init_client(
        addr: &str,
        game_event_generator: Arc<dyn NetworkEventToGameEventGenerator + Send + Sync>,
        sys: &System,
        options: NetworkClientInitOptions,
        packet_plugins: Arc<Vec<Arc<dyn NetworkPluginPacket>>>,
        connection_plugins: Arc<Vec<Arc<dyn NetworkPluginConnection>>>,
    ) -> (Self, NetworkEventNotifier) {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .thread_name("network-client")
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
        let endpoint = E::make_client_endpoint(client_addr, &options).unwrap();

        let counter = Arc::new(NetworkConnectionIDCounter::new());

        let debug_priting = options.base.debug_printing.unwrap_or(false);

        let endpoint_thread = endpoint.clone();
        let pool = Pool::with_sized(512, || Vec::with_capacity(4096));
        let mut res = Self {
            _is_server: false,
            is_closed: Arc::new(AtomicBool::new(false)),
            _endpoint: endpoint,
            thread: Arc::new(StdMutex::<NetworkThread<E, C>>::new(
                NetworkThread::<E, C> {
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
                    packet_plugins,
                    connection_plugins,
                },
            )),
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
            _connecting: Default::default(),
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
        self.run_thread = Some(
            std::thread::Builder::new()
                .name("network".into())
                .spawn(move || {
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
                    let packet_plugins = network_thread.packet_plugins.clone();
                    let connection_plugins = network_thread.connection_plugins.clone();
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
                                NetworkConnections::handle_connection(
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
                                    &packet_plugins,
                                    &connection_plugins,
                                )
                                .await;
                            }
                        }));
                    }
                    Self::run(
                        &mut network_thread,
                        &is_closed,
                        &*events,
                        &*events_cond,
                        &runtime,
                    );
                    Arc::try_unwrap(runtime)
                        .unwrap()
                        .shutdown_timeout(Duration::from_secs(2));
                })
                .unwrap(),
        );
    }

    fn close(&mut self) {
        let mut writer = self.events.lock().unwrap();
        writer.events.push_back(NetworkLogicEvent::Close());
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

    // TODO: remove this, use RAII like network instead
    pub fn disconnect(&mut self, connection_id: &NetworkConnectionID) {
        let mut writer = self.events.lock().unwrap();
        writer
            .events
            .push_back(NetworkLogicEvent::Disconnect(*connection_id));
        self.connecting_connection_id = INVALID_NETWORK_CON_IDENTIFIER;
        self.events_cond.notify_all();
    }

    // TODO: remove this merge it with init_client directly
    pub fn connect(&mut self, connect_addr: &str) -> NetworkConnectionID {
        if self.connecting_connection_id != INVALID_NETWORK_CON_IDENTIFIER {
            self.disconnect(&self.connecting_connection_id.clone());
        }
        self.connecting_connection_id = self.connection_id_generator.get_next();

        let mut writer = self.events.lock().unwrap();
        writer.events.push_back(NetworkLogicEvent::Connect(
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
        T: Serialize,
    {
        let mut packet = self.packet_pool.new();
        bincode::serde::encode_into_std_write(msg, packet.deref_mut(), bincode::config::standard())
            .unwrap();
        let mut writer = self.events.lock().unwrap();
        if writer.events.len() > 1024 {
            writer = self
                .events_cond
                .wait_while(writer, |g| g.events.len() > 4096)
                .unwrap();
        }
        writer
            .events
            .push_back(NetworkLogicEvent::Send((*connection_id, packet, send_type)));
        self.events_cond.notify_all();
    }

    pub fn send_unordered_to<T>(&self, msg: &T, connection_id: &NetworkConnectionID)
    where
        T: Serialize,
    {
        self.send_to_impl(msg, connection_id, NetworkEventSendType::ReliableUnordered);
    }

    pub fn send_in_order_to<T>(
        &self,
        msg: &T,
        connection_id: &NetworkConnectionID,
        channel: NetworkInOrderChannel,
    ) where
        T: Serialize,
    {
        self.send_to_impl(
            msg,
            connection_id,
            NetworkEventSendType::ReliableOrdered(channel),
        );
    }

    pub fn send_unreliable_to<T>(&self, msg: &T, connection_id: &NetworkConnectionID)
    where
        T: Serialize,
    {
        self.send_to_impl(
            msg,
            connection_id,
            NetworkEventSendType::UnreliableUnordered,
        );
    }

    /// Only use this if `connect` was used
    pub fn send_unordered_to_server<T>(&self, msg: &T)
    where
        T: Serialize,
    {
        if self.connecting_connection_id != INVALID_NETWORK_CON_IDENTIFIER {
            self.send_unordered_to(msg, &self.connecting_connection_id.clone());
        }
    }

    /// Only use this if `connect` was used
    pub fn send_in_order_to_server<T>(&self, msg: &T, channel: NetworkInOrderChannel)
    where
        T: Serialize,
    {
        if self.connecting_connection_id != INVALID_NETWORK_CON_IDENTIFIER {
            self.send_in_order_to(msg, &self.connecting_connection_id.clone(), channel);
        }
    }

    /// Only use this if `connect` was used
    pub fn send_unreliable_to_server<T>(&self, msg: &T)
    where
        T: Serialize,
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

impl<E, C, Z> Drop for Network<E, C, Z>
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
    fn drop(&mut self) {
        if self.connecting_connection_id != INVALID_NETWORK_CON_IDENTIFIER {
            let id = self.connecting_connection_id;
            self.disconnect(&id);
        }
        self.close()
    }
}
