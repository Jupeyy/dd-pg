use std::{
    fmt::Debug,
    net::IpAddr,
    num::NonZeroUsize,
    path::Path,
    sync::{atomic::AtomicBool, Arc, RwLock},
    time::{Duration, SystemTime},
};

use account_client::certs::certs_to_pub_keys;
use accounts_shared::game_server::user_id::{UserId, VerifyingKey};
use anyhow::anyhow;
use base::{
    hash::{fmt_hash, generate_hash_for, Hash},
    system::{System, SystemTimeInterface},
};
use base_fs::filesys::FileSystem;
use base_http::http::HttpClient;
use base_io::{
    io::Io,
    io_batcher::{IoBatcher, IoBatcherTask},
};
use client_http_fs::{client::ClientHttpTokioFs, fs::Fs};
use config::config::ConfigEngine;
use ed25519_dalek::SigningKey;
use game_config::config::{ConfigDebug, ConfigGame, ConfigServerDatabase};
use game_database::{dummy::DummyDb, traits::DbInterface};
use game_database_backend::GameDbBackend;
use hashlink::{LinkedHashMap, LinkedHashSet};
use http_accounts::http::AccountHttp;
use master_server_types::response::RegisterResponse;
use network::network::{
    connection::NetworkConnectionId,
    connection_ban::ConnectionBans,
    event::NetworkEvent,
    network::{Network, NetworkServerCertAndKey, NetworkServerCertMode, NetworkServerInitOptions},
    packet_compressor::DefaultNetworkPacketCompressor,
    packet_dict::ZstdNetworkDictTrainer,
    plugins::{NetworkPluginPacket, NetworkPlugins},
    quinn_network::QuinnNetwork,
    types::NetworkInOrderChannel,
};
use pool::{datatypes::PoolLinkedHashMap, mt_datatypes::PoolCow, pool::Pool};
use rand::RngCore;
use shared::game::state_wasm_manager::GameStateWasmManager;
use sql::database::{Database, DatabaseDetails};
use tokio::time::Instant;
use x509_cert::der::{Decode, Encode};

use crate::{
    auto_map_votes::AutoMapVotes,
    client::{
        ClientSnapshotForDiff, ClientSnapshotStorage, Clients, ServerClient, ServerClientPlayer,
        ServerNetworkClient, ServerNetworkQueuedClient,
    },
    rcon::Rcon,
    server_game::{
        ClientAuth, ServerExtraVoteInfo, ServerGame, ServerVote, RESERVED_DDNET_NAMES,
        RESERVED_VANILLA_NAMES,
    },
};

use shared_base::{
    game_types::{is_next_tick, time_until_tick},
    network::{
        messages::{
            MsgClChatMsg, MsgClLoadVotes, MsgClSnapshotAck, MsgSvChatMsg, MsgSvServerInfo,
            PlayerInputChainable,
        },
        server_info::{ServerDbgGame, ServerInfo},
        types::chat::{NetChatMsg, NetChatMsgPlayerChannel},
    },
    server_browser::{ServerBrowserInfo, ServerBrowserInfoMap, ServerBrowserPlayer},
};

use game_interface::{
    chat_commands::ClientChatCommand,
    client_commands::ClientCommand,
    events::EventClientInfo,
    interface::GameStateInterface,
    rcon_commands::{AuthLevel, ClientRconCommand},
    types::{
        game::{GameEntityId, GameTickType},
        input::CharacterInput,
        network_stats::PlayerNetworkStats,
        player_info::{PlayerClientInfo, PlayerDropReason, PlayerUniqueId},
        snapshot::SnapshotClientInfo,
    },
    votes::{MapVote, VoteState, VoteType, Voted},
};

use shared_network::{
    game_event_generator::{GameEventGenerator, GameEvents},
    messages::{
        ClientToServerMessage, ClientToServerPlayerMessage, GameMessage, MsgSvInputAck,
        MsgSvLoadVotes, ServerToClientMessage,
    },
};

type DbSetup = (
    Option<Arc<Database>>,
    Arc<dyn DbInterface>,
    Option<Arc<account_game_server::shared::Shared>>,
);

pub struct Server {
    pub clients: Clients,
    pub player_count_of_all_clients: usize,

    max_players: usize,

    // network
    network: QuinnNetwork,
    connection_bans: Arc<ConnectionBans>,

    is_open: Arc<AtomicBool>,

    has_new_events_server: Arc<AtomicBool>,
    game_event_generator_server: Arc<GameEventGenerator>,

    game_server: ServerGame,

    config_game: ConfigGame,
    // for master server register
    server_port: u16,
    thread_pool: Arc<rayon::ThreadPool>,
    io: Io,

    sys: System,

    last_tick_time: Duration,
    last_register_time: Option<Duration>,
    register_task: Option<IoBatcherTask<()>>,
    last_register_serial: u32,

    last_network_stats_time: Duration,

    shared_info: Arc<ServerInfo>,

    // for server register
    cert_sha256_fingerprint: Hash,

    // rcon
    rcon: Rcon,

    // votes
    map_votes: Vec<MapVote>,
    map_votes_hash: Hash,

    // database
    db: Option<Arc<Database>>,
    game_db: Arc<dyn DbInterface>,
    accounts: Option<Arc<account_game_server::shared::Shared>>,
    account_server_public_key: Arc<RwLock<Vec<VerifyingKey>>>,
    // intentionally unused
    _account_server_cert_downloader_task: Option<IoBatcherTask<()>>,

    // pools
    player_ids_pool: Pool<LinkedHashSet<GameEntityId>>,
    player_snap_pool: Pool<Vec<u8>>,
    player_network_stats_pool: Pool<LinkedHashMap<GameEntityId, PlayerNetworkStats>>,

    // helpers
    input_deser: Pool<Vec<u8>>,
}

impl Server {
    pub async fn db_setup(config_db: &ConfigServerDatabase) -> anyhow::Result<Arc<Database>> {
        Ok(Arc::new(
            Database::new(DatabaseDetails {
                host: config_db.host.clone(),
                port: config_db.port,
                database: config_db.database.clone(),
                username: config_db.username.clone(),
                password: config_db.password.clone(),
                ca_cert_path: config_db.ca_cert_path.clone(),
                connection_count: config_db.connection_count as usize,
            })
            .await?,
        ))
    }

