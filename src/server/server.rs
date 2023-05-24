use std::{
    collections::HashMap,
    num::NonZeroUsize,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use network::network::{
    network::{Network, NetworkConnectionID, NetworkGameEvent},
    quinn_network::QuinnNetwork,
};
use rcgen::Certificate;
use tokio::sync::Mutex;

use crate::{
    game::{simulation_pipe::SimulationPipe, snapshot::SnapshotClientInfo},
    hash_queue::HashQueue,
    network::{
        game_event_generator::{GameEventGenerator, GameEvents},
        messages::{
            ClientToServerMessage, GameMessage, MsgSvPlayerInfo, MsgSvServerInfo, NetworkStr,
            ServerToClientMessage,
        },
    },
    server_game::{ServerGame, ServerGamePlayerInputForPipe, ServerPlayerID},
};

use base::system::{System, SystemTimeInterface};

/**
 * A network queued client is a client that isn't actually part of the game,
 * but e.g. waiting for a slot.
 */
pub struct ServerNetworkQueuedClient {
    network_id: NetworkConnectionID,
    connect_timestamp: Duration,
}

impl ServerNetworkQueuedClient {
    pub fn new(network_id: NetworkConnectionID, connect_timestamp: &Duration) -> Self {
        Self {
            network_id: network_id,
            connect_timestamp: *connect_timestamp,
        }
    }
}

/**
 * A network client is a client that will be part of the game, but is not yet ready,
 * e.g. downloading the map etc.
 */
pub struct ServerNetworkClient {
    network_id: NetworkConnectionID,
    connect_timestamp: Duration,
}

impl ServerNetworkClient {
    pub fn new(network_id: NetworkConnectionID, connect_timestamp: &Duration) -> Self {
        Self {
            network_id: network_id,
            connect_timestamp: *connect_timestamp,
        }
    }
}

/**
 * A server client is a client that is part of the game.
 */
pub struct ServerClient {
    network_id: NetworkConnectionID,
    player_id: ServerPlayerID,
    connect_timestamp: Duration,
}

impl ServerClient {
    pub fn new(network_id: NetworkConnectionID, connect_timestamp: &Duration) -> Self {
        Self {
            network_id: network_id,
            player_id: Default::default(),
            connect_timestamp: *connect_timestamp,
        }
    }
}

pub struct Server {
    // TODO: O(n) sucks, use hash map?
    pub network_queued_clients: HashQueue<NetworkConnectionID, ServerNetworkQueuedClient>,
    pub network_clients: HashMap<NetworkConnectionID, ServerNetworkClient>,
    pub clients: HashMap<NetworkConnectionID, ServerClient>,

    max_clients: usize,

    network: QuinnNetwork,

    is_open: Arc<AtomicBool>,

    has_new_events_server: Arc<AtomicBool>,
    game_event_generator_server: Arc<Mutex<GameEventGenerator>>,

    game: ServerGame,

    sys: System,
}

impl Server {
    pub fn new(
        sys: System,
        is_open: Arc<AtomicBool>,
        cert: &Certificate,
        max_clients: usize,
    ) -> Self {
        let has_new_events_server = Arc::new(AtomicBool::new(false));
        let game_event_generator_server = Arc::new(Mutex::new(GameEventGenerator::new(
            has_new_events_server.clone(),
            sys.time.clone(),
        )));

        let (network_server, _cert) = Network::init_server(
            "127.0.0.1:8305",
            game_event_generator_server.clone(),
            cert,
            sys.time.clone(),
            Some(2),
        );

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

        Self {
            network_queued_clients: HashQueue::new(),
            network_clients: HashMap::new(),
            clients: HashMap::new(),

            max_clients: max_clients,

            network: network_server,

            is_open: is_open,

            has_new_events_server: has_new_events_server,
            game_event_generator_server: game_event_generator_server,

            game: ServerGame::new(&sys.time, "cb2", &thread_pool),

            sys: sys,
        }
    }

    pub fn try_client_connect(&mut self, con_id: &NetworkConnectionID, timestamp: &Duration) {
        // check if the client can be part of the game
        if self.clients.len() + self.network_clients.len() < self.max_clients {
            self.network_clients
                .insert(*con_id, ServerNetworkClient::new(*con_id, timestamp));

            // tell the client about all data required to join the server
            let server_info = MsgSvServerInfo {
                map: NetworkStr::from(&self.game.map.raw.name).unwrap(),
                game_type: NetworkStr::from("idm").unwrap(),
            };
            self.network.send_to(
                &GameMessage::ServerToClient(ServerToClientMessage::ServerInfo(server_info)),
                con_id,
            );
        } else {
            // else add it to the network queue and inform it about that
            self.network_queued_clients.add_or_set(
                con_id.clone(),
                ServerNetworkQueuedClient::new(*con_id, timestamp),
            );

            // TODO self.network.send_to(&"you need to wait".as_bytes(), con_id);
        }
    }

    pub fn client_disconnect(&mut self, con_id: &NetworkConnectionID, _reason: &str) {
        // find client in queued clients
        if self.network_queued_clients.remove(con_id) {
            return;
        }

        // else find in waiting clients
        let found = self.network_clients.remove(con_id);
        if found.is_some() {
            return;
        }

        // else find in clients
        let found = self.clients.remove(con_id);
        if found.is_some() {
            return;
        }
    }

    pub fn try_client_ready(&mut self, con_id: &NetworkConnectionID) -> bool {
        // check if the client can be part of the game
        let found = self.network_clients.remove(con_id);
        match found {
            Some(net_client) => {
                println!("client ready");
                self.clients.insert(
                    *con_id,
                    ServerClient::new(*con_id, &net_client.connect_timestamp.clone()),
                );
                return true;
            }
            None => {}
        }
        false
    }

    pub fn send_player_infos(&mut self, connection_id: &NetworkConnectionID) {
        self.game.players.for_each_in_queue_order(|player| {
            self.network.send_to(
                &GameMessage::ServerToClient(ServerToClientMessage::PlayerInfo(MsgSvPlayerInfo {
                    info: player.player_info.clone(),
                })),
                connection_id,
            );
        })
    }

    pub fn run(&mut self) {
        let mut cur_time = self.sys.time_get_nanoseconds();
        let mut last_tick_time = cur_time;
        let _last_inp_time = cur_time;
        let time_until_tick = Duration::from_secs(1).as_nanos() / 50;

        let game_event_generator = self.game_event_generator_server.clone();
        while self.is_open.load(std::sync::atomic::Ordering::Relaxed) {
            if self
                .has_new_events_server
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                let mut game_ev_gen = game_event_generator.blocking_lock();
                for (con_id, timestamp_nanos, event) in &game_ev_gen.events {
                    match event {
                        GameEvents::NetworkEvent(net_ev) => match net_ev {
                            NetworkGameEvent::Connected => {
                                println!("connect time sv: {}", timestamp_nanos.as_nanos());
                                self.try_client_connect(con_id, timestamp_nanos);
                            }
                            NetworkGameEvent::Disconnected(reason) => {
                                println!("got connected event from network");
                                self.client_disconnect(con_id, reason.as_str());
                            }
                            NetworkGameEvent::NetworkStats(_stats) => {
                                /*println!(
                                    "server ping: {}, inc latency: {}, out latency: {}",
                                    stats.ping.unwrap_or_default().as_millis(),
                                    stats.incoming_latency.unwrap_or_default().as_millis(),
                                    stats.outgoing_latency.unwrap_or_default().as_millis()
                                );*/
                            }
                            _ => todo!(),
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
                                                    .game
                                                    .player_join(con_id, &player_info.player_info);
                                                let mut client =
                                                    self.clients.get_mut(con_id).unwrap();
                                                client.player_id = player_id;

                                                let snap_client = SnapshotClientInfo {
                                                    client_player_id: player_id,
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
                                                    .game
                                                    .snap_shot_builder
                                                    .build_for(&self.game.game, &snap_client);
                                                self.network.send_to(
                                                    &GameMessage::ServerToClient(
                                                        ServerToClientMessage::Snapshot(snap),
                                                    ),
                                                    con_id,
                                                );
                                            }
                                        }
                                        ClientToServerMessage::Input(inp) => {
                                            let player = self.clients.get_mut(con_id);
                                            if let Some(player) = player {
                                                self.game.player_inp(&player.player_id, inp);
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    // ignore any other packet
                                }
                            }
                        }
                        _ => todo!(),
                    }
                }
                game_ev_gen.events.clear();
                game_ev_gen
                    .has_events
                    .store(false, std::sync::atomic::Ordering::Relaxed);
            }

            while (cur_time - last_tick_time).as_nanos() > time_until_tick {
                // game ticks
                self.game.game.tick(&mut SimulationPipe {
                    player_inputs: &ServerGamePlayerInputForPipe {
                        players: &self.game.players,
                    },
                    collision: &self.game.map.collision,
                });

                // snap shot building
                for (con_id, client) in &self.clients {
                    let snap_client = SnapshotClientInfo {
                        client_player_id: client.player_id,
                        snap_everything: false,
                        snap_other_stages: false,
                        time_since_connect_nanos: (self.sys.time_get_nanoseconds()
                            - client.connect_timestamp)
                            .as_nanos() as u64,
                    };
                    let snap = self
                        .game
                        .snap_shot_builder
                        .build_for(&self.game.game, &snap_client);
                    self.network.send_to(
                        &GameMessage::ServerToClient(ServerToClientMessage::Snapshot(snap)),
                        &con_id,
                    );
                }

                last_tick_time += Duration::from_nanos(time_until_tick as u64);
            }

            // time and sleeps
            let next_tick_time = Duration::from_nanos(
                (time_until_tick - (cur_time - last_tick_time).as_nanos()) as u64,
            );
            cur_time = self.sys.time_get_nanoseconds();

            //let mut guard = self.game_event_generator_server.blocking_lock();
            //guard = guard.ev_cond.wait_timeout(guard.into(), next_tick_time);
            std::thread::sleep(next_tick_time);
        }
    }
}

pub fn ddnet_server_main(sys: System, cert: &Certificate, is_open: Arc<AtomicBool>) {
    let mut server = Server::new(sys, is_open, cert, 16 /* TODO */);

    server.run();
}
