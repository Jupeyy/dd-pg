use std::{
    collections::{BTreeMap, VecDeque},
    net::SocketAddr,
    rc::Rc,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use base::{
    hash::Hash,
    system::{System, SystemTimeInterface},
};
use base_io::{io::Io, io_batcher::IoBatcherTask};
use binds::binds::{
    gen_local_player_action_hash_map, syn_to_bind, BindActions, BindActionsLocalPlayer,
};
use client_accounts::accounts::Accounts;
use client_demo::{DemoRecorder, DemoRecorderCreateProps};
use client_map::client_map::{ClientMapFile, ClientMapLoading, GameMap};
use client_render_game::render_game::{ObservedPlayer, RenderGameForPlayer};
use client_types::console::{entries_to_parser, ConsoleEntry};
use client_ui::{
    connecting::user_data::{ConnectMode, ConnectModes},
    ingame_menu::server_info::{GameInfo, GameServerInfo},
};
use command_parser::parser::{self, CommandType};
use config::config::ConfigEngine;
use game_config::config::ConfigGame;
use game_interface::{
    events::GameEvents,
    interface::GameStateCreateOptions,
    types::{
        character_info::NetworkCharacterInfo,
        game::GameEntityId,
        input::{CharacterInput, CharacterInputConsumableDiff},
        network_string::NetworkString,
        reduced_ascii_str::ReducedAsciiString,
        resource_key::NetworkResourceKey,
        snapshot::SnapshotLocalPlayers,
        weapons::WeaponType,
    },
    votes::{MapVote, VoteState, Voted},
};
use graphics::graphics::graphics::Graphics;
use graphics_backend::backend::GraphicsBackend;
use hashlink::LinkedHashMap;
use log::info;
use math::math::vector::{luffixed, vec2};
use native::{
    input::binds::{BindKey, MouseExtra},
    native::{KeyCode, MouseButton, PhysicalKey},
};
use network::network::{
    network::{NetworkClientCertCheckMode, NetworkClientCertMode, NetworkClientInitOptions},
    packet_compressor::DefaultNetworkPacketCompressor,
    plugins::{NetworkPluginPacket, NetworkPlugins},
    quinn_network::QuinnNetwork,
};
use pool::{
    datatypes::{PoolVecDeque, StringPool},
    mt_pool::Pool as MtPool,
    pool::Pool,
    rc::PoolRc,
};
use shared_base::{
    network::{
        messages::{
            GameModification, MsgClInputPlayerChain, MsgClReady, MsgClSnapshotAck,
            PlayerInputChainable,
        },
        server_info::ServerInfo,
        types::chat::NetChatMsg,
    },
    player_input::PlayerInput,
};
use shared_network::{
    game_event_generator::GameEventGenerator,
    messages::{ClientToServerMessage, GameMessage, ServerToClientMessage},
};
use sound::sound::SoundManager;
use ui_base::{font_data::UiFontData, types::UiState};
use url::Url;

use crate::localplayer::{ClientPlayer, LocalPlayers};

use super::{
    client::ClientPlayerInputPerTick,
    component::GameMsgPipeline,
    components::{network_logic::NetworkLogic, prediction_timing::PredictionTiming},
    input::input_handling::DeviceToLocalPlayerIndex,
};

#[derive(Debug, Default)]
pub struct NetworkByteStats {
    pub last_timestamp: Duration,
    pub last_bytes_sent: u64,
    pub last_bytes_recv: u64,
    pub bytes_per_sec_sent: luffixed,
    pub bytes_per_sec_recv: luffixed,
}

#[derive(Debug)]
pub struct SnapshotStorageItem {
    pub snapshot: Vec<u8>,
    pub monotonic_tick: u64,
}

pub struct GameData {
    pub local_players: LocalPlayers,

    /// Snapshot that still has to be acknowledged.
    pub snap_acks: Vec<MsgClSnapshotAck>,

    pub device_to_local_player_index: DeviceToLocalPlayerIndex, // TODO: keyboard and mouse are different devices
    pub input_per_tick: ClientPlayerInputPerTick,

    /// This is only used to make sure old snapshots are not handled.
    pub handled_snap_id: Option<u64>,

    /// Ever increasing id for sending input packages.
    pub input_id: u64,

    /// last (few) snapshot diffs & id client used
    pub snap_storage: BTreeMap<u64, SnapshotStorageItem>,

    /// A tracker of sent inputs and their time
    /// used to evaluate the estimated RTT/ping.
    pub sent_input_ids: BTreeMap<u64, Duration>,

    pub prediction_timing: PredictionTiming,
    pub net_byte_stats: NetworkByteStats,

    pub last_game_tick: Duration,
    pub last_frame_time: Duration,
    pub intra_tick_time: Duration,

    pub chat_msgs_pool: Pool<VecDeque<NetChatMsg>>,
    pub chat_msgs: PoolVecDeque<NetChatMsg>,
    pub player_inp_pool: Pool<LinkedHashMap<GameEntityId, PlayerInput>>,
    pub player_snap_pool: Pool<Vec<u8>>,

    /// current vote in the game and the network timestamp when it arrived
    pub vote: Option<(PoolRc<VoteState>, Option<Voted>, Duration)>,

    pub map_votes: Vec<MapVote>,
}

impl GameData {
    fn new(cur_time: Duration, prediction_timing: PredictionTiming) -> Self {
        let chat_and_system_msgs_pool = Pool::with_capacity(2);
        Self {
            local_players: LocalPlayers::new(),

            snap_acks: Vec::with_capacity(16),

            input_id: 0,

            snap_storage: Default::default(),

            device_to_local_player_index: Default::default(),
            input_per_tick: Default::default(),

            sent_input_ids: Default::default(),

            handled_snap_id: None,
            prediction_timing,
            net_byte_stats: Default::default(),

            last_game_tick: cur_time,
            intra_tick_time: Duration::ZERO,
            last_frame_time: cur_time,

            chat_msgs: chat_and_system_msgs_pool.new(),
            chat_msgs_pool: chat_and_system_msgs_pool,
            player_inp_pool: Pool::with_capacity(64),
            player_snap_pool: Pool::with_capacity(2),

            vote: None,
            map_votes: Default::default(),
        }
    }
}

impl GameData {
    pub fn handle_local_players_from_snapshot(
        &mut self,
        config: &ConfigGame,
        console_entries: &[ConsoleEntry],
        local_players: SnapshotLocalPlayers,
    ) {
        self.local_players
            .retain_with_order(|player_id, _| local_players.contains_key(player_id));
        local_players.iter().for_each(|(id, snap_player)| {
            if !self.local_players.contains_key(id) {
                let mut local_player: ClientPlayer = ClientPlayer {
                    is_dummy: snap_player.is_dummy,
                    ..Default::default()
                };
                let binds = &mut local_player.binds;

                let map = gen_local_player_action_hash_map();
                for bind in &config.players[0].binds {
                    let cmds = parser::parse(bind, &entries_to_parser(console_entries));
                    for cmd in &cmds {
                        if let CommandType::Full(cmd) = cmd {
                            let (keys, actions) = syn_to_bind(&cmd.args, &map).unwrap();

                            binds.register_bind(&keys, actions);
                        }
                    }
                }

                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::KeyA))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::MoveLeft)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::KeyD))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::MoveRight)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Space))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Jump)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Escape))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::OpenMenu)],
                );
                binds.register_bind(
                    &[BindKey::Mouse(MouseButton::Left)],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Fire)],
                );
                binds.register_bind(
                    &[BindKey::Mouse(MouseButton::Right)],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Hook)],
                );
                binds.register_bind(
                    &[BindKey::Extra(MouseExtra::WheelDown)],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::PrevWeapon)],
                );
                binds.register_bind(
                    &[BindKey::Extra(MouseExtra::WheelUp)],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::NextWeapon)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Digit1))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Weapon(
                        WeaponType::Hammer,
                    ))],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Digit2))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Weapon(
                        WeaponType::Gun,
                    ))],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Digit3))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Weapon(
                        WeaponType::Shotgun,
                    ))],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Digit4))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Weapon(
                        WeaponType::Grenade,
                    ))],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Digit5))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Weapon(
                        WeaponType::Laser,
                    ))],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::KeyG))],
                    vec![BindActions::LocalPlayer(
                        BindActionsLocalPlayer::ToggleDummyCopyMoves,
                    )],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Enter))],
                    vec![BindActions::LocalPlayer(
                        BindActionsLocalPlayer::ActivateChatInput,
                    )],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::KeyT))],
                    vec![BindActions::LocalPlayer(
                        BindActionsLocalPlayer::ActivateChatInput,
                    )],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Tab))],
                    vec![BindActions::LocalPlayer(
                        BindActionsLocalPlayer::ShowScoreboard,
                    )],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::KeyU))],
                    vec![BindActions::LocalPlayer(
                        BindActionsLocalPlayer::ShowChatHistory,
                    )],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::ShiftLeft))],
                    vec![BindActions::LocalPlayer(
                        BindActionsLocalPlayer::ShowEmoteWheel,
                    )],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::KeyQ))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Kill)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::F3))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::VoteYes)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::F4))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::VoteNo)],
                );
                self.local_players.insert(*id, local_player);
            }
            // sort
            self.local_players.to_back(id);
        });
    }
}

