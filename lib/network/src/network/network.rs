use std::{
    borrow::Cow,
    future::Future,
    marker::PhantomData,
    net::SocketAddr,
    num::NonZeroUsize,
    ops::DerefMut,
    sync::{Arc, Mutex as StdMutex},
    time::Duration,
};

use anyhow::anyhow;
use base::{
    hash::Hash,
    system::{System, SystemTime, SystemTimeInterface},
};
use ed25519_dalek::SigningKey;
use pool::{mt_datatypes::PoolVec, mt_pool::Pool};
use quinn::VarInt;
use serde::Serialize;
use thiserror::Error;
use tokio::{sync::Mutex as TokioMutex, task::JoinHandle};

use std::sync::mpsc::{Receiver, SyncSender as Sender};

use super::{
    connection::{ConnectionStats, NetworkConnectionId},
    connections::{NetworkConnectionIdCounter, NetworkConnections},
    event::NetworkEvent,
    event_generator::{InternalGameEventGenerator, NetworkEventToGameEventGenerator},
    notifier::NetworkEventNotifier,
    plugins::NetworkPlugins,
    types::{
        NetworkEventSendType, NetworkInOrderChannel, NetworkInOrderPackets, NetworkLogicEvent,
    },
};

#[derive(Debug, Default)]
pub struct NetworkSharedInitOptions {
    pub debug_printing: Option<bool>,
    pub timeout: Option<Duration>,
    /// Id generator that can be shared if multiple network implementations are required.
    pub id_generator: Arc<NetworkConnectionIdCounter>,
    /// How many packets should the backend assume to be used.
    pub packet_capacity: Option<usize>,
    /// How big should a single packet be assumed.
    /// __Caution__: this value is multiplied with
    /// [`NetworkSharedInitOptions::packet_capacity`].
    pub packet_size: Option<usize>,
    /// Max reordering of packets before it's considered lost.
    /// Should not be less than 3, per RFC5681.
    /// Note: ignored if not supported.
    pub packet_reorder_threshold: Option<u32>,
    /// Maximum reordering in time space before time based loss detection
    /// considers a packet lost, as a factor of RTT.
    /// Note: ignored if not supported.
    pub packet_time_threshold: Option<f32>,
    /// This threshold represents the number of ack-eliciting packets an endpoint
    /// may receive without immediately sending an ACK.
    pub ack_eliciting_threshold: Option<u32>,
    /// This parameter represents the maximum amount of time that an endpoint waits
    /// before sending an ACK when the ack-eliciting threshold hasn’t been reached.
    /// The effective max_ack_delay will be clamped to be at least the peer’s min_ack_delay
    /// transport parameter, and at most the greater of the current path RTT or 25ms.
    pub max_ack_delay: Option<Duration>,
    /// This threshold represents the amount of out-of-order packets that will trigger
    /// an endpoint to send an ACK, without waiting for ack_eliciting_threshold
    /// to be exceeded or for max_ack_delay to be elapsed.
    pub ack_reordering_threshold: Option<u32>,
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

    pub fn with_packet_capacity_and_size(mut self, capacity: usize, size: usize) -> Self {
        self.base.packet_capacity = Some(capacity);
        self.base.packet_size = Some(size);
        self
    }

    /// See [`NetworkSharedInitOptions::packet_reorder_threahold`] and
    /// [`NetworkSharedInitOptions::packet_time_threshold`]
    pub fn with_loss_detection_cfg(
        mut self,
        max_packet_reorder: u32,
        max_time_factor_reorder: f32,
    ) -> Self {
        self.base.packet_reorder_threshold = Some(max_packet_reorder);
        self.base.packet_time_threshold = Some(max_time_factor_reorder);
        self
    }

    /// See [`NetworkSharedInitOptions::ack_eliciting_threshold`],
    /// [`NetworkSharedInitOptions::max_ack_delay`] and
    /// [`NetworkSharedInitOptions::ack_reordering_threshold`] for more
    /// information.
    pub fn with_ack_config(
        mut self,
        ack_eliciting_threshold: u32,
        max_ack_delay: Duration,
        ack_reordering_threshold: u32,
    ) -> Self {
        self.base.ack_eliciting_threshold = Some(ack_eliciting_threshold);
        self.base.max_ack_delay = Some(max_ack_delay);
        self.base.ack_reordering_threshold = Some(ack_reordering_threshold);
        self
    }
}

