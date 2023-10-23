use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use base::{
    benchmark::Benchmark,
    system::{System, SystemTimeInterface},
};
use base_fs::filesys::FileSystem;

use base_io::io::IO;
use base_log::log::SystemLogGroup;
use client_containers::{
    ctf::CTFContainer, emoticons::EmoticonsContainer, entities::EntitiesContainer,
    hooks::HookContainer, hud::HudContainer, particles::ParticlesContainer,
    pickups::PickupContainer, skins::SkinContainer, weapons::WeaponContainer,
};
use client_map::client_map::{ClientMap, ClientMapFile};
use client_render::{
    chat::render::{ChatRender, ChatRenderOptions, ChatRenderPipe},
    console::console::{ConsoleRender, ConsoleRenderPipe},
    killfeed::render::{KillfeedRender, KillfeedRenderPipe},
    scoreboard::render::{ScoreboardRender, ScoreboardRenderPipe},
};
use client_render_base::map::render_pipe::{
    Camera, GameStateRenderInfo, RenderPipeline, RenderPipelineBase,
};
use client_types::{chat::ServerMsg, killfeed::KillfeedMsg};
use config::config::Config;
use graphics::utils::{render_blur, render_swapped_frame};
use graphics_backend::{
    backend::{GraphicsBackend, GraphicsBackendBase},
    types::{Graphics, GraphicsBackendLoadIOPipe, GraphicsBackendLoadWhileIOPipe},
    window::BackendWindow,
};
use graphics_base_traits::traits::GraphicsSizeQuery;
use hashlink::LinkedHashMap;
use math::math::vector::{vec2, vec4};
use native::{
    input::{
        binds::{BindKey, Binds},
        InputEventHandler,
    },
    native::{FromNativeImpl, FromNativeLoadingImpl, Native, NativeCreateOptions, NativeImpl},
};
use network::network::{
    network::{NetworkClientInitOptions, NetworkConnectionID},
    quinn_network::QuinnNetwork,
};
use pool::{datatypes::StringPool, pool::Pool};
use rayon::ThreadPool;
use server::server::ServerInfo;
use shared_game::{player::player::PlayerInput, state::state::GameStateInterface};
use sound::sound::SoundManager;
use ui_base::types::{UINativePipe, UIPipe};
use ui_wasm_manager::{UIWasmLoadingType, UIWasmManager, UIWinitWrapperPipe};
use winit::{event::WindowEvent, keyboard::KeyCode};

use crate::{
    client::components::{
        players::{PlayerRenderPipe, Players},
        render::Render,
    },
    localplayer::LocalPlayers,
    render::{
        cursor::{RenderCursor, RenderCursorPipe},
        hud::{RenderHud, RenderHudPipe},
    },
    ui::pages::{
        demo::DemoPage,
        editor::tee::TeeEditor,
        loading::LoadingPage,
        menu::{
            connect_error::ConnectErrorMenu, connecting::ConnectingMenu, ingame::IngameMenu,
            main_menu::MainMenu, queue::QueueMenu, ClientUIFeedback,
        },
        not_found::Error404Page,
        test::ColorTest,
    },
};

use shared_base::{
    binds::{BindActions, BindActionsHotkey},
    game_types::{intra_tick_time, intra_tick_time_to_ratio, is_next_tick, TGameElementID},
    network::messages::MsgClInput,
    types::GameTickType,
};

use shared_network::{
    game_event_generator::GameEventGenerator,
    messages::{ClientToServerMessage, ClientToServerPlayerMessage, GameMessage},
};

use super::{
    component::ComponentUpdatePipe,
    components::{
        client_stats::{ClientStats, ClientStatsRenderPipe},
        network_logic::NetworkLogic,
        particle_manager::{ParticleGroup, ParticleManager},
        render::RenderPipe,
    },
    game_events::{GameEventPipeline, GameEventsClient},
    input::input_handling::{DeviceToLocalPlayerIndex, InputHandling, InputPipe},
};

pub struct ClientData {
    pub cur_server: NetworkConnectionID,
    pub server_connect_time: Duration,

    // the ping between the client and the server
    pub ping: Duration,

    pub queue_info: String,
    pub network_err: String,

    pub local_players: LocalPlayers,
    pub device_to_local_player_index: DeviceToLocalPlayerIndex, // TODO: keyboard and mouse are different devices
    pub input_per_tick: ClientPlayerInputPerTick,