pub struct ActiveGame {
    pub network_logic: NetworkLogic,
    pub network: QuinnNetwork,
    pub game_event_generator_client: Arc<GameEventGenerator>,
    pub has_new_events_client: Arc<AtomicBool>,

    pub map: GameMap,
    pub demo_recorder: Option<DemoRecorder>,

    pub demo_recorder_props: DemoRecorderCreateProps,

    pub game_data: GameData,

    pub events: Option<GameEvents>,

    pub map_votes_loaded: bool,

    pub render_players_pool: Pool<Vec<RenderGameForPlayer>>,
    pub pred_player_inputs_pool:
        Pool<LinkedHashMap<GameEntityId, (CharacterInput, CharacterInputConsumableDiff)>>,
    pub render_observers_pool: Pool<Vec<ObservedPlayer>>,

    pub player_inputs_pool: MtPool<LinkedHashMap<GameEntityId, PlayerInputChainable>>,
    pub player_inputs_chain_pool: MtPool<LinkedHashMap<GameEntityId, MsgClInputPlayerChain>>,
    pub player_inputs_chain_data_pool: MtPool<Vec<u8>>,
    pub player_inputs_ser_helper_pool: Pool<Vec<u8>>,

    addr: SocketAddr,
}

pub struct PrepareConnectGame {
    connect_info: ConnectMode,
    cert: Vec<u8>,
    addr: SocketAddr,
    task: Option<IoBatcherTask<NetworkClientCertMode>>,
    dicts_task: IoBatcherTask<(Vec<u8>, Vec<u8>)>,
}

