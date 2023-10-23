use std::{
    collections::HashMap,
    net::SocketAddr,
    num::NonZeroUsize,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use base::system::{System, SystemTimeInterface};
use base_fs::filesys::FileSystem;
use base_io::io::IO;
use base_log::log::{LogLevel, SystemLogGroup, SystemLogInterface};
use config::config::Config;
use hashlink::{LinkedHashMap, LinkedHashSet};
use network::network::{
    network::{
        Network, NetworkConnectionID, NetworkGameEvent, NetworkInOrderChannel,
        NetworkServerInitOptions,
    },
    quinn_network::QuinnNetwork,
};
use pool::{datatypes::PoolLinkedHashSet, mt_pool::Pool as MtPool, pool::Pool};
use rcgen::Certificate;

use crate::server_game::ServerGame;

use shared_base::{
    game_types::{is_next_tick, time_until_tick, TGameElementID},
    network::{
        messages::{
            MsgClChatMsg, MsgObjPlayerInfo, MsgSvChatMsg, MsgSvPlayerInfo, MsgSvServerInfo,
            MsgSvSystemMsg, NetworkStr,
        },
        types::chat::{NetChatMsg, NetChatMsgPlayerChannel, NetMsgSystem},
    },
};

use shared_game::{snapshot::snapshot::SnapshotClientInfo, state::state::GameStateInterface};

use shared_network::{
    game_event_generator::{GameEventGenerator, GameEvents},
    messages::{
        ClientToServerMessage, ClientToServerPlayerMessage, GameMessage, ServerToClientMessage,
    },
};

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
    player_ids: PoolLinkedHashSet<TGameElementID>,
    connect_timestamp: Duration,
}

impl ServerClient {
    pub fn new(
        connect_timestamp: &Duration,
        pool: &mut Pool<LinkedHashSet<TGameElementID>>,
    ) -> Self {
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
    pub player_count_of_all_clients: usize,

    max_players: usize,

    network: QuinnNetwork,

    is_open: Arc<AtomicBool>,

    has_new_events_server: Arc<AtomicBool>,
    game_event_generator_server: Arc<GameEventGenerator>,

    game_server: ServerGame,

    config: Config,
    thread_pool: Arc<rayon::ThreadPool>,
    io: IO,

    sys: System,
    logger: SystemLogGroup,

    last_tick_time: Duration,

    // pools
    player_ids_pool: Pool<LinkedHashSet<TGameElementID>>,
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
        io: IO,
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
            NetworkServerInitOptions::new()
                .with_max_thread_count(6)
                .with_timeout(config.net.timeout),
        );
        *shared_info.sock_addr.lock().unwrap() = Some(sock_addr);

