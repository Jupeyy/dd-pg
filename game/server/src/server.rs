use std::{
    collections::HashMap,
    net::SocketAddr,
    num::NonZeroUsize,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::Duration,
};

use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use config::config::Config;
use hashlink::LinkedHashMap;
use network::network::{
    network::{Network, NetworkConnectionID, NetworkGameEvent, NetworkServerInitOptions},
    quinn_network::QuinnNetwork,
};
use pool::{datatypes::PoolVec, mt_pool::Pool as MtPool, pool::Pool};
use rcgen::Certificate;

use crate::server_game::ServerGame;

use shared_base::{
    game_types::{is_next_tick, TGameElementID, TIME_UNTIL_TICK},
    network::messages::{MsgSvPlayerInfo, MsgSvServerInfo, NetworkStr},
};

use shared_game::{snapshot::snapshot::SnapshotClientInfo, state::state::GameStateInterface};

use shared_network::{
    game_event_generator::{GameEventGenerator, GameEvents},
    messages::{ClientToServerMessage, GameMessage, ServerToClientMessage},
};

use base::system::{LogLevel, System, SystemLogGroup, SystemLogInterface, SystemTimeInterface};

/**
 * A network queued client is a client that isn't actually part of the game,
 * but e.g. waiting for a slot.
 */
pub struct ServerNetworkQueuedClient {
    connect_timestamp: Duration,
}

impl ServerNetworkQueuedClient {
    pub fn new(connect_timestamp: &Duration) -> Self {
        Self {
            connect_timestamp: *connect_timestamp,
        }
    }
}

/**
 * A network client is a client that will be part of the game, but is not yet ready,
 * e.g. downloading the map etc.
 */
pub struct ServerNetworkClient {
    connect_timestamp: Duration,
}

impl ServerNetworkClient {
    pub fn new(connect_timestamp: &Duration) -> Self {
        Self {
            connect_timestamp: *connect_timestamp,
        }
    }
}

/**
 * A server client is a client that is part of the game.
 */
pub struct ServerClient {
    player_ids: PoolVec<TGameElementID>,
    connect_timestamp: Duration,
}

impl ServerClient {
    pub fn new(connect_timestamp: &Duration, pool: &mut Pool<Vec<TGameElementID>>) -> Self {
        Self {
            player_ids: pool.new(),
            connect_timestamp: *connect_timestamp,
        }
    }
}

pub struct ServerInfo {
    pub sock_addr: std::sync::Mutex<Option<SocketAddr>>,
}

pub struct Server {
    pub network_queued_clients: LinkedHashMap<NetworkConnectionID, ServerNetworkQueuedClient>,
    pub network_clients: HashMap<NetworkConnectionID, ServerNetworkClient>,
    pub clients: HashMap<NetworkConnectionID, ServerClient>,
    pub player_count: usize,

    max_players: usize,

    network: QuinnNetwork,

    is_open: Arc<AtomicBool>,

    has_new_events_server: Arc<AtomicBool>,
    game_event_generator_server: Arc<GameEventGenerator>,

    game_server: ServerGame,

    config: Config,
    thread_pool: Arc<rayon::ThreadPool>,
    fs: Arc<FileSystem>,
    io_batcher: Arc<Mutex<TokIOBatcher>>,

    sys: System,
    logger: SystemLogGroup,

    last_tick_time: Duration,

    // pools
    player_ids_pool: Pool<Vec<TGameElementID>>,
    player_infos_pool: MtPool<Vec<MsgSvPlayerInfo>>,
}