    pub last_game_tick: Duration,
    pub last_inp_tick: Duration,
    // input sent, when close to the next game tick
    pub last_inp_near_game_tick: Duration,
    pub intra_tick_time: Duration,

    pub chat_and_system_msgs: VecDeque<ServerMsg>,
    pub killfeed_msgs: VecDeque<KillfeedMsg>,
}

impl Default for ClientData {
    fn default() -> Self {
        Self {
            cur_server: Default::default(),
            server_connect_time: Duration::default(),

            ping: Duration::default(),

            queue_info: String::new(),
            network_err: String::new(),

            local_players: LocalPlayers::new(),
            device_to_local_player_index: Default::default(),
            input_per_tick: Default::default(),

            last_game_tick: Duration::ZERO,
            last_inp_tick: Duration::ZERO,
            last_inp_near_game_tick: Duration::ZERO,
            intra_tick_time: Duration::ZERO,

            chat_and_system_msgs: Default::default(),
            killfeed_msgs: Default::default(),
        }
    }
}

pub type ClientPlayerInputPerTick =
    LinkedHashMap<GameTickType, LinkedHashMap<TGameElementID, PlayerInput>>;

pub struct Client {
    pub network_logic: NetworkLogic,
    client_stats: ClientStats,

    pub map: ClientMap,

    pub client_data: ClientData,
}

pub fn ddnet_main(mut sys: System, cert: &[u8], shared_info: Arc<ServerInfo>) {
    let cam = Camera {
        pos: vec2::default(),
        zoom: 1.0,
        animation_start_tick: Default::default(),
    };

    let io = IO::new(Arc::new(FileSystem::new(&sys.log, "org", "", "DDNet")));

    let mut config = config_fs::load(&io);
    config.dbg.bench = true;

    let network_logic = NetworkLogic::new();
    let client_stats = ClientStats::new(&mut sys);
    let client = Client {
        network_logic,
        client_stats,

        map: ClientMap::None,

        client_data: Default::default(),
    };

    let mut graphics_backend = GraphicsBackendBase::new();
    let sound = SoundManager::new();

    let benchmark = Benchmark::new(config.dbg.bench);

    // first prepare all io tasks of all components
    let mut io_pipe = GraphicsBackendLoadIOPipe {
        io: &io,
        config: &config.gfx,
    };
    graphics_backend.load_io(&mut io_pipe);
    benchmark.bench("load_io of graphics and component");

    let sys_time = sys.time.clone();
    let do_bench = config.dbg.bench;
    let dbg_input = config.inp.dbg_mode;
    let client = ClientNativeLoadingImpl {
        _logger: sys.log.logger("client"),
        sys,
        cert: cert.to_vec(),
        shared_info,
        io,
        config,
        client,
        graphics_backend,
        sound,
        cam,
    };
    Native::run_loop::<ClientNativeImpl, _>(
        client,
        NativeCreateOptions {
            do_bench,
            title: "DDNet".to_string(),
            sys: &sys_time,
            dbg_input,
        },
    )
    .unwrap(); // TODO: simply unwrap?
}

struct ClientNativeLoadingImpl {
    _logger: SystemLogGroup,
    sys: System,
    cert: Vec<u8>,
    shared_info: Arc<ServerInfo>,
    io: IO,
    config: Config,
    graphics_backend: GraphicsBackendBase,
    sound: SoundManager,
    client: Client,
    cam: Camera,
}

struct ClientNativeImpl {
    sys: System,
    _cert: Vec<u8>,
    _shared_info: Arc<ServerInfo>,

    game_events: GameEventsClient,
    game_event_generator_client: Arc<GameEventGenerator>,
    has_new_events_client: Arc<AtomicBool>,
    network_client: QuinnNetwork,
    _sound: SoundManager,
    client: Client,
    thread_pool: Arc<ThreadPool>,
    io: IO,
    config: Config,
    cur_time: Duration,
    last_refresh_rate_time: Duration,
    cam: Camera,

    players: Players,
    render: Render,
    cursor_render: RenderCursor,
    chat: ChatRender,
    killfeed: KillfeedRender,
    scoreboard: ScoreboardRender,
    console: ConsoleRender,
    console_logs: String,
    hud: RenderHud,
    particles: ParticleManager,

    skin_container: SkinContainer,
    weapon_container: WeaponContainer,
    hook_container: HookContainer,
    ctf_container: CTFContainer,
    pickup_container: PickupContainer,
    entities_container: EntitiesContainer, // TODO:
    hud_container: HudContainer,
    emoticons_container: EmoticonsContainer,
    particles_container: ParticlesContainer,

