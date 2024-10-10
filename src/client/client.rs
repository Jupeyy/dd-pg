use std::{num::NonZeroUsize, rc::Rc, sync::Arc, time::Duration};

use base::{
    benchmark::Benchmark,
    hash::fmt_hash,
    system::{System, SystemTimeInterface},
};
use base_fs::filesys::FileSystem;

use base_http::http::HttpClient;
use base_io::io::{Io, IoFileSys};
use binds::binds::BindActionsHotkey;
use client_accounts::accounts::{Accounts, AccountsLoading};
use client_console::console::{
    console::ConsoleRenderPipe,
    local_console::{LocalConsole, LocalConsoleBuilder, LocalConsoleEvent},
    remote_console::RemoteConsoleEvent,
};
use client_containers::entities::{EntitiesContainer, ENTITIES_CONTAINER_PATH};
use client_demo::{DemoVideoEncodeProperties, DemoViewer, EncoderSettings};
use client_map::client_map::{ClientMapFile, ClientMapLoading, GameMap};
use client_render_base::map::{
    map::RenderMap,
    map_pipeline::MapPipeline,
    render_pipe::{Camera, GameTimeInfo, RenderPipeline, RenderPipelineBase},
};
use client_render_game::render_game::{
    ObservedAnchoredSize, ObservedPlayer, PlayerFeedbackEvent, RenderForPlayer,
    RenderGameCreateOptions, RenderGameForPlayer, RenderGameInput, RenderGameInterface,
    RenderGameSettings, RenderPlayerCameraMode,
};
use client_ui::{
    chat::user_data::ChatEvent,
    client_info::ClientInfo,
    connect::{
        page::ConnectingUi,
        user_data::{ConnectMode, ConnectModes},
    },
    events::{UiEvent, UiEvents},
    ingame_menu::{
        account_info::AccountInfo, page::IngameMenuUi, server_info::GameServerInfo,
        server_players::ServerPlayers, votes::Votes,
    },
    main_menu::{
        monitors::{UiMonitor, UiMonitorVideoMode, UiMonitors},
        page::MainMenuUi,
        player_settings_ntfy::PlayerSettingsSync,
        spatial_chat::SpatialChat,
    },
};
use config::config::{ConfigEngine, ConfigMonitor};
use demo::recorder::DemoRecorder;
use editor::editor::{EditorInterface, EditorResult};
use egui::CursorIcon;
use game_config::config::{Config, ConfigGame, ConfigMap};
use graphics::graphics::graphics::Graphics;
use graphics_backend::{
    backend::{
        GraphicsBackend, GraphicsBackendBase, GraphicsBackendIoLoading, GraphicsBackendLoading,
    },
    window::BackendWindow,
};

use game_interface::{
    client_commands::ClientFreeCamMode,
    events::EventClientInfo,
    interface::GameStateInterface,
    types::{
        character_info::NetworkCharacterInfo,
        game::{GameEntityId, GameTickType},
        id_gen::IdGeneratorIdType,
        input::{CharacterInputConsumableDiff, CharacterInputInfo},
        network_string::NetworkString,
        render::{
            character::{CharacterInfo, PlayerCameraMode},
            game::game_match::MatchSide,
            stage::StageRenderInfo,
        },
        snapshot::SnapshotClientInfo,
        ticks::TickOptions,
    },
    votes::{VoteType, Voted},
};
use graphics_types::rendering::ColorRgba;
use hashlink::LinkedHashMap;
use math::math::vector::vec2;
use native::{
    input::{
        binds::{BindKey, Binds},
        InputEventHandler,
    },
    native::{
        app::NativeApp, FromNativeImpl, FromNativeLoadingImpl, KeyCode, Native,
        NativeCreateOptions, NativeImpl, NativeWindowMonitorDetails, NativeWindowOptions,
        PhysicalKey, PhysicalSize, WindowEvent,
    },
};
use network::network::types::NetworkInOrderChannel;
use pool::{
    datatypes::{PoolLinkedHashMap, StringPool},
    pool::Pool,
};
use raw_window_handle::RawDisplayHandle;
use rayon::ThreadPool;
use server::server::Server;
use shared::editor::editor_wasm_manager::EditorWasmManager;
use sound::{scene_object::SceneObject, sound::SoundManager};
use sound_backend::sound_backend::SoundBackend;
use steam::{init_steam, traits::SteamRaii};
use ui_base::{
    font_data::{UiFontData, UiFontDataLoading},
    types::UiRenderPipe,
    ui::UiCreator,
};
use ui_wasm_manager::{UiManagerBase, UiPageLoadingType, UiWasmManagerErrorPageErr};

use crate::{
    game::Game,
    localplayer::ClientPlayer,
    ui::pages::{
        editor::tee::TeeEditor, loading::LoadingPage, not_found::Error404Page, test::ColorTest,
    },
};

use shared_base::{
    game_types::{intra_tick_time, intra_tick_time_to_ratio, is_next_tick},
    network::{
        messages::{GameModification, MsgClAddLocalPlayer, MsgClChatMsg, MsgClLoadVotes},
        server_info::ServerInfo,
    },
    player_input::PlayerInput,
};

use shared_network::messages::{ClientToServerMessage, ClientToServerPlayerMessage, GameMessage};

use super::{
    components::{
        client_stats::{ClientStats, ClientStatsRenderPipe},
        debug_hud::{DebugHud, DebugHudRenderPipe},
    },
    game::{DisconnectAutoCleanup, ServerCertMode},
    game_events::{GameEventPipeline, GameEventsClient},
    input::input_handling::{InputHandling, InputHandlingEvent},
    localplayer::ClientPlayerInputPerTick,
    spatial_chat::spatial_chat::{self, SpatialChatGameWorldTy, SpatialChatGameWorldTyRef},
};

type UiManager = UiManagerBase<Config>;

pub fn ddnet_main(
    start_arguments: Vec<String>,
    sys: System,
    cert: &[u8],
    shared_info: Arc<ServerInfo>,
    app: NativeApp,
) -> anyhow::Result<()> {
    let io =
        IoFileSys::new(|rt| Arc::new(FileSystem::new(rt, "org", "", "DDNet", "DDNet-Accounts")));

    let config_engine = config_fs::load(&io);

    let benchmark = Benchmark::new(config_engine.dbg.bench);

    let config_game = game_config_fs::fs::load(&io);
    benchmark.bench("loading client config");

    let graphics_backend_io_loading = GraphicsBackendIoLoading::new(&config_engine.gfx, &io);
    // first prepare all io tasks of all components
    benchmark.bench("load_io of graphics backend");

    let sys_time = sys.time.clone();
    let do_bench = config_engine.dbg.bench;
    let dbg_input = config_engine.inp.dbg_mode;

    let config_wnd = config_engine.wnd.clone();

    let client = ClientNativeLoadingImpl {
        sys,
        cert: cert.to_vec(),
        shared_info,
        io,
        config_engine,
        config_game,
        graphics_backend_io_loading,
        graphics_backend_loading: None,
    };
    Native::run_loop::<ClientNativeImpl, _>(
        client,
        app,
        NativeCreateOptions {
            do_bench,
            title: "DDNet".to_string(),
            sys: &sys_time,
            dbg_input,
            start_arguments,
            window: native::native::NativeWindowOptions {
                #[cfg(target_os = "android")]
                fullscreen: false,
                #[cfg(not(target_os = "android"))]
                fullscreen: config_wnd.fullscreen,
                decorated: config_wnd.decorated,
                maximized: config_wnd.maximized,
                width: config_wnd.width,
                height: config_wnd.height,
                refresh_rate_milli_hertz: config_wnd.refresh_rate_mhz,
                monitor: (!config_wnd.monitor.name.is_empty()
                    && config_wnd.monitor.width != 0
                    && config_wnd.monitor.height != 0)
                    .then_some(NativeWindowMonitorDetails {
                        name: config_wnd.monitor.name,
                        size: PhysicalSize {
                            width: config_wnd.monitor.width,
                            height: config_wnd.monitor.height,
                        },
                    }),
            },
        },
    )?;
    Ok(())
}

struct ClientNativeLoadingImpl {
    sys: System,
    cert: Vec<u8>,
    shared_info: Arc<ServerInfo>,
    io: IoFileSys,
    config_engine: ConfigEngine,
    config_game: ConfigGame,
    graphics_backend_io_loading: GraphicsBackendIoLoading,
    graphics_backend_loading: Option<GraphicsBackendLoading>,
}

struct ClientNativeImpl {
    sys: System,
    shared_info: Arc<ServerInfo>,

