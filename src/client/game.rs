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
use client_accounts::accounts::{Accounts, GameServerMode};
use client_demo::DemoRecorder;
use client_map::client_map::{ClientMapFile, ClientMapLoading, GameMap};
use client_render_base::map::render_pipe::Camera;
use client_render_game::render_game::RenderGameForPlayer;
use client_types::console::{
    entries_to_parser,
    parser::{self, CommandType},
    ConsoleEntry,
};
use client_ui::connecting::user_data::{ConnectMode, ConnectModes};
use config::config::ConfigEngine;
use game_config::config::ConfigGame;
use game_interface::{
    events::{GameEvents, FIRST_EVENT_ID},
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
};
use graphics::graphics::graphics::Graphics;
use graphics_backend::backend::GraphicsBackend;
use hashlink::LinkedHashMap;
use log::info;
use math::math::vector::vec2;
use native::input::binds::{BindKey, MouseExtra};
use network::network::{
    network::{NetworkClientCertCheckMode, NetworkClientCertMode, NetworkClientInitOptions},
    packet_compressor::NetworkPacketCompressor,
    quinn_network::QuinnNetwork,
};
use pool::{
    datatypes::{PoolVecDeque, StringPool},
    mt_datatypes::PoolLinkedHashMap,
    pool::Pool,
};
use shared_base::{
    network::{messages::MsgClReady, server_info::ServerInfo, types::chat::NetChatMsg},
    player_input::PlayerInput,
};
use shared_network::{
    game_event_generator::GameEventGenerator,
    messages::{ClientToServerMessage, GameMessage, ServerToClientMessage},
};
use sound::sound::SoundManager;
use ui_base::types::UiState;
use url::Url;
use winit::{
    event::MouseButton,
    keyboard::{KeyCode, PhysicalKey},
};

use crate::localplayer::{ClientPlayer, LocalPlayers};

use super::{
    client::ClientPlayerInputPerTick,
    component::GameMsgPipeline,
    components::{network_logic::NetworkLogic, prediction_timing2::PredictionTiming},
    input::input_handling::DeviceToLocalPlayerIndex,
};

pub struct GameData {
    // the ping between the client and the server
    pub ping: Duration,

    pub local_players: LocalPlayers,

    /// last snapshot the client knows about
    pub client_snap_id: Option<u64>,

    pub device_to_local_player_index: DeviceToLocalPlayerIndex, // TODO: keyboard and mouse are different devices
    pub input_per_tick: ClientPlayerInputPerTick,

    pub handled_snap_id: Option<u64>,
    pub prev_snapshots: BTreeMap<u64, Vec<u8>>,

    pub prediction_timing: PredictionTiming,

    pub last_game_tick: Duration,
    pub last_frame_time: Duration,
    pub intra_tick_time: Duration,

    pub chat_msgs_pool: Pool<VecDeque<NetChatMsg>>,
    pub chat_msgs: PoolVecDeque<NetChatMsg>,
    pub player_inp_pool: Pool<LinkedHashMap<GameEntityId, PlayerInput>>,
    pub player_snap_pool: Pool<Vec<u8>>,
}

impl GameData {
    fn new(cur_time: Duration, prediction_timing: PredictionTiming) -> Self {
        let chat_and_system_msgs_pool = Pool::with_capacity(2);
        Self {
            ping: Duration::default(),

            local_players: LocalPlayers::new(),

            client_snap_id: None,
            prev_snapshots: Default::default(),

            device_to_local_player_index: Default::default(),
            input_per_tick: Default::default(),

            handled_snap_id: None,
            prediction_timing,

            last_game_tick: cur_time,
            intra_tick_time: Duration::ZERO,
            last_frame_time: cur_time,

            chat_msgs: chat_and_system_msgs_pool.new(),
            chat_msgs_pool: chat_and_system_msgs_pool,
            player_inp_pool: Pool::with_capacity(64),
            player_snap_pool: Pool::with_capacity(2),
        }
    }
}