pub struct ConnectingGame {
    pub network: QuinnNetwork,
    pub game_event_generator_client: Arc<GameEventGenerator>,
    pub has_new_events_client: Arc<AtomicBool>,
    pub connect_info: ConnectMode,
    server_connect_time: Duration,
    addr: SocketAddr,
}

pub struct LoadingGame {
    pub network: QuinnNetwork,
    pub game_event_generator_client: Arc<GameEventGenerator>,
    pub has_new_events_client: Arc<AtomicBool>,
    map: ClientMapLoading,
    ping: Duration,
    prediction_timing: PredictionTiming,
    hint_start_camera_pos: vec2,
    addr: SocketAddr,
    pub demo_recorder_props: DemoRecorderCreateProps,
}

pub enum Game {
    /// the game is currently inactive, e.g. if the client
    /// is still in the main menu
    None,
    /// prepare to connect to a server
    /// e.g. load private key or whatever
    PrepareConnect(PrepareConnectGame),
    /// the game is connecting
    Connecting(ConnectingGame),
    /// the game is loading
    Loading(LoadingGame),
    WaitingForFirstSnapshot(Box<ActiveGame>),
    Active(Box<ActiveGame>),
}

impl Game {
    pub fn new(
        io: &Io,
        connect_info: &ConnectMode,
        cert: &[u8],
        addr: SocketAddr,
        accounts: &Arc<Accounts>,
    ) -> anyhow::Result<Self> {
        let accounts = accounts.clone();
        let task = io.io_batcher.spawn(async move {
            let (game_key, cert, _) = accounts.connect_to_game_server().await;
            Ok(NetworkClientCertMode::FromCertAndPrivateKey {
                cert,
                private_key: game_key.private_key,
            })
        });

        let fs = io.fs.clone();
        let zstd_dicts = io.io_batcher.spawn(async move {
            let client_send = fs.read_file("dict/client_send".as_ref()).await;
            let server_send = fs.read_file("dict/server_send".as_ref()).await;

            Ok(client_send.and_then(|c| server_send.map(|s| (c, s)))?)
        });

        Ok(Self::PrepareConnect(PrepareConnectGame {
            connect_info: connect_info.clone(),
            cert: cert.to_owned(),
            addr,
            task: Some(task),
            dicts_task: zstd_dicts,
        }))
    }