    client_info: ClientInfo,
    account_info: AccountInfo,
    spatial_chat: spatial_chat::SpatialChat,
    player_settings_sync: PlayerSettingsSync,

    scene: SceneObject,

    sound: SoundManager,
    sound_backend: Rc<SoundBackend>,
    game: Game,
    cert: Vec<u8>,
    connect_info: ConnectMode,
    demo_player: Option<DemoViewer>,
    client_stats: ClientStats,
    debug_hud: DebugHud,
    thread_pool: Arc<ThreadPool>,
    io: Io,
    config: Config,
    cur_time: Duration,
    last_refresh_rate_time: Duration,

    editor: Option<EditorWasmManager>,

    entities_container: EntitiesContainer,

    local_console: LocalConsole,
    console_logs: String,

    ui_manager: UiManager,
    ui_events: UiEvents,
    font_data: Arc<UiFontData>,
    ui_creator: UiCreator,

    /// RAII object that must live as long as the app
    _steam_rt: Box<dyn SteamRaii>,

    // ui-shared objects
    accounts: Arc<Accounts>,
    server_players: ServerPlayers,
    game_server_info: GameServerInfo,
    votes: Votes,

    menu_map: ClientMapLoading,

    global_binds: Binds<BindActionsHotkey>,

    // pools & helpers
    string_pool: StringPool,

    // input & helper
    inp_manager: InputHandling,

    // put graphics at the end, so it's dropped last
    graphics: Graphics,
    graphics_backend: Rc<GraphicsBackend>,
}

impl ClientNativeImpl {
    fn render_menu_background_map(&mut self) {
        if let Some(map) = self.menu_map.continue_loading() {
            let intra_tick_time = self.sys.time_get_nanoseconds();
            let ClientMapFile::Menu { render } = &map else {
                panic!("this was not a menu map")
            };
            let render = render.try_get().unwrap();
            render.render.render_full_design(
                &render.data.buffered_map.map_visual,
                &mut RenderPipeline {
                    base: RenderPipelineBase {
                        map: &render.data.buffered_map.map_visual,
                        config: &ConfigMap::default(),
                        cur_time: &self.sys.time_get_nanoseconds(),
                        cur_anim_time: &RenderMap::calc_anim_time(
                            50.try_into().unwrap(),
                            (self.sys.time_get_nanoseconds().as_millis() / (1000 / 50)).max(1)
                                as GameTickType,
                            &intra_tick_time,
                        ),
                        camera: &Camera {
                            pos: vec2::new(21.0, 15.0),
                            zoom: 1.0,
                        },
                        entities_container: &mut self.entities_container,
                        entities_key: None,
                        physics_group_name: "vanilla",
                        map_sound_volume: self.config.game.snd.map_sound_volume
                            * self.config.game.snd.global_volume,
                    },
                    buffered_map: &render.data.buffered_map,
                },
            )
        }
    }

    fn render_game(&mut self, native: &mut dyn NativeImpl) {
        if let Game::Active(game) = &mut self.game {
            // prepare input
            let events = std::mem::replace(&mut game.events, game.events_pool.new());

            let GameMap {
                render,
                game: game_state,
            } = &mut game.map;
            let is_menu_open = self.ui_manager.ui.ui_state.is_ui_open;

            let intra_tick_ratio = intra_tick_time_to_ratio(
                game.game_data.intra_tick_time,
                game_state.game_tick_speed(),
            );

            let character_infos = game_state.collect_characters_info();

            if self.server_players.needs_player_infos() {
                self.server_players.fill_player_info(
                    character_infos
                        .iter()
                        .filter_map(|(&id, char)| {
                            char.player_info
                                .is_some()
                                .then_some((id, (**char.info).clone()))
                        })
                        .collect(),
                );
            }

            let stages = game_state.all_stages(intra_tick_ratio);

            if let SpatialChatGameWorldTy::World(spatial_world) = &mut game.spatial_world {
                spatial_chat::SpatialChat::on_entity_positions(
                    Some(spatial_world),
                    stages
                        .values()
                        .flat_map(|stage| {
                            stage
                                .world
                                .characters
                                .iter()
                                .map(|(id, c)| (*id, c.lerped_pos))
                        })
                        .collect(),
                );
            }

            let mut render_game_input = RenderGameInput {
                players: game.render_players_pool.new(),
                dummies: game.game_data.player_ids_pool.new(),
                events,
                chat_msgs: {
                    let mut chat_msgs = game.game_data.chat_msgs_pool.new();
                    chat_msgs.append(&mut game.game_data.chat_msgs);
                    chat_msgs
                },
                vote: game.game_data.vote.as_ref().map(|(v, voted, timestamp)| {
                    (
                        v.clone(),
                        *voted,
                        v.remaining_time.saturating_sub(
                            self.cur_time
                                .saturating_sub(*timestamp)
                                .saturating_sub(game.game_data.prediction_timer.ping_average()),
                        ),
                    )
                }),
                character_infos,
                stages,
                scoreboard_info: None,
                game_time_info: GameTimeInfo {
                    ticks_per_second: game_state.game_tick_speed(),
                    intra_tick_time: game.game_data.intra_tick_time,
                },
                settings: RenderGameSettings {
                    spartial_sound: self.config.game.snd.spartial,
                    sound_playback_speed: 1.0,
                    nameplates: self.config.game.cl.nameplates,
                    nameplate_own: self.config.game.cl.own_nameplate,
                    ingame_sound_volume: self.config.game.snd.ingame_sound_volume
                        * self.config.game.snd.global_volume,
                    map_sound_volume: self.config.game.snd.map_sound_volume
                        * self.config.game.snd.global_volume,
                },
            };

            let mut fill_for_player =
                |client_player: (&GameEntityId, &mut ClientPlayer),
                 character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
                 stages_render_infos: &mut PoolLinkedHashMap<GameEntityId, StageRenderInfo>,
                 requires_scoreboard: &mut bool|
                 -> (GameEntityId, RenderGameForPlayer) {
                    let (&player_id, client_player) = client_player;
                    let local_player_render_info =
                        game_state.collect_character_local_render_info(&player_id);

                    *requires_scoreboard |= client_player.show_scoreboard;

                    let character_info = character_infos.get(&player_id);
                    if let Some(player) = character_info.and_then(|c| {
                        c.stage_id
                            .and_then(|stage_id| stages_render_infos.get_mut(&stage_id))
                            .and_then(|s| s.world.characters.get_mut(&player_id))
                    }) {
                        player.cursor_pos = client_player.input.inp.cursor.to_vec2();
                    }

                    (
                        player_id,
                        RenderGameForPlayer {
                            render_for_player: RenderForPlayer {
                                chat_info: if client_player.chat_input_active && !is_menu_open {
                                    Some((
                                        std::mem::take(&mut client_player.chat_msg),
                                        self.inp_manager.clone_inp().egui,
                                    ))
                                } else {
                                    None
                                },
                                emote_wheel_input: if client_player.emote_wheel_active
                                    && !is_menu_open
                                {
                                    Some(self.inp_manager.clone_inp().egui)
                                } else {
                                    None
                                },
                                chat_show_all: client_player.show_chat_all,
                                scoreboard_active: client_player.show_scoreboard,

                                local_player_info: local_player_render_info,

                                zoom: client_player.zoom,
                                cam_mode: match character_info.and_then(|c| c.player_info.as_ref())
                                {
                                    Some(info) => match info.cam_mode {
                                        PlayerCameraMode::Default => {
                                            RenderPlayerCameraMode::Default
                                        }
                                        PlayerCameraMode::Free => {
                                            RenderPlayerCameraMode::AtPos(vec2::new(
                                                client_player.free_cam_pos.x as f32,
                                                client_player.free_cam_pos.y as f32,
                                            ))
                                        }
                                        PlayerCameraMode::LockedTo(pos) => {
                                            RenderPlayerCameraMode::AtPos(pos)
                                        }
                                    },
                                    None => RenderPlayerCameraMode::Default,
                                },
                            },
                            observed_players: game.render_observers_pool.new(),
                            observed_anchored_size_props: ObservedAnchoredSize {
                                width: self
                                    .config
                                    .game
                                    .cl
                                    .dummy
                                    .screen_width
                                    .max(1)
                                    .try_into()
                                    .unwrap(),
                                height: self
                                    .config
                                    .game
                                    .cl
                                    .dummy
                                    .screen_height
                                    .max(1)
                                    .try_into()
                                    .unwrap(),
                            },
                        },
                    )
                };

            let mut requires_scoreboard = false;
            let ids = game
                .game_data
                .local_players
                .iter_mut()
                .filter(|(_, player)| !player.is_dummy);
            ids.for_each(|client_player| {
                let (player_id, render_for_player) = fill_for_player(
                    client_player,
                    &render_game_input.character_infos,
                    &mut render_game_input.stages,
                    &mut requires_scoreboard,
                );
                render_game_input
                    .players
                    .insert(player_id, render_for_player);
            });
            let dummies = game
                .game_data
                .local_players
                .iter()
                .filter_map(|(&id, player)| player.is_dummy.then_some(id));
            render_game_input.dummies.extend(dummies);

            // if miniscreens of the dummies should show up, add additional infor for player.
            if self.config.game.cl.dummy.mini_screen {
                if let Some((_, player)) = render_game_input.players.iter_mut().next() {
                    player
                        .observed_players
                        .extend(render_game_input.dummies.iter().map(|&player_id| {
                            ObservedPlayer::Dummy {
                                local_player_info: game_state
                                    .collect_character_local_render_info(&player_id),
                                player_id,
                                anchor: self.config.game.cl.dummy.screen_anchor.into(),
                            }
                        }));
                }
            }

            if requires_scoreboard {
                let scoreboard_info = game_state.collect_scoreboard_info();
                render_game_input.scoreboard_info = Some(scoreboard_info);
            }

            let res = render.render(&self.config.game.map, &self.cur_time, render_game_input);

            // handle results
            for (player_id, player_events) in res.player_events {
                let local_player = game.game_data.local_players.get_mut(&player_id).unwrap();
                for player_event in player_events {
                    match player_event {
                        PlayerFeedbackEvent::Chat(ev) => match ev {
                            ChatEvent::MsgSend(msg) => {
                                game.network.send_in_order_to_server(
                                    &GameMessage::ClientToServer(ClientToServerMessage::PlayerMsg(
                                        (
                                            player_id,
                                            ClientToServerPlayerMessage::Chat(
                                                MsgClChatMsg::Global {
                                                    msg: NetworkString::new(&msg).unwrap(),
                                                },
                                            ),
                                        ),
                                    )),
                                    NetworkInOrderChannel::Global,
                                );
                                local_player.chat_msg.clear();
                            }
                            ChatEvent::CurMsg(msg) => {
                                local_player.chat_msg = msg;
                            }
                            ChatEvent::ChatClosed => {
                                local_player.chat_input_active = false;
                            }
                            ChatEvent::PlatformOutput(output) => {
                                // no matter what egui reports, we don't want a cursor ingame
                                self.inp_manager
                                    .handle_platform_output(native, output, true);
                            }
                        },
                        PlayerFeedbackEvent::EmoteWheel(ev) => {
                            local_player.last_emote_wheel_selection = Some(ev);
                        }
                    }
                }
            }

            if self.debug_hud.ui.ui_state.is_ui_open {
                self.debug_hud.render(&mut DebugHudRenderPipe {
                    graphics: &self.graphics,
                    prediction_timer: &game.game_data.prediction_timer,
                    byte_stats: &game.game_data.net_byte_stats,
                    ingame_timer: &game.game_data.last_game_tick,
                });
            }
        } else {
            // menu background map
            self.render_menu_background_map();
            self.graphics.backend_handle.consumble_multi_samples();
        }
    }