impl GameData {
    pub fn handle_local_players_from_snapshot(
        &mut self,
        config: &ConfigGame,
        console_entries: &Vec<ConsoleEntry>,
        local_players: SnapshotLocalPlayers,
    ) {
        self.local_players.retain_with_order(|player_id, _| {
            if !local_players.contains_key(player_id) {
                false
            } else {
                true
            }
        });
        local_players.iter().for_each(|(id, snap_player)| {
            if !self.local_players.contains_key(&id) {
                let mut local_player: ClientPlayer = Default::default();
                local_player.is_dummy = snap_player.is_dummy;
                let binds = &mut local_player.binds;

                let map = gen_local_player_action_hash_map();
                for bind in &config.players[0].binds {
                    let cmds = parser::parse(&bind, entries_to_parser(console_entries));
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
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::KeyQ))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Kill)],
                );
                self.local_players.insert(id.clone(), local_player);
            }
            // sort
            self.local_players.to_back(&id);
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

    pub client_data: GameData,

    pub events: GameEvents,

    pub render_players_pool: Pool<Vec<RenderGameForPlayer>>,
    pub player_inputs_pool:
        Pool<LinkedHashMap<GameEntityId, (CharacterInput, CharacterInputConsumableDiff)>>,

    addr: SocketAddr,
}