    ui_manager: UIWasmManager,

    menu_map: ClientMap,

    global_binds: Binds<BindActions>,

    // pools & helpers
    player_inps_helper: Vec<(TGameElementID, PlayerInput)>,
    player_ids_helper: Pool<Vec<TGameElementID>>,
    string_pool: StringPool,

    // put graphics at the end, so it's dropped last
    graphics: Graphics,
    graphics_backend: GraphicsBackend,
}

impl ClientNativeImpl {
    pub fn get_inp_manager<'a>(&'a mut self) -> InputHandling<'a> {
        InputHandling {
            pipe: InputPipe {
                console_ui: &mut self.console.ui,
                local_players: &mut self.client.client_data.local_players,
                ui: &mut self.ui_manager.ui,
                chat_ui: &mut self.chat.ui,
                config: &self.config,
                device_to_local_player: &self.client.client_data.device_to_local_player_index,

                global_binds: &mut self.global_binds,
                graphics: &mut self.graphics,
                io: &self.io,
            },
        }
    }

    fn render_ingame(
        &mut self,
        ticks_per_second: GameTickType,
        local_player_id: Option<&TGameElementID>,
        native: &mut dyn NativeImpl,
    ) {
        let map = self.client.map.unwrap();

        let char = match local_player_id {
            Some(id) => {
                let player_id = id.clone();
                if map.game.player_exists(&player_id) {
                    let player = self.client.client_data.local_players.get_mut(id).unwrap();
                    Some((
                        map.game.lerp_core_pos(
                            &player_id,
                            intra_tick_time_to_ratio(
                                self.client.client_data.intra_tick_time,
                                ticks_per_second,
                            ),
                        ),
                        map.game.cursor_vec2(&player_id),
                        player.chat_input_active && !self.ui_manager.ui.ui_state.is_ui_open,
                    ))
                } else {
                    None
                }
            }
            None => None,
        };
        if let Some((char_pos, _, _)) = char {
            self.cam.pos = char_pos;
            self.cam.animation_start_tick = Default::default(); // TODO!: use animation tick from player
        }

        let game_render_info = GameStateRenderInfo {
            cur_tick: map.game.cur_monotonic_tick(),
            ticks_per_second: map.game.game_tick_speed(),
        };
        let mut render_pipe = RenderPipeline::new(
            &map.data.raw,
            &map.data.images,
            &map.data.buffered_map,
            &self.config.map,
            &mut self.graphics,
            &self.sys,
            &self.client.client_data.intra_tick_time,
            &game_render_info,
            &self.cam,
            &mut self.entities_container,
            &self.io,
            &self.thread_pool,
            false,
        );
        map.data.render.render_background(&mut render_pipe);
        self.players.render(&mut PlayerRenderPipe {
            graphics: &mut self.graphics,
            sys: &self.sys,
            runtime_thread_pool: &self.thread_pool,
            config: &mut self.config,
            game: &map.game,
            client_data: &self.client.client_data,

            skins: &mut self.skin_container,
            hooks: &mut self.hook_container,
            weapons: &mut self.weapon_container,
            emoticons: &mut self.emoticons_container,

            collision: &map.data.collision,
            io: &self.io,
            camera: &self.cam,
        });
        self.render.render(&mut RenderPipe {
            particle_manager: &mut self.particles,
            sys: &self.sys,
            graphics: &mut self.graphics,
            client_data: &self.client.client_data,
            cur_tick: map.game.cur_monotonic_tick(),
            map: map,

            ctf_container: &mut self.ctf_container,
            pickup_container: &mut self.pickup_container,
            weapon_container: &mut self.weapon_container,

            camera: &self.cam,

            io: &self.io,
            runtime_thread_pool: &self.thread_pool,
        });
        self.particles
            .update(&self.sys.time_get_nanoseconds(), &map.data.collision);
        self.particles.render_group(
            ParticleGroup::ProjectileTrail,
            &mut self.particles_container,
            &mut self.graphics,
            &self.io,
            &self.thread_pool,
            &self.cam,
        );
        let game_render_info = GameStateRenderInfo {
            cur_tick: map.game.cur_monotonic_tick(),
            ticks_per_second: map.game.game_tick_speed(),
        };
        let mut render_pipe = RenderPipeline::new(
            &map.data.raw,
            &map.data.images,
            &map.data.buffered_map,
            &self.config.map,
            &mut self.graphics,
            &self.sys,
            &self.client.client_data.intra_tick_time,
            &game_render_info,
            &self.cam,
            &mut self.entities_container,
            &self.io,
            &self.thread_pool,
            false,
        );
        map.data.render.render_foreground(&mut render_pipe);

        if let Some(local_player_id) = local_player_id {
            let mut dummy_str: String = Default::default();
            let mut dummy_str_ref = &mut dummy_str;
            let mut dummy_state = &mut None;
            let mut chat_active = char.map(|(_, _, is_open)| is_open).unwrap_or(false);

            if chat_active {
                let player = self
                    .client
                    .client_data
                    .local_players
                    .get_mut(local_player_id)
                    .unwrap();
                dummy_str_ref = &mut player.chat_msg;
                dummy_state = &mut player.chat_state;
            }
            self.chat.render(&mut ChatRenderPipe {
                graphics: &mut self.graphics,
                sys: &self.sys,
                config: &mut self.config,
                msgs: &self.client.client_data.chat_and_system_msgs,
                msg: dummy_str_ref,
                options: ChatRenderOptions {
                    is_chat_input_active: &mut chat_active,
                    is_chat_show_all: false, // TODO:
                },
                ui_pipe: dummy_state,
                window: native.borrow_window(),
                network: &mut self.network_client,
                player_id: local_player_id,
            });
            if !chat_active {
                self.client
                    .client_data
                    .local_players
                    .get_mut(local_player_id)
                    .unwrap()
                    .chat_input_active = false;
            }
        }
        self.killfeed.render(&mut KillfeedRenderPipe {
            graphics: &mut self.graphics,
            sys: &self.sys,
            config: &mut self.config,
            msgs: &self.client.client_data.killfeed_msgs,
        });
        self.scoreboard.render(&mut ScoreboardRenderPipe {
            graphics: &mut self.graphics,
            sys: &self.sys,
            config: &mut self.config,
            entries: &Default::default(),
        });
        if let Some((_, mouse_cursor, _)) = char {
            self.cursor_render.render(&mut RenderCursorPipe {
                graphics: &mut self.graphics,
                io: &self.io,
                runtime_thread_pool: &self.thread_pool,
                mouse_cursor,
                weapon_container: &mut self.weapon_container,
            });
            self.hud.render(&mut RenderHudPipe {
                graphics: &mut self.graphics,
                io: &self.io,
                runtime_thread_pool: &self.thread_pool,
                hud_container: &mut self.hud_container,
            });
        }
    }

    fn render_menu_background_map(&mut self) {
        if let Some(map) =
            self.menu_map
                .continue_loading(&self.io, &mut self.graphics, &self.config, &self.sys)
        {
            let intra_tick_time = self.sys.time_get_nanoseconds();
            map.data.render.render_full(&mut RenderPipeline {
                base: RenderPipelineBase {
                    map: &map.data.raw,
                    map_images: &map.data.images,
                    config: &self.config.map,
                    graphics: &mut self.graphics,
                    sys: &self.sys,
                    intra_tick_time: &intra_tick_time,
                    game: &GameStateRenderInfo {
                        cur_tick: 10,
                        ticks_per_second: 50,
                    },
                    camera: &Camera {
                        pos: vec2::new(700.0, 500.0),
                        zoom: 1.0,
                        animation_start_tick: 0,
                    },
                    entities_container: &mut self.entities_container,
                    io: &self.io,
                    runtime_thread_pool: &self.thread_pool,
                    force_full_design_render: true,
                },
                buffered_map: &map.data.buffered_map,
            })
        }
    }

    fn render(
        &mut self,
        is_ingame: bool,
        native: &mut dyn NativeImpl,
        ticks_per_second: GameTickType,
    ) {
        //self.graphics.switch_to_dual_pass();
        self.graphics.next_switch_pass(); // TODO: remove this. backend needs to copy present fb to inactive swapping fb
        if is_ingame {
            let mut local_player_ids = self.player_ids_helper.new();

            let local_player_ids = if self.client.client_data.local_players.is_empty() {
                None
            } else {
                local_player_ids.extend(self.client.client_data.local_players.keys());
                Some(local_player_ids)
            };
            match local_player_ids {
                Some(ids) => {
                    let player_count = ids.len();
                    if player_count == 1 {
                        self.render_ingame(ticks_per_second, ids.iter().next(), native);
                    } else {
                        let players_per_row = (player_count as f64).sqrt().ceil() as usize;
                        ids.iter().enumerate().for_each(|(index, id)| {
                            let x = index % players_per_row;
                            let y = index / players_per_row;
                            let w_splitted =
                                self.graphics.window_width() as usize / players_per_row;
                            let mut h_splitted =
                                self.graphics.window_height() as usize / players_per_row;

                            if player_count <= (players_per_row * players_per_row) - players_per_row
                            {
                                h_splitted =
                                    self.graphics.window_height() as usize / (players_per_row - 1);
                            }

                            let (x, y, w, h) = (
                                (x * w_splitted) as i32,
                                (y * h_splitted) as i32,
                                w_splitted as u32,
                                h_splitted as u32,
                            );

                            self.graphics.update_viewport(x, y, w, h);
                            self.render_ingame(ticks_per_second, Some(id), native);
                        });
                        self.graphics.reset_viewport();
                    }
                }
                None => self.render_ingame(ticks_per_second, None, native),
            };
        } else {
            // menu background map
            self.render_menu_background_map();
        }

        // render ui last
        self.ui_manager.render_if_open(
            &self.config.ui.path.name.clone(), // TODO: useless heap allocation
            &self.io,
            &mut self.graphics,
            &self.graphics_backend,
            &mut UIPipe::new(
                &mut ClientUIFeedback::new(
                    &mut self.network_client,
                    &mut self.client.map,
                    &mut self.client.client_data,
                ),
                self.sys.time_get_nanoseconds(),
                &mut self.config,
                (),
            ),
            &mut UINativePipe {
                raw_inp_generator: &UIWinitWrapperPipe {
                    window: native.borrow_window(),
                },
            },
            native.borrow_window(),
            true,
        );

        if self.console.ui.ui_state.is_ui_open {
            let mut pipe = ConsoleRenderPipe {
                graphics: &mut self.graphics,
                sys: &self.sys,
                config: &mut self.config,
                msgs: &mut self.console_logs,
                window: native.borrow_window(),
                player_id: &Default::default(),
            };
            if self.console.has_blur() {
                pipe.graphics.next_switch_pass();
                self.console.render_main_frame(&mut pipe);
                render_blur(pipe.graphics, true, 13.0, &vec4::new(1.0, 1.0, 1.0, 0.15));
                render_swapped_frame(pipe.graphics);
            }

            self.console.render(&mut pipe);
        }

        // render components that want to be rendered
        self.client.client_stats.render(&mut ClientStatsRenderPipe {
            graphics: &mut self.graphics,
            sys: &self.sys,
            runtime_thread_pool: &self.thread_pool,
            config: &mut self.config,
        });

        self.graphics.swap();
    }
}