    pub fn db_setup_task(
        io_batcher: &IoBatcher,
        config_db: ConfigServerDatabase,
    ) -> IoBatcherTask<DbSetup> {
        io_batcher.spawn(async move {
            if !config_db.host.is_empty()
                && !config_db.database.is_empty()
                && config_db.port != 0
                && config_db.connection_count != 0
            {
                let db = Self::db_setup(&config_db).await?;

                let game_db: Arc<dyn DbInterface> = Arc::new(GameDbBackend::new(db.clone())?);

                let accounts = if config_db.enable_accounts {
                    account_game_server::setup::setup(&db.pool).await?;
                    Some(account_game_server::prepare::prepare(&db.pool).await?)
                } else {
                    None
                };

                Ok((Some(db), game_db, accounts))
            } else {
                let game_db: Arc<dyn DbInterface> = Arc::new(DummyDb);
                Ok((None, game_db, None))
            }
        })
    }

    async fn download_account_server_cert(
        http_accounts: &ClientHttpTokioFs,
    ) -> anyhow::Result<Vec<x509_cert::Certificate>> {
        Ok(account_client::certs::download_certs(http_accounts).await?)
    }

    fn read_mod_config(io: &Io, mod_name: &str) -> IoBatcherTask<Vec<u8>> {
        let mod_name = mod_name.to_string();
        let fs = io.fs.clone();
        io.io_batcher.spawn(async move {
            let config_mod = fs
                .read_file(format!("config/{mod_name}.json").as_ref())
                .await?;

            Ok(config_mod)
        })
    }

    fn config_mod_name(config_game: &ConfigGame) -> String {
        let mut mod_name = config_game.sv.game_mod.clone();
        if RESERVED_VANILLA_NAMES.contains(&mod_name.as_str()) {
            mod_name = "vanilla".to_string();
        } else if RESERVED_DDNET_NAMES.contains(&mod_name.as_str()) {
            mod_name = "ddnet".to_string();
        }
        mod_name
    }

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
        let config_db = config_game.sv.db.clone();
        let accounts_enabled = config_db.enable_accounts;
        let task = Self::db_setup_task(&io.io_batcher, config_db);
        let auto_map_votes = shared_info.is_internal_server.then(|| {
            let fs = io.fs.clone();
            io.io_batcher
                .spawn(async move { AutoMapVotes::new(&fs).await })
        });

        let fs = io.fs.clone();
        let zstd_dicts = io.io_batcher.spawn(async move {
            let client_send = fs.read_file("dict/client_send".as_ref()).await;
            let server_send = fs.read_file("dict/server_send".as_ref()).await;

            Ok(client_send.and_then(|c| server_send.map(|s| (c, s)))?)
        });

        // load mod config
        let mod_name = Self::config_mod_name(&config_game);
        let config_mod_task = Self::read_mod_config(&io, &mod_name);

        let fs = io.fs.clone();
        let http = io.http.clone();
        let path = io.fs.get_secure_path();
        let http_accounts = io
            .io_batcher
            .spawn(async move {
                Ok(Arc::new(ClientHttpTokioFs {
                    http: Arc::new(AccountHttp::new_with_url(
                        "https://pg.ddnet.org:5555/".try_into().unwrap(),
                        http.clone(),
                    )),
                    fs: Fs::new(path).await?,
                }))
            })
            .get_storage()?;
        let http_accounts_clone = http_accounts.clone();
        let account_server_cert = io.io_batcher.spawn(async move {
            if accounts_enabled {
                // try to read the key from disk
                let file = fs
                    .read_file("account_server_certs.json".as_ref())
                    .await
                    .map_err(|err| anyhow!(err))
                    .and_then(|cert_json| {
                        serde_json::from_slice::<Vec<Vec<u8>>>(&cert_json)
                            .map_err(|err| anyhow!(err))
                            .and_then(|certs_der| {
                                certs_der
                                    .into_iter()
                                    .map(|cert_der| {
                                        x509_cert::Certificate::from_der(&cert_der)
                                            .map_err(|err| anyhow!(err))
                                    })
                                    .collect::<anyhow::Result<Vec<x509_cert::Certificate>>>()
                            })
                    });

                match file {
                    Ok(file) => Ok(file),
                    Err(_) => {
                        // try to download latest cert instead
                        let certs =
                            Self::download_account_server_cert(&http_accounts_clone).await?;

                        let _ = fs
                            .write_file(
                                "account_server_certs.json".as_ref(),
                                serde_json::to_vec(
                                    &certs
                                        .iter()
                                        .map(|cert| cert.to_der().map_err(|err| anyhow!(err)))
                                        .collect::<anyhow::Result<Vec<_>>>()?,
                                )?,
                            )
                            .await;

                        Ok(certs)
                    }
                }
            } else {
                Ok(vec![])
            }
        });

        let has_new_events_server = Arc::new(AtomicBool::new(false));
        let game_event_generator_server = Arc::new(GameEventGenerator::new(
            has_new_events_server.clone(),
            sys.time.clone(),
        ));

        let connection_bans = Arc::new(ConnectionBans::default());

        let mut packet_plugins: Vec<Arc<dyn NetworkPluginPacket>> = vec![];

        if config_game.sv.train_packet_dictionary {
            packet_plugins.push(Arc::new(ZstdNetworkDictTrainer::new(
                config_game.sv.train_packet_dictionary_max_size as usize,
            )));
        }

        if let Ok((client_send, server_send)) = zstd_dicts.get_storage() {
            packet_plugins.push(Arc::new(DefaultNetworkPacketCompressor::new_with_dict(
                server_send,
                client_send,
            )));
        } else {
            packet_plugins.push(Arc::new(DefaultNetworkPacketCompressor::new()));
        }

        let cert_sha256_fingerprint = cert_and_private_key
            .0
            .tbs_certificate
            .subject_public_key_info
            .fingerprint_bytes()?;

        let (network_server, _cert, sock_addr, _notifer_server) = Network::init_server(
            &format!("0.0.0.0:{port}"),
            game_event_generator_server.clone(),
            NetworkServerCertMode::FromCertAndPrivateKey(Box::new(NetworkServerCertAndKey {
                cert: cert_and_private_key.0,
                private_key: cert_and_private_key.1,
            })),
            &sys,
            NetworkServerInitOptions::new()
                .with_max_thread_count(if shared_info.is_internal_server { 2 } else { 6 })
                .with_timeout(config_engine.net.timeout)
                .with_disable_retry_on_connect(
                    config_engine.net.disable_retry_on_connect || shared_info.is_internal_server,
                )
                .with_packet_capacity_and_size(
                    if shared_info.is_internal_server {
                        8
                    } else {
                        64
                    },
                    256,
                )
                // since there are many packets, increase loss detection thresholds
                .with_loss_detection_cfg(25, 2.0)
                .with_ack_config(25, Duration::from_secs(1), 25 - 1),
            NetworkPlugins {
                packet_plugins: Arc::new(packet_plugins),
                connection_plugins: Arc::new(vec![connection_bans.clone()]),
            },
        );
        *shared_info.sock_addr.lock().unwrap() = Some(sock_addr);

