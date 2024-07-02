use std::{num::NonZeroUsize, rc::Rc, sync::Arc, time::Duration};

use base::{
    benchmark::Benchmark,
    system::{System, SystemTimeInterface},
};
use base_fs::filesys::FileSystem;

use base_http::http::HttpClient;
use base_io::io::{Io, IoFileSys};
use base_log::log::SystemLogGroup;
use binds::binds::BindActionsHotkey;
use client_accounts::accounts::{Accounts, AccountsLoading};
use client_console::console::console::{ConsoleEvent, ConsoleRender, ConsoleRenderPipe};
use client_containers_new::entities::{EntitiesContainer, ENTITIES_CONTAINER_PATH};
use client_demo::DemoViewer;
use client_map::client_map::{ClientMapFile, ClientMapLoading, GameMap};
use client_render_base::map::{
    map_pipeline::MapPipeline,
    render_pipe::{Camera, GameStateRenderInfo, RenderPipeline, RenderPipelineBase},
};
use client_render_game::render_game::{
    RenderForPlayer, RenderGameForPlayer, RenderGameInput, RenderGameInterface,
};
use client_ui::{
    chat::user_data::ChatEvent,
    client_info::ClientInfo,
    connecting::{
        page::ConnectingUi,
        user_data::{ConnectMode, ConnectModes},
    },
    events::{UiEvent, UiEvents},
    ingame_menu::page::IngameMenuUi,
    main_menu::{
        page::MainMenuUi,
        user_data::{UiMonitor, UiMonitorVideoMode, UiMonitors},
    },
};
use config::config::{ConfigEngine, ConfigMonitor};
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
    events::{GameEvents, FIRST_EVENT_ID},
    interface::GameStateInterface,
    types::{
        character_info::NetworkCharacterInfo,
        game::{GameEntityId, GameTickType},
        network_string::NetworkString,
        render::character::CharacterRenderInfo,
    },
};
use hashlink::LinkedHashMap;
use math::math::vector::vec2;
use native::{
    input::{
        binds::{BindKey, Binds},
        InputEventHandler,
    },
    native::{
        FromNativeImpl, FromNativeLoadingImpl, KeyCode, Native, NativeCreateOptions, NativeImpl,
        NativeWindowMonitorDetails, NativeWindowOptions, PhysicalKey, PhysicalSize, WindowEvent,
    },
};
use network::network::types::NetworkInOrderChannel;
use pool::{
    datatypes::{PoolLinkedHashMap, StringPool},
    mt_datatypes::PoolLinkedHashMap as MtPoolLinkedHashMap,
    pool::Pool,
};
use raw_window_handle::RawDisplayHandle;
use rayon::ThreadPool;
use server::server::Server;
use shared::editor::editor_wasm_manager::EditorWasmManager;
use sound::sound::SoundManager;
use sound_backend::sound_backend::SoundBackend;
use ui_base::{
    font_data::{UiFontData, UiFontDataLoading},
    types::UiRenderPipe,
};
use ui_wasm_manager::{UiManagerBase, UiPageLoadingType, UiWasmManagerErrorPageErr};

use crate::{
    game::Game,
    localplayer::ClientPlayer,
    ui::pages::{
        demo::DemoPage, editor::tee::TeeEditor, loading::LoadingPage, not_found::Error404Page,
        test::ColorTest,
    },
};