#[derive(Debug)]
pub enum NetworkClientCertCheckMode<'a> {
    CheckByCert { cert: Cow<'a, [u8]> },
    CheckByPubKeyHash { hash: &'a Hash },
    // not recommended, only useful for debugging
    DisableCheck,
}

pub enum NetworkClientCertMode {
    FromCertAndPrivateKey {
        cert: x509_cert::Certificate,
        private_key: SigningKey,
    },
}

pub enum NetworkServerCertModeResult {
    Cert { cert: Box<x509_cert::Certificate> },
    PubKeyHash { hash: Hash },
}

pub struct NetworkServerCertAndKey {
    pub cert: x509_cert::Certificate,
    pub private_key: SigningKey,
}

pub enum NetworkServerCertMode {
    FromCertAndPrivateKey(Box<NetworkServerCertAndKey>),
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

    /// See [`NetworkSharedInitOptions::packet_reorder_threahold`] and
    /// [`NetworkSharedInitOptions::packet_time_threshold`]
    pub fn with_loss_detection_cfg(
        mut self,
        max_packet_reorder: u32,
        max_time_factor_reorder: f32,
    ) -> Self {
        self.base.packet_reorder_threshold = Some(max_packet_reorder);
        self.base.packet_time_threshold = Some(max_time_factor_reorder);
        self
    }

    /// See [`NetworkSharedInitOptions::ack_eliciting_threshold`],
    /// [`NetworkSharedInitOptions::max_ack_delay`] and
    /// [`NetworkSharedInitOptions::ack_reordering_threshold`] for more
    /// information.
    pub fn with_ack_config(
        mut self,
        ack_eliciting_threshold: u32,
        max_ack_delay: Duration,
        ack_reordering_threshold: u32,
    ) -> Self {
        self.base.ack_eliciting_threshold = Some(ack_eliciting_threshold);
        self.base.max_ack_delay = Some(max_ack_delay);
        self.base.ack_reordering_threshold = Some(ack_reordering_threshold);
        self
    }
}

#[async_trait::async_trait]
pub trait NetworkEndpointInterface<Z, I>: Clone + Send + Sync + 'static
where
    Self: Sized,
{
    fn close(&self, error_code: VarInt, reason: &[u8]);
    fn connect(&self, addr: SocketAddr, server_name: &str) -> anyhow::Result<Z>;
    async fn accept(&self) -> Option<I>;
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
    is_debug: bool,
    packet_pool: Pool<Vec<u8>>,

    // plugins
    plugins: NetworkPlugins,
}

/// The result of a [`NetworkConnectionInterface::send_unreliable_unordered`] request.
#[derive(Error, Debug)]
pub enum UnreliableUnorderedError {
    /// A http like error occurred.
    #[error("connection was closed: {0}")]
    ConnectionClosed(anyhow::Error),
    #[error("unreliable unordered packets are not supported")]
    Disabled,
    #[error("packet too large for a single unreliable unordered packet.")]
    TooLarge,
}

/// the interface for connections. This includes sending receiving etc.
/// If a function returns an error, this usually results into a drop of the connection
#[async_trait::async_trait]
pub trait NetworkConnectionInterface: Clone + Send + Sync + 'static {
    async fn close(&self, error_code: VarInt, reason: &[u8]);

    async fn send_unreliable_unordered(
        &self,
        data: PoolVec<u8>,
    ) -> anyhow::Result<(), (PoolVec<u8>, UnreliableUnorderedError)>;
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
    fn peer_identity(&self) -> x509_cert::Certificate;
    fn stats(&self) -> ConnectionStats;
}

pub trait NetworkConnectingInterface<C>:
    Send + Sync + 'static + Future<Output = Result<C, String>> + Unpin
where
    Self: Sized,
{
    fn remote_addr(&self) -> SocketAddr;
}

pub trait NetworkIncomingInterface<Z>: Send + Sync + 'static
where
    Self: Sized,
{
    fn remote_addr(&self) -> SocketAddr;
    fn accept(self) -> anyhow::Result<Z>;
}