impl Server {
    pub fn new(
        sys: System,
        is_open: Arc<AtomicBool>,
        cert: &Certificate,
        shared_info: Arc<ServerInfo>,
        port: u16,
        config: Config,
        thread_pool: Arc<rayon::ThreadPool>,
        fs: Arc<FileSystem>,
        io_batcher: Arc<Mutex<TokIOBatcher>>,
    ) -> Self {
        let has_new_events_server = Arc::new(AtomicBool::new(false));
        let game_event_generator_server = Arc::new(GameEventGenerator::new(
            has_new_events_server.clone(),
            sys.time.clone(),
        ));

        let (network_server, _cert, sock_addr, _notifer_server) = Network::init_server(
            &("0.0.0.0:".to_string() + &port.to_string()),
            game_event_generator_server.clone(),
            cert,
            &sys,
            Some(
                NetworkServerInitOptions::new()
                    .with_max_thread_count(6)
                    .with_timeout(config.net_timeout),
            ),
        );
        *shared_info.sock_addr.lock().unwrap() = Some(sock_addr);

        Self {
            network_queued_clients: Default::default(),
            network_clients: HashMap::new(),
            clients: HashMap::new(),
            player_count: 0,

            max_players: config.sv_max_players,

            network: network_server,

            is_open,

            has_new_events_server,
            game_event_generator_server,

            game_server: ServerGame::new(&sys.time, &config.sv_map, &thread_pool, &fs, &io_batcher),

            thread_pool,
            fs,
            io_batcher,

            logger: sys.log.logger("server"),
            sys,

            last_tick_time: Duration::ZERO,

            player_ids_pool: Pool::with_sized(config.sv_max_players, || {
                Vec::with_capacity(config.sv_max_players_per_ip)
            }),
            player_infos_pool: MtPool::with_sized(config.sv_max_players, || {
                Vec::with_capacity(config.sv_max_players)
            }),

            config,
        }
    }

    pub fn try_client_connect(&mut self, con_id: &NetworkConnectionID, timestamp: &Duration) {
        // check if the client can be part of the game
        if self.player_count + self.network_clients.len() < self.max_players {
            self.network_clients
                .insert(*con_id, ServerNetworkClient::new(timestamp));

            // tell the client about all data required to join the server
            let server_info = MsgSvServerInfo {
                map: NetworkStr::from(&self.game_server.map.raw.name).unwrap(),
                game_type: NetworkStr::from("idm").unwrap(),
            };
            self.network.send_unordered_to(
                &GameMessage::ServerToClient(ServerToClientMessage::ServerInfo(server_info)),
                con_id,
            );

            self.player_count += 1;
        } else {
            // else add it to the network queue and inform it about that
            self.network_queued_clients
                .insert(con_id.clone(), ServerNetworkQueuedClient::new(timestamp));

            self.network.send_unordered_to(
                &GameMessage::ServerToClient(ServerToClientMessage::QueueInfo(format!(
                    "The server is full.\nYou are queued at position: #{}",
                    self.network_queued_clients.len()
                ))),
                con_id,
            );
        }
    }

    fn drop_client_from_queue(&mut self, con_id: &NetworkConnectionID) {
        let mut found = false;
        // TODO: implement an iterator that skips to a specific key
        self.network_queued_clients
            .keys()
            .enumerate()
            .for_each(|(index, net_id)| {
                if *net_id == *con_id {
                    found = true;
                } else if found {
                    self.network.send_unordered_to(
                        &GameMessage::ServerToClient(ServerToClientMessage::QueueInfo(format!(
                            "The server is full.\nYou are queued at position: #{}",
                            index
                        ))),
                        net_id,
                    );
                }
            });
        self.network_queued_clients.remove(con_id);
    }

    pub fn client_disconnect(
        &mut self,
        con_id: &NetworkConnectionID,
        _reason: &str,
    ) -> Option<PoolVec<TGameElementID>> {
        // find client in queued clients
        if self.network_queued_clients.contains_key(con_id) {
            self.drop_client_from_queue(con_id);
            return None;
        }

        // else find in waiting clients
        let found = self.network_clients.remove(con_id);
        if found.is_some() {
            if !self.network_queued_clients.is_empty() {
                let con_id_queue = *self.network_queued_clients.front().unwrap().0;
                let timestamp_queue = self
                    .network_queued_clients
                    .front()
                    .unwrap()
                    .1
                    .connect_timestamp;
                self.drop_client_from_queue(&con_id_queue);
                self.try_client_connect(&con_id_queue, &timestamp_queue);
            }
            self.player_count -= 1;
            return None;
        }

        // else find in clients
        let found = self.clients.remove(con_id);
        if let Some(p) = found {
            for _ in 0..p.player_ids.len() {
                if !self.network_queued_clients.is_empty() {
                    let con_id_queue = *self.network_queued_clients.front().unwrap().0;
                    let timestamp_queue = self
                        .network_queued_clients
                        .front()
                        .unwrap()
                        .1
                        .connect_timestamp;
                    self.drop_client_from_queue(&con_id_queue);
                    self.try_client_connect(&con_id_queue, &timestamp_queue);
                }
            }
            self.player_count -= p.player_ids.len();
            return Some(p.player_ids);
        }
        return None;
    }