    fn render(&mut self, native: &mut dyn NativeImpl) {
        // first unload editor => then reload. else native library doesn't get a reload
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.should_reload())
        {
            self.editor = None;

            self.editor = Some(EditorWasmManager::new(
                &self.sound,
                &self.graphics,
                &self.graphics_backend,
                &self.io,
                &self.thread_pool,
                &self.font_data,
            ));
        }
        if let Some(editor) = &mut self.editor {
            match editor.render(
                if self.local_console.ui.ui_state.is_ui_open || self.game.remote_console_open() {
                    Default::default()
                } else {
                    self.inp_manager.take_inp().egui.unwrap_or_default()
                },
                &self.config.engine,
            ) {
                EditorResult::PlatformOutput(output) => {
                    self.inp_manager.handle_platform_output(
                        native,
                        output,
                        self.local_console.ui.ui_state.is_ui_open
                            || self.game.remote_console_open(),
                    );
                }
                EditorResult::Close => {
                    self.editor = None;
                }
            }
        } else {
            self.render_game(native);

            // if demo viewer is active, render it
            if let Some(demo_player) = &mut self.demo_player {
                if let Some(demo_viewer) = demo_player.try_get_mut() {
                    if demo_viewer
                        .render(
                            if self.local_console.ui.ui_state.is_ui_open
                                || self.game.remote_console_open()
                            {
                                Default::default()
                            } else {
                                self.inp_manager.take_inp().egui.unwrap_or_default()
                            },
                        )
                        .is_err()
                        || demo_viewer.is_closed()
                    {
                        self.demo_player = None;
                    }
                } else {
                    demo_player
                        .continue_loading(
                            &self.sound,
                            &self.graphics,
                            &self.graphics_backend,
                            &self.sound_backend,
                            &self.config.engine,
                            &self.sys,
                            &self.ui_creator,
                        )
                        .unwrap();
                }
            } else if self.ui_manager.ui.ui_state.is_ui_open {
                // render ui last
                if let Some(output) = self.ui_manager.render(
                    &self.config.engine.ui.path.name.clone(), // TODO: useless heap allocation
                    &self.io,
                    &self.graphics,
                    &self.graphics_backend,
                    &mut self.sound,
                    &mut UiRenderPipe::new(self.sys.time_get_nanoseconds(), &mut self.config),
                    if self.local_console.ui.ui_state.is_ui_open || self.game.remote_console_open()
                    {
                        Default::default()
                    } else {
                        self.inp_manager.take_inp().egui.unwrap_or_default()
                    },
                    true,
                ) {
                    self.inp_manager.handle_platform_output(
                        native,
                        output,
                        self.local_console.ui.ui_state.is_ui_open
                            || self.game.remote_console_open(),
                    );
                }
                let ui_events = self.ui_events.take();
                for ui_event in ui_events {
                    match ui_event {
                        UiEvent::PlayDemo { name } => {
                            self.demo_player = Some(DemoViewer::new(
                                &self.io,
                                &self.thread_pool,
                                name.as_ref(),
                                self.font_data.clone(),
                                None,
                            ));
                        }
                        UiEvent::EncodeDemoToVideo { name, video_name } => {
                            self.demo_player = Some(DemoViewer::new(
                                &self.io,
                                &self.thread_pool,
                                name.as_ref(),
                                self.font_data.clone(),
                                Some(DemoVideoEncodeProperties {
                                    file_name: format!("videos/{}.mp4", video_name).into(),
                                    pixels_per_point: self.config.game.cl.recorder.pixels_per_point,
                                    encoder_settings: EncoderSettings {
                                        fps: self.config.game.cl.recorder.fps,
                                        width: self.config.game.cl.recorder.width,
                                        height: self.config.game.cl.recorder.height,
                                        hw_accel: self.config.game.cl.recorder.hw_accel.clone(),
                                        max_threads: std::thread::available_parallelism()
                                            .map(|v| v.get() + 2)
                                            .unwrap_or_default()
                                            .max(2)
                                            as u64,
                                        sample_rate: self.config.game.cl.recorder.sample_rate,
                                        crf: self.config.game.cl.recorder.crf,
                                    },
                                }),
                            ));
                        }
                        UiEvent::RecordDemo => {
                            if let Game::Active(game) = &mut self.game {
                                game.demo_recorder = Some(DemoRecorder::new(
                                    game.demo_recorder_props.clone(),
                                    game.map.game.game_tick_speed(),
                                    None,
                                ));
                            }
                        }
                        UiEvent::StartEditor => {
                            self.editor = Some(EditorWasmManager::new(
                                &self.sound,
                                &self.graphics,
                                &self.graphics_backend,
                                &self.io,
                                &self.thread_pool,
                                &self.font_data,
                            ));
                        }
                        UiEvent::Connect {
                            addr,
                            rcon_secret,
                            cert_hash,
                        } => {
                            self.client_info.set_local_player_count(1);
                            self.account_info.fill_account_info(None);
                            self.config.engine.ui.path.route("connect");
                            self.connect_info.set(ConnectModes::Connecting);
                            self.game = Game::new(
                                &self.graphics,
                                &self.graphics_backend,
                                &self.sound,
                                &self.sys,
                                &self.io,
                                &self.connect_info,
                                cert_hash
                                    .map(ServerCertMode::Hash)
                                    .unwrap_or_else(|| ServerCertMode::Cert(self.cert.clone())),
                                addr,
                                &self.accounts,
                                rcon_secret,
                                DisconnectAutoCleanup {
                                    spatial_chat: self.spatial_chat.spatial_chat.clone(),
                                    client_info: self.client_info.clone(),
                                    account_info: self.account_info.clone(),
                                    player_settings_sync: self.player_settings_sync.clone(),
                                    votes: self.votes.clone(),
                                },
                            )
                            .unwrap();
                        }
                        UiEvent::Disconnect => {
                            self.game = Game::None;
                        }
                        UiEvent::ConnectLocalPlayer { as_dummy } => {
                            if let Game::Active(game) = &mut self.game {
                                self.client_info.set_local_player_count(
                                    self.client_info.local_player_count() + 1,
                                );
                                game.network.send_unordered_to_server(
                                    &GameMessage::ClientToServer(
                                        ClientToServerMessage::AddLocalPlayer(
                                            MsgClAddLocalPlayer {
                                                player_info: if let Some((info, copy_info)) =
                                                    as_dummy
                                                        .then(|| {
                                                            self.config
                                                                .game
                                                                .players
                                                                .get(
                                                                    self.config
                                                                        .game
                                                                        .profiles
                                                                        .dummy
                                                                        .index
                                                                        as usize,
                                                                )
                                                                .zip(self.config.game.players.get(
                                                                    self.config.game.profiles.main
                                                                        as usize,
                                                                ))
                                                        })
                                                        .flatten()
                                                {
                                                    Game::network_char_info_from_config_for_dummy(
                                                        info,
                                                        copy_info,
                                                        &self.config.game.profiles.dummy,
                                                    )
                                                } else {
                                                    // TODO
                                                    NetworkCharacterInfo::explicit_default()
                                                },
                                                as_dummy,
                                            },
                                        ),
                                    ),
                                );
                            }
                        }
                        UiEvent::DisconnectLocalPlayer => {
                            if let Game::Active(game) = &mut self.game {
                                self.client_info.set_local_player_count(
                                    self.client_info.local_player_count().saturating_sub(1),
                                );
                                if game.game_data.local_players.len() > 1 {
                                    let (player_id, _) =
                                        game.game_data.local_players.pop_back().unwrap();
                                    game.network.send_unordered_to_server(
                                        &GameMessage::ClientToServer(
                                            ClientToServerMessage::PlayerMsg((
                                                player_id,
                                                ClientToServerPlayerMessage::RemLocalPlayer,
                                            )),
                                        ),
                                    );
                                }
                            }
                        }
                        UiEvent::Quit => {
                            native.quit();
                        }
                        UiEvent::Kill => {
                            if let Game::Active(game) = &mut self.game {
                                if let Some((player_id, _)) = game
                                    .game_data
                                    .local_players
                                    .iter()
                                    .find(|(_, player)| !player.is_dummy)
                                {
                                    game.network.send_unordered_to_server(
                                        &GameMessage::ClientToServer(
                                            ClientToServerMessage::PlayerMsg((
                                                *player_id,
                                                ClientToServerPlayerMessage::Kill,
                                            )),
                                        ),
                                    );
                                }
                            }
                        }
                        UiEvent::JoinSpectators => {
                            if let Game::Active(game) = &mut self.game {
                                if let Some((player_id, _)) = game
                                    .game_data
                                    .local_players
                                    .iter()
                                    .find(|(_, player)| !player.is_dummy)
                                {
                                    game.network.send_unordered_to_server(
                                        &GameMessage::ClientToServer(
                                            ClientToServerMessage::PlayerMsg((
                                                *player_id,
                                                ClientToServerPlayerMessage::JoinSpectator,
                                            )),
                                        ),
                                    );
                                }
                            }
                        }
                        UiEvent::SwitchToFreeCam => {
                            if let Game::Active(game) = &mut self.game {
                                if let Some((player_id, _)) = game
                                    .game_data
                                    .local_players
                                    .iter()
                                    .find(|(_, player)| !player.is_dummy)
                                {
                                    game.network.send_unordered_to_server(
                                        &GameMessage::ClientToServer(
                                            ClientToServerMessage::PlayerMsg((
                                                *player_id,
                                                ClientToServerPlayerMessage::SwitchToFreeCam(
                                                    ClientFreeCamMode::Normal,
                                                ),
                                            )),
                                        ),
                                    );
                                }
                            }
                        }
                        UiEvent::WindowChange => {
                            let config_wnd = &self.config.engine.wnd;

                            // TODO: don't ignore error?
                            let _ = native.set_window_config(native::native::NativeWindowOptions {
                                fullscreen: config_wnd.fullscreen,
                                decorated: config_wnd.decorated,
                                maximized: config_wnd.maximized,
                                width: config_wnd.width,
                                height: config_wnd.height,
                                refresh_rate_milli_hertz: config_wnd.refresh_rate_mhz,
                                monitor: (!config_wnd.monitor.name.is_empty()
                                    && config_wnd.monitor.width != 0
                                    && config_wnd.monitor.height != 0)
                                    .then_some(NativeWindowMonitorDetails {
                                        name: config_wnd.monitor.name.clone(),
                                        size: PhysicalSize {
                                            width: config_wnd.monitor.width,
                                            height: config_wnd.monitor.height,
                                        },
                                    }),
                            });
                        }
                        UiEvent::VsyncChanged => {
                            self.graphics.vsync(self.config.engine.gl.vsync);
                        }
                        UiEvent::MsaaChanged => {
                            self.graphics
                                .multi_sampling(self.config.engine.gl.msaa_samples);
                        }
                        UiEvent::VoteKickPlayer { voted_player_id } => {
                            if let Game::Active(game) = &mut self.game {
                                if let Some((player_id, _)) = game
                                    .game_data
                                    .local_players
                                    .iter()
                                    .find(|(_, player)| !player.is_dummy)
                                {
                                    game.network.send_unordered_to_server(
                                        &GameMessage::ClientToServer(
                                            ClientToServerMessage::PlayerMsg((
                                                *player_id,
                                                ClientToServerPlayerMessage::StartVote(
                                                    VoteType::VoteKickPlayer { voted_player_id },
                                                ),
                                            )),
                                        ),
                                    );
                                }
                            }
                        }
                        UiEvent::VoteSpecPlayer { voted_player_id } => {
                            if let Game::Active(game) = &mut self.game {
                                if let Some((player_id, _)) = game
                                    .game_data
                                    .local_players
                                    .iter()
                                    .find(|(_, player)| !player.is_dummy)
                                {
                                    game.network.send_unordered_to_server(
                                        &GameMessage::ClientToServer(
                                            ClientToServerMessage::PlayerMsg((
                                                *player_id,
                                                ClientToServerPlayerMessage::StartVote(
                                                    VoteType::VoteSpecPlayer { voted_player_id },
                                                ),
                                            )),
                                        ),
                                    );
                                }
                            }
                        }
                        UiEvent::VoteMap { voted_map } => {
                            if let Game::Active(game) = &mut self.game {
                                if let Some((player_id, _)) = game
                                    .game_data
                                    .local_players
                                    .iter()
                                    .find(|(_, player)| !player.is_dummy)
                                {
                                    game.network.send_unordered_to_server(
                                        &GameMessage::ClientToServer(
                                            ClientToServerMessage::PlayerMsg((
                                                *player_id,
                                                ClientToServerPlayerMessage::StartVote(
                                                    VoteType::Map(voted_map),
                                                ),
                                            )),
                                        ),
                                    );
                                }
                            }
                        }
                        UiEvent::VoteMisc { misc } => {
                            if let Game::Active(game) = &mut self.game {
                                if let Some((player_id, _)) = game
                                    .game_data
                                    .local_players
                                    .iter()
                                    .find(|(_, player)| !player.is_dummy)
                                {
                                    game.network.send_unordered_to_server(
                                        &GameMessage::ClientToServer(
                                            ClientToServerMessage::PlayerMsg((
                                                *player_id,
                                                ClientToServerPlayerMessage::StartVote(
                                                    VoteType::Misc(misc),
                                                ),
                                            )),
                                        ),
                                    );
                                }
                            }
                        }
                        UiEvent::JoinOwnTeam { name, color } => {
                            if let Game::Active(game) = &mut self.game {
                                for (player_id, _) in game.game_data.local_players.iter() {
                                    game.network.send_unordered_to_server(
                                        &GameMessage::ClientToServer(
                                            ClientToServerMessage::PlayerMsg((
                                                *player_id,
                                                ClientToServerPlayerMessage::JoinTeam {
                                                    name: name.as_str().try_into().unwrap(),
                                                    color: [color.r(), color.g(), color.b()],
                                                },
                                            )),
                                        ),
                                    );
                                }
                            }
                        }
                        UiEvent::JoinOtherTeam => {
                            // TODO:
                        }
                        UiEvent::JoinVanillaSide { is_red_side } => {
                            if let Game::Active(game) = &mut self.game {
                                for (player_id, _) in game.game_data.local_players.iter() {
                                    game.network.send_unordered_to_server(
                                        &GameMessage::ClientToServer(
                                            ClientToServerMessage::PlayerMsg((
                                                *player_id,
                                                ClientToServerPlayerMessage::JoinVanillaSide(
                                                    if is_red_side {
                                                        MatchSide::Red
                                                    } else {
                                                        MatchSide::Blue
                                                    },
                                                ),
                                            )),
                                        ),
                                    );
                                }
                            }
                        }
                        UiEvent::ChangeAccountName { name } => {
                            if let Game::Active(game) = &mut self.game {
                                game.network.send_unordered_to_server(
                                    &GameMessage::ClientToServer(
                                        ClientToServerMessage::AccountChangeName { new_name: name },
                                    ),
                                );
                            }
                        }
                        UiEvent::RequestAccountInfo => {
                            if let Game::Active(game) = &mut self.game {
                                if !std::mem::replace(&mut game.requested_account_details, true) {
                                    game.network.send_unordered_to_server(
                                        &GameMessage::ClientToServer(
                                            ClientToServerMessage::AccountRequestInfo,
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // make sure no msaa blocks ui rendering
        self.graphics.backend_handle.consumble_multi_samples();
        if self.local_console.ui.ui_state.is_ui_open {
            let mut pipe = ConsoleRenderPipe {
                graphics: &self.graphics,
                sys: &self.sys,
                config: &mut self.config,
                msgs: &mut self.console_logs,
            };
            let (events, platform_output) = self.local_console.render(
                self.inp_manager.take_inp().egui.unwrap_or_default(),
                &mut pipe,
            );
            self.handle_console_events(native, events);
            self.inp_manager
                .handle_platform_output(native, platform_output, false);
        } else if let Some(remote_console) = self
            .game
            .remote_console_open()
            .then_some(self.game.get_remote_console_mut())
            .flatten()
        {
            let mut pipe = ConsoleRenderPipe {
                graphics: &self.graphics,
                sys: &self.sys,
                config: &mut self.config,
                msgs: &mut self.console_logs,
            };
            let (events, platform_output) = remote_console.render(
                self.inp_manager.take_inp().egui.unwrap_or_default(),
                &mut pipe,
            );
            if let Game::Active(game) = &mut self.game {
                for event in events {
                    match event {
                        RemoteConsoleEvent::Exec { name, args } => {
                            game.network.send_in_order_to_server(
                                &GameMessage::ClientToServer(ClientToServerMessage::RconExec {
                                    name,
                                    args,
                                }),
                                NetworkInOrderChannel::Custom(
                                    7302, // reads as "rcon"
                                ),
                            )
                        }
                    }
                }
            }
            self.inp_manager
                .handle_platform_output(native, platform_output, false);
        }

        // fps (& debug)
        self.client_stats.render(&mut ClientStatsRenderPipe {
            graphics: &self.graphics,
            sys: &self.sys,
        });

        self.sound.swap();
        self.graphics.swap();
        self.graphics.backend_handle.update_clear_color(ColorRgba {
            r: self.config.engine.gl.clear_color.r as f32 / 255.0,
            g: self.config.engine.gl.clear_color.g as f32 / 255.0,
            b: self.config.engine.gl.clear_color.b as f32 / 255.0,
            a: 0.0,
        });
    }

    fn handle_console_events(
        &mut self,
        native: &mut dyn NativeImpl,
        events: Vec<LocalConsoleEvent>,
    ) {
        for event in events {
            match event {
                LocalConsoleEvent::Quit => native.quit(),
            }
        }
    }
}

impl FromNativeLoadingImpl<ClientNativeLoadingImpl> for ClientNativeImpl {
    fn new(
        mut loading: ClientNativeLoadingImpl,
        native: &mut dyn NativeImpl,
    ) -> anyhow::Result<Self> {
        let benchmark = Benchmark::new(loading.config_engine.dbg.bench);
        let io = Io::from(loading.io, Arc::new(HttpClient::new()));
        benchmark.bench("upgrading io with http client");

        let font_loading = UiFontDataLoading::new(&io);
        let accounts_loading = AccountsLoading::new(&io);
        benchmark.bench("loading client files");

        let thread_pool = Arc::new(
            rayon::ThreadPoolBuilder::new()
                .thread_name(|index| format!("client-rayon {index}"))
                .num_threads(
                    std::thread::available_parallelism()
                        .unwrap_or(NonZeroUsize::new(2).unwrap())
                        .get()
                        .max(4)
                        - 2,
                )
                .build()?,
        );
        benchmark.bench("creating rayon thread pool");

        // read window props
        let wnd = native.window_options();
        let config_wnd = &mut loading.config_engine.wnd;
        config_wnd.fullscreen = wnd.fullscreen;
        config_wnd.decorated = wnd.decorated;
        config_wnd.maximized = wnd.maximized;
        config_wnd.width = wnd.width;
        config_wnd.height = wnd.height;
        config_wnd.refresh_rate_mhz = wnd.refresh_rate_milli_hertz;
        config_wnd.monitor = wnd
            .monitor
            .map(|monitor| ConfigMonitor {
                name: monitor.name,
                width: monitor.size.width,
                height: monitor.size.height,
            })
            .unwrap_or_default();

        let mut ui_creator = UiCreator::default();
        // prepare network stuff while waiting for io
        let client_stats = ClientStats::new(&loading.sys, &ui_creator);

        let sound_backend = SoundBackend::new(&loading.config_engine.snd)?;
        let sound = SoundManager::new(sound_backend.clone())?;

        benchmark.bench("sound");

        let monitors: Vec<_> = native
            .monitors()
            .into_iter()
            .map(|monitor| {
                let mut video_modes: Vec<_> = monitor
                    .video_modes()
                    .map(|mode| {
                        let size = mode.size();
                        UiMonitorVideoMode {
                            width: size.width,
                            height: size.height,
                            refresh_rate_mhz: mode.refresh_rate_millihertz(),
                        }
                    })
                    .collect();
                let video_modes = if video_modes.is_empty() {
                    let size = monitor.size();
                    vec![UiMonitorVideoMode {
                        width: size.width,
                        height: size.height,
                        refresh_rate_mhz: monitor.refresh_rate_millihertz().unwrap_or_default(),
                    }]
                } else {
                    // that the parameter names are swapped is intentional
                    // bcs what we actually want is the sort into the other direction
                    video_modes.sort_by(|v2, v1| {
                        let mut cmp = v1.width.cmp(&v2.width);
                        if matches!(cmp, std::cmp::Ordering::Equal) {
                            cmp = v1.height.cmp(&v2.height);
                            if matches!(cmp, std::cmp::Ordering::Equal) {
                                cmp = v1.refresh_rate_mhz.cmp(&v2.refresh_rate_mhz);
                            };
                        }
                        cmp
                    });
                    video_modes
                };
                UiMonitor {
                    name: monitor.name().unwrap_or_else(|| "invalid".to_string()),
                    video_modes,
                }
            })
            .collect();
        let monitors = UiMonitors::new(monitors);

        // then prepare components allocations etc.
        let (graphics_backend, stream_data) = GraphicsBackendBase::new(
            loading.graphics_backend_io_loading,
            loading.graphics_backend_loading.take().unwrap(),
            &thread_pool,
            BackendWindow::Winit {
                window: native.borrow_window(),
            },
        )?;
        benchmark.bench("init of graphics backend");

        let window_props = graphics_backend.get_window_props();
        let graphics_backend = GraphicsBackend::new(graphics_backend);
        let mut graphics = Graphics::new(graphics_backend.clone(), stream_data, window_props);

        let graphics_memory_usage = graphics_backend.memory_usage();
        let debug_hud = DebugHud::new(
            &loading.sys,
            graphics_memory_usage.texture_memory_usage,
            graphics_memory_usage.buffer_memory_usage,
            graphics_memory_usage.stream_memory_usage,
            graphics_memory_usage.staging_memory_usage,
            &ui_creator,
        );

        benchmark.bench("init of graphics");

        let default_entities =
            EntitiesContainer::load_default(&io, ENTITIES_CONTAINER_PATH.as_ref());
        let scene = sound.scene_handle.create(Default::default());
        let entities_container = EntitiesContainer::new(
            io.clone(),
            thread_pool.clone(),
            default_entities,
            None,
            None,
            "entities-container",
            &graphics,
            &sound,
            &scene,
            ENTITIES_CONTAINER_PATH.as_ref(),
        );

        benchmark.bench("init of components");

        let font_data = UiFontData::new(font_loading)?;

        let mut menu_map_path = format!(
            "themes/{}",
            loading.config_game.cl.menu_background_map.as_str()
        );
        let menu_map_path_exists = menu_map_path.clone();
        let fs = io.fs.clone();
        let exists = io
            .io_batcher
            .spawn(async move {
                Ok(fs
                    .file_exists(format!("{}/{}.twmap", menu_map_path_exists, "day").as_ref())
                    .await)
            })
            .get_storage()
            .unwrap();
        if !exists {
            // TODO: this should be "default"
            menu_map_path = "themes/autumn".to_string();
        }
        let menu_map = ClientMapLoading::new(
            &sound,
            &graphics,
            &graphics_backend,
            &loading.sys,
            menu_map_path.as_ref(),
            &"day".try_into().unwrap(),
            None,
            &io,
            &thread_pool,
            GameModification::Native,
            true,
            &loading.config_engine.dbg,
            Default::default(),
            RenderGameCreateOptions {
                physics_group_name: "vanilla".try_into().unwrap(),
                resource_download_server: None,
                fonts: font_data.clone(),
                sound_props: Default::default(),
            },
        );
        benchmark.bench("menu map");

        let loading_page = Box::new(LoadingPage::new());
        let page_err = UiWasmManagerErrorPageErr::default();
        let page_404 = Box::new(Error404Page::new(page_err.clone()));
        ui_creator.load_font(&font_data);
        let mut ui_manager = UiManager::new(
            &io.fs,
            (page_404, page_err),
            UiPageLoadingType::ShowLoadingPage(loading_page),
            &ui_creator,
        );

        let connect_info = ConnectMode::new(ConnectModes::Connecting);
        let ui_events = UiEvents::new();
        let client_info = ClientInfo::default();

        let (steam_client, steam_rt) = init_steam(412220)?;

        // ui shared objects
        let accounts = Arc::new(Accounts::new(accounts_loading, steam_client));
        let server_players = ServerPlayers::default();
        let game_server_info = GameServerInfo::default();
        let votes = Votes::default();
        let account_info = AccountInfo::default();
        let spatial_chat = SpatialChat::default();
        let player_settings_sync = PlayerSettingsSync::default();

        let main_menu = Box::new(MainMenuUi::new(
            &graphics,
            &sound,
            loading.shared_info.clone(),
            client_info.clone(),
            ui_events.clone(),
            io.clone(),
            thread_pool.clone(),
            accounts.clone(),
            monitors.clone(),
            spatial_chat.clone(),
            player_settings_sync.clone(),
            &loading.config_game,
        ));
        let connecting_menu = Box::new(ConnectingUi::new(connect_info.clone(), ui_events.clone()));
        let ingame_menu = Box::new(IngameMenuUi::new(
            &graphics,
            &sound,
            loading.shared_info.clone(),
            client_info.clone(),
            ui_events.clone(),
            io.clone(),
            thread_pool.clone(),
            accounts.clone(),
            monitors.clone(),
            spatial_chat.clone(),
            player_settings_sync.clone(),
            &loading.config_game,
            server_players.clone(),
            game_server_info.clone(),
            account_info.clone(),
            votes.clone(),
        ));
        let tee_editor = Box::new(TeeEditor::new(&mut graphics));
        let color_test = Box::new(ColorTest::new());
        ui_manager.register_path("", "", main_menu);
        ui_manager.register_path("", "connect", connecting_menu);
        ui_manager.register_path("", "ingame", ingame_menu);
        ui_manager.register_path("editor", "tee", tee_editor);
        ui_manager.register_path("", "color", color_test);

        let cur_time = loading.sys.time_get_nanoseconds();
        let last_refresh_rate_time = cur_time;

        native.mouse_grab();

        let mut global_binds = Binds::default();
        global_binds.register_bind(
            &[BindKey::Key(PhysicalKey::Code(KeyCode::F10))],
            BindActionsHotkey::Screenshot,
        );
        // TODO: remove this hack
        #[cfg(target_os = "android")]
        global_binds.register_bind(
            &[BindKey::Key(PhysicalKey::Code(KeyCode::Digit0))],
            BindActionsHotkey::Console,
        );
        #[cfg(not(target_os = "android"))]
        global_binds.register_bind(
            &[BindKey::Key(PhysicalKey::Code(KeyCode::F1))],
            BindActionsHotkey::LocalConsole,
        );
        global_binds.register_bind(
            &[BindKey::Key(PhysicalKey::Code(KeyCode::F2))],
            BindActionsHotkey::RemoteConsole,
        );
        global_binds.register_bind(
            &[BindKey::Key(PhysicalKey::Code(KeyCode::Escape))],
            BindActionsHotkey::ConsoleClose,
        );
        global_binds.register_bind(
            &[
                BindKey::Key(PhysicalKey::Code(KeyCode::ControlLeft)),
                BindKey::Key(PhysicalKey::Code(KeyCode::ShiftLeft)),
                BindKey::Key(PhysicalKey::Code(KeyCode::KeyD)),
            ],
            BindActionsHotkey::DebugHud,
        );

        let inp_manager = InputHandling::new(native.borrow_window());

        let mut local_console = LocalConsoleBuilder::build(&ui_creator);

        let start_cmd = native.start_arguments().join(" ");
        local_console.parse_cmd(
            &start_cmd,
            &mut loading.config_game,
            &mut loading.config_engine,
        );

        local_console.ui.ui_state.is_ui_open = false;

        benchmark.bench("finish init of client");

        let mut client = Self {
            menu_map,

            cur_time,
            sys: loading.sys,
            shared_info: loading.shared_info,
            client_info,

            entities_container,

            graphics,
            graphics_backend,

            sound,
            sound_backend,
            game: Game::None,
            cert: loading.cert,
            connect_info,
            demo_player: None,
            client_stats,
            debug_hud,
            thread_pool,
            io,
            config: Config::new(loading.config_game, loading.config_engine),
            last_refresh_rate_time,
            editor: None,

            local_console,
            console_logs: Default::default(),

            ui_manager,
            ui_events,
            font_data,
            ui_creator,

            _steam_rt: steam_rt,

            accounts,
            server_players,
            game_server_info,
            votes,
            account_info,
            player_settings_sync,
            spatial_chat: spatial_chat::SpatialChat::new(spatial_chat),

            scene,

            global_binds,
            inp_manager,

            // pools & helpers
            string_pool: Pool::with_sized(256, || String::with_capacity(256)), // TODO: random values rn
        };

        let events = client.local_console.get_events();
        client.handle_console_events(native, events);

        Ok(client)
    }

    fn load_with_display_handle(
        loading: &mut ClientNativeLoadingImpl,
        display_handle: RawDisplayHandle,
    ) -> anyhow::Result<()> {
        let map_pipe = MapPipeline::new_boxed();

        let graphics_backend_loading = GraphicsBackendLoading::new(
            &loading.config_engine.gfx,
            &loading.config_engine.dbg,
            &loading.config_engine.gl,
            graphics_backend::window::BackendRawDisplayHandle::Winit {
                handle: display_handle,
            },
            Some(Arc::new(parking_lot::RwLock::new(vec![map_pipe]))),
            loading.io.clone(),
        )?;
        loading.graphics_backend_loading = Some(graphics_backend_loading);
        Ok(())
    }
}

impl InputEventHandler for ClientNativeImpl {
    fn key_down(
        &mut self,
        window: &native::native::Window,
        device: &native::native::DeviceId,
        key: PhysicalKey,
    ) {
        self.inp_manager.key_down(window, device, &key)
    }

    fn key_up(
        &mut self,
        window: &native::native::Window,
        device: &native::native::DeviceId,
        key: PhysicalKey,
    ) {
        self.inp_manager.key_up(window, device, &key)
    }

    fn mouse_down(
        &mut self,
        window: &native::native::Window,
        device: &native::native::DeviceId,
        x: f64,
        y: f64,
        btn: &native::native::MouseButton,
    ) {
        self.inp_manager.mouse_down(window, device, x, y, btn)
    }

    fn mouse_up(
        &mut self,
        window: &native::native::Window,
        device: &native::native::DeviceId,
        x: f64,
        y: f64,
        btn: &native::native::MouseButton,
    ) {
        self.inp_manager.mouse_up(window, device, x, y, btn)
    }

    fn mouse_move(
        &mut self,
        window: &native::native::Window,
        device: &native::native::DeviceId,
        x: f64,
        y: f64,
        xrel: f64,
        yrel: f64,
    ) {
        self.inp_manager
            .mouse_move(window, device, x, y, xrel, yrel)
    }

    fn scroll(
        &mut self,
        window: &native::native::Window,
        device: &native::native::DeviceId,
        x: f64,
        y: f64,
        delta: &native::native::MouseScrollDelta,
    ) {
        self.inp_manager.scroll(window, device, x, y, delta)
    }

    fn raw_window_event(&mut self, window: &native::native::Window, event: &WindowEvent) -> bool {
        self.inp_manager.raw_event(window, event);
        // we never actually consume events
        false
    }
}

impl FromNativeImpl for ClientNativeImpl {
    fn run(&mut self, native: &mut dyn NativeImpl) {
        self.inp_manager.collect_events();
        self.inp_manager.handle_global_binds(
            &mut self.global_binds,
            &mut self.local_console.ui,
            self.game
                .get_remote_console_mut()
                .map(|console| &mut console.ui),
            &mut self.debug_hud.ui,
            &self.graphics,
            &self.io,
        );

        let sys = &mut self.sys;
        self.cur_time = sys.time_get_nanoseconds();

        self.game
            .update(&self.config.engine, &self.config.game, &self.ui_creator);

        GameEventsClient::update(&mut GameEventPipeline {
            client: &mut self.game,
            runtime_thread_pool: &mut self.thread_pool,
            io: &self.io,
            config: &mut self.config.engine,
            config_game: &mut self.config.game,
            shared_info: &self.shared_info,
            ui: &mut self.ui_manager.ui.ui_state,
            sys,
            string_pool: &mut self.string_pool,
            console_entries: &self.local_console.entries,
            game_server_info: &self.game_server_info,
            fonts: &self.font_data,
            account_info: &self.account_info,
            spatial_chat: &mut self.spatial_chat,
            spatial_chat_scene: &self.scene,
        });
        let has_input = !self.ui_manager.ui.ui_state.is_ui_open
            && !self.local_console.ui.ui_state.is_ui_open
            && !self.game.remote_console_open()
            && self.editor.is_none()
            && self.demo_player.is_none();
        if let Game::Active(game) = &mut self.game {
            // check loading of votes
            if self.votes.needs_map_votes() {
                if !game.map_votes_loaded {
                    game.map_votes_loaded = true;
                    game.network
                        .send_unordered_to_server(&GameMessage::ClientToServer(
                            ClientToServerMessage::LoadVotes(MsgClLoadVotes::Map {
                                cached_votes: None,
                            }),
                        ));
                }
                self.votes.fill_map_votes(
                    game.game_data
                        .map_votes
                        .iter()
                        .map(|vote| {
                            let key = format!(
                                "{}_{}",
                                vote.name.as_str(),
                                vote.hash.map(|hash| fmt_hash(&hash)).unwrap_or_default(),
                            );
                            (key, vote.clone())
                        })
                        .collect(),
                );
            }

            if has_input {
                native.toggle_cursor(false);
                self.inp_manager
                    .set_last_known_cursor(&self.config.engine, CursorIcon::None);

                let evs = self.inp_manager.handle_player_binds(
                    &mut game.game_data.local_players,
                    &mut self.ui_manager.ui,
                    &game.game_data.device_to_local_player_index,
                    &mut self.config.engine,
                    &mut self.config.game,
                    &self.graphics,
                );

                let player = game
                    .game_data
                    .local_players
                    .iter()
                    .find(|(_, player)| !player.is_dummy);

                for ev in evs {
                    match ev {
                        InputHandlingEvent::Kill { local_player_id } => game
                            .network
                            .send_unordered_to_server(&GameMessage::ClientToServer(
                                ClientToServerMessage::PlayerMsg((
                                    local_player_id,
                                    ClientToServerPlayerMessage::Kill,
                                )),
                            )),
                        InputHandlingEvent::VoteYes | InputHandlingEvent::VoteNo => {
                            if let Some((player_id, _)) = player {
                                let voted = if matches!(ev, InputHandlingEvent::VoteYes) {
                                    Voted::Yes
                                } else {
                                    Voted::No
                                };
                                if let Some((_, cur_voted, _)) = &mut game.game_data.vote {
                                    *cur_voted = Some(voted);
                                    game.network.send_unordered_to_server(
                                        &GameMessage::ClientToServer(
                                            ClientToServerMessage::PlayerMsg((
                                                *player_id,
                                                ClientToServerPlayerMessage::Voted(voted),
                                            )),
                                        ),
                                    );
                                }
                            }
                        }
                        InputHandlingEvent::Emoticon {
                            local_player_id,
                            emoticon,
                        } => {
                            game.map
                                .game
                                .set_player_emoticon(&local_player_id, emoticon);
                            game.network
                                .send_unordered_to_server(&GameMessage::ClientToServer(
                                    ClientToServerMessage::PlayerMsg((
                                        local_player_id,
                                        ClientToServerPlayerMessage::Emoticon(emoticon),
                                    )),
                                ));
                        }
                        InputHandlingEvent::ChangeEyes {
                            local_player_id,
                            eye,
                        } => {
                            game.map
                                .game
                                .set_player_eye(&local_player_id, eye, Duration::MAX);
                            game.network
                                .send_unordered_to_server(&GameMessage::ClientToServer(
                                    ClientToServerMessage::PlayerMsg((
                                        local_player_id,
                                        ClientToServerPlayerMessage::ChangeEyes {
                                            eye,
                                            duration: Duration::MAX,
                                        },
                                    )),
                                ));
                        }
                    }
                }
            }

            game.game_data.prediction_timer.add_frametime(
                self.cur_time.saturating_sub(game.game_data.last_frame_time),
                self.cur_time,
            );
            game.game_data.last_frame_time = self.cur_time;
            let game_state = &mut game.map.game;

            let tick_of_inp = game_state.predicted_game_monotonic_tick + 1;
            let ticks_per_second = game_state.game_tick_speed();

            // save the current input of all users for possible recalculations later
            let tick_inps = &mut game.game_data.input_per_tick;

            let mut player_inputs = game.player_inputs_pool.new();

            let time_per_tick = Duration::from_nanos(
                (Duration::from_secs(1).as_nanos() / ticks_per_second.get() as u128) as u64,
            );
            let ticks_to_send = game
                .game_data
                .prediction_timer
                .time_units_to_respect(time_per_tick, 7.try_into().unwrap())
                as GameTickType;
            ClientPlayer::get_and_update_latest_input(
                &mut game.game_data.local_players,
                self.cur_time,
                time_per_tick,
                ticks_to_send,
                tick_of_inp,
                &mut player_inputs,
                &game.player_inputs_chainable_pool,
                tick_inps,
            );

            game.send_input(&player_inputs, sys);
            let game_state = &mut game.map.game;
            let tick_inps = &mut game.game_data.input_per_tick;

            let add_input =
                |tick_of_inp: GameTickType, input_per_tick: &mut ClientPlayerInputPerTick| {
                    if !input_per_tick.contains_key(&tick_of_inp) {
                        input_per_tick.insert(tick_of_inp, game.game_data.player_inp_pool.new());
                    }

                    // apply input of local player to player
                    game.game_data.local_players.iter().for_each(
                        |(local_player_id, local_player)| {
                            let player_inp = input_per_tick.get_mut(&tick_of_inp).unwrap();
                            player_inp.insert(*local_player_id, local_player.sent_input);
                        },
                    );
                };
            add_input(tick_of_inp, tick_inps);

            let time_for_prediction = self.cur_time;

            let instant_input = self.config.game.cl.instant_input;
            // Reset the game state if needed
            if instant_input {
                if let Some(cur_state_snap) = game.game_data.cur_state_snap.take() {
                    let _ = game_state.build_from_snapshot(&cur_state_snap, Default::default());
                }
            }

            fn apply_input(
                predicted_game_monotonic_tick: GameTickType,
                tick_inps: &mut LinkedHashMap<
                    u64,
                    PoolLinkedHashMap<IdGeneratorIdType, PlayerInput>,
                >,
                fallback_to_prev_input: bool,
                mut on_apply: impl FnMut(&GameEntityId, &PlayerInput, CharacterInputConsumableDiff),
            ) {
                let tick_of_inp = predicted_game_monotonic_tick + 1;
                let (next_input, prev_input) = (
                    tick_inps.get(&tick_of_inp).or_else(|| {
                        tick_inps
                            .iter()
                            .rev()
                            .find_map(|(&tick, inp)| (tick <= tick_of_inp).then_some(inp))
                    }),
                    tick_inps.get(&predicted_game_monotonic_tick),
                );
                let check_input = if fallback_to_prev_input {
                    next_input.or(prev_input)
                } else {
                    next_input
                };
                if let Some(inputs) = check_input {
                    for (id, tick_inp) in inputs.iter() {
                        let mut inp = PlayerInput::default();
                        if let Some(prev_inp) =
                            prev_input.or(next_input).and_then(|inp| inp.get(id))
                        {
                            inp.inp = prev_inp.inp;
                        }
                        if let Some(diff) =
                            inp.try_overwrite(&tick_inp.inp, tick_inp.version(), true)
                        {
                            on_apply(id, tick_inp, diff);
                        }
                    }
                }
            }

            // do the ticks if necessary
            while is_next_tick(
                time_for_prediction,
                &mut game.game_data.last_game_tick,
                ticks_per_second,
            ) {
                // apply input of players
                apply_input(
                    game_state.predicted_game_monotonic_tick,
                    tick_inps,
                    false,
                    |id, tick_inp, diff| {
                        let mut inps = game.game_data.player_inputs_state_pool.new();
                        inps.insert(
                            *id,
                            CharacterInputInfo {
                                inp: tick_inp.inp,
                                diff,
                            },
                        );
                        game_state.set_player_inputs(inps);
                    },
                );

                let cur_snap = game_state.snapshot_for(SnapshotClientInfo::Everything);
                game_state.build_from_snapshot_for_prev(&cur_snap);

                game_state.predicted_game_monotonic_tick += 1;
                game_state.tick(Default::default());

                Server::dbg_game(
                    &self.config.game.dbg,
                    &game.game_data.last_game_tick,
                    game_state,
                    tick_inps
                        .get(&game_state.predicted_game_monotonic_tick)
                        .map(|inps| inps.values().map(|inp| &inp.inp)),
                    game_state.predicted_game_monotonic_tick,
                    ticks_per_second.get(),
                    &self.shared_info,
                    "client",
                );

                let mut player_ids = game.game_data.player_ids_pool.new();
                player_ids.extend(game.game_data.local_players.keys());
                let events = game_state.events_for(EventClientInfo {
                    client_player_ids: player_ids,
                    everything: true,
                    other_stages: true,
                });
                if !events.is_empty() {
                    game.events
                        .entry(game_state.predicted_game_monotonic_tick)
                        .or_insert_with(|| (events, true));
                }
                game_state.clear_events();

                // add a "dummy" input for the next tick already, since in a bad
                // case this while-loop might run again
                add_input(game_state.predicted_game_monotonic_tick + 1, tick_inps);
            }

            // next intra tick time
            game.game_data.intra_tick_time = intra_tick_time(
                self.cur_time,
                game.game_data.last_game_tick,
                ticks_per_second,
            );

            if instant_input {
                let cur_state_snap = game_state.snapshot_for(SnapshotClientInfo::Everything);
                game_state.build_from_snapshot_for_prev(&cur_state_snap);
                game.game_data.cur_state_snap = Some(cur_state_snap);

                // there is always a prediction tick
                // apply input of players for it as if it's the next tick
                let mut pred_inps = game.game_data.player_inputs_state_pool.new();
                apply_input(
                    game_state.predicted_game_monotonic_tick,
                    tick_inps,
                    true,
                    |id, tick_inp, diff| {
                        pred_inps.insert(
                            *id,
                            CharacterInputInfo {
                                inp: tick_inp.inp,
                                diff,
                            },
                        );
                    },
                );
                game_state.tick(TickOptions {
                    is_future_tick_prediction: true,
                });
            }

            game.game_data.last_game_tick = Duration::from_secs_f64(
                (game.game_data.last_game_tick.as_secs_f64()
                    + game.game_data.prediction_timer.smooth_adjustment_time())
                .clamp(0.0, f64::MAX),
            );
        }

        // rendering
        self.render(native);

        self.spatial_chat.update(
            &self.scene,
            if let Game::Active(game) = &mut self.game {
                game.spatial_world.zip_mut(
                    game.game_data
                        .local_players
                        .front()
                        .map(|(id, _)| (*id, &mut game.network)),
                )
            } else {
                SpatialChatGameWorldTyRef::None
            },
            &self.config.game,
        );

        // sleep time related stuff
        let cur_time = self.sys.time_get_nanoseconds();

        // force limit fps in menus
        let refresh_rate = if self.ui_manager.ui.ui_state.is_ui_open && self.demo_player.is_none() {
            ((self.config.engine.wnd.refresh_rate_mhz as u64 + 999) / 1000)
                .clamp(60, u64::MAX)
                .min(if self.config.game.cl.refresh_rate > 0 {
                    self.config.game.cl.refresh_rate
                } else {
                    u64::MAX
                })
        } else {
            self.config.game.cl.refresh_rate
        };
        if refresh_rate > 0 {
            let time_until_tick_nanos = Duration::from_secs(1).as_nanos() as u64 / refresh_rate;

            let sleep_time_nanos = time_until_tick_nanos as i64
                - (cur_time.as_nanos() as i64 - self.last_refresh_rate_time.as_nanos() as i64);
            if sleep_time_nanos > 0 {
                std::thread::sleep(Duration::from_nanos(sleep_time_nanos as u64));
            }

            self.last_refresh_rate_time = Duration::from_nanos(
                (cur_time.as_nanos() as i64 + sleep_time_nanos.clamp(-16666666666, 16666666666))
                    as u64,
            );
        } else {
            self.last_refresh_rate_time = cur_time;
        }

        self.inp_manager.new_frame();
    }

    fn resized(&mut self, native: &mut dyn NativeImpl, new_width: u32, new_height: u32) {
        let window_props = self.graphics_backend.resized(
            &self.graphics.backend_handle.backend_cmds,
            self.graphics.stream_handle.stream_data(),
            native,
            new_width,
            new_height,
        );
        self.graphics.resized(window_props);
        // update config variables
        let wnd = &mut self.config.engine.wnd;
        let window = native.borrow_window();
        wnd.width = new_width;
        wnd.height = new_height;
        if let Some(monitor) = window.current_monitor() {
            wnd.refresh_rate_mhz = monitor
                .refresh_rate_millihertz()
                .unwrap_or(wnd.refresh_rate_mhz);
        }
    }

    fn window_options_changed(&mut self, wnd: NativeWindowOptions) {
        let config_wnd = &mut self.config.engine.wnd;
        config_wnd.fullscreen = wnd.fullscreen;
        config_wnd.decorated = wnd.decorated;
        config_wnd.maximized = wnd.maximized;
        config_wnd.width = wnd.width;
        config_wnd.height = wnd.height;
        config_wnd.refresh_rate_mhz = wnd.refresh_rate_milli_hertz;
        config_wnd.monitor = wnd
            .monitor
            .map(|monitor| ConfigMonitor {
                name: monitor.name,
                width: monitor.size.width,
                height: monitor.size.height,
            })
            .unwrap_or_default();
    }

    fn destroy(mut self) {
        #[cfg(feature = "alloc-track")]
        {
            let report = alloc_track::backtrace_report(|_, stats| stats.allocated != stats.freed);
            std::fs::write("trace.txt", format!("BACKTRACES\n{report}"));
        }

        if !self.config.engine.ui.keep {
            self.config.engine.ui.path = Default::default();
        }

        // destroy everything
        config_fs::save(&self.config.engine, &self.io);
        game_config_fs::fs::save(&self.config.game, &self.io);
    }

    fn window_created_ntfy(&mut self, native: &mut dyn NativeImpl) -> anyhow::Result<()> {
        self.graphics_backend.window_created_ntfy(
            BackendWindow::Winit {
                window: native.borrow_window(),
            },
            &self.config.engine.dbg,
        )
    }

    fn window_destroyed_ntfy(&mut self, _native: &mut dyn NativeImpl) -> anyhow::Result<()> {
        self.graphics_backend.window_destroyed_ntfy()
    }
}
