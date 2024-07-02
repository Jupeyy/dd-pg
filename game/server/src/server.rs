use std::{
    collections::{BTreeMap, HashMap},
    fmt::Debug,
    num::NonZeroUsize,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use base::system::{System, SystemTimeInterface};
use base_fs::filesys::FileSystem;
use base_http::http::HttpClient;
use base_io::{io::Io, io_batcher::IoBatcherTask};
use base_log::log::{LogLevel, SystemLogGroup, SystemLogInterface};
use config::config::ConfigEngine;
use ed25519_dalek::SigningKey;
use game_config::config::{ConfigDebug, ConfigGame};
use hashlink::{LinkedHashMap, LinkedHashSet};
use network::network::{
    connection::NetworkConnectionID,
    event::NetworkEvent,
    network::{Network, NetworkServerCertMode, NetworkServerInitOptions},
    packet_compressor::DefaultNetworkPacketCompressor,
    plugins::NetworkPlugins,
    quinn_network::QuinnNetwork,
    types::NetworkInOrderChannel,
};
use pool::{datatypes::PoolLinkedHashMap, pool::Pool};
use shared::game::state_wasm_manager::GameStateWasmManager;

use crate::server_game::ServerGame;

use shared_base::{
    game_types::{is_next_tick, time_until_tick},
    network::{
        messages::{MsgClChatMsg, MsgSvChatMsg, MsgSvServerInfo, PlayerInputChainable},
        server_info::{ServerDbgGame, ServerInfo},
        types::chat::{NetChatMsg, NetChatMsgPlayerChannel},
    },
};

use game_interface::{
    client_commands::ClientCommand,
    events::EventClientInfo,
    interface::GameStateInterface,
    types::{
        game::{GameEntityId, GameTickType},
        input::CharacterInput,
        player_info::{PlayerClientInfo, PlayerDropReason},
        snapshot::SnapshotClientInfo,
    },
};

use shared_network::{
    game_event_generator::{GameEventGenerator, GameEvents},
    messages::{
        ClientToServerMessage, ClientToServerPlayerMessage, GameMessage, ServerToClientMessage,
    },
};

pub struct ClientAuth {
    cert: Arc<x509_cert::Certificate>,
}

/// A network queued client is a client that isn't actually part of the game,
/// but e.g. waiting for a slot.
pub struct ServerNetworkQueuedClient {
    connect_timestamp: Duration,
    auth: ClientAuth,
}

impl ServerNetworkQueuedClient {
    pub fn new(connect_timestamp: &Duration, auth: ClientAuth) -> Self {
        Self {
            connect_timestamp: *connect_timestamp,
            auth,
        }
    }
}

/// A network client is a client that will be part of the game, but is not yet ready,
/// e.g. downloading the map etc.
pub struct ServerNetworkClient {
    connect_timestamp: Duration,
    auth: ClientAuth,
}

impl ServerNetworkClient {
    pub fn new(connect_timestamp: &Duration, cert: Arc<x509_cert::Certificate>) -> Self {
        Self {
            connect_timestamp: *connect_timestamp,
            auth: ClientAuth { cert },
        }
    }
}

pub struct ServerClientPlayer {
    /// latest input id the server knows about
    server_input_id: Option<u64>,
}

pub struct ClientSnapshotDiff {
    pub snap_id: u64,
    pub snapshot: Vec<u8>,
}

/// A server client is a client that is part of the game.
pub struct ServerClient {
    players: PoolLinkedHashMap<GameEntityId, ServerClientPlayer>,
    connect_timestamp: Duration,
    snap_id: u64,
    /// latest snap id the client knows about
    client_snap_id: Option<ClientSnapshotDiff>,
    client_snap_storage: BTreeMap<u64, Vec<u8>>,

    auth: ClientAuth,
}

impl ServerClient {
    pub fn new(
        connect_timestamp: &Duration,
        pool: &mut Pool<LinkedHashMap<GameEntityId, ServerClientPlayer>>,
        auth: ClientAuth,
    ) -> Self {
        Self {
            players: pool.new(),
            connect_timestamp: *connect_timestamp,

            snap_id: 0,
            client_snap_id: None,
            client_snap_storage: Default::default(),
            auth,
        }
    }
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

    config_game: ConfigGame,
    thread_pool: Arc<rayon::ThreadPool>,
    io: Io,

    sys: System,
    logger: SystemLogGroup,

    last_tick_time: Duration,
    last_register_time: Duration,
    register_task: Option<IoBatcherTask<()>>,

    shared_info: Arc<ServerInfo>,

    // pools
    players_pool: Pool<LinkedHashMap<GameEntityId, ServerClientPlayer>>,
    player_ids_pool: Pool<LinkedHashSet<GameEntityId>>,
    player_snap_pool: Pool<Vec<u8>>,

    // helpers
    input_deser: Pool<Vec<u8>>,
}

impl Server {
    pub fn new(
        sys: System,
        is_open: Arc<AtomicBool>,
        cert_and_private_key: (x509_cert::Certificate, SigningKey),
        shared_info: Arc<ServerInfo>,
        port: u16,
        config_engine: ConfigEngine,
        config_game: ConfigGame,
        thread_pool: Arc<rayon::ThreadPool>,
        io: Io,
    ) -> anyhow::Result<Self> {
        let has_new_events_server = Arc::new(AtomicBool::new(false));
        let game_event_generator_server = Arc::new(GameEventGenerator::new(
            has_new_events_server.clone(),
            sys.time.clone(),
        ));

        let (network_server, _cert, sock_addr, _notifer_server) = Network::init_server(
            &("0.0.0.0:".to_string() + &port.to_string()),
            game_event_generator_server.clone(),
            NetworkServerCertMode::FromCertAndPrivateKey {
                cert: cert_and_private_key.0,
                private_key: cert_and_private_key.1,
            },
            &sys,
            NetworkServerInitOptions::new()
                .with_max_thread_count(if shared_info.is_internal_server { 2 } else { 6 })
                .with_timeout(config_engine.net.timeout)
                .with_disable_retry_on_connect(
                    config_engine.net.disable_retry_on_connect || shared_info.is_internal_server,
                ),
            NetworkPlugins {
                packet_plugins: Arc::new(vec![Arc::new(DefaultNetworkPacketCompressor::new())]),
                connection_plugins: Default::default(),
            },
        );
        *shared_info.sock_addr.lock().unwrap() = Some(sock_addr);

        Ok(Self {
            network_queued_clients: Default::default(),
            network_clients: HashMap::new(),
            clients: HashMap::new(),
            player_count_of_all_clients: 0,

            max_players: config_game.sv.max_players as usize,

            network: network_server,

            is_open,

            has_new_events_server,
            game_event_generator_server,

            game_server: ServerGame::new(
                &config_game.sv.map,
                &config_game.sv.game_mod,
                &config_game.sv.game_type,
                &thread_pool,
                &io,
            )?,

            thread_pool,
            io,

            logger: sys.log.logger("server"),

            last_tick_time: sys.time_get_nanoseconds(),
            last_register_time: sys.time_get_nanoseconds(),
            register_task: None,

            sys,

            shared_info,

            players_pool: Pool::with_sized(config_game.sv.max_players as usize, || {
                LinkedHashMap::with_capacity(config_game.sv.max_players_per_ip as usize)
            }),
            player_ids_pool: Pool::with_sized(config_game.sv.max_players as usize, || {
                LinkedHashSet::with_capacity(config_game.sv.max_players_per_ip as usize)
            }),
            player_snap_pool: Pool::with_capacity(2),

            // helpers
            input_deser: Pool::with_capacity(3),

            config_game,
        })
    }

    fn can_another_player_connect(&self) -> bool {
        self.player_count_of_all_clients + self.network_clients.len() < self.max_players
    }

    pub fn try_client_connect(
        &mut self,
        con_id: &NetworkConnectionID,
        timestamp: &Duration,
        cert: Arc<x509_cert::Certificate>,
    ) {
        // check if the client can be part of the game
        if self.can_another_player_connect() {
            self.network_clients
                .insert(*con_id, ServerNetworkClient::new(timestamp, cert));

            // tell the client about all data required to join the server
            let server_info = MsgSvServerInfo {
                map: self.game_server.map.name.as_str().try_into().unwrap(),
                map_blake3_hash: self.game_server.map_blake3_hash.into(),
                game_mod: self.game_server.game_mod_name.as_str().try_into().unwrap(),
                game_mod_blake3_hash: self.game_server.game_mod_blake3_hash.clone(),
                resource_server_fallback: self
                    .game_server
                    .http_server
                    .as_ref()
                    .map(|server| server.port),
                hint_start_camera_pos: self.game_server.game.get_client_camera_join_pos(),
                game_type: self.game_server.game_type.as_str().try_into().unwrap(),
            };
            self.network.send_unordered_to(
                &GameMessage::ServerToClient(ServerToClientMessage::ServerInfo {
                    info: server_info,
                }),
                con_id,
            );

            self.player_count_of_all_clients += 1;
        } else {
            // else add it to the network queue and inform it about that
            self.network_queued_clients.insert(
                con_id.clone(),
                ServerNetworkQueuedClient::new(timestamp, ClientAuth { cert }),
            );

            self.network.send_unordered_to(
                &GameMessage::ServerToClient(ServerToClientMessage::QueueInfo(format!(
                    "The server is full.\nYou are queued at position: #{}",
                    self.network_queued_clients.len()
                ))),
                con_id,
            );
        }
    }

    fn drop_client_from_queue(
        &mut self,
        con_id: &NetworkConnectionID,
    ) -> Option<ServerNetworkQueuedClient> {
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
        self.network_queued_clients.remove(con_id)
    }

    pub fn client_disconnect(
        &mut self,
        con_id: &NetworkConnectionID,
        _reason: &str,
    ) -> Option<PoolLinkedHashMap<GameEntityId, ServerClientPlayer>> {
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
                let p = self.drop_client_from_queue(&con_id_queue).unwrap();
                self.try_client_connect(&con_id_queue, &timestamp_queue, p.auth.cert);
            }
            return None;
        }

        // else find in clients, connect one from queue if this client disconnected
        let found = self.clients.remove(con_id);
        if let Some(p) = found {
            self.player_count_of_all_clients -= p.players.len();
            for _ in 0..p.players.len() {
                if !self.network_queued_clients.is_empty() {
                    let con_id_queue = *self.network_queued_clients.front().unwrap().0;
                    let timestamp_queue = self
                        .network_queued_clients
                        .front()
                        .unwrap()
                        .1
                        .connect_timestamp;
                    let drop_player = self.drop_client_from_queue(&con_id_queue).unwrap();
                    self.try_client_connect(&con_id_queue, &timestamp_queue, drop_player.auth.cert);
                }
            }
            return Some(p.players);
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
                        &mut self.players_pool,
                        net_client.auth,
                    ),
                );
                true
            }
            None => false,
        }
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
        mut player_info: PlayerClientInfo,
        is_additional_player: bool,
    ) {
        if let Some(client) = self.clients.get_mut(con_id) {
            player_info.player_index = client.players.len();
            let player_id = self.game_server.player_join(con_id, &player_info);
            client.players.insert(
                player_id.clone(),
                ServerClientPlayer {
                    server_input_id: None,
                },
            );
            if is_additional_player {
                self.player_count_of_all_clients += 1;
            }

            // if this is the first connect to the server, send a snapshot
            if client.players.len() == 1 {
                let mut client_player_ids_dummy = self.player_ids_pool.new();
                client_player_ids_dummy.insert(player_id);
                let snap_client = SnapshotClientInfo {
                    client_player_ids: client_player_ids_dummy,
                    snap_everything: false,
                    snap_other_stages: false,
                };
                let snap_id = client.snap_id;
                client.snap_id += 1;

                let snap = self.game_server.game.snapshot_for(snap_client);

                client.client_snap_storage.insert(snap_id, snap.clone());

                self.network.send_unordered_to(
                    &GameMessage::ServerToClient(ServerToClientMessage::Snapshot {
                        overhead_time: (self.sys.time_get_nanoseconds() - self.last_tick_time),
                        snapshot: snap,
                        diff_id: None,
                        snap_id,
                        game_monotonic_tick: self.game_server.cur_monotonic_tick,
                        as_diff: true,
                    }),
                    con_id,
                );
            }
        }
    }

    fn handle_player_msg(
        &mut self,
        con_id: &NetworkConnectionID,
        player_id: &GameEntityId,
        player_msg: ClientToServerPlayerMessage,
    ) {
        let client = self.clients.get_mut(con_id);
        if let Some(player) = client {
            if player.players.contains_key(player_id) {
                match player_msg {
                    ClientToServerPlayerMessage::RemLocalPlayer => {
                        if player.players.len() > 1 && player.players.remove(player_id).is_some() {
                            self.game_server
                                .player_drop(player_id, PlayerDropReason::Disconnect);
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
                    ClientToServerPlayerMessage::Kill => {
                        self.game_server
                            .game
                            .client_command(player_id, ClientCommand::Kill);
                    }
                }
            }
        }
    }

    fn handle_msg(&mut self, con_id: &NetworkConnectionID, game_msg: GameMessage) {
        match game_msg {
            GameMessage::ClientToServer(client_to_server_msg) => {
                match client_to_server_msg {
                    ClientToServerMessage::Ready(player_info) => {
                        // if client is actually waiting, make it part of the game
                        let was_client_readied = self.try_client_ready(con_id);
                        if was_client_readied {
                            let unique_identifier = if let Some(ClientAuth { cert }) =
                                self.clients.get_mut(con_id).map(|client| &client.auth)
                            {
                                let hash = cert
                                    .tbs_certificate
                                    .subject_public_key_info
                                    .fingerprint_bytes()
                                    .ok();
                                hash
                            } else {
                                None
                            };

                            self.add_player_for_client(
                                con_id,
                                PlayerClientInfo {
                                    info: player_info.player_info,
                                    is_dummy: false,
                                    player_index: 0,
                                    unique_identifier,
                                },
                                false,
                            );
                        }
                    }
                    ClientToServerMessage::AddLocalPlayer(player_info) => {
                        if self.can_another_player_connect() {
                            self.add_player_for_client(
                                con_id,
                                PlayerClientInfo {
                                    info: player_info.player_info,
                                    is_dummy: player_info.as_dummy,
                                    player_index: 0,
                                    unique_identifier: if let Some(ClientAuth { cert }) =
                                        self.clients.get(con_id).map(|con| &con.auth)
                                    {
                                        let hash = cert
                                            .tbs_certificate
                                            .subject_public_key_info
                                            .fingerprint_bytes()
                                            .ok();
                                        hash
                                    } else {
                                        None
                                    },
                                },
                                true,
                            );
                        }
                    }
                    ClientToServerMessage::PlayerMsg((player_id, player_msg)) => {
                        self.handle_player_msg(con_id, &player_id, player_msg);
                    }
                    ClientToServerMessage::Inputs(inp) => {
                        for (player_id, inp_chain) in inp.inputs.iter() {
                            let client = self.clients.get_mut(con_id);
                            if let Some(player) = client {
                                if player.players.contains_key(player_id) {
                                    let def_inp = PlayerInputChainable::default();

                                    let mut def = self.input_deser.new();
                                    let def_len = bincode::serde::encode_into_std_write(
                                        def_inp,
                                        &mut *def,
                                        bincode::config::standard().with_fixed_int_encoding(),
                                    )
                                    .unwrap();
                                    let mut old = def;
                                    let mut offset = 0;

                                    while let Some(patch) =
                                        inp_chain.data.get(offset..offset + def_len)
                                    {
                                        let mut new = self.input_deser.new();
                                        bin_patch::patch_exact_size(&old, patch, &mut new).unwrap();

                                        if let Ok((inp, _)) = bincode::serde::decode_from_slice::<
                                            PlayerInputChainable,
                                            _,
                                        >(
                                            &new,
                                            bincode::config::standard().with_fixed_int_encoding(),
                                        ) {
                                            self.game_server.player_inp(
                                                player_id,
                                                inp.inp.clone(),
                                                inp.for_monotonic_tick,
                                            );
                                        }

                                        offset += def_len;
                                        old = new;
                                    }
                                }
                            }
                        }
                    }
                    ClientToServerMessage::SnapshotAck { snap_id, as_diff } => {
                        if let Some(client) = self.clients.get_mut(con_id) {
                            if let Some(snap) = client.client_snap_storage.remove(&snap_id) {
                                if as_diff {
                                    client.client_snap_id = Some(ClientSnapshotDiff {
                                        snap_id,
                                        snapshot: snap,
                                    });
                                }
                            }
                            while client
                                .client_snap_storage
                                .first_entry()
                                .is_some_and(|entry| *entry.key() < snap_id)
                            {
                                client.client_snap_storage.pop_first();
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

    pub fn dbg_game<'a>(
        config: &ConfigDebug,
        last_tick_time: &Duration,
        game: &mut GameStateWasmManager,
        inputs: Option<impl Iterator<Item = &'a CharacterInput> + Debug>,
        cur_tick: GameTickType,
        ticks_in_a_second: GameTickType,
        shared_info: &Arc<ServerInfo>,
        caller_name: &str,
    ) {
        if config.client_server_sync_log.time
            || config.client_server_sync_log.inputs
            || config.client_server_sync_log.players
        {
            game.pred_tick(PoolLinkedHashMap::new_without_pool());
            let player_infos = game.collect_characters_render_info(0.0);

            let players = format!("{:?}", player_infos);
            let inputs = format!("{:?}", inputs);

            let mut dbg_games = shared_info.dbg_game.lock().unwrap();
            let dbg_game = dbg_games.get(&cur_tick);
            if let Some(dbg_game) = dbg_game {
                let now = std::time::Instant::now();
                if ((*last_tick_time).max(dbg_game.tick_time)
                    - (*last_tick_time).min(dbg_game.tick_time)
                    > Duration::from_millis(1000 / ticks_in_a_second)
                    || now.duration_since(dbg_game.time)
                        > Duration::from_millis(1000 / ticks_in_a_second))
                    && config.client_server_sync_log.time
                {
                    println!(
                        "out of sync: instant: {:?}, tick_time: {:?}, tick: {:?}",
                        now.duration_since(dbg_game.time),
                        (*last_tick_time).max(dbg_game.tick_time)
                            - (*last_tick_time).min(dbg_game.tick_time),
                        cur_tick,
                    );
                }
                let diff_players = difference::Changeset::new(&dbg_game.players, &players, " ");
                if diff_players
                    .diffs
                    .iter()
                    .find(|diff| !matches!(diff, difference::Difference::Same(_)))
                    .is_some()
                    && config.client_server_sync_log.players
                {
                    println!(
                        "players-{} vs {caller_name}:\n{}",
                        dbg_game.caller, diff_players
                    );
                }
                let diff_inputs = difference::Changeset::new(&dbg_game.inputs, &inputs, " ");
                if diff_inputs
                    .diffs
                    .iter()
                    .find(|diff| !matches!(diff, difference::Difference::Same(_)))
                    .is_some()
                    && config.client_server_sync_log.inputs
                {
                    println!(
                        "inputs-{} vs {caller_name}:\n{}",
                        dbg_game.caller, diff_inputs
                    );
                }
            } else {
                dbg_games.insert(
                    cur_tick,
                    ServerDbgGame {
                        time: std::time::Instant::now(),
                        tick_time: *last_tick_time,
                        players,
                        inputs,
                        caller: caller_name.to_string(),
                    },
                );
                while dbg_games.len() > 250 {
                    dbg_games.pop_front();
                }
            }
        }
    }

    pub fn register(&mut self) {
        if !self.config_game.sv.register {
            return;
        }

        let http = self.io.http.clone();
        self.register_task = Some(
            self.io
                .io_batcher
                .spawn(async move {
                    http.custom_request(
                        "https://master1.ddnet.org/ddnet/15/register"
                            .try_into()
                            .unwrap(),
                        vec![
                            ("Address", "TODO:").into(),
                            ("Secret", "TODO:").into(),
                            ("Challenge-Secret", "TODO:").into(),
                            ("Info-Serial", "TODO:").into(),
                        ],
                    )
                    .await?;

                    Ok(())
                })
                .abortable(),
        );
    }

    pub fn run(&mut self) {
        let mut cur_time = self.sys.time_get_nanoseconds();
        self.last_tick_time = cur_time;
        self.last_register_time = cur_time;

        let game_event_generator = self.game_event_generator_server.clone();
        while self.is_open.load(std::sync::atomic::Ordering::Relaxed) {
            cur_time = self.sys.time_get_nanoseconds();
            if cur_time - self.last_register_time > Duration::from_secs(10) {
                self.register();
                self.last_register_time = cur_time;
            }

            if self
                .has_new_events_server
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                let game_ev_gen = &game_event_generator;
                let mut events = game_ev_gen.events.blocking_lock();
                for (con_id, timestamp_nanos, event) in events.drain(..) {
                    match event {
                        GameEvents::NetworkEvent(net_ev) => match net_ev {
                            NetworkEvent::Connected { cert, .. } => {
                                self.logger
                                    .log(LogLevel::Debug)
                                    .msg("connect time sv: ")
                                    .msg_var(&timestamp_nanos.as_nanos());
                                self.try_client_connect(&con_id, &timestamp_nanos, cert);
                            }
                            NetworkEvent::Disconnected { reason, graceful } => {
                                self.logger
                                    .log(LogLevel::Debug)
                                    .msg("got disconnected event from network");
                                if let Some(players) =
                                    self.client_disconnect(&con_id, reason.as_str())
                                {
                                    for player_id in players.keys() {
                                        self.game_server.player_drop(
                                            player_id,
                                            if graceful {
                                                PlayerDropReason::Disconnect
                                            } else {
                                                PlayerDropReason::Timeout
                                            },
                                        );
                                    }
                                }
                            }
                            NetworkEvent::NetworkStats(stats) => {
                                self.logger
                                    .log(LogLevel::Verbose)
                                    .msg("server ping: ")
                                    .msg_var(&stats.ping.as_millis());
                            }
                            NetworkEvent::ConnectingFailed(_) => {
                                // server usually does not connect, so does not care
                            }
                        },
                        GameEvents::NetworkMsg(game_msg) => self.handle_msg(&con_id, game_msg),
                    }
                }
                game_ev_gen
                    .has_events
                    .store(false, std::sync::atomic::Ordering::Relaxed);
            }

            let ticks_in_a_second = self.game_server.game.game_tick_speed();

            // get time before checking ticks
            cur_time = self.sys.time_get_nanoseconds();
            while is_next_tick(cur_time, &mut self.last_tick_time, ticks_in_a_second) {
                // apply all queued inputs
                if let Some(mut inputs) = self
                    .game_server
                    .queued_inputs
                    .remove(&(self.game_server.cur_monotonic_tick + 1))
                {
                    for (player_id, inp) in inputs.drain() {
                        if let Some(player) = self.game_server.players.get_mut(&player_id) {
                            if let Some(diff) =
                                player.inp.try_overwrite(&inp.inp, inp.version(), false)
                            {
                                self.game_server.game.set_player_input(
                                    &player_id,
                                    &player.inp.inp,
                                    diff,
                                );
                            }
                        }
                    }
                }

                self.game_server.cur_monotonic_tick += 1;

                // game ticks
                self.game_server.game.tick();

                Self::dbg_game(
                    &self.config_game.dbg,
                    &self.last_tick_time,
                    &mut self.game_server.game,
                    Some(self.game_server.players.values().map(|p| &p.inp.inp)),
                    self.game_server.cur_monotonic_tick,
                    ticks_in_a_second,
                    &self.shared_info,
                    "server",
                );

                // snap shot building
                for (con_id, client) in &mut self.clients {
                    let mut player_ids = self.player_ids_pool.new();
                    player_ids.extend(client.players.keys());
                    let snap_client = SnapshotClientInfo {
                        client_player_ids: player_ids,
                        snap_everything: false,
                        snap_other_stages: false,
                    };

                    let snap_id = client.snap_id;
                    client.snap_id += 1;

                    let mut snap = self.game_server.game.snapshot_for(snap_client);

                    let as_diff = if client.client_snap_storage.len() < 10 {
                        client.client_snap_storage.insert(snap_id, snap.clone());
                        true
                    } else {
                        false
                    };

                    let (snap_diff, diff_id) = if let Some(client_snap_id) = &client.client_snap_id
                    {
                        let mut new_snap = self.player_snap_pool.new();
                        new_snap.clone_from(&snap);
                        snap.clear();
                        if let Ok(_) =
                            bin_patch::diff(&client_snap_id.snapshot, &new_snap, &mut *snap)
                        {
                            (snap, Some(client_snap_id.snap_id))
                        } else {
                            snap.clear();
                            snap.append(&mut new_snap);

                            (snap, None)
                        }
                    } else {
                        (snap, None)
                    };

                    if client.snap_id % 2 == 0 {
                        self.network.send_unordered_to(
                            &GameMessage::ServerToClient(ServerToClientMessage::Snapshot {
                                overhead_time: (self.sys.time_get_nanoseconds()
                                    - self.last_tick_time),
                                snapshot: snap_diff,
                                diff_id,
                                snap_id,
                                game_monotonic_tick: self.game_server.cur_monotonic_tick,
                                as_diff,
                            }),
                            &con_id,
                        );
                    }

                    // events building
                    let mut player_ids = self.player_ids_pool.new();
                    player_ids.extend(client.players.keys());
                    let events = self.game_server.game.events_for(EventClientInfo {
                        client_player_ids: player_ids,
                        everything: false,
                        other_stages: false,
                    });
                    if !events.is_empty() {
                        self.network.send_unordered_to(
                            &GameMessage::ServerToClient(ServerToClientMessage::Events {
                                game_monotonic_tick: self.game_server.cur_monotonic_tick,
                                events,
                            }),
                            &con_id,
                        );
                    }
                }
            }
            self.game_server.game.clear_events();

            // after tick checks
            // if the game should reload, reload all game related stuff
            // send the client a load event, which is used for map reloads etc.
            if self.game_server.should_reload() {
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
    }

    fn reload(&mut self) {
        let snapshot = self.game_server.game.snapshot_for_hotreload();
        // reload the whole game server, including the map
        self.game_server = ServerGame::new(
            &self.config_game.sv.map,
            &self.config_game.sv.game_mod,
            &self.config_game.sv.game_type,
            &self.thread_pool,
            &self.io,
        )
        .unwrap();
        if let Some(snapshot) = snapshot {
            self.game_server
                .game
                .build_from_snapshot_by_hotreload(&snapshot);
        }
        // put all players back to a loading state
        self.clients.drain().for_each(|(net_id, client)| {
            self.network_clients.insert(
                net_id,
                ServerNetworkClient {
                    connect_timestamp: client.connect_timestamp,
                    auth: client.auth,
                },
            );
        });
        let server_info = MsgSvServerInfo {
            map: self.game_server.map.name.as_str().try_into().unwrap(),
            map_blake3_hash: self.game_server.map_blake3_hash.into(),
            game_mod: self.game_server.game_mod_name.as_str().try_into().unwrap(),
            game_mod_blake3_hash: self.game_server.game_mod_blake3_hash.clone(),
            hint_start_camera_pos: self.game_server.game.get_client_camera_join_pos(),
            resource_server_fallback: self
                .game_server
                .http_server
                .as_ref()
                .map(|server| server.port),
            game_type: self.game_server.game_type.as_str().try_into().unwrap(),
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
    cert_and_private_key: (x509_cert::Certificate, SigningKey),
    is_open: Arc<AtomicBool>,
    shared_info: Arc<ServerInfo>,
) {
    let thread_pool = Arc::new(
        rayon::ThreadPoolBuilder::new()
            .thread_name(|index| format!("server-rayon {index}"))
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

    let io = Io::new(
        |rt| {
            Arc::new(FileSystem::new(
                rt,
                &sys.log,
                "org",
                "",
                "DDNet",
                "DDNet-Accounts",
            ))
        },
        Arc::new(HttpClient::new()),
    );

    let config_engine = config_fs::load(&io.clone().into());
    let config_game = game_config_fs::fs::load(&io.clone().into());

    let mut server = Server::new(
        sys,
        is_open,
        cert_and_private_key,
        shared_info,
        if IS_INTERNAL_SERVER {
            config_game.sv.port_internal
        } else {
            config_game.sv.port
        },
        config_engine,
        config_game,
        thread_pool,
        io,
    )
    .unwrap();

    server.run();
}