    fn connect(
        connect_info: &ConnectMode,
        sys: &System,
        server_cert: &Vec<u8>,
        config: &ConfigEngine,
        addr: SocketAddr,
        cert: NetworkClientCertMode,
        dicts: Option<(Vec<u8>, Vec<u8>)>,
    ) -> Self {
        let has_new_events_client = Arc::new(AtomicBool::new(false));
        let game_event_generator_client = Arc::new(GameEventGenerator::new(
            has_new_events_client.clone(),
            sys.time.clone(),
        ));

        let mut packet_plugins: Vec<Arc<dyn NetworkPluginPacket>> = vec![];

        if let Some((client_send, server_send)) = dicts {
            packet_plugins.push(Arc::new(DefaultNetworkPacketCompressor::new_with_dict(
                client_send,
                server_send,
            )));
        } else {
            packet_plugins.push(Arc::new(DefaultNetworkPacketCompressor::new()));
        }

        let (network_client, _game_event_notifier) = QuinnNetwork::init_client(
            "0.0.0.0:0",
            game_event_generator_client.clone(),
            sys,
            NetworkClientInitOptions::new(
                if config.dbg.untrusted_cert {
                    NetworkClientCertCheckMode::DisableCheck
                } else {
                    NetworkClientCertCheckMode::CheckByCert {
                        cert: server_cert.into(),
                    }
                },
                cert,
            )
            .with_timeout(config.net.timeout)
            // since there are many packets, increase loss detection thresholds
            .with_loss_detection_cfg(25, 2.0)
            .with_ack_config(25, Duration::from_secs(1), 25 - 1),
            NetworkPlugins {
                packet_plugins: Arc::new(packet_plugins),
                connection_plugins: Default::default(),
            },
            &addr.to_string(),
        );

        Self::Connecting(ConnectingGame {
            network: network_client,
            game_event_generator_client,
            has_new_events_client,
            connect_info: connect_info.clone(),
            server_connect_time: sys.time_get_nanoseconds(),
            addr,
        })
    }

    fn load(
        network: QuinnNetwork,
        game_event_generator_client: Arc<GameEventGenerator>,
        has_new_events_client: Arc<AtomicBool>,
        tp: &Arc<rayon::ThreadPool>,
        io: &Io,
        map: &ReducedAsciiString,
        map_blake3_hash: &Hash,
        game_mod: GameModification,
        resource_download_server: Option<Url>,
        timestamp: Duration,
        server_connect_time: Duration,
        hint_start_camera_pos: vec2,
        ui: &mut UiState,
        config: &mut ConfigEngine,
        addr: SocketAddr,
        game_options: GameStateCreateOptions,
    ) -> Self {
        info!("loading map: {}", map.as_str());
        let ping = timestamp.saturating_sub(server_connect_time);

        ui.is_ui_open = false;
        config.ui.path.route("ingame");

        let demo_recorder_props = DemoRecorderCreateProps {
            map: map.clone(),
            map_hash: *map_blake3_hash,
            game_options: game_options.clone(),
            required_resources: Default::default(), /* TODO: */
            physics_module: game_mod.clone(),
            render_module: GameModification::Native,
            io: io.clone(),
        };
        Self::Loading(LoadingGame {
            network,
            game_event_generator_client,
            has_new_events_client,
            map: ClientMapLoading::new(
                "map/maps".as_ref(),
                map,
                Some(*map_blake3_hash),
                resource_download_server,
                io,
                tp,
                game_mod,
                false,
                game_options,
            ),
            ping,
            prediction_timing: PredictionTiming::new(ping, timestamp),
            hint_start_camera_pos,
            addr,
            demo_recorder_props,
        })
    }