impl FromNativeLoadingImpl<ClientNativeLoadingImpl> for ClientNativeImpl {
    fn new(mut loading: ClientNativeLoadingImpl, native: &mut dyn NativeImpl) -> Self {
        let benchmark = Benchmark::new(loading.config.dbg.bench);

        let loading_page = Box::new(LoadingPage::new());
        let page_404 = Box::new(Error404Page::new());
        let mut ui_manager = UIWasmManager::new(
            native,
            &loading.io.fs,
            page_404,
            UIWasmLoadingType::ShowLoadingPage(loading_page),
        );

        // prepare network stuff while waiting for io
        let game_events = GameEventsClient::new();
        let has_new_events_client = Arc::new(AtomicBool::new(false));
        let game_event_generator_client = Arc::new(GameEventGenerator::new(
            has_new_events_client.clone(),
            loading.sys.time.clone(),
        ));
        let (network_client, _game_event_notifier) = QuinnNetwork::init_client(
            "0.0.0.0:0",
            loading.cert.as_slice(),
            game_event_generator_client.clone(),
            &loading.sys,
            NetworkClientInitOptions::new()
                .with_skip_cert_check(loading.config.dbg.untrusted_cert)
                .with_timeout(loading.config.net.timeout),
        );
        benchmark.bench("initializing network");

        // then prepare components allocations etc.
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

        let mut pipe = GraphicsBackendLoadWhileIOPipe {
            runtime_threadpool: &thread_pool,
            config: &loading.config,
            sys: &loading.sys,
            window_handling: BackendWindow::Winit {
                window: native.borrow_window(),
            },
        };
        loading.graphics_backend.init_while_io(&mut pipe);
        benchmark.bench("init_while_io of graphics and other components");

        let stream_data = loading.graphics_backend.init().unwrap();
        benchmark.bench("init of graphics");

        let window_props = *loading.graphics_backend.get_window_props();
        let graphics_backend = GraphicsBackend::new(loading.graphics_backend);
        let mut graphics = Graphics::new(graphics_backend.clone(), stream_data, window_props);

        let default_skin = SkinContainer::load("default", &loading.io, &thread_pool, &mut graphics);
        let default_weapon =
            WeaponContainer::load("default", &loading.io, &thread_pool, &mut graphics);
        let default_hook = HookContainer::load("default", &loading.io, &thread_pool, &mut graphics);
        let default_ctf = CTFContainer::load("default", &loading.io, &thread_pool, &mut graphics);
        let default_pickup =
            PickupContainer::load("default", &loading.io, &thread_pool, &mut graphics);
        let default_entities =
            EntitiesContainer::load("default", &loading.io, &thread_pool, &mut graphics);
        let default_hud = HudContainer::load("default", &loading.io, &thread_pool, &mut graphics);
        let default_emoticons =
            EmoticonsContainer::load("default", &loading.io, &thread_pool, &mut graphics);
        let default_particles =
            ParticlesContainer::load("default", &loading.io, &thread_pool, &mut graphics);

        let skin_container = SkinContainer::new(
            loading.io.clone(),
            thread_pool.clone(),
            default_skin,
            &loading.sys.log,
            "skin-container",
        );
        let weapon_container = WeaponContainer::new(
            loading.io.clone(),
            thread_pool.clone(),
            default_weapon,
            &loading.sys.log,
            "weapon-container",
        );
        let hook_container = HookContainer::new(
            loading.io.clone(),
            thread_pool.clone(),
            default_hook,
            &loading.sys.log,
            "hook-container",
        );
        let ctf_container = CTFContainer::new(
            loading.io.clone(),
            thread_pool.clone(),
            default_ctf,
            &loading.sys.log,
            "ctf-container",
        );
        let pickup_container = PickupContainer::new(
            loading.io.clone(),
            thread_pool.clone(),
            default_pickup,
            &loading.sys.log,
            "pickup-container",
        );
        let entities_container = EntitiesContainer::new(
            loading.io.clone(),
            thread_pool.clone(),
            default_entities,
            &loading.sys.log,
            "entities-container",
        );
        let hud_container = HudContainer::new(
            loading.io.clone(),
            thread_pool.clone(),
            default_hud,
            &loading.sys.log,
            "hud-container",
        );
        let emoticons_container = EmoticonsContainer::new(
            loading.io.clone(),
            thread_pool.clone(),
            default_emoticons,
            &loading.sys.log,
            "emoticons-container",
        );
        let particles_container = ParticlesContainer::new(
            loading.io.clone(),
            thread_pool.clone(),
            default_particles,
            &loading.sys.log,
            "particles-container",
        );

        let players = Players::new(&mut graphics);
        let render = Render::new(&loading.sys, &mut graphics);
        let cursor_render = RenderCursor::new(&mut graphics);
        let hud = RenderHud::new(&mut graphics);
        let particles = ParticleManager::new(&mut graphics, loading.sys.time.as_ref());

        let main_menu = Box::new(MainMenu::new(loading.shared_info.clone()));
        let connecting_menu = Box::new(ConnectingMenu::new());
        let ingame_menu = Box::new(IngameMenu::new());
        let connecting_queue_menu = Box::new(QueueMenu::new());
        let connect_err_menu = Box::new(ConnectErrorMenu::new());
        let tee_editor = Box::new(TeeEditor::new(&mut graphics));
        let color_test = Box::new(ColorTest::new());
        let demo_page = Box::new(DemoPage::new());
        ui_manager.register_path("", "", main_menu);
        ui_manager.register_path("", "connecting", connecting_menu);
        ui_manager.register_path("", "ingame", ingame_menu);
        ui_manager.register_path("", "queue", connecting_queue_menu);
        ui_manager.register_path("", "connecterror", connect_err_menu);
        ui_manager.register_path("editor", "tee", tee_editor);
        ui_manager.register_path("", "color", color_test);
        ui_manager.register_path("", "demo", demo_page);

        let cur_time = loading.sys.time_get_nanoseconds();
        loading.client.client_data.last_game_tick = cur_time;
        loading.client.client_data.last_inp_tick = cur_time;

        let last_refresh_rate_time = cur_time;

        native.mouse_grab();

        let mut global_binds = Binds::default();
        global_binds.register_bind(
            &[BindKey::Key(KeyCode::F10)],
            BindActions::Hotkeys(BindActionsHotkey::Screenshot),
        );
        global_binds.register_bind(
            &[BindKey::Key(KeyCode::F1)],
            BindActions::Hotkeys(BindActionsHotkey::Console),
        );

        let chat = ChatRender::new();
        let killfeed = KillfeedRender::new();
        let scoreboard = ScoreboardRender::new();
        let mut console = ConsoleRender::new(native);
        console.ui.ui_state.is_ui_open = false;

        benchmark.bench("finish init of client");

        Self {
            menu_map: ClientMap::new(
                &thread_pool,
                &("../themes/".to_string() + &loading.config.ui.menu_background_map),
                &loading.io,
                &mut graphics,
                &loading.config,
            ),

            sys: loading.sys,
            _cert: loading.cert,
            _shared_info: loading.shared_info,

            game_events,
            game_event_generator_client,
            has_new_events_client,
            network_client,
            graphics,
            graphics_backend,
            _sound: loading.sound,
            client: loading.client,
            thread_pool,
            io: loading.io,
            config: loading.config,
            cur_time,
            last_refresh_rate_time,
            cam: loading.cam,

            players,
            render,
            cursor_render,
            chat,
            killfeed,
            scoreboard,
            console,
            console_logs: Default::default(),
            hud,
            particles,

            skin_container,
            weapon_container,
            hook_container,
            ctf_container,
            pickup_container,
            entities_container,
            hud_container,
            emoticons_container,
            particles_container,

            ui_manager,

            global_binds,

            // pools & helpers
            player_inps_helper: Default::default(),
            player_ids_helper: Pool::with_sized(1, || Vec::with_capacity(4)),
            string_pool: Pool::with_sized(256, || String::with_capacity(256)), // TODO: random values rn
        }
    }
}