        Self {
            network_queued_clients: Default::default(),
            network_clients: HashMap::new(),
            clients: HashMap::new(),
            player_count_of_all_clients: 0,

            max_players: config.sv.max_players as usize,

            network: network_server,

            is_open,

            has_new_events_server,
            game_event_generator_server,

            game_server: ServerGame::new(&sys, &config.sv.map, &thread_pool, &io),

            thread_pool,
            io,

            logger: sys.log.logger("server"),
            sys,

            last_tick_time: Duration::ZERO,

            player_ids_pool: Pool::with_sized(config.sv.max_players as usize, || {
                LinkedHashSet::with_capacity(config.sv.max_players_per_ip as usize)
            }),
            player_infos_pool: MtPool::with_sized(config.sv.max_players as usize, || {
                Vec::with_capacity(config.sv.max_players as usize)
            }),

            config,
        }
    }

    fn can_another_player_connect(&self) -> bool {
        self.player_count_of_all_clients + self.network_clients.len() < self.max_players
    }

    pub fn try_client_connect(&mut self, con_id: &NetworkConnectionID, timestamp: &Duration) {
        // check if the client can be part of the game
        if self.can_another_player_connect() {
            self.network_clients
                .insert(*con_id, ServerNetworkClient::new(timestamp));

            // tell the client about all data required to join the server
            let server_info = MsgSvServerInfo {
                map: NetworkStr::from(&self.game_server.map.raw.name).unwrap(),
                hint_start_camera_pos: self.game_server.game.get_client_camera_start_pos(),
                game_type: NetworkStr::from("idm").unwrap(),
            };
            self.network.send_unordered_to(
                &GameMessage::ServerToClient(ServerToClientMessage::ServerInfo(server_info)),
                con_id,
            );

            self.player_count_of_all_clients += 1;
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
        let mut iter = self.network_queued_clients.iter_at_key(con_id).unwrap();
        iter.next();

        iter.enumerate().for_each(|(index, (net_id, _))| {
            self.network.send_unordered_to(
                &GameMessage::ServerToClient(ServerToClientMessage::QueueInfo(format!(
                    "The server is full.\nYou are queued at position: #{}",
                    index
                ))),
                net_id,
            );
        });
        self.network_queued_clients.remove(con_id);
    }

    pub fn client_disconnect(
        &mut self,
        con_id: &NetworkConnectionID,
        _reason: &str,
    ) -> Option<PoolLinkedHashSet<TGameElementID>> {
        // find client in queued clients
        if self.network_queued_clients.contains_key(con_id) {
            self.drop_client_from_queue(con_id);
            return None;
        }

        // else find in waiting clients, connect the waiting client
        let found = self.network_clients.remove(con_id);
        if found.is_some() {
            self.player_count_of_all_clients -= 1;
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
            return None;
        }

        // else find in clients, connect one from queue if this client disconnected
        let found = self.clients.remove(con_id);
        if let Some(p) = found {
            self.player_count_of_all_clients -= p.player_ids.len();
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
                true
            }
            None => false,
        }
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

    fn broadcast_in_order(&self, packet: GameMessage) {
        self.clients.keys().for_each(|send_con_id| {
            self.network
                .send_in_order_to(&packet, &send_con_id, NetworkInOrderChannel::Global);
        });
    }

    fn add_player_for_client(
        &mut self,
        con_id: &NetworkConnectionID,
        player_info: &MsgObjPlayerInfo,
        is_additional_player: bool,
    ) {
        if let Some(client) = self.clients.get_mut(con_id) {
            let player_id = self.game_server.player_join(con_id, player_info);
            client.player_ids.insert(player_id.clone());
            if is_additional_player {
                self.player_count_of_all_clients += 1;
            }

            // if this is the first connect to the server, send a snapshot
            if client.player_ids.len() == 1 {
                let mut client_player_ids_dummy = self.player_ids_pool.new();
                client_player_ids_dummy.insert(player_id);
                let snap_client = SnapshotClientInfo {
                    client_player_ids: client_player_ids_dummy,
                    snap_everything: false,
                    snap_other_stages: false,
                    time_since_connect_nanos: (self.sys.time_get_nanoseconds()
                        - client.connect_timestamp)
                        .as_nanos() as u64,
                };
                self.send_player_infos(con_id);
                let snap = self.game_server.game.build_for(snap_client);
                self.network.send_unordered_to(
                    &GameMessage::ServerToClient(ServerToClientMessage::Snapshot {
                        overhead_time: (self.sys.time_get_nanoseconds() - self.last_tick_time),
                        snapshot: snap,
                    }),
                    con_id,
                );
            }

            self.broadcast_in_order(GameMessage::ServerToClient(ServerToClientMessage::System(
                MsgSvSystemMsg {
                    msg: NetMsgSystem {
                        msg: player_info.name.as_str().to_string() + " has joined the game. TODO",
                    },
                },
            )));
        }
    }

    fn handle_player_msg(
        &mut self,
        con_id: &NetworkConnectionID,
        player_id: &TGameElementID,
        player_msg: &ClientToServerPlayerMessage,
    ) {
        let player = self.clients.get_mut(con_id);
        if let Some(player) = player {
            if player.player_ids.contains(player_id) {
                match player_msg {
                    ClientToServerPlayerMessage::Input(inp) => {
                        self.game_server
                            .player_inp(player_id, &inp.inp, inp.version);
                    }
                    ClientToServerPlayerMessage::RemLocalPlayer => {
                        if player.player_ids.len() > 1 && player.player_ids.remove(player_id) {
                            self.game_server.try_player_drop(player_id);

                            self.broadcast_in_order(GameMessage::ServerToClient(
                                ServerToClientMessage::System(MsgSvSystemMsg {
                                    msg: NetMsgSystem {
                                        msg: "TODO: TODO has disconnected from the game. TODO"
                                            .to_string(),
                                    },
                                }),
                            ));
                        }
                    }
                    ClientToServerPlayerMessage::Chat(msg) => match msg {
                        MsgClChatMsg::Global { msg } => {
                            self.broadcast_in_order(GameMessage::ServerToClient(
                                ServerToClientMessage::Chat(MsgSvChatMsg {
                                    msg: NetChatMsg {
                                        player_id: player_id.clone(),
                                        msg: msg.as_str().to_string(),
                                        channel: NetChatMsgPlayerChannel::Global,
                                    },
                                }),
                            ));
                        }
                        MsgClChatMsg::GameTeam { .. } => todo!(),
                        MsgClChatMsg::Whisper { .. } => todo!(),
                    },
                }
            }
        }
    }

    fn handle_msg(&mut self, con_id: &NetworkConnectionID, game_msg: &GameMessage) {
        match game_msg {
            GameMessage::ClientToServer(client_to_server_msg) => {
                match client_to_server_msg {
                    ClientToServerMessage::Ready(player_info) => {
                        // if client is actually waiting, make it part of the game
                        let was_client_readied = self.try_client_ready(con_id);
                        if was_client_readied {
                            self.add_player_for_client(con_id, &player_info.player_info, false);
                        }
                    }
                    ClientToServerMessage::AddLocalPlayer(player_info) => {
                        if self.can_another_player_connect() {
                            self.add_player_for_client(con_id, &player_info.player_info, true);
                        }
                    }
                    ClientToServerMessage::PlayerMsg((player_index, player_msg)) => {
                        self.handle_player_msg(con_id, player_index, player_msg);
                    }
                }
            }
            _ => {
                // ignore any other packet
            }
        }
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
                        GameEvents::NetworkMsg(game_msg) => self.handle_msg(con_id, game_msg),
                    }
                }
                events.clear();
                game_ev_gen
                    .has_events
                    .store(false, std::sync::atomic::Ordering::Relaxed);
            }

            let ticks_in_a_second = self.game_server.game.game_tick_speed();

            // get time before checking ticks
            cur_time = self.sys.time_get_nanoseconds();
            while is_next_tick(cur_time, &mut self.last_tick_time, ticks_in_a_second) {
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
                        &GameMessage::ServerToClient(ServerToClientMessage::Snapshot {
                            overhead_time: (self.sys.time_get_nanoseconds() - self.last_tick_time),
                            snapshot: snap,
                        }),
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
                ticks_in_a_second,
            ) {
                std::thread::yield_now();
            } else {
                let next_tick_time =
                    time_until_tick(ticks_in_a_second) - (cur_time - self.last_tick_time);

                //let mut guard = self.game_event_generator_server.blocking_lock();
                //guard = guard.ev_cond.wait_timeout(guard.into(), next_tick_time);
                std::thread::sleep(next_tick_time);
            }
        }

        self.network.close();
    }

    fn reload(&mut self) {
        // reload the whole game server, including the map
        self.game_server =
            ServerGame::new(&self.sys, &self.config.sv.map, &self.thread_pool, &self.io);
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
            hint_start_camera_pos: self.game_server.game.get_client_camera_start_pos(),
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

    let io = IO::new(Arc::new(FileSystem::new(&sys.log, "org", "", "DDNet")));

    let config = config_fs::load(&io);

    let mut server = Server::new(
        sys,
        is_open,
        cert,
        shared_info,
        if IS_INTERNAL_SERVER {
            config.sv.port_internal
        } else {
            config.sv.port
        },
        config,
        thread_pool,
        io,
    );

    server.run();
}