pub struct Network<E, C, Z, I>
where
    C: NetworkConnectionInterface,
    Z: NetworkConnectingInterface<C>,
    I: NetworkIncomingInterface<Z>,
    E: NetworkEndpointInterface<Z, I>,
{
    // some attributes are shared with the NetworkThread struct
    // so that the endpoint can be closed without requiring
    // an additional lock
    is_server: bool,
    endpoint: E,
    thread: Arc<StdMutex<NetworkThread<E, C>>>,
    events_send: Sender<NetworkLogicEvent>,
    run_thread: Option<std::thread::JoinHandle<()>>,

    // for the client to remember the last server it connected to
    connecting_connection_id: NetworkConnectionId,
    packet_pool: Pool<Vec<u8>>,
    _sys: Arc<SystemTime>,
    _is_debug: bool,

    _connecting: PhantomData<Z>,
    _incoming: PhantomData<I>,
}

impl<E, C, Z, I> Network<E, C, Z, I>
where
    C: NetworkConnectionInterface,
    Z: NetworkConnectingInterface<C>,
    I: NetworkIncomingInterface<Z>,
    E: NetworkEndpointInterface<Z, I>,
{
    fn run(
        thread: &mut NetworkThread<E, C>,
        events: Receiver<NetworkLogicEvent>,
        runtime: &Arc<tokio::runtime::Runtime>,
    ) {
        let pool = thread.packet_pool.clone();
        while let Ok(event) = events.recv() {
            match event {
                NetworkLogicEvent::Connect(con_id, addr) => {
                    log::debug!(target: "network", "connecting to {addr}");
                    let conn_res = thread
                        .endpoint
                        .connect(addr.as_str().parse().unwrap(), "localhost");
                    match conn_res {
                        Ok(conn) => {
                            let connections = thread.connections.clone();
                            let game_event_generator = thread.game_event_generator.clone();
                            let sys = thread.sys.clone();
                            let is_server = thread.is_server;
                            let all_in_order_packets = thread.all_in_order_packets.clone();
                            let is_debug = thread.is_debug;
                            let packet_plugins = thread.plugins.packet_plugins.clone();
                            let connection_plugins = thread.plugins.connection_plugins.clone();
                            // handle the connect sync (since it's client side only)
                            if let Err(err) = runtime.block_on(tokio::spawn(async move {
                                NetworkConnections::handle_connection(
                                    &connections,
                                    &game_event_generator,
                                    conn,
                                    Some(&con_id),
                                    sys,
                                    is_server,
                                    &all_in_order_packets,
                                    is_debug,
                                    &packet_plugins,
                                    &connection_plugins,
                                )
                                .await
                                .await
                            })) {
                                let game_event_generator_clone =
                                    thread.game_event_generator.clone();
                                let timestamp = thread.sys.as_ref().time_get_nanoseconds();
                                tokio::spawn(async move {
                                    game_event_generator_clone
                                        .generate_from_network_event(
                                            timestamp,
                                            &con_id,
                                            &NetworkEvent::ConnectingFailed(err.to_string()),
                                        )
                                        .await;
                                });
                            }
                        }
                        Err(conn) => {
                            let game_event_generator_clone = thread.game_event_generator.clone();
                            let timestamp = thread.sys.as_ref().time_get_nanoseconds();
                            tokio::spawn(async move {
                                game_event_generator_clone
                                    .generate_from_network_event(
                                        timestamp,
                                        &con_id,
                                        &NetworkEvent::ConnectingFailed(conn.to_string()),
                                    )
                                    .await;
                            });
                        }
                    }
                }
                NetworkLogicEvent::Disconnect(connection_id) => {
                    log::debug!("disconnecting");
                    let connections_ = thread.connections.clone();
                    let con_id = connection_id;
                    // handle the disconnect sync (since it's client side only)
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
                    // the end of this connection
                    return;
                }
                NetworkLogicEvent::Kick(connection_id) => {
                    log::debug!("kick {connection_id:?}");
                    let connections_ = thread.connections.clone();
                    let con_id = connection_id;
                    // try to kick the connection, if exists
                    tokio::spawn(async move {
                        let mut connections_guard = connections_.connections.lock().await;
                        let connections = &mut *connections_guard;
                        // remove the connection if it exists
                        let con = connections.remove(&con_id);
                        drop(connections_guard);
                        if let Some(conn) = con {
                            conn.conn.close(VarInt::default(), &[]).await;
                        }
                    });
                }
                NetworkLogicEvent::Send((connection_id, packet, packet_order)) => {
                    let packet_send = packet;
                    let connections_ = thread.connections.clone();
                    let con_id = connection_id;
                    let debug_printing = thread.is_debug;
                    let packet_plugins = thread.plugins.packet_plugins.clone();
                    match packet_order {
                        NetworkEventSendType::ReliableOrdered(channel) => {
                            let mut in_order = thread.all_in_order_packets.blocking_lock();
                            in_order.entry(con_id).or_default();
                            let channels = in_order.get_mut(&con_id).unwrap();
                            channels.entry(channel).or_default();
                            let channel_packets = channels.get(&channel).unwrap().clone();
                            drop(in_order);
                            channel_packets.blocking_lock().push_back(packet_send);

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
                                            match con_clone.send_one_ordered_reliable(channel).await
                                            {
                                                Ok(_) => {}
                                                Err(err) => {
                                                    if debug_printing {
                                                        log::debug!("error: send ordered packet failed: {err}");
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
                            let packet_plugins = thread.plugins.packet_plugins.clone();
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
                                            Err((_, err)) => {
                                                if debug_printing {
                                                    log::debug!("error: send unreliable unordered packet failed: {err}");
                                                }
                                            }
                                        }
                                    }
                                }
                            });
                        }
                        NetworkEventSendType::ReliableUnordered => {
                            let pool = pool.clone();
                            let packet_plugins = thread.plugins.packet_plugins.clone();
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
                                        match con_clone.send_unordered_reliable(write_packet).await
                                        {
                                            Ok(_) => {}
                                            Err(err) => {
                                                if debug_printing {
                                                    log::debug!("error: send reliable unordered packet failed: {err}");
                                                }
                                            }
                                        }
                                    }
                                }
                            });
                        }
                        NetworkEventSendType::UnorderedAuto => {
                            let pool = pool.clone();
                            let packet_plugins = thread.plugins.packet_plugins.clone();
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
                                            Err((write_packet, err)) => match err {
                                                UnreliableUnorderedError::ConnectionClosed(err) => {
                                                    if debug_printing {
                                                        log::debug!("error: send auto unordered packet failed: {err}");
                                                    }
                                                }
                                                UnreliableUnorderedError::Disabled
                                                | UnreliableUnorderedError::TooLarge => {
                                                    // try unordered reliable
                                                    if let Err(err) = con_clone
                                                        .send_unordered_reliable(write_packet)
                                                        .await
                                                    {
                                                        if debug_printing {
                                                            log::debug!("error: send auto unordered packet failed: {err}");
                                                        }
                                                    }
                                                }
                                            },
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

    /// returns a tuple of:
    /// Self, server_cert, server_addr, net_event_notifier
    pub fn init_server(
        addr: &str,
        game_event_generator: Arc<dyn NetworkEventToGameEventGenerator + Send + Sync>,
        cert_mode: NetworkServerCertMode,
        sys: &System,
        options: NetworkServerInitOptions,
        plugins: NetworkPlugins,
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

        let server_addr = addr.parse().unwrap();
        let server = E::make_server_endpoint(server_addr, cert_mode, &options);
        if let Err(err) = &server {
            log::info!("{err}");
        }
        let (endpoint, server_cert) = server.unwrap();

        let sock_addr = endpoint.sock_addr().unwrap();

        let counter = options.base.id_generator;

        let debug_priting = options.base.debug_printing.unwrap_or(false);

        let endpoint_thread = endpoint.clone();
        let pool = Pool::with_sized(options.base.packet_capacity.unwrap_or(64), || {
            Vec::with_capacity(options.base.packet_size.unwrap_or(256))
        });

        let (send, recv) = std::sync::mpsc::sync_channel(1024);

        let mut res = Network {
            is_server: true,
            endpoint,
            thread: Arc::new(StdMutex::<NetworkThread<E, C>>::new(
                NetworkThread::<E, C> {
                    is_server: true,
                    endpoint: endpoint_thread,
                    connections: NetworkConnections::new(counter.clone()),
                    all_in_order_packets: Default::default(),
                    game_event_generator: InternalGameEventGenerator {
                        game_event_generator,
                        game_event_notifier: event_notifier.clone(),
                    },
                    sys: sys.time.clone(),
                    is_debug: debug_priting,
                    packet_pool: pool.clone(),
                    plugins,
                },
            )),
            events_send: send,
            run_thread: None,
            connecting_connection_id: counter.get_next(),
            packet_pool: pool,
            _sys: sys.time.clone(),
            _is_debug: debug_priting,
            _connecting: Default::default(),
            _incoming: Default::default(),
        };
        drop(runtime_guard);
        res.init(runtime, recv);
        (res, server_cert, sock_addr, event_notifier)
    }

    pub fn init_client(
        addr: &str,
        game_event_generator: Arc<dyn NetworkEventToGameEventGenerator + Send + Sync>,
        sys: &System,
        options: NetworkClientInitOptions,
        plugins: NetworkPlugins,
        connect_addr: &str,
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

        let counter = options.base.id_generator;

        let debug_priting = options.base.debug_printing.unwrap_or(false);

        let endpoint_thread = endpoint.clone();
        let pool = Pool::with_sized(options.base.packet_capacity.unwrap_or(8), || {
            Vec::with_capacity(options.base.packet_size.unwrap_or(256))
        });

        let (send, recv) = std::sync::mpsc::sync_channel(1024);

        let mut res = Self {
            is_server: false,
            endpoint,
            thread: Arc::new(StdMutex::<NetworkThread<E, C>>::new(
                NetworkThread::<E, C> {
                    is_server: false,
                    endpoint: endpoint_thread,
                    connections: NetworkConnections::new(counter.clone()),
                    all_in_order_packets: Default::default(),
                    game_event_generator: InternalGameEventGenerator {
                        game_event_generator,
                        game_event_notifier: event_notifier.clone(),
                    },
                    sys: sys.time.clone(),
                    is_debug: debug_priting,
                    packet_pool: pool.clone(),
                    plugins,
                },
            )),
            events_send: send,
            run_thread: None,
            connecting_connection_id: counter.get_next(),
            packet_pool: pool,
            _sys: sys.time.clone(),
            _is_debug: debug_priting,
            _connecting: Default::default(),
            _incoming: Default::default(),
        };

        drop(runtime_guard);
        res.init(runtime, recv);
        res.connect(connect_addr);
        (res, event_notifier)
    }

    fn init(&mut self, runtime: Arc<tokio::runtime::Runtime>, events: Receiver<NetworkLogicEvent>) {
        let network = self.thread.clone();

        let pre_defined_id = if self.is_server {
            None
        } else {
            Some(self.connecting_connection_id)
        };
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
                    let all_packets_in_order = network_thread.all_in_order_packets.clone();

                    let is_server = network_thread.is_server;
                    let is_debug = network_thread.is_debug;
                    let packet_plugins = network_thread.plugins.packet_plugins.clone();
                    let connection_plugins = network_thread.plugins.connection_plugins.clone();
                    if is_server {
                        tokio::spawn(async move {
                            log::debug!("server: starting to accept connections");
                            while let Some(conn) = endpoint.accept().await {
                                let mut should_accept = true;

                                for plugin in connection_plugins.iter() {
                                    should_accept &= plugin
                                        .on_incoming(&conn.remote_addr())
                                        .await
                                        .unwrap_or_default();
                                }

                                if let Ok(conn) = conn.accept().and_then(|conn| {
                                    should_accept
                                        .then_some(conn)
                                        .ok_or_else(|| anyhow!("connection refused"))
                                }) {
                                    log::debug!("server: accepted a connection");
                                    NetworkConnections::handle_connection(
                                        &connections,
                                        &game_event_generator,
                                        conn,
                                        pre_defined_id.as_ref(),
                                        sys.clone(),
                                        is_server,
                                        &all_packets_in_order,
                                        is_debug,
                                        &packet_plugins,
                                        &connection_plugins,
                                    )
                                    .await;
                                }
                            }
                        });
                    }
                    Self::run(&mut network_thread, events, &runtime);
                    Arc::try_unwrap(runtime)
                        .unwrap()
                        .shutdown_timeout(Duration::from_secs(2));
                })
                .unwrap(),
        );
    }

    fn close(&mut self) {
        if !self.is_server {
            let id = self.connecting_connection_id;
            self.disconnect(&id);
        } else {
            self.events_send
                .send(NetworkLogicEvent::Disconnect(self.connecting_connection_id))
                .unwrap();
        }

        let mut run_thread: Option<std::thread::JoinHandle<()>> = None;
        std::mem::swap(&mut run_thread, &mut self.run_thread);
        if run_thread.unwrap().join().is_err() {
            // TODO logging
        }
        self.endpoint.close(VarInt::default(), &[]);
    }

    fn disconnect(&mut self, connection_id: &NetworkConnectionId) {
        self.events_send
            .send(NetworkLogicEvent::Disconnect(*connection_id))
            .unwrap();
    }

    fn connect(&mut self, connect_addr: &str) {
        self.events_send
            .send(NetworkLogicEvent::Connect(
                self.connecting_connection_id,
                connect_addr.to_string(),
            ))
            .unwrap();
    }

    pub fn kick(&self, connection_id: &NetworkConnectionId) {
        self.events_send
            .send(NetworkLogicEvent::Kick(*connection_id))
            .unwrap();
    }

    fn send_to_impl<T>(
        &self,
        msg: &T,
        connection_id: &NetworkConnectionId,
        send_type: NetworkEventSendType,
    ) where
        T: Serialize,
    {
        let mut packet = self.packet_pool.new();
        bincode::serde::encode_into_std_write(msg, packet.deref_mut(), bincode::config::standard())
            .unwrap();
        self.events_send
            .send(NetworkLogicEvent::Send((*connection_id, packet, send_type)))
            .unwrap();
    }

    /// Tries to send as unrealible first, if unsupported
    /// or packet too big for a single packet, falls back
    /// to reliable.
    pub fn send_unordered_auto_to<T>(&self, msg: &T, connection_id: &NetworkConnectionId)
    where
        T: Serialize,
    {
        self.send_to_impl(msg, connection_id, NetworkEventSendType::UnorderedAuto);
    }

    pub fn send_unordered_to<T>(&self, msg: &T, connection_id: &NetworkConnectionId)
    where
        T: Serialize,
    {
        self.send_to_impl(msg, connection_id, NetworkEventSendType::ReliableUnordered);
    }

    pub fn send_in_order_to<T>(
        &self,
        msg: &T,
        connection_id: &NetworkConnectionId,
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

    pub fn send_unreliable_to<T>(&self, msg: &T, connection_id: &NetworkConnectionId)
    where
        T: Serialize,
    {
        self.send_to_impl(
            msg,
            connection_id,
            NetworkEventSendType::UnreliableUnordered,
        );
    }

    /// Tries to send as unrealible first, if unsupported
    /// or packet too big for a single packet, falls back
    /// to reliable.
    pub fn send_unordered_auto_to_server<T>(&self, msg: &T)
    where
        T: Serialize,
    {
        self.send_unordered_auto_to(msg, &self.connecting_connection_id.clone());
    }

    /// Only use this if `connect` was used
    pub fn send_unordered_to_server<T>(&self, msg: &T)
    where
        T: Serialize,
    {
        self.send_unordered_to(msg, &self.connecting_connection_id.clone());
    }

    /// Only use this if `connect` was used
    pub fn send_in_order_to_server<T>(&self, msg: &T, channel: NetworkInOrderChannel)
    where
        T: Serialize,
    {
        self.send_in_order_to(msg, &self.connecting_connection_id.clone(), channel);
    }

    /// Only use this if `connect` was used
    pub fn send_unreliable_to_server<T>(&self, msg: &T)
    where
        T: Serialize,
    {
        self.send_unreliable_to(msg, &self.connecting_connection_id.clone());
    }
}

impl<E, C, Z, I> Drop for Network<E, C, Z, I>
where
    C: NetworkConnectionInterface,
    Z: NetworkConnectingInterface<C>,
    I: NetworkIncomingInterface<Z>,
    E: NetworkEndpointInterface<Z, I>,
{
    fn drop(&mut self) {
        self.close()
    }
}