pub struct PrepareConnectGame {
    connect_info: ConnectMode,
    cert: Vec<u8>,
    addr: SocketAddr,
    task: Option<IoBatcherTask<NetworkClientCertMode>>,
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
        cert: &Vec<u8>,
        addr: SocketAddr,
        accounts: &Arc<Accounts>,
    ) -> anyhow::Result<Self> {
        let accounts = accounts.clone();
        let ip = addr.ip();
        let task = io.io_batcher.spawn(async move {
            let game_key = accounts.connect_game_server(GameServerMode::Ip(ip)).await?;
            Ok(NetworkClientCertMode::FromCertifiedKeyPair {
                cert: Accounts::generate_self_signed(game_key)?,
            })
        });

        Ok(Self::PrepareConnect(PrepareConnectGame {
            connect_info: connect_info.clone(),
            cert: cert.clone(),
            addr,
            task: Some(task),
        }))
    }

    fn connect(
        connect_info: &ConnectMode,
        sys: &System,
        server_cert: &Vec<u8>,
        config: &ConfigEngine,
        addr: SocketAddr,
        cert: NetworkClientCertMode,
    ) -> Self {
        let has_new_events_client = Arc::new(AtomicBool::new(false));
        let game_event_generator_client = Arc::new(GameEventGenerator::new(
            has_new_events_client.clone(),
            sys.time.clone(),
        ));
        let (mut network_client, _game_event_notifier) = QuinnNetwork::init_client(
            "0.0.0.0:0",
            game_event_generator_client.clone(),
            sys,
            NetworkClientInitOptions::new(
                if config.dbg.untrusted_cert {
                    NetworkClientCertCheckMode::DisableCheck
                } else {
                    NetworkClientCertCheckMode::CheckByCert { cert: &server_cert }
                },
                cert,
            )
            .with_timeout(config.net.timeout),
            Arc::new(vec![Arc::new(NetworkPacketCompressor::new())]),
            Default::default(),
        );
        network_client.connect(&addr.to_string());

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
        game_mod: ReducedAsciiString,
        game_mod_blake3_hash: Option<Hash>,
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
        let ping = timestamp - server_connect_time;

        ui.is_ui_open = false;
        config.ui.path.route("ingame");

        Self::Loading(LoadingGame {
            network,
            game_event_generator_client,
            has_new_events_client,
            map: ClientMapLoading::new(
                "map/maps".as_ref(),
                map,
                Some((*map_blake3_hash).into()),
                resource_download_server,
                io,
                tp,
                &game_mod,
                game_mod_blake3_hash,
                false,
                game_options,
            ),
            ping,
            prediction_timing: PredictionTiming::new(ping, timestamp),
            hint_start_camera_pos,
            addr,
        })
    }

    pub fn update(
        &mut self,
        graphics: &mut Graphics,
        graphics_backend: &Rc<GraphicsBackend>,
        sound: &SoundManager,
        config: &ConfigEngine,
        config_game: &ConfigGame,
        cam: &mut Camera,
        sys: &System,
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
            }) => Self::connect(
                &connect_info,
                sys,
                &cert,
                config,
                addr,
                task.map(|task| task.get_storage().ok()).flatten().unwrap(),
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
            }) => {
                if map.is_fully_loaded() {
                    let mut player_info = NetworkCharacterInfo::explicit_default();
                    if let Some(p) = config_game.players.get(0) {
                        player_info.name = NetworkString::new(&p.name).unwrap();
                        player_info.clan = NetworkString::new(&p.clan).unwrap();
                        player_info.skin = NetworkResourceKey::from_str_lossy(&p.skin.name);
                    }
                    // TODO: *pipe.demo_recorder = Some(DemoRecorder::new("ctf1", 50, pipe.io));
                    network.send_unordered_to_server(&GameMessage::ClientToServer(
                        ClientToServerMessage::Ready(MsgClReady { player_info }),
                    ));
                    cam.pos = hint_start_camera_pos;
                    let ClientMapLoading::Map(ClientMapFile::Game(map)) = map else {
                        panic!("remove this in future.")
                    };
                    Self::WaitingForFirstSnapshot(Box::new(ActiveGame {
                        network_logic: NetworkLogic::new(),
                        network,
                        game_event_generator_client,
                        has_new_events_client,
                        map,
                        demo_recorder: None,
                        client_data: GameData::new(sys.time_get_nanoseconds(), prediction_timing),

                        events: GameEvents {
                            worlds: PoolLinkedHashMap::new_without_pool(),
                            event_id: FIRST_EVENT_ID,
                        },

                        render_players_pool: Pool::with_capacity(64),
                        player_inputs_pool: Pool::with_capacity(2),
                        addr,
                    }))
                } else {
                    map.continue_loading(sound, graphics, graphics_backend, config, &sys);
                    Self::Loading(LoadingGame {
                        network,
                        game_event_generator_client,
                        has_new_events_client,
                        map,
                        ping,
                        prediction_timing,
                        hint_start_camera_pos,
                        addr,
                    })
                }
            }
        }
    }

    pub fn on_msg(
        &mut self,
        timestamp: Duration,
        msg: ServerToClientMessage,
        sys: &System,
        tp: &Arc<rayon::ThreadPool>,
        io: &Io,
        ui: &mut UiState,
        config: &mut ConfigEngine,
        config_game: &mut ConfigGame,
        shared_info: &Arc<ServerInfo>,
        string_pool: &StringPool,
        console_entries: &Vec<ConsoleEntry>,
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
                ServerToClientMessage::ServerInfo { info } => {
                    *self = Self::load(
                        connecting.network,
                        connecting.game_event_generator_client,
                        connecting.has_new_events_client,
                        tp,
                        io,
                        &info.map,
                        &info.map_blake3_hash,
                        info.game_mod.into(),
                        info.game_mod_blake3_hash,
                        info.resource_server_fallback.map(|port| {
                            Url::try_from(
                                format!("http://{}:{}", connecting.addr.ip(), port).as_str(),
                            )
                            .unwrap()
                        }),
                        timestamp,
                        connecting.server_connect_time,
                        info.hint_start_camera_pos,
                        ui,
                        config,
                        connecting.addr,
                        GameStateCreateOptions {
                            hint_max_characters: None, // TODO: get from server
                            game_type: info.game_type.as_str().to_string(),
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
                        info.game_mod.into(),
                        info.game_mod_blake3_hash,
                        info.resource_server_fallback.map(|port| {
                            format!("http://{}:{}", game.addr.ip(), port)
                                .as_str()
                                .try_into()
                                .unwrap()
                        }),
                        timestamp,
                        timestamp,
                        info.hint_start_camera_pos,
                        ui,
                        config,
                        game.addr,
                        GameStateCreateOptions {
                            hint_max_characters: None, // TODO: get from server
                            game_type: info.game_type.as_str().to_string(),
                        },
                    );
                } else {
                    if let ServerToClientMessage::Snapshot { overhead_time, .. } = &msg {
                        if is_waiting {
                            // set the first ping based on the intial packets,
                            // later prefer the network stats
                            let last_game_tick = sys.time_get_nanoseconds()
                                - *overhead_time
                                - game.client_data.prediction_timing.pred_max_smooth();
                            game.client_data.last_game_tick = last_game_tick;
                            is_waiting = false;
                        }
                    }

                    game.network_logic.on_msg(
                        &timestamp,
                        msg,
                        &mut GameMsgPipeline {
                            demo_recorder: &mut game.demo_recorder,
                            network: &mut game.network,
                            runtime_thread_pool: &tp,
                            io,
                            map: &mut game.map,
                            client_data: &mut game.client_data,
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