        let (db, game_db, accounts) = task.get_storage()?;
        let account_server_certs = account_server_cert.get_storage().unwrap_or_default();
        let account_server_public_key =
            Arc::new(RwLock::new(certs_to_pub_keys(&account_server_certs)));

        let account_server_cert_downloader_task = if !account_server_certs.is_empty() {
            let account_server_public_key = account_server_public_key.clone();
            Some(
                io.io_batcher
                    .spawn::<(), _>(async move {
                        let mut account_server_certs = account_server_certs;
                        loop {
                            let invalid_in = account_server_certs
                                .iter()
                                .map(|c| {
                                    c.tbs_certificate
                                        .validity
                                        .not_after
                                        .to_date_time()
                                        .to_system_time()
                                        .duration_since(
                                            SystemTime::now()
                                                + Duration::from_secs(7 * 24 * 60 * 60),
                                        )
                                        .unwrap_or(Duration::MAX)
                                })
                                .min();

                            // either if first cert is about to invalidate or when one week passed
                            let one_week = Duration::from_secs(7 * 24 * 60 * 60);
                            let duration_offset = invalid_in
                                .map(|d| d.saturating_sub(Duration::from_secs(24 * 60 * 60)))
                                .unwrap_or(one_week)
                                .min(one_week);

                            tokio::time::sleep_until(Instant::now() + duration_offset).await;

                            if let Ok(certs) =
                                Self::download_account_server_cert(&http_accounts).await
                            {
                                account_server_certs = certs;
                                let new_account_server_public_key =
                                    certs_to_pub_keys(&account_server_certs);

                                *account_server_public_key.write().unwrap() =
                                    new_account_server_public_key;
                            }
                        }
                    })
                    .abortable(),
            )
        } else {
            None
        };

        let mut map_votes: Vec<_> =
            if let Some(Ok(votes)) = auto_map_votes.map(|task| task.get_storage()) {
                votes
                    .map_files
                    .into_iter()
                    .filter_map(|map| {
                        map.file_stem()
                            .and_then(|s| s.to_str().and_then(|s| s.try_into().ok()))
                            .map(|name| MapVote {
                                name,
                                hash: None,
                                thumbnail_resource: false,
                            })
                    })
                    .collect()
            } else {
                Default::default()
            };
        map_votes.sort_by(|v1, v2| v1.name.cmp(&v2.name));
        let map_votes_hash = generate_hash_for(&serde_json::to_vec(&map_votes).unwrap());

        let config_mod = config_mod_task.get_storage().ok();

        let rcon = Rcon::new(&io);
        // share secret with client (if exists)
        *shared_info.rcon_secret.lock().unwrap() = Some(rcon.rcon_secret);