impl InputEventHandler for ClientNativeImpl {
    fn key_down(
        &mut self,
        window: &winit::window::Window,
        device: &winit::event::DeviceId,
        key: KeyCode,
    ) {
        self.get_inp_manager().key_down(window, device, &key)
    }

    fn key_up(
        &mut self,
        window: &winit::window::Window,
        device: &winit::event::DeviceId,
        key: KeyCode,
    ) {
        self.get_inp_manager().key_up(window, device, &key)
    }

    fn mouse_down(
        &mut self,
        window: &winit::window::Window,
        device: &winit::event::DeviceId,
        x: f64,
        y: f64,
        btn: &winit::event::MouseButton,
    ) {
        self.get_inp_manager().mouse_down(window, device, x, y, btn)
    }

    fn mouse_up(
        &mut self,
        window: &winit::window::Window,
        device: &winit::event::DeviceId,
        x: f64,
        y: f64,
        btn: &winit::event::MouseButton,
    ) {
        self.get_inp_manager().mouse_up(window, device, x, y, btn)
    }

    fn mouse_move(
        &mut self,
        window: &winit::window::Window,
        device: &winit::event::DeviceId,
        x: f64,
        y: f64,
        xrel: f64,
        yrel: f64,
    ) {
        self.get_inp_manager()
            .mouse_move(window, device, x, y, xrel, yrel)
    }