    pub fn update(
        &mut self,
        graphics: &mut Graphics,
        graphics_backend: &Rc<GraphicsBackend>,
        sound: &SoundManager,
        config: &ConfigEngine,
        config_game: &ConfigGame,
        sys: &System,
        fonts: &Arc<UiFontData>,
    ) {
        let mut selfi = Self::None;
        std::mem::swap(&mut selfi, self);
        *self = match selfi {
            Game::None | Game::Active(_) | Game::WaitingForFirstSnapshot(_) => {
                // nothing to do
                selfi
            }
            Game::Connecting(game) => Self::Connecting(game),
            Game::PrepareConnect(PrepareConnectGame {
                connect_info,
                cert,
                addr,
                task,
                dicts_task,
            }) => Self::connect(
                &connect_info,
                sys,
                &cert,
                config,
                addr,
                task.map(|task| task.get_storage().unwrap()).unwrap(),
                dicts_task.get_storage().ok(),
            ),
            Game::Loading(LoadingGame {
                network,
                game_event_generator_client,
                has_new_events_client,
                mut map,
                ping,
                prediction_timing,
                hint_start_camera_pos,
                addr,
                demo_recorder_props,
            }) => {
                if map.is_fully_loaded() {
                    let mut player_info = NetworkCharacterInfo::explicit_default();
                    if let Some(p) = config_game.players.first() {
                        player_info.name = NetworkString::new(&p.name).unwrap();
                        player_info.clan = NetworkString::new(&p.clan).unwrap();
                        player_info.skin = NetworkResourceKey::from_str_lossy(&p.skin.name);
                    }
                    network.send_unordered_to_server(&GameMessage::ClientToServer(
                        ClientToServerMessage::Ready(MsgClReady { player_info }),
                    ));
                    let ClientMapLoading::Map(ClientMapFile::Game(map)) = map else {
                        panic!("remove this in future.")
                    };

                    let demo_recorder =
                        DemoRecorder::new(demo_recorder_props.clone(), map.game.game_tick_speed());

                    Self::WaitingForFirstSnapshot(Box::new(ActiveGame {
                        network_logic: NetworkLogic::new(),
                        network,
                        game_event_generator_client,
                        has_new_events_client,
                        map,
                        demo_recorder: Some(demo_recorder),
                        demo_recorder_props,
                        game_data: GameData::new(sys.time_get_nanoseconds(), prediction_timing),

                        events: None,
                        map_votes_loaded: Default::default(),

                        render_players_pool: Pool::with_capacity(64),
                        pred_player_inputs_pool: Pool::with_capacity(2),
                        render_observers_pool: Pool::with_capacity(2),

                        player_inputs_pool: MtPool::with_capacity(4),
                        player_inputs_chain_pool: MtPool::with_capacity(4),
                        player_inputs_chain_data_pool: MtPool::with_capacity(4),
                        player_inputs_ser_helper_pool: Pool::with_capacity(4),

                        addr,
                    }))
                } else {
                    map.continue_loading(sound, graphics, graphics_backend, config, sys, fonts);
                    Self::Loading(LoadingGame {
                        network,
                        game_event_generator_client,
                        has_new_events_client,
                        map,
                        ping,
                        prediction_timing,
                        hint_start_camera_pos,
                        addr,
                        demo_recorder_props,
                    })
                }
            }
        }
    }