    pub fn try_client_ready(&mut self, con_id: &NetworkConnectionID) -> bool {
        // check if the client can be part of the game
        let found = self.network_clients.remove(con_id);
        match found {
            Some(net_client) => {
                self.logger.log(LogLevel::Info).msg("client ready");
                self.clients.insert(
                    *con_id,
                    ServerClient::new(
                        &net_client.connect_timestamp.clone(),
                        &mut self.player_ids_pool,
                    ),
                );
                return true;
            }
            None => {}
        }
        false
    }

    pub fn send_player_infos(&mut self, connection_id: &NetworkConnectionID) {
        let mut player_infos = self.player_infos_pool.new();
        self.game_server
            .game
            .get_player_and_no_char_player_infos(&mut player_infos);
        self.network.send_unordered_to(
            &GameMessage::ServerToClient(ServerToClientMessage::PlayerInfos(player_infos)),
            connection_id,
        );
    }

    pub fn run(&mut self) {
        let mut cur_time = self.sys.time_get_nanoseconds();
        let _last_inp_time = cur_time;

        let game_event_generator = self.game_event_generator_server.clone();
        while self.is_open.load(std::sync::atomic::Ordering::Relaxed) {
            if self
                .has_new_events_server
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                let game_ev_gen = &game_event_generator;
                let mut events = game_ev_gen.events.blocking_lock();
                for (con_id, timestamp_nanos, event) in &*events {
                    match event {
                        GameEvents::NetworkEvent(net_ev) => match net_ev {
                            NetworkGameEvent::Connected => {
                                self.logger
                                    .log(LogLevel::Debug)
                                    .msg("connect time sv: ")
                                    .msg_var(&timestamp_nanos.as_nanos());
                                self.try_client_connect(con_id, timestamp_nanos);
                            }
                            NetworkGameEvent::Disconnected(reason) => {
                                self.logger
                                    .log(LogLevel::Debug)
                                    .msg("got connected event from network");
                                if let Some(player_ids) =
                                    self.client_disconnect(con_id, reason.as_str())
                                {
                                    for player_id in player_ids.iter() {
                                        self.game_server.try_player_drop(player_id);
                                    }
                                }
                            }
                            NetworkGameEvent::NetworkStats(stats) => {
                                self.logger
                                    .log(LogLevel::Verbose)
                                    .msg("server ping: ")
                                    .msg_var(&stats.ping.as_millis());
                            }
                            NetworkGameEvent::ConnectingFailed(_) => {
                                // server usually does not connect, so does not care
                            }
                        },
                        GameEvents::NetworkMsg(game_msg) => {
                            match game_msg {
                                GameMessage::ClientToServer(client_to_server_msg) => {
                                    match client_to_server_msg {
                                        ClientToServerMessage::Ready(player_info) => {
                                            // if client is actually waiting, make it part of the game
                                            let was_client_readied = self.try_client_ready(con_id);
                                            if was_client_readied {
                                                let player_id = self
                                                    .game_server
                                                    .player_join(con_id, &player_info.player_info);
                                                let client = self.clients.get_mut(con_id).unwrap();
                                                client.player_ids.push(player_id.clone());

                                                // if this is the first connect to the server, send a snapshot
                                                if client.player_ids.len() == 1 {
                                                    let mut client_player_ids_dummy =
                                                        self.player_ids_pool.new();
                                                    client_player_ids_dummy.push(player_id);
                                                    let snap_client = SnapshotClientInfo {
                                                        client_player_ids: client_player_ids_dummy,
                                                        snap_everything: false,
                                                        snap_other_stages: false,
                                                        time_since_connect_nanos: (self
                                                            .sys
                                                            .time_get_nanoseconds()
                                                            - client.connect_timestamp)
                                                            .as_nanos()
                                                            as u64,
                                                    };
                                                    self.send_player_infos(con_id);
                                                    let snap = self
                                                        .game_server
                                                        .game
                                                        .build_for(snap_client);
                                                    self.network.send_unordered_to(
                                                        &GameMessage::ServerToClient(
                                                            ServerToClientMessage::Snapshot((
                                                                (self.sys.time_get_nanoseconds()
                                                                    - self.last_tick_time),
                                                                snap,
                                                            )),
                                                        ),
                                                        con_id,
                                                    );
                                                }
                                            }
                                        }
                                        ClientToServerMessage::Input(inp) => {
                                            let player = self.clients.get_mut(con_id);
                                            if let Some(player) = player {
                                                if (inp.index as usize) < player.player_ids.len() {
                                                    self.game_server.player_inp(
                                                        &player.player_ids[inp.index as usize],
                                                        &inp.inp,
                                                        inp.version,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    // ignore any other packet
                                }
                            }
                        }
                    }
                }
                events.clear();
                game_ev_gen
                    .has_events
                    .store(false, std::sync::atomic::Ordering::Relaxed);
            }

            // get time before checking ticks
            cur_time = self.sys.time_get_nanoseconds();
            while is_next_tick(cur_time, &mut self.last_tick_time) {
                // game ticks
                self.game_server.game.tick();

                // snap shot building
                for (con_id, client) in &self.clients {
                    let mut player_ids = self.player_ids_pool.new();
                    player_ids.clone_from(&client.player_ids);
                    let snap_client = SnapshotClientInfo {
                        client_player_ids: player_ids,
                        snap_everything: false,
                        snap_other_stages: false,
                        time_since_connect_nanos: (self.sys.time_get_nanoseconds()
                            - client.connect_timestamp)
                            .as_nanos() as u64,
                    };
                    let snap = self.game_server.game.build_for(snap_client);
                    self.network.send_unordered_to(
                        &GameMessage::ServerToClient(ServerToClientMessage::Snapshot((
                            (self.sys.time_get_nanoseconds() - self.last_tick_time),
                            snap,
                        ))),
                        &con_id,
                    );
                }
            }

            // after tick checks
            // if the game should reload, reload all game related stuff
            // send the client a load event, which is used for map reloads etc.
            if self.game_server.game.should_reload() {
                self.reload();
            }

            // time and sleeps
            cur_time = self.sys.time_get_nanoseconds();

            if is_next_tick(
                cur_time,
                &mut self.last_tick_time.clone(), /* <-- dummy */
            ) {
                std::thread::yield_now();
            } else {
                let next_tick_time = TIME_UNTIL_TICK - (cur_time - self.last_tick_time);

                //let mut guard = self.game_event_generator_server.blocking_lock();
                //guard = guard.ev_cond.wait_timeout(guard.into(), next_tick_time);
                std::thread::sleep(next_tick_time);
            }
        }

        self.network.close();
    }

    fn reload(&mut self) {
        // reload the whole game server, including the map
        self.game_server = ServerGame::new(
            &self.sys.time,
            &self.config.sv_map,
            &self.thread_pool,
            &self.fs,
            &self.io_batcher,
        );
        // put all players back to a loading state
        self.clients.drain().for_each(|(net_id, client)| {
            self.network_clients.insert(
                net_id,
                ServerNetworkClient {
                    connect_timestamp: client.connect_timestamp,
                },
            );
        });
        let server_info = MsgSvServerInfo {
            map: NetworkStr::from(&self.game_server.map.raw.name).unwrap(),
            game_type: NetworkStr::from("idm").unwrap(),
        };
        self.network_clients.keys().for_each(|net_id| {
            self.network.send_unordered_to(
                &GameMessage::ServerToClient(ServerToClientMessage::Load(server_info.clone())),
                net_id,
            );
        });
        self.last_tick_time = self.sys.time_get_nanoseconds();
    }
}

pub fn ddnet_server_main<const IS_INTERNAL_SERVER: bool>(
    sys: System,
    cert: &Certificate,
    is_open: Arc<AtomicBool>,
    shared_info: Arc<ServerInfo>,
) {
    let thread_pool = Arc::new(
        rayon::ThreadPoolBuilder::new()
            .num_threads(
                std::thread::available_parallelism()
                    .unwrap_or(NonZeroUsize::new(2).unwrap())
                    .get()
                    .max(4)
                    - 2,
            )
            .build()
            .unwrap(),
    );

    let fs: Arc<FileSystem> = Arc::new(FileSystem::new(&sys.log));

    // tokio runtime for client side tasks
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2) // should be at least 2
        .max_blocking_threads(2) // must be at least 2
        .build()
        .unwrap();

    let io_batcher = Arc::new(std::sync::Mutex::new(TokIOBatcher::new(rt, &sys.log)));

    let config = config_fs::load(&fs, &io_batcher);

    let mut server = Server::new(
        sys,
        is_open,
        cert,
        shared_info,
        if IS_INTERNAL_SERVER {
            config.sv_port_internal
        } else {
            config.sv_port
        },
        config,
        thread_pool,
        fs,
        io_batcher,
    );

    server.run();
}