    fn scroll(
        &mut self,
        window: &winit::window::Window,
        device: &winit::event::DeviceId,
        x: f64,
        y: f64,
        delta: &winit::event::MouseScrollDelta,
    ) {
        self.get_inp_manager().scroll(window, device, x, y, delta)
    }

    fn raw_window_event(&mut self, window: &winit::window::Window, event: &WindowEvent) -> bool {
        self.get_inp_manager().raw_event(window, event)
    }
}

impl FromNativeImpl for ClientNativeImpl {
    fn run(&mut self, native: &mut dyn NativeImpl) {
        let sys = &mut self.sys;
        self.game_events.update(&mut GameEventPipeline {
            event_generator: &*self.game_event_generator_client,
            event_generator_has_events: &self.has_new_events_client,
            network: &mut self.network_client,
            graphics: &mut self.graphics,
            client: &mut self.client,
            cam: &mut self.cam,
            runtime_thread_pool: &mut self.thread_pool,
            io: &self.io,
            config: &mut self.config,
            ui: &mut self.ui_manager.ui.ui_state,
            sys: &sys,
            skin_container: &mut self.skin_container,
            string_pool: &mut self.string_pool,
        });
        self.cur_time = sys.time_get_nanoseconds();
        // update components that want to be updated
        let pipe = &mut ComponentUpdatePipe {
            io: &self.io,
            config: &self.config,
            map: &self.client.map,
            network: &mut self.network_client,
            sys: sys,
            client_data: &mut self.client.client_data,
        };
        self.client.network_logic.update(pipe);

        /*while is_next_inp_tick(
            cur_time,
            client.client_data.last_game_tick,
            &mut client.client_data.last_inp_tick,
        ) {
            for (index, local_player) in client.client_data.local_players.values_mut().enumerate() {
                let net_inp = &mut local_player.input;
                net_inp.version += 1;
                network_client.send_unreliable_to_server(&GameMessage::ClientToServer(
                    ClientToServerMessage::Input(MsgClInput {
                        index: index as u64,
                        version: net_inp.version,
                        inp: net_inp.inp,
                    }),
                ));
            }
        }

        // send an input as late as possible before the next tick
        if client.client_data.last_inp_near_game_tick < client.client_data.last_game_tick {
            client.client_data.last_inp_near_game_tick = client.client_data.last_game_tick;
        }
        if is_next_tick(
            cur_time + Duration::from_micros(1000),
            &mut client.client_data.last_inp_near_game_tick,
        ) {
            for (index, local_player) in client.client_data.local_players.values_mut().enumerate() {
                let net_inp = &mut local_player.input;
                net_inp.version += 1;
                network_client.send_unreliable_to_server(&GameMessage::ClientToServer(
                    ClientToServerMessage::Input(MsgClInput {
                        index: index as u64,
                        version: net_inp.version,
                        inp: net_inp.inp,
                    }),
                ));
            }
        }*/
        for (id, local_player) in self.client.client_data.local_players.iter_mut() {
            if local_player.sent_input != local_player.input {
                let net_inp = &mut local_player.input;
                net_inp.version += 1;
                local_player.sent_input = net_inp.clone();
                self.network_client
                    .send_unreliable_to_server(&GameMessage::ClientToServer(
                        ClientToServerMessage::PlayerMsg((
                            id.clone(),
                            ClientToServerPlayerMessage::Input(MsgClInput {
                                version: net_inp.version,
                                inp: net_inp.inp,
                            }),
                        )),
                    ));
            }
        }

        let has_map = self
            .client
            .map
            .continue_loading(&self.io, &mut self.graphics, &self.config, &sys)
            .is_some();

        let mut ticks_per_second = 0;
        if has_map {
            let (_, game) = self.client.map.unwrap_data_and_game_mut();
            // apply input of local player to player
            self.client.client_data.local_players.iter().for_each(
                |(local_player_id, local_player)| {
                    game.set_player_inp(
                        &local_player_id,
                        &local_player.input.inp,
                        local_player.input.version,
                        false,
                    );
                },
            );

            ticks_per_second = game.game_tick_speed();

            let tick_of_inp = game.cur_monotonic_tick() + 1;
            // save the current input of all users for possible recalculations later
            let tick_inps = &mut self.client.client_data.input_per_tick;
            if !tick_inps.contains_key(&tick_of_inp) {
                tick_inps.insert(tick_of_inp, Default::default());
            }

            game.players_inputs(&mut self.player_inps_helper);
            for (player_id, player_inp) in self.player_inps_helper.drain(..) {
                tick_inps
                    .get_mut(&tick_of_inp)
                    .unwrap()
                    .insert(player_id, player_inp);
            }

            // do the ticks if necessary
            while is_next_tick(
                self.cur_time,
                &mut self.client.client_data.last_game_tick,
                ticks_per_second,
            ) {
                game.tick();
            }

            // after the tick, there is always a prediction tick
            game.pred_tick();

            self.client.client_data.intra_tick_time = intra_tick_time(
                self.cur_time,
                self.client.client_data.last_game_tick,
                ticks_per_second,
            );
        }

        // rendering
        self.render(has_map, native, ticks_per_second);

        // sleep time related stuff
        let cur_time = self.sys.time_get_nanoseconds();
        if self.config.cl.refresh_rate > 0 {
            let time_until_tick_nanos =
                Duration::from_secs(1).as_nanos() as u64 / self.config.cl.refresh_rate as u64;

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
    }

    fn resized(&mut self, native: &mut dyn NativeImpl, new_width: u32, new_height: u32) {
        let window_props = self.graphics_backend.resized(
            &self.graphics.backend_handle.backend_cmds,
            &self.graphics.stream_handle.stream_data,
            native,
            new_width,
            new_height,
        );
        self.graphics.resized(window_props)
    }

    fn destroy(mut self) {
        // destroy everything
        config_fs::save(&self.config, &self.io);

        self.network_client.close();
    }
}