        Ok(Self {
            clients: Clients::new(
                config_game.sv.max_players as usize,
                config_game.sv.max_players_per_ip as usize,
            ),
            player_count_of_all_clients: 0,

            max_players: config_game.sv.max_players as usize,

            network: network_server,
            connection_bans,

            is_open,

            has_new_events_server,
            game_event_generator_server,

            game_server: ServerGame::new(
                &config_game.sv.map,
                &mod_name,
                config_mod,
                &thread_pool,
                &io,
                &game_db,
            )?,

            last_tick_time: sys.time_get_nanoseconds(),
            last_register_time: None,
            register_task: None,
            last_register_serial: 0,

            last_network_stats_time: sys.time_get_nanoseconds(),

            sys,

            shared_info,

            // for server register
            cert_sha256_fingerprint,

            // rcon
            rcon,

            // votes
            map_votes,
            map_votes_hash,

            // database
            db,
            game_db,
            accounts,
            account_server_public_key,
            _account_server_cert_downloader_task: account_server_cert_downloader_task,

            player_ids_pool: Pool::with_sized(config_game.sv.max_players as usize, || {
                LinkedHashSet::with_capacity(config_game.sv.max_players_per_ip as usize)
            }),
            player_snap_pool: Pool::with_capacity(2),
            player_network_stats_pool: Pool::with_capacity(config_game.sv.max_players as usize),

            // helpers
            input_deser: Pool::with_capacity(3),

            thread_pool,
            io,

            config_game,
            server_port: port,
        })
    }

    fn can_another_player_connect(&self) -> bool {
        self.player_count_of_all_clients + self.clients.network_clients.len() < self.max_players
    }

    pub fn try_client_connect(
        &mut self,
        con_id: &NetworkConnectionId,
        timestamp: &Duration,
        ip: IpAddr,
        cert: Arc<x509_cert::Certificate>,
        network_stats: PlayerNetworkStats,
    ) {
        // check if the client can be part of the game
        if self.can_another_player_connect() {
            self.clients.network_clients.insert(
                *con_id,
                ServerNetworkClient::new(timestamp, ip, cert, network_stats),
            );

            // tell the client about all data required to join the server
            let server_info = MsgSvServerInfo {
                map: self.game_server.map.name.as_str().try_into().unwrap(),
                map_blake3_hash: self.game_server.map_blake3_hash,
                game_mod: self.game_server.game_mod.clone(),
                mod_config: self.game_server.game.info.config.clone(),
                resource_server_fallback: self
                    .game_server
                    .http_server
                    .as_ref()
                    .map(|server| server.port),
                hint_start_camera_pos: self.game_server.game.get_client_camera_join_pos(),
            };
            self.network.send_unordered_to(
                &GameMessage::ServerToClient(ServerToClientMessage::ServerInfo {
                    info: server_info,
                    overhead: self.sys.time_get_nanoseconds().saturating_sub(*timestamp),
                }),
                con_id,
            );

            self.player_count_of_all_clients += 1;
        } else {
            // else add it to the network queue and inform it about that
            self.clients.network_queued_clients.insert(
                *con_id,
                ServerNetworkQueuedClient::new(
                    timestamp,
                    ip,
                    ClientAuth {
                        cert,
                        level: Default::default(),
                    },
                    network_stats,
                ),
            );

            self.network.send_unordered_to(
                &GameMessage::ServerToClient(ServerToClientMessage::QueueInfo(format!(
                    "The server is full.\nYou are queued at position: #{}",
                    self.clients.network_queued_clients.len()
                ))),
                con_id,
            );
        }
    }

    fn drop_client_from_queue(
        &mut self,
        con_id: &NetworkConnectionId,
    ) -> Option<ServerNetworkQueuedClient> {
        let mut iter = self
            .clients
            .network_queued_clients
            .iter_at_key(con_id)
            .unwrap();
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
        self.clients.network_queued_clients.remove(con_id)
    }

    pub fn client_disconnect(
        &mut self,
        con_id: &NetworkConnectionId,
        _reason: &str,
    ) -> Option<PoolLinkedHashMap<GameEntityId, ServerClientPlayer>> {
        // find client in queued clients
        if self.clients.network_queued_clients.contains_key(con_id) {
            self.drop_client_from_queue(con_id);
            return None;
        }

        // else find in waiting clients, connect the waiting client
        let found = self.clients.network_clients.remove(con_id);
        if found.is_some() {
            self.player_count_of_all_clients -= 1;
            if !self.clients.network_queued_clients.is_empty() {
                let con_id_queue = *self.clients.network_queued_clients.front().unwrap().0;
                let timestamp_queue = self
                    .clients
                    .network_queued_clients
                    .front()
                    .unwrap()
                    .1
                    .connect_timestamp;
                let p = self.drop_client_from_queue(&con_id_queue).unwrap();
                self.try_client_connect(
                    &con_id_queue,
                    &timestamp_queue,
                    p.ip,
                    p.auth.cert,
                    p.network_stats,
                );
            }
            return None;
        }

        // else find in clients, connect one from queue if this client disconnected
        let found = self.clients.clients.remove(con_id);
        if let Some(p) = found {
            // update vote if nessecary
            if let Some(vote) = &mut self.game_server.cur_vote {
                if let Some(voted) = vote.participating_clients.remove(con_id) {
                    match voted {
                        Voted::Yes => vote.state.yes_votes -= 1,
                        Voted::No => vote.state.no_votes -= 1,
                    }
                }
                vote.state.allowed_to_vote_count -= 1;

                let vote_state = vote.state.clone();
                let started_at = vote.started_at;
                self.send_vote(Some(vote_state), started_at);
            }

            self.player_count_of_all_clients -= p.players.len();
            for _ in 0..p.players.len() {
                if !self.clients.network_queued_clients.is_empty() {
                    let con_id_queue = *self.clients.network_queued_clients.front().unwrap().0;
                    let timestamp_queue = self
                        .clients
                        .network_queued_clients
                        .front()
                        .unwrap()
                        .1
                        .connect_timestamp;
                    let drop_player = self.drop_client_from_queue(&con_id_queue).unwrap();
                    self.try_client_connect(
                        &con_id_queue,
                        &timestamp_queue,
                        drop_player.ip,
                        drop_player.auth.cert,
                        drop_player.network_stats,
                    );
                }
            }
            return Some(p.players);
        }
        None
    }

    fn broadcast_in_order(&self, packet: GameMessage, channel: NetworkInOrderChannel) {
        self.clients.clients.keys().for_each(|send_con_id| {
            self.network.send_in_order_to(&packet, send_con_id, channel);
        });
    }

    fn send_vote(&self, vote_state: Option<VoteState>, start_time: Duration) {
        self.broadcast_in_order(
            GameMessage::ServerToClient(ServerToClientMessage::Vote(vote_state.map(
                |mut vote_state| {
                    vote_state.remaining_time = Duration::from_secs(25)
                        .saturating_sub(self.sys.time_get_nanoseconds().saturating_sub(start_time));
                    vote_state
                },
            ))),
            NetworkInOrderChannel::Custom(7013), // This number reads as "vote".
        )
    }

    fn add_player_for_client(
        &mut self,
        con_id: &NetworkConnectionId,
        mut player_info: PlayerClientInfo,
        is_additional_player: bool,
    ) {
        if let Some(client) = self.clients.clients.get_mut(con_id) {
            player_info.player_index = client.players.len();
            let player_id = self.game_server.player_join(con_id, &player_info);
            client.players.insert(
                player_id,
                ServerClientPlayer {
                    input_storage: Default::default(),
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

                client.client_snap_storage.insert(
                    snap_id,
                    ClientSnapshotStorage {
                        snapshot: snap.to_vec(),
                        monotonic_tick: self.game_server.cur_monotonic_tick,
                    },
                );

                self.network.send_unordered_auto_to(
                    &GameMessage::ServerToClient(ServerToClientMessage::Snapshot {
                        overhead_time: self
                            .sys
                            .time_get_nanoseconds()
                            .saturating_sub(self.last_tick_time),
                        snapshot: snap,
                        diff_id: None,
                        snap_id_diffed: snap_id,
                        game_monotonic_tick_diff: self.game_server.cur_monotonic_tick,
                        as_diff: true,
                        input_ack: PoolCow::new_without_pool(),
                    }),
                    con_id,
                );
            }
        }
    }

    fn handle_player_msg(
        &mut self,
        con_id: &NetworkConnectionId,
        player_id: &GameEntityId,
        player_msg: ClientToServerPlayerMessage,
    ) {
        let client = self.clients.clients.get_mut(con_id);
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
                            if !msg.is_empty() {
                                if self
                                    .game_server
                                    .game
                                    .info
                                    .chat_commands
                                    .prefixes
                                    .contains(&msg.chars().next().unwrap())
                                {
                                    self.game_server.game.client_command(
                                        player_id,
                                        ClientCommand::Chat(ClientChatCommand {
                                            raw: msg.chars().skip(1).collect(),
                                        }),
                                    );
                                } else {
                                    self.broadcast_in_order(
                                        GameMessage::ServerToClient(ServerToClientMessage::Chat(
                                            MsgSvChatMsg {
                                                msg: NetChatMsg {
                                                    player_id: *player_id,
                                                    msg: msg.as_str().to_string(),
                                                    channel: NetChatMsgPlayerChannel::Global,
                                                },
                                            },
                                        )),
                                        NetworkInOrderChannel::Custom(3841), // This number reads as "chat".
                                    );
                                }
                            }
                        }
                        MsgClChatMsg::GameTeam { .. } => todo!(),
                        MsgClChatMsg::Whisper { .. } => todo!(),
                    },
                    ClientToServerPlayerMessage::Kill => {
                        self.game_server
                            .game
                            .client_command(player_id, ClientCommand::Kill);
                    }
                    ClientToServerPlayerMessage::StartVote(vote) => {
                        match vote {
                            VoteType::Map(_) => {
                                // if no current vote exist, try map vote
                                if self.game_server.cur_vote.is_none() {
                                    self.game_server.cur_vote = Some(ServerVote {
                                        state: VoteState {
                                            vote,
                                            // filled on the fly instead
                                            remaining_time: Duration::ZERO,
                                            // vote starter get a yes vote
                                            yes_votes: 1,
                                            no_votes: 0,
                                            allowed_to_vote_count: self.clients.clients.len(),
                                        },
                                        extra_vote_info: ServerExtraVoteInfo::None,
                                        started_at: self.sys.time_get_nanoseconds(),
                                        participating_clients: [(*con_id, Voted::Yes)]
                                            .into_iter()
                                            .collect(),
                                    });
                                    let vote_state = self.game_server.cur_vote.as_ref().map(|v| {
                                        let mut state = v.state.clone();
                                        state.remaining_time = Duration::from_secs(25);
                                        state
                                    });
                                    self.broadcast_in_order(
                                        GameMessage::ServerToClient(ServerToClientMessage::Vote(
                                            vote_state,
                                        )),
                                        NetworkInOrderChannel::Custom(7013), // This number reads as "vote".
                                    )
                                }
                            }
                            VoteType::VoteSpecPlayer { voted_player_id }
                            | VoteType::VoteKickPlayer { voted_player_id } => {
                                if *player_id != voted_player_id {
                                    if let Some((kick_con_id, player)) =
                                        self.game_server.players.get(&voted_player_id).and_then(
                                            |p| {
                                                self.clients
                                                    .clients
                                                    .get(&p.network_id)
                                                    .map(|c| (p.network_id, c))
                                            },
                                        )
                                    {
                                        // if the player exists and no current vote exists, start the vote
                                        if self.game_server.cur_vote.is_none() {
                                            self.game_server.cur_vote = Some(ServerVote {
                                                state: VoteState {
                                                    vote,
                                                    // filled on the fly instead
                                                    remaining_time: Duration::ZERO,
                                                    // vote starter get a yes vote
                                                    yes_votes: 1,
                                                    no_votes: 0,
                                                    allowed_to_vote_count: self
                                                        .clients
                                                        .clients
                                                        .len(),
                                                },
                                                extra_vote_info: ServerExtraVoteInfo::Player {
                                                    to_kick_player: kick_con_id,
                                                    ip: player.ip,
                                                    account_info: player.auth.clone(),
                                                },
                                                started_at: self.sys.time_get_nanoseconds(),
                                                participating_clients: [(*con_id, Voted::Yes)]
                                                    .into_iter()
                                                    .collect(),
                                            });
                                            let vote_state =
                                                self.game_server.cur_vote.as_ref().map(|v| {
                                                    let mut state = v.state.clone();
                                                    state.remaining_time = Duration::from_secs(25);
                                                    state
                                                });
                                            self.broadcast_in_order(
                                                GameMessage::ServerToClient(
                                                    ServerToClientMessage::Vote(vote_state),
                                                ),
                                                NetworkInOrderChannel::Custom(7013), // This number reads as "vote".
                                            )
                                        }
                                    }
                                }
                            }
                            VoteType::Misc() => todo!(),
                        }
                    }
                    ClientToServerPlayerMessage::Voted(voted) => {
                        if let Some(vote) = &mut self.game_server.cur_vote {
                            let prev_vote = vote.participating_clients.insert(*con_id, voted);
                            match voted {
                                game_interface::votes::Voted::Yes => vote.state.yes_votes += 1,
                                game_interface::votes::Voted::No => vote.state.no_votes += 1,
                            }
                            if let Some(prev_vote) = prev_vote {
                                match prev_vote {
                                    Voted::Yes => vote.state.yes_votes -= 1,
                                    Voted::No => vote.state.no_votes -= 1,
                                }
                            }
                            let vote_state = vote.state.clone();
                            let started_at = vote.started_at;
                            self.send_vote(Some(vote_state), started_at);
                        }
                    }
                    ClientToServerPlayerMessage::Emoticon(emoticon) => {
                        self.game_server.set_player_emoticon(player_id, emoticon);
                    }
                    ClientToServerPlayerMessage::ChangeEyes { eye, duration } => {
                        self.game_server.set_player_eye(player_id, eye, duration);
                    }
                }
            }
        }
    }

    fn user_id(
        account_server_public_key: &Arc<RwLock<Vec<VerifyingKey>>>,
        auth: &ClientAuth,
    ) -> UserId {
        accounts_shared::game_server::user_id::user_id_from_cert(
            account_server_public_key.read().unwrap().as_slice(),
            auth.cert.to_der().unwrap(),
        )
    }

    fn user_id_to_player_unique_id(user_id: &UserId) -> PlayerUniqueId {
        user_id
            .account_id
            .map(PlayerUniqueId::Account)
            .unwrap_or_else(|| PlayerUniqueId::CertFingerprint(user_id.public_key))
    }

    fn client_snap_ack(client: &mut ServerClient, snap_id: u64) {
        if let Some(snap) = client.client_snap_storage.remove(&snap_id) {
            client.latest_client_snap = Some(ClientSnapshotForDiff {
                snap_id,
                snapshot: snap.snapshot,
                monotonic_tick: snap.monotonic_tick,
            });
        }
        while client
            .client_snap_storage
            .first_entry()
            .is_some_and(|entry| *entry.key() < snap_id)
        {
            client.client_snap_storage.pop_first();
        }
    }

    fn send_rcon_commands(&self, con_id: &NetworkConnectionId) {
        self.network.send_in_order_to(
            &GameMessage::ServerToClient(ServerToClientMessage::RconCommands(
                self.game_server.game.info.rcon_commands.clone(),
            )),
            con_id,
            NetworkInOrderChannel::Custom(
                7302, // reads as "rcon"
            ),
        );
    }

    fn handle_msg(
        &mut self,
        timestamp: &Duration,
        con_id: &NetworkConnectionId,
        game_msg: GameMessage,
    ) {
        match game_msg {
            GameMessage::ClientToServer(client_to_server_msg) => {
                match client_to_server_msg {
                    ClientToServerMessage::Ready(ready_info) => {
                        // if client is actually waiting, make it part of the game
                        let account_server_public_key = self.account_server_public_key.clone();
                        let client = self.clients.try_client_ready(con_id);
                        let check_vote = client.is_some();
                        if let Some(client) = client {
                            let user_id = Self::user_id(&account_server_public_key, &client.auth);
                            let unique_identifier = Self::user_id_to_player_unique_id(&user_id);

                            let send_rcon = self.rcon.try_rcon_auth(
                                client,
                                ready_info.rcon_secret.as_ref(),
                                &unique_identifier,
                            );

                            let initial_network_stats = client.network_stats;
                            self.add_player_for_client(
                                con_id,
                                PlayerClientInfo {
                                    info: ready_info.player_info,
                                    is_dummy: false,
                                    player_index: 0,
                                    unique_identifier,
                                    initial_network_stats,
                                },
                                false,
                            );
                            if send_rcon {
                                self.send_rcon_commands(con_id);
                            }

                            if let Some((account, db)) =
                                self.accounts.as_ref().zip(self.db.as_ref())
                            {
                                let account = account.clone();
                                let pool = db.pool.clone();
                                self.io.io_batcher.spawn_without_lifetime(async move {
                                    Ok(account_game_server::auto_login::auto_login(
                                        account, &pool, &user_id,
                                    )
                                    .await
                                    .map(|_| ())?)
                                });
                            }
                        }

                        if check_vote {
                            // update vote if nessecary
                            if let Some(vote) = &mut self.game_server.cur_vote {
                                vote.state.allowed_to_vote_count += 1;

                                let vote_state = vote.state.clone();
                                let started_at = vote.started_at;
                                self.send_vote(Some(vote_state), started_at);
                            }
                        }
                    }
                    ClientToServerMessage::AddLocalPlayer(player_info) => {
                        if self.can_another_player_connect() {
                            if let Some(client) = self.clients.clients.get(con_id) {
                                let player_info = PlayerClientInfo {
                                    info: player_info.player_info,
                                    is_dummy: player_info.as_dummy,
                                    player_index: 0,
                                    unique_identifier: Self::user_id_to_player_unique_id(
                                        &Self::user_id(
                                            &self.account_server_public_key,
                                            &client.auth,
                                        ),
                                    ),
                                    initial_network_stats: client.network_stats,
                                };
                                self.add_player_for_client(con_id, player_info, true);
                            }
                        }
                    }
                    ClientToServerMessage::PlayerMsg((player_id, player_msg)) => {
                        self.handle_player_msg(con_id, &player_id, player_msg);
                    }
                    ClientToServerMessage::Inputs {
                        inputs,
                        snap_ack,
                        id,
                    } => {
                        let client = self.clients.clients.get_mut(con_id);
                        if let Some(client) = client {
                            // add ack early to make the timing more accurate
                            client.inputs_to_ack.push(MsgSvInputAck {
                                id,
                                // reuse this field this one time
                                logic_overhead: *timestamp,
                            });

                            for (player_id, inp_chain) in inputs.iter() {
                                if let Some(player) = client.players.get_mut(player_id) {
                                    let Some(def_inp) = (if let Some(diff_id) = inp_chain.diff_id {
                                        player.input_storage.get(&diff_id).copied()
                                    } else {
                                        Some(PlayerInputChainable::default())
                                    }) else {
                                        log::debug!(target: "server", "had to drop an input from the client for diff id: {:?}", inp_chain.diff_id);
                                        continue;
                                    };

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
                                            let as_diff = inp_chain.as_diff;
                                            if as_diff {
                                                // this should be higher than the number of inputs saved on the client
                                                // (since reordering of packets etc.)
                                                while player.input_storage.len() >= 50 {
                                                    player.input_storage.pop_first();
                                                }
                                                player.input_storage.insert(id, inp);
                                            }

                                            self.game_server.player_inp(
                                                player_id,
                                                inp.inp,
                                                inp.for_monotonic_tick,
                                            );
                                        }

                                        offset += def_len;
                                        old = new;
                                    }
                                }
                            }
                            for MsgClSnapshotAck { snap_id } in snap_ack.iter() {
                                Self::client_snap_ack(client, *snap_id);
                            }
                        }
                    }
                    ClientToServerMessage::LoadVotes(votes) => {
                        if let Some(client) = self.clients.clients.get_mut(con_id) {
                            match votes {
                                MsgClLoadVotes::Map { cached_votes } => {
                                    if !client.loaded_map_votes {
                                        client.loaded_map_votes = true;

                                        if !cached_votes
                                            .is_some_and(|hash| hash == self.map_votes_hash)
                                        {
                                            self.network.send_unordered_to(
                                                &GameMessage::ServerToClient(
                                                    ServerToClientMessage::LoadVote(
                                                        MsgSvLoadVotes::Map {
                                                            votes: self.map_votes.clone(),
                                                        },
                                                    ),
                                                ),
                                                con_id,
                                            );
                                        }
                                    }
                                }
                                MsgClLoadVotes::Misc { cached_votes } => todo!(),
                            }
                        }
                    }
                    ClientToServerMessage::RconExec(cmd) => {
                        if let Some((auth, Some((player_id, _)))) = self
                            .clients
                            .clients
                            .get(con_id)
                            .map(|c| (&c.auth.level, c.players.front()))
                        {
                            if matches!(auth, AuthLevel::Moderator | AuthLevel::Admin) {
                                self.game_server.game.client_command(
                                    player_id,
                                    ClientCommand::Rcon(ClientRconCommand {
                                        raw: cmd,
                                        auth_level: *auth,
                                    }),
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
            let stages = game.all_stages(0.0);
            let player_infos: LinkedHashMap<_, _> = stages
                .iter()
                .flat_map(|s| s.1.world.characters.iter())
                .collect();

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
                    .any(|diff| !matches!(&diff, difference::Difference::Same(_)))
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
                    .any(|diff| !matches!(&diff, difference::Difference::Same(_)))
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

        let master_servers = [
            //"https://master1.ddnet.org/ddnet/15/register",
            "https://pg.ddnet.org:4444/ddnet/15/register",
        ];

        let http = self.io.http.clone();
        let port = self.server_port;

        let mut characters = self.game_server.game.collect_characters_info();
        let register_info = serde_json::to_string(&ServerBrowserInfo {
            name: "TODO: No name yet".into(),
            game_type: "TODO: game type".into(),
            version: "TODO: version".into(),
            map: ServerBrowserInfoMap {
                name: self.game_server.map.name.clone(),
                blake3: self.game_server.map_blake3_hash,
                size: self.game_server.map.map_file.len(),
            },
            players: characters
                .drain()
                .map(|(_, c)| ServerBrowserPlayer {
                    score: "TODO:".to_string(),
                    name: c.info.name.to_string(),
                    country: -1, // TODO:
                })
                .collect::<Vec<_>>(),
            passworded: false, // TODO:
            cert_sha256_fingerprint: self.cert_sha256_fingerprint,
        })
        .unwrap();

        let next_serial = self.last_register_serial + 1;
        let serial = std::mem::replace(&mut self.last_register_serial, next_serial);

        self.register_task = Some(
            self.io
                .io_batcher
                .spawn(async move {
                    for master_server in master_servers {
                        let mut secret: [u8; 32] = Default::default();
                        rand::rngs::OsRng.fill_bytes(&mut secret);
                        let mut challenge_secret: [u8; 32] = Default::default();
                        rand::rngs::OsRng.fill_bytes(&mut challenge_secret);
                        match http
                            .custom_request(
                                master_server.try_into().unwrap(),
                                vec![
                                    (
                                        "Address",
                                        format!(
                                            "ddrs-0.1+quic://connecting-address.invalid:{}",
                                            port
                                        )
                                        .as_str(),
                                    )
                                        .into(),
                                    ("Secret", fmt_hash(&secret).as_str()).into(),
                                    ("Challenge-Secret", fmt_hash(&challenge_secret).as_str())
                                        .into(),
                                    ("Info-Serial", serial.to_string().as_str()).into(),
                                    ("content-type", "application/json").into(),
                                ],
                                Some(register_info.as_bytes().to_vec()),
                            )
                            .await
                            .map_err(|err| anyhow!(err))
                            .and_then(|res| {
                                serde_json::from_slice::<RegisterResponse>(&res)
                                    .map_err(|err| anyhow!(err))
                            })
                            .and_then(|res| match res {
                                RegisterResponse::Success => Ok(()),
                                RegisterResponse::NeedChallenge => {
                                    Err(anyhow!("Challenge is not supported."))
                                }
                                RegisterResponse::NeedInfo => {
                                    Err(anyhow!("Need info is not supported."))
                                }
                                RegisterResponse::Error(err) => Err(anyhow!(err.message)),
                            }) {
                            Ok(_) => {
                                dbg!((master_server, "registered server"));
                                return Ok(());
                            }
                            Err(err) => {
                                dbg!((master_server, err, &register_info));
                            }
                        }
                    }

                    Ok(())
                })
                .abortable(),
        );
    }

    pub fn run(&mut self) {
        let mut cur_time = self.sys.time_get_nanoseconds();
        self.last_tick_time = cur_time;
        self.last_register_time = None;

        let game_event_generator = self.game_event_generator_server.clone();
        while self.is_open.load(std::sync::atomic::Ordering::Relaxed) {
            cur_time = self.sys.time_get_nanoseconds();
            if !self
                .last_register_time
                .is_some_and(|time| cur_time - time <= Duration::from_secs(10))
            {
                self.register();
                self.last_register_time = Some(cur_time);
            }

            if self
                .has_new_events_server
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                let game_ev_gen = &game_event_generator;
                let mut events = game_ev_gen.events.blocking_lock();
                for (con_id, timestamp, event) in events.drain(..) {
                    match event {
                        GameEvents::NetworkEvent(net_ev) => match net_ev {
                            NetworkEvent::Connected {
                                cert,
                                initial_network_stats,
                                addr,
                            } => {
                                log::debug!(target: "server", "connect time sv: {}", timestamp.as_nanos());
                                self.try_client_connect(
                                    &con_id,
                                    &timestamp,
                                    addr.ip(),
                                    cert,
                                    PlayerNetworkStats {
                                        ping: initial_network_stats.ping,
                                        packet_loss: initial_network_stats.packets_lost as f32
                                            / initial_network_stats.packets_sent.clamp(1, u64::MAX)
                                                as f32,
                                    },
                                );
                            }
                            NetworkEvent::Disconnected { reason, graceful } => {
                                log::debug!(target: "server", "got disconnected event from network");
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
                                log::debug!(target: "server", "server ping: {}", stats.ping.as_millis());
                                let network_stats = PlayerNetworkStats {
                                    ping: stats.ping,
                                    packet_loss: stats.packets_lost as f32
                                        / stats.packets_sent.clamp(1, u64::MAX) as f32,
                                };
                                if let Some(client) = self.clients.clients.get_mut(&con_id) {
                                    client.network_stats = network_stats;
                                } else if let Some(client) =
                                    self.clients.network_clients.get_mut(&con_id)
                                {
                                    client.network_stats = network_stats;
                                } else if let Some(client) =
                                    self.clients.network_queued_clients.get_mut(&con_id)
                                {
                                    client.network_stats = network_stats;
                                }
                                // every second
                                let cur_time = self.sys.time_get_nanoseconds();
                                if cur_time - self.last_network_stats_time > Duration::from_secs(1)
                                {
                                    self.last_network_stats_time = cur_time;
                                    let mut player_stats = self.player_network_stats_pool.new();
                                    for client in self.clients.clients.values() {
                                        for player_id in client.players.keys() {
                                            player_stats.insert(*player_id, client.network_stats);
                                        }
                                    }
                                    self.game_server.game.network_stats(player_stats);
                                }
                            }
                            NetworkEvent::ConnectingFailed(_) => {
                                // server usually does not connect, so does not care
                            }
                        },
                        GameEvents::NetworkMsg(game_msg) => {
                            self.handle_msg(&timestamp, &con_id, game_msg)
                        }
                    }
                }
                game_ev_gen
                    .has_events
                    .store(false, std::sync::atomic::Ordering::Relaxed);
            }

            let ticks_in_a_second = self.game_server.game.game_tick_speed();

            // get time before checking ticks
            cur_time = self.sys.time_get_nanoseconds();

            // update vote
            if let Some(vote) = &mut self.game_server.cur_vote {
                // check if vote is over
                if vote.state.yes_votes == vote.state.allowed_to_vote_count
                    || cur_time - vote.started_at > Duration::from_secs(25)
                {
                    let vote = self.game_server.cur_vote.take().unwrap();
                    // fake democracy
                    if vote.state.yes_votes > vote.state.no_votes {
                        match &vote.state.vote {
                            VoteType::Map(map) => {
                                self.load_map(map.name.as_str());
                            }
                            VoteType::VoteKickPlayer { voted_player_id } => {
                                if let ServerExtraVoteInfo::Player {
                                    to_kick_player,
                                    ip,
                                    account_info,
                                } = &vote.extra_vote_info
                                {
                                    // kick that player
                                    let ids = self.connection_bans.ban_ip(*ip);
                                    for id in ids {
                                        self.network.kick(&id);
                                    }
                                    self.network.kick(to_kick_player);
                                }
                            }
                            VoteType::VoteSpecPlayer { voted_player_id } => todo!(),
                            VoteType::Misc() => todo!(),
                        }
                    }

                    self.send_vote(None, Duration::ZERO);
                }
            }

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
                    ticks_in_a_second.get(),
                    &self.shared_info,
                    "server",
                );

                // snap shot building
                for (con_id, client) in &mut self.clients.clients {
                    let mut player_ids = self.player_ids_pool.new();
                    player_ids.extend(client.players.keys());
                    let snap_client = SnapshotClientInfo {
                        client_player_ids: player_ids,
                        snap_everything: false,
                        snap_other_stages: false,
                    };

                    let snap_id = client.snap_id;
                    client.snap_id += 1;

                    if client.snap_id % self.config_game.sv.ticks_per_snapshot == 0 {
                        let mut snap = self.game_server.game.snapshot_for(snap_client);

                        // this should be smaller than the number of snapshots saved on the client
                        let as_diff = if client.client_snap_storage.len() < 10 {
                            client.client_snap_storage.insert(
                                snap_id,
                                ClientSnapshotStorage {
                                    snapshot: snap.to_vec(),
                                    monotonic_tick: self.game_server.cur_monotonic_tick,
                                },
                            );
                            true
                        } else {
                            false
                        };

                        let (snap_diff, diff_id, diff_monotonic_tick) =
                            if let Some(latest_client_snap) = &client.latest_client_snap {
                                let mut new_snap = self.player_snap_pool.new();
                                new_snap.resize(snap.len(), Default::default());
                                new_snap.clone_from_slice(&snap);
                                let snap_vec = snap.to_mut();
                                snap_vec.clear();
                                if bin_patch::diff(
                                    &latest_client_snap.snapshot,
                                    &new_snap,
                                    snap_vec,
                                )
                                .is_ok()
                                {
                                    (
                                        snap,
                                        Some(latest_client_snap.snap_id),
                                        Some(latest_client_snap.monotonic_tick),
                                    )
                                } else {
                                    snap_vec.clear();
                                    snap_vec.append(&mut new_snap);

                                    (snap, None, None)
                                }
                            } else {
                                (snap, None, None)
                            };

                        // quickly rewrite the input ack's logic overhead
                        let cur_time = self.sys.time_get_nanoseconds();
                        client.inputs_to_ack.iter_mut().for_each(|inp| {
                            inp.logic_overhead = cur_time.saturating_sub(inp.logic_overhead);
                        });
                        self.network.send_unordered_auto_to(
                            &GameMessage::ServerToClient(ServerToClientMessage::Snapshot {
                                overhead_time: (self.sys.time_get_nanoseconds()
                                    - self.last_tick_time),
                                snapshot: snap_diff.as_ref().into(),
                                diff_id,
                                snap_id_diffed: diff_id
                                    .map(|diff_id| snap_id - diff_id)
                                    .unwrap_or(snap_id),
                                game_monotonic_tick_diff: diff_monotonic_tick
                                    .map(|diff_monotonic_tick| {
                                        self.game_server.cur_monotonic_tick - diff_monotonic_tick
                                    })
                                    .unwrap_or(self.game_server.cur_monotonic_tick),
                                as_diff,
                                input_ack: client.inputs_to_ack.as_slice().into(),
                            }),
                            con_id,
                        );
                        client.inputs_to_ack.clear();
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
                        self.network.send_in_order_to(
                            &GameMessage::ServerToClient(ServerToClientMessage::Events {
                                game_monotonic_tick: self.game_server.cur_monotonic_tick,
                                events,
                            }),
                            con_id,
                            // If you cannot see "events" in the number 373215, skill issue
                            NetworkInOrderChannel::Custom(373215),
                        );
                    }
                }

                self.game_server.game.clear_events();
            }

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

    fn load_impl(&mut self, snapshot: Option<PoolCow<'static, [u8]>>, map: &str) {
        // reload the whole game server, including the map
        let mod_name = Self::config_mod_name(&self.config_game);
        let config = Self::read_mod_config(&self.io, &mod_name)
            .get_storage()
            .ok();
        self.game_server = ServerGame::new(
            map,
            &mod_name,
            config,
            &self.thread_pool,
            &self.io,
            &self.game_db,
        )
        .unwrap();
        if let Some(snapshot) = snapshot {
            self.game_server
                .game
                .build_from_snapshot_by_hotreload(&snapshot);
        }
        // put all players back to a loading state
        self.clients.clients.drain().for_each(|(net_id, client)| {
            self.clients.network_clients.insert(
                net_id,
                ServerNetworkClient {
                    connect_timestamp: client.connect_timestamp,
                    ip: client.ip,
                    auth: client.auth,
                    network_stats: client.network_stats,
                },
            );
        });
        let server_info = MsgSvServerInfo {
            map: self.game_server.map.name.as_str().try_into().unwrap(),
            map_blake3_hash: self.game_server.map_blake3_hash,
            game_mod: self.game_server.game_mod.clone(),
            hint_start_camera_pos: self.game_server.game.get_client_camera_join_pos(),
            resource_server_fallback: self
                .game_server
                .http_server
                .as_ref()
                .map(|server| server.port),
            mod_config: self.game_server.game.info.config.clone(),
        };
        self.clients.network_clients.keys().for_each(|net_id| {
            self.network.send_unordered_to(
                &GameMessage::ServerToClient(ServerToClientMessage::Load(server_info.clone())),
                net_id,
            );
        });
        self.last_tick_time = self.sys.time_get_nanoseconds();
    }

    fn reload(&mut self) {
        let snapshot = self.game_server.game.snapshot_for_hotreload();
        self.load_impl(snapshot, &self.config_game.sv.map.clone())
    }

    fn load_map(&mut self, map: &str) {
        self.load_impl(None, map)
    }
}

pub fn load_config(game_cfg_path: Option<&Path>) -> (Io, ConfigEngine, ConfigGame) {
    let io = Io::new(
        |rt| Arc::new(FileSystem::new(rt, "org", "", "DDNet", "DDNet-Accounts")),
        Arc::new(HttpClient::new()),
    );

    let config_engine = config_fs::load(&io.clone().into());
    let config_game = if let Some(game_cfg_path) = game_cfg_path {
        game_config_fs::fs::load_in(&io.clone().into(), game_cfg_path)
    } else {
        game_config_fs::fs::load(&io.clone().into())
    };

    (io, config_engine, config_game)
}

pub fn ddnet_server_main<const IS_INTERNAL_SERVER: bool>(
    sys: System,
    cert_and_private_key: (x509_cert::Certificate, SigningKey),
    is_open: Arc<AtomicBool>,
    shared_info: Arc<ServerInfo>,
    game_cfg_path: Option<&Path>,
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

    let (io, config_engine, config_game) = load_config(game_cfg_path);

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