    pub fn on_msg(
        &mut self,
        timestamp: Duration,
        msg: ServerToClientMessage<'static>,
        sys: &System,
        tp: &Arc<rayon::ThreadPool>,
        io: &Io,
        ui: &mut UiState,
        config: &mut ConfigEngine,
        config_game: &mut ConfigGame,
        shared_info: &Arc<ServerInfo>,
        string_pool: &StringPool,
        console_entries: &Vec<ConsoleEntry>,
        game_server_info: &GameServerInfo,
    ) {
        let mut selfi = Self::None;
        std::mem::swap(&mut selfi, self);
        let mut is_waiting = matches!(&selfi, Game::WaitingForFirstSnapshot(_));
        match selfi {
            Game::None => {}
            Game::PrepareConnect(game) => {
                *self = Self::PrepareConnect(game);
            }
            Game::Connecting(connecting) => match msg {
                ServerToClientMessage::ServerInfo { info, overhead } => {
                    game_server_info.fill_game_info(GameInfo {
                        map_name: info.map.to_string(),
                    });
                    *self = Self::load(
                        connecting.network,
                        connecting.game_event_generator_client,
                        connecting.has_new_events_client,
                        tp,
                        io,
                        &info.map,
                        &info.map_blake3_hash,
                        info.game_mod,
                        info.resource_server_fallback.map(|port| {
                            Url::try_from(
                                format!("http://{}:{}", connecting.addr.ip(), port).as_str(),
                            )
                            .unwrap()
                        }),
                        timestamp.saturating_sub(overhead),
                        connecting.server_connect_time,
                        info.hint_start_camera_pos,
                        ui,
                        config,
                        connecting.addr,
                        GameStateCreateOptions {
                            hint_max_characters: None, // TODO: get from server
                            config: info.mod_config,
                        },
                    );
                }
                ServerToClientMessage::QueueInfo(info) => {
                    connecting
                        .connect_info
                        .set(ConnectModes::Queue { msg: info });
                    config.ui.path.route("connecting");
                    *self = Self::Connecting(connecting);
                }
                _ => {
                    // collect msgs
                    *self = Self::Connecting(connecting);
                }
            },
            Game::Loading(loading) => {
                *self = Self::Loading(loading);
            }
            Game::WaitingForFirstSnapshot(mut game) | Game::Active(mut game) => {
                if let ServerToClientMessage::Load(info) = msg {
                    *self = Self::load(
                        game.network,
                        game.game_event_generator_client,
                        game.has_new_events_client,
                        tp,
                        io,
                        &info.map,
                        &info.map_blake3_hash,
                        info.game_mod,
                        info.resource_server_fallback.map(|port| {
                            format!("http://{}:{}", game.addr.ip(), port)
                                .as_str()
                                .try_into()
                                .unwrap()
                        }),
                        timestamp,
                        timestamp.saturating_sub(game.game_data.prediction_timing.ping_max()),
                        info.hint_start_camera_pos,
                        ui,
                        config,
                        game.addr,
                        GameStateCreateOptions {
                            hint_max_characters: None, // TODO: get from server
                            config: info.mod_config,
                        },
                    );
                } else {
                    if let ServerToClientMessage::Snapshot { overhead_time, .. } = &msg {
                        if is_waiting {
                            // set the first ping based on the intial packets,
                            // later prefer the network stats
                            let last_game_tick = sys.time_get_nanoseconds()
                                - *overhead_time
                                - game.game_data.prediction_timing.pred_max_smooth();
                            game.game_data.last_game_tick = last_game_tick;
                            is_waiting = false;
                        }
                    }
                    game.network_logic.on_msg(
                        &timestamp,
                        msg,
                        &mut GameMsgPipeline {
                            demo_recorder: &mut game.demo_recorder,
                            network: &mut game.network,
                            runtime_thread_pool: tp,
                            io,
                            map: &mut game.map,
                            game_data: &mut game.game_data,
                            events: &mut game.events,
                            config,
                            config_game,
                            shared_info,
                            ui,
                            sys,
                            string_pool,
                            console_entries,
                        },
                    );

                    if is_waiting {
                        *self = Self::WaitingForFirstSnapshot(game);
                    } else {
                        *self = Self::Active(game);
                    }
                }
            }
        }
    }
}