use shared_base::{
    game_types::{intra_tick_time, intra_tick_time_to_ratio, is_next_tick},
    network::{
        messages::{
            MsgClAddLocalPlayer, MsgClChatMsg, MsgClInputPlayerChain, MsgClInputs,
            PlayerInputChainable,
        },
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
    game_events::{GameEventPipeline, GameEventsClient},
    input::input_handling::{InputHandling, InputHandlingEvent},
};

type UiManager = UiManagerBase<Config>;

pub type ClientPlayerInputPerTick =
    LinkedHashMap<GameTickType, PoolLinkedHashMap<GameEntityId, PlayerInput>>;

pub fn ddnet_main(
    start_arguments: Vec<String>,
    sys: System,
    cert: &[u8],
    shared_info: Arc<ServerInfo>,
) -> anyhow::Result<()> {
    let io = IoFileSys::new(|rt| {
        Arc::new(FileSystem::new(
            rt,
            &sys.log,
            "org",
            "",
            "DDNet",
            "DDNet-Accounts",
        ))
    });

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
        _logger: sys.log.logger("client"),
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
        NativeCreateOptions {
            do_bench,
            title: "DDNet".to_string(),
            sys: &sys_time,
            dbg_input,
            start_arguments,
            window: native::native::NativeWindowOptions {
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
    _logger: SystemLogGroup,
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

    sound: SoundManager,
    client: Game,
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
    cam: Camera,

    editor: Option<EditorWasmManager>,

    entities_container: EntitiesContainer,

    console: ConsoleRender,
    console_logs: String,

    ui_manager: UiManager,
    ui_events: UiEvents,
    font_data: Arc<UiFontData>,

    accounts: Arc<Accounts>,

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
        if let Some(map) = self.menu_map.continue_loading(
            &self.sound,
            &mut self.graphics,
            &self.graphics_backend,
            &self.config.engine,
            &self.sys,
        ) {
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
                        game: &GameStateRenderInfo {
                            ticks_per_second: 50,
                            intra_tick_time,
                        },
                        camera: &Camera {
                            pos: vec2::new(21.0, 15.0),
                            zoom: 1.0,
                            animation_ticks_passed: (self.sys.time_get_nanoseconds().as_millis()
                                / (1000 / 50))
                                .max(1)
                                as GameTickType,
                        },
                        entities_container: &mut self.entities_container,
                    },
                    buffered_map: &render.data.buffered_map,
                },
            )
        }
    }

    fn render_game(&mut self, native: &mut dyn NativeImpl) {
        if let Game::Active(game) = &mut self.client {
            // prepare input
            let mut events = GameEvents {
                worlds: MtPoolLinkedHashMap::new_without_pool(),
                event_id: FIRST_EVENT_ID,
            };
            std::mem::swap(&mut game.events, &mut events);

            let GameMap {
                render,
                game: game_state,
            } = &mut game.map;
            let is_menu_open = self.ui_manager.ui.ui_state.is_ui_open;

            let intra_tick_ratio = intra_tick_time_to_ratio(
                game.client_data.intra_tick_time,
                game_state.game_tick_speed(),
            );

            let player_render_infos =
                game_state.collect_characters_render_info(intra_tick_time_to_ratio(
                    game.client_data.intra_tick_time,
                    game_state.game_tick_speed(),
                ));
            let character_infos = game_state.collect_characters_info();

            let projectiles = game_state.all_projectiles(intra_tick_ratio);
            let flags = game_state.all_ctf_flags(intra_tick_ratio);
            let lasers = game_state.all_lasers(intra_tick_ratio);
            let pickups = game_state.all_pickups(intra_tick_ratio);

            let scoreboard_info = game_state.collect_scoreboard_info();

            let mut render_game_input = RenderGameInput {
                players: game.render_players_pool.new(),
                events,
                chat_msgs: {
                    let mut chat_msgs = game.client_data.chat_msgs_pool.new();
                    chat_msgs.append(&mut game.client_data.chat_msgs);
                    chat_msgs
                },
                character_render_infos: player_render_infos,
                character_infos,
                projectiles,
                flags,
                lasers,
                pickups,
                scoreboard_info,
            };

            let mut fill_for_player =
                |mut client_player: Option<(&GameEntityId, &mut ClientPlayer)>,
                 player_render_infos: &mut PoolLinkedHashMap<GameEntityId, CharacterRenderInfo>|
                 -> RenderGameForPlayer {
                    let render_for_player = match &mut client_player {
                        Some((&player_id, client_player)) => {
                            let local_player_render_info =
                                game_state.collect_character_local_render_info(&player_id);

                            Some(RenderForPlayer {
                                chat_info: if client_player.chat_input_active && !is_menu_open {
                                    Some((
                                        std::mem::take(&mut client_player.chat_msg),
                                        self.inp_manager.clone_inp().egui,
                                    ))
                                } else {
                                    None
                                },
                                scoreboard_active: client_player.show_scoreboard,

                                local_player_info: local_player_render_info,

                                player_id: player_id,
                            })
                        }
                        None => None,
                    };

                    let game_info = GameStateRenderInfo {
                        ticks_per_second: game_state.game_tick_speed(),
                        intra_tick_time: game.client_data.intra_tick_time,
                    };

                    if let Some((id, local_player)) = client_player {
                        if let Some(player) = player_render_infos.get_mut(id) {
                            player.cursor_pos = local_player.input.inp.cursor.to_vec2();
                        }
                    }

                    RenderGameForPlayer {
                        render_for_player,
                        game_state_info: game_info,
                    }
                };

            if game.client_data.local_players.is_empty() {
                render_game_input.players.push(fill_for_player(
                    None,
                    &mut render_game_input.character_render_infos,
                ))
            } else {
                let mut ids = game.client_data.local_players.iter_mut();
                let player_count = 1; // ids.len();
                if player_count == 1 {
                    let client_player = ids.next();
                    render_game_input.players.push(fill_for_player(
                        client_player,
                        &mut render_game_input.character_render_infos,
                    ));
                } else {
                    ids.for_each(|client_player| {
                        render_game_input.players.push(fill_for_player(
                            Some(client_player),
                            &mut render_game_input.character_render_infos,
                        ));
                    });
                }
            };

            let res = render.render(
                &self.config.game.map,
                &self.sys.time_get_nanoseconds(),
                render_game_input,
            );

            // handle results
            for (player_id, player_events) in res.player_events {
                for player_event in player_events {
                    match player_event {
                        ChatEvent::MsgSend(msg) => {
                            game.network.send_in_order_to_server(
                                &GameMessage::ClientToServer(ClientToServerMessage::PlayerMsg((
                                    player_id.clone(),
                                    ClientToServerPlayerMessage::Chat(MsgClChatMsg::Global {
                                        msg: NetworkString::new(&msg).unwrap(),
                                    }),
                                ))),
                                NetworkInOrderChannel::Global,
                            );
                            game.client_data
                                .local_players
                                .get_mut(&player_id)
                                .unwrap()
                                .chat_msg = msg;
                        }
                        ChatEvent::CurMsg(msg) => {
                            game.client_data
                                .local_players
                                .get_mut(&player_id)
                                .unwrap()
                                .chat_msg = msg;
                        }
                        ChatEvent::ChatClosed => {
                            game.client_data
                                .local_players
                                .get_mut(&player_id)
                                .unwrap()
                                .chat_input_active = false;
                        }
                        ChatEvent::PlatformOutput(output) => {
                            // no matter what egui reports, we don't want a cursor ingame
                            self.inp_manager
                                .handle_platform_output(native, output, true);
                        }
                    }
                }
            }

            if self.debug_hud.ui.ui_state.is_ui_open {
                self.debug_hud.render(&mut DebugHudRenderPipe {
                    graphics: &mut self.graphics,
                    prediction_timing: &game.client_data.prediction_timing,
                    ingame_timer: &game.client_data.last_game_tick,
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
                if self.console.ui.ui_state.is_ui_open {
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
                        self.console.ui.ui_state.is_ui_open,
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
                    demo_viewer.render();
                    if demo_viewer.is_finished() {
                        self.demo_player = None;
                    }
                } else {
                    demo_player
                        .continue_loading(
                            &self.sound,
                            &self.graphics,
                            &self.graphics_backend,
                            &self.config.engine,
                            &self.sys,
                        )
                        .unwrap();
                }
            } else if self.ui_manager.ui.ui_state.is_ui_open {
                // render ui last
                if let Some(output) = self.ui_manager.render(
                    &self.config.engine.ui.path.name.clone(), // TODO: useless heap allocation
                    &self.io,
                    &mut self.graphics,
                    &self.graphics_backend,
                    &mut self.sound,
                    &mut UiRenderPipe::new(self.sys.time_get_nanoseconds(), &mut self.config),
                    if self.console.ui.ui_state.is_ui_open {
                        Default::default()
                    } else {
                        self.inp_manager.take_inp().egui.unwrap_or_default()
                    },
                    native.borrow_window(),
                    true,
                ) {
                    self.inp_manager.handle_platform_output(
                        native,
                        output,
                        self.console.ui.ui_state.is_ui_open,
                    );
                }
                let ui_events = self.ui_events.take();
                for ui_event in ui_events {
                    match ui_event {
                        UiEvent::StartDemo { name } => {
                            self.demo_player =
                                Some(DemoViewer::new(&self.io, &self.thread_pool, name))
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
                        UiEvent::Connect { addr } => {
                            self.client = Game::new(
                                &self.io,
                                &self.connect_info,
                                &self.cert,
                                addr,
                                &self.accounts,
                            )
                            .unwrap();
                        }
                        UiEvent::Disconnect => {
                            self.client = Game::None;
                        }
                        UiEvent::ConnectLocalPlayer { as_dummy } => {
                            if let Game::Active(game) = &mut self.client {
                                self.client_info.set_local_player_count(
                                    self.client_info.local_player_count() + 1,
                                );
                                game.network.send_unordered_to_server(
                                    &GameMessage::ClientToServer(
                                        ClientToServerMessage::AddLocalPlayer(
                                            MsgClAddLocalPlayer {
                                                player_info: NetworkCharacterInfo::explicit_default(
                                                ), // TODO
                                                as_dummy,
                                            },
                                        ),
                                    ),
                                );
                            }
                        }
                        UiEvent::DisconnectLocalPlayer => {
                            if let Game::Active(game) = &mut self.client {
                                self.client_info.set_local_player_count(
                                    self.client_info.local_player_count() - 1,
                                );
                                if game.client_data.local_players.len() > 1 {
                                    let (player_id, _) =
                                        game.client_data.local_players.pop_back().unwrap();
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
                    }
                }
            }
        }

        // make sure no msaa blocks ui rendering
        self.graphics.backend_handle.consumble_multi_samples();
        if self.console.ui.ui_state.is_ui_open {
            let mut pipe = ConsoleRenderPipe {
                graphics: &mut self.graphics,
                sys: &self.sys,
                config: &mut self.config,
                msgs: &mut self.console_logs,
            };
            let (events, platform_output) = self.console.render(
                self.inp_manager.take_inp().egui.unwrap_or_default(),
                &mut pipe,
            );
            self.handle_console_events(native, events);
            self.inp_manager
                .handle_platform_output(native, platform_output, false);
        }

        // fps (& debug)
        self.client_stats.render(&mut ClientStatsRenderPipe {
            graphics: &mut self.graphics,
            sys: &self.sys,
            runtime_thread_pool: &self.thread_pool,
            config: &mut self.config.engine,
        });

        self.sound.swap();
        self.graphics.swap();
    }

    fn handle_console_events(&mut self, native: &mut dyn NativeImpl, events: Vec<ConsoleEvent>) {
        for event in events {
            match event {
                ConsoleEvent::Quit => native.quit(),
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

        // prepare network stuff while waiting for io
        let client_stats = ClientStats::new(&mut loading.sys);

        let sound_backend = SoundBackend::new(&loading.config_engine.snd)?;
        let sound = SoundManager::new(sound_backend)?;

        benchmark.bench("sound");

        // then prepare components allocations etc.
        let (graphics_backend, stream_data) = GraphicsBackendBase::new(
            loading.graphics_backend_io_loading,
            loading.graphics_backend_loading.take().unwrap(),
            &thread_pool,
            BackendWindow::Winit {
                window: native.borrow_window(),
            },
            &loading.config_engine.dbg,
            &loading.config_engine.gl,
        )?;
        benchmark.bench("init of graphics backend");

        let window_props = graphics_backend.get_window_props();
        let graphics_backend = GraphicsBackend::new(graphics_backend);
        let mut graphics = Graphics::new(graphics_backend.clone(), stream_data, window_props);

        let graphics_memory_usage = graphics_backend.memory_usage();
        let debug_hud = DebugHud::new(
            &mut loading.sys,
            graphics_memory_usage.texture_memory_usage,
            graphics_memory_usage.buffer_memory_usage,
            graphics_memory_usage.stream_memory_usage,
            graphics_memory_usage.staging_memory_usage,
        );

        benchmark.bench("init of graphics");

        let menu_map = ClientMapLoading::new(
            "themes/".as_ref(),
            &loading
                .config_engine
                .ui
                .menu_background_map
                .as_str()
                .try_into()
                .unwrap(),
            None,
            None,
            &io,
            &thread_pool,
            &"".try_into().unwrap(),
            None,
            true,
            Default::default(),
        );
        benchmark.bench("menu map");

        let default_entities =
            EntitiesContainer::load_default(&io, ENTITIES_CONTAINER_PATH.as_ref());
        let scene = sound.scene_handle.create();
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

        let loading_page = Box::new(LoadingPage::new());
        let page_err = UiWasmManagerErrorPageErr::default();
        let page_404 = Box::new(Error404Page::new(page_err.clone()));
        let font_data = UiFontData::new(font_loading)?;
        let mut ui_manager = UiManager::new(
            &io.fs,
            (page_404, page_err),
            UiPageLoadingType::ShowLoadingPage(loading_page),
            &font_data,
        );

        let connect_info = ConnectMode::new(ConnectModes::Connecting);
        let ui_events = UiEvents::new();
        let client_info = ClientInfo::default();

        let accounts = Arc::new(Accounts::new(accounts_loading));

        let monitors: Vec<_> = native
            .monitors()
            .into_iter()
            .map(|monitor| {
                let video_modes: Vec<_> = monitor
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
                    video_modes
                };
                UiMonitor {
                    name: monitor.name().unwrap_or_else(|| "invalid".to_string()),
                    video_modes,
                }
            })
            .collect();
        let monitors = UiMonitors::new(monitors);
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
        ));
        let tee_editor = Box::new(TeeEditor::new(&mut graphics));
        let color_test = Box::new(ColorTest::new());
        let demo_page = Box::new(DemoPage::new());
        ui_manager.register_path("", "", main_menu);
        ui_manager.register_path("", "connecting", connecting_menu);
        ui_manager.register_path("", "ingame", ingame_menu);
        ui_manager.register_path("editor", "tee", tee_editor);
        ui_manager.register_path("", "color", color_test);
        ui_manager.register_path("", "demo", demo_page);

        let cur_time = loading.sys.time_get_nanoseconds();
        let last_refresh_rate_time = cur_time;

        native.mouse_grab();

        let mut global_binds = Binds::default();
        global_binds.register_bind(
            &[BindKey::Key(PhysicalKey::Code(KeyCode::F10))],
            BindActionsHotkey::Screenshot,
        );
        global_binds.register_bind(
            &[BindKey::Key(PhysicalKey::Code(KeyCode::F1))],
            BindActionsHotkey::Console,
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

        let mut console =
            ConsoleRender::new(&mut loading.config_game, &mut loading.config_engine, native);
        console.ui.ui_state.is_ui_open = false;

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
            client: Game::None,
            cert: loading.cert,
            connect_info,
            demo_player: None,
            client_stats,
            debug_hud,
            thread_pool,
            io,
            config: Config::new(loading.config_game, loading.config_engine),
            last_refresh_rate_time,
            cam: Camera {
                pos: vec2::default(),
                zoom: 1.0,
                animation_ticks_passed: Default::default(),
            },
            editor: None,

            console,
            console_logs: Default::default(),

            ui_manager,
            ui_events,
            font_data,

            accounts,

            global_binds,
            inp_manager,

            // pools & helpers
            string_pool: Pool::with_sized(256, || String::with_capacity(256)), // TODO: random values rn
        };

        let events = client.console.get_events();
        client.handle_console_events(native, events);

        Ok(client)
    }

    fn load_with_display_handle(
        loading: &mut ClientNativeLoadingImpl,
        display_handle: RawDisplayHandle,
    ) -> anyhow::Result<()> {
        let map_pipe = MapPipeline::new();

        let graphics_backend_loading = GraphicsBackendLoading::new(
            &loading.config_engine.gfx,
            &loading.config_engine.dbg,
            &loading.config_engine.gl,
            &loading.sys,
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
        self.inp_manager.collect_events(native.borrow_window());
        self.inp_manager.handle_global_binds(
            &mut self.global_binds,
            &mut self.console.ui,
            &mut self.debug_hud.ui,
            &mut self.graphics,
            &self.io,
        );

        let sys = &mut self.sys;
        self.cur_time = sys.time_get_nanoseconds();

        self.client.update(
            &mut self.graphics,
            &self.graphics_backend,
            &self.sound,
            &self.config.engine,
            &self.config.game,
            &mut self.cam,
            sys,
        );

        GameEventsClient::update(&mut GameEventPipeline {
            graphics: &mut self.graphics,
            client: &mut self.client,
            cam: &mut self.cam,
            runtime_thread_pool: &mut self.thread_pool,
            io: &self.io,
            config: &mut self.config.engine,
            config_game: &mut self.config.game,
            shared_info: &self.shared_info,
            ui: &mut self.ui_manager.ui.ui_state,
            sys: &sys,
            string_pool: &mut self.string_pool,
            console_entries: &self.console.entries,
        });
        let has_input = !self.ui_manager.ui.ui_state.is_ui_open
            && !self.console.ui.ui_state.is_ui_open
            && self.editor.is_none();
        if let Game::Active(game) = &mut self.client {
            if has_input {
                native.toggle_cursor(false);
                self.inp_manager
                    .set_last_known_cursor(&self.config.engine, CursorIcon::None);

                let evs = self.inp_manager.handle_player_binds(
                    &mut game.client_data.local_players,
                    &mut self.ui_manager.ui,
                    &game.client_data.device_to_local_player_index,
                    &mut self.config.engine,
                    &mut self.config.game,
                );

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
                    }
                }
            }

            game.client_data.prediction_timing.add_frametime(
                self.cur_time
                    .saturating_sub(game.client_data.last_frame_time),
                self.cur_time,
            );
            game.client_data.last_frame_time = self.cur_time;
            let game_state = &mut game.map.game;

            let tick_of_inp = game_state.predicted_game_monotonic_tick + 1;
            let ticks_per_second = game_state.game_tick_speed();

            // save the current input of all users for possible recalculations later
            let tick_inps = &mut game.client_data.input_per_tick;

            let mut player_inputs = game.player_inputs_pool.new();

            let mut copied_input = None;
            for (local_player_id, local_player) in game.client_data.local_players.iter_mut() {
                if local_player.dummy_copy_moves {
                    copied_input = Some(local_player.input.inp.clone());
                } else if let Some(copied_input) = &copied_input
                    .and_then(|copied_input| local_player.is_dummy.then(|| copied_input))
                {
                    local_player.input.try_overwrite(
                        copied_input,
                        local_player.input.version() + 1,
                        true,
                    );
                }

                let should_send_rates = !local_player
                    .sent_input_time
                    .is_some_and(|time| self.cur_time - time < Duration::from_millis(20));
                if (local_player.sent_input.inp.consumable != local_player.input.inp.consumable
                    && (!local_player
                        .input
                        .inp
                        .consumable
                        .only_weapon_diff_changed(&local_player.sent_input.inp.consumable)
                        || should_send_rates))
                    || local_player.sent_input.inp.state != local_player.input.inp.state
                    || (local_player.sent_input.inp.cursor != local_player.input.inp.cursor
                        && should_send_rates)
                {
                    local_player.sent_input_time = Some(self.cur_time);

                    let net_inp = &mut local_player.input;
                    net_inp.inc_version();
                    local_player.sent_input = net_inp.clone();

                    player_inputs.insert(
                        *local_player_id,
                        PlayerInputChainable {
                            for_monotonic_tick: tick_of_inp,
                            inp: net_inp.clone(),
                        },
                    );
                }
            }

            if !player_inputs.is_empty() {
                let mut player_inputs_send = game.player_inputs_chain_pool.new();
                for (player_id, player_input) in player_inputs.iter() {
                    let mut data = game.player_inputs_chain_data_pool.new();
                    let def_inp = PlayerInputChainable::default();

                    let mut def = game.player_inputs_ser_helper_pool.new();
                    bincode::serde::encode_into_std_write(
                        def_inp,
                        &mut *def,
                        bincode::config::standard().with_fixed_int_encoding(),
                    )
                    .unwrap();
                    let mut inp = game.player_inputs_ser_helper_pool.new();
                    bincode::serde::encode_into_std_write(
                        player_input,
                        &mut *inp,
                        bincode::config::standard().with_fixed_int_encoding(),
                    )
                    .unwrap();

                    bin_patch::diff_exact_size(&def, &inp, &mut data).unwrap();

                    player_inputs_send.insert(
                        *player_id,
                        MsgClInputPlayerChain {
                            data,
                            diff_id: None,
                        },
                    );
                }
                game.network
                    .send_unordered_to_server(&GameMessage::ClientToServer(
                        ClientToServerMessage::Inputs(MsgClInputs {
                            inputs: player_inputs_send,
                        }),
                    ));
            }

            let add_input =
                |tick_of_inp: GameTickType, input_per_tick: &mut ClientPlayerInputPerTick| {
                    if !input_per_tick.contains_key(&tick_of_inp) {
                        input_per_tick.insert(tick_of_inp, game.client_data.player_inp_pool.new());
                    }

                    // apply input of local player to player
                    game.client_data.local_players.iter().for_each(
                        |(local_player_id, local_player)| {
                            let player_inp = input_per_tick.get_mut(&tick_of_inp).unwrap();
                            player_inp.insert(*local_player_id, local_player.sent_input.clone());
                        },
                    );
                };
            add_input(tick_of_inp, tick_inps);

            // do the ticks if necessary
            while is_next_tick(
                self.cur_time,
                &mut game.client_data.last_game_tick,
                ticks_per_second,
            ) {
                // apply input of players
                let tick_of_inp = game_state.predicted_game_monotonic_tick + 1;
                if let (Some(inputs), prev_input) = (
                    tick_inps.get(&tick_of_inp).or_else(|| {
                        tick_inps
                            .iter()
                            .rev()
                            .find_map(|(&tick, inp)| (tick <= tick_of_inp).then_some(inp))
                    }),
                    tick_inps.get(&game_state.predicted_game_monotonic_tick),
                ) {
                    for (id, tick_inp) in inputs.iter() {
                        let mut inp = PlayerInput::default();
                        if let Some(prev_inp) =
                            prev_input.or(Some(inputs)).map(|inp| inp.get(id)).flatten()
                        {
                            inp.inp = prev_inp.inp;
                        }
                        if let Some(diff) =
                            inp.try_overwrite(&tick_inp.inp, tick_inp.version(), true)
                        {
                            game_state.set_player_input(id, &tick_inp.inp, diff);
                        }
                    }
                }

                game_state.predicted_game_monotonic_tick += 1;
                game_state.tick();

                Server::dbg_game(
                    &self.config.game.dbg,
                    &game.client_data.last_game_tick,
                    game_state,
                    tick_inps
                        .get(&game_state.predicted_game_monotonic_tick)
                        .map(|inps| inps.values().map(|inp| &inp.inp)),
                    game_state.predicted_game_monotonic_tick,
                    ticks_per_second,
                    &self.shared_info,
                    "client",
                );

                // add a "dummy" input for the next tick already, since in a bad
                // case this while-loop might run again
                add_input(game_state.predicted_game_monotonic_tick + 1, tick_inps);
            }

            // there is always a prediction tick
            // apply input of players for it as if it's the next tick
            let tick_of_inp = game_state.predicted_game_monotonic_tick + 1;
            let (next_input, prev_input) = (
                tick_inps.get(&tick_of_inp).or_else(|| {
                    tick_inps
                        .iter()
                        .rev()
                        .find_map(|(&tick, inp)| (tick <= tick_of_inp).then_some(inp))
                }),
                tick_inps.get(&game_state.predicted_game_monotonic_tick),
            );
            let mut pred_inps = game.pred_player_inputs_pool.new();
            if let Some(inputs) = next_input.or(prev_input) {
                for (id, tick_inp) in inputs.iter() {
                    let mut inp = PlayerInput::default();
                    if let Some(prev_inp) =
                        prev_input.or(next_input).map(|inp| inp.get(id)).flatten()
                    {
                        inp.inp = prev_inp.inp;
                    }
                    if let Some(diff) = inp.try_overwrite(&tick_inp.inp, tick_inp.version(), true) {
                        pred_inps.insert(*id, (tick_inp.inp, diff));
                    }
                }
            }
            game_state.pred_tick(pred_inps);

            game.client_data.intra_tick_time = intra_tick_time(
                self.cur_time,
                game.client_data.last_game_tick,
                ticks_per_second,
            );
            game.client_data.last_game_tick = Duration::from_secs_f64(
                (game.client_data.last_game_tick.as_secs_f64()
                    + game.client_data.prediction_timing.smooth_adjustment_time())
                .clamp(0.0, f64::MAX),
            );
        }

        // rendering
        self.render(native);

        // sleep time related stuff
        let cur_time = self.sys.time_get_nanoseconds();
        if self.config.game.cl.refresh_rate > 0 {
            let time_until_tick_nanos =
                Duration::from_secs(1).as_nanos() as u64 / self.config.game.cl.refresh_rate as u64;

            let sleep_time_nanos = time_until_tick_nanos as i64
                - (cur_time.as_nanos() as i64 - self.last_refresh_rate_time.as_nanos() as i64)
                    as i64;
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

    fn destroy(self) {
        // destroy everything
        config_fs::save(&self.config.engine, &self.io);
        game_config_fs::fs::save(&self.config.game, &self.io);
    }
}
