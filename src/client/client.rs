use std::{
    num::NonZeroUsize,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use base::{
    benchmark,
    system::{System, SystemTimeInterface},
};
use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};

use base_log::log::SystemLogGroup;
use client_render::{
    containers::{
        ctf::CTFContainer, entities::EntitiesContainer, hooks::HookContainer, hud::HudContainer,
        pickups::PickupContainer, skins::SkinContainer, weapons::WeaponContainer,
    },
    map::{
        client_map::ClientMap,
        render_pipe::{Camera, RenderPipeline},
    },
};
use config::config::Config;
use graphics_backend::{
    backend::{GraphicsBackend, GraphicsBackendBase},
    types::{Graphics, GraphicsBackendLoadIOPipe, GraphicsBackendLoadWhileIOPipe},
    window::BackendWindow,
};
use graphics_base_traits::traits::GraphicsSizeQuery;
use hashlink::LinkedHashMap;
use math::math::vector::{dvec2, vec2};
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
use pool::pool::Pool;
use rayon::ThreadPool;
use server::server::ServerInfo;
use shared_game::{player::player::PlayerInput, state::state::GameStateInterface};
use sound::sound::SoundManager;
use ui_base::types::UIPipe;
use ui_wasm_manager::{UIWasmLoadingType, UIWasmManager, UIWinitWrapperPipe};
use winit::{event::WindowEvent, keyboard::KeyCode};

use crate::{
    client::{
        component::ComponentLoadable,
        components::{
            players::{PlayerRenderPipe, Players},
            render::Render,
        },
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
    component::{
        ComponentLoadIOPipe, ComponentLoadPipe, ComponentLoadWhileIOPipe, ComponentUpdatable,
        ComponentUpdatePipe,
    },
    components::{
        client_stats::{ClientStats, ClientStatsRenderPipe},
        network_logic::NetworkLogic,
        particle_manager::ParticleManager,
        render::RenderPipe,
    },
    game_events::{GameEventPipeline, GameEventsClient},
    input::{DeviceToLocalPlayerIndex, InputHandling, InputPipe},
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

    let fs = Arc::new(FileSystem::new(&sys.log, "org", "", "DDNet"));

    // tokio runtime for client side tasks
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2) // should be at least 2
        .max_blocking_threads(2) // must be at least 2
        .build()
        .unwrap();

    let io_batcher = TokIOBatcher::new(rt);

    let mut config = config_fs::load(&fs, &io_batcher);
    config.dbg.bench = true;

    let network_logic = NetworkLogic::new();
    let client_stats = ClientStats::new(&mut sys);
    let mut client = Client {
        network_logic,
        client_stats,

        map: ClientMap::None,

        client_data: Default::default(),
    };

    let mut graphics_backend = GraphicsBackendBase::new();
    let sound = SoundManager::new();
    // load all components
    benchmark!(
        config.dbg.bench,
        sys,
        "load_io of graphics and component",
        || {
            // first prepare all io tasks of all components
            let mut io_pipe = GraphicsBackendLoadIOPipe {
                fs: &fs,
                io_batcher: &io_batcher,
                config: &config.gfx,
            };
            graphics_backend.load_io(&mut io_pipe);
            // first prepare all io tasks of all components
            let mut io_pipe = ComponentLoadIOPipe {
                fs: &fs,
                batcher: &io_batcher,
                config: &config,
            };
            client.network_logic.load_io(&mut io_pipe);
            client.client_stats.load_io(&mut io_pipe);
        }
    );

    let sys_time = sys.time.clone();
    let do_bench = config.dbg.bench;
    let dbg_input = config.inp.dbg_mode;
    let client = ClientNativeLoadingImpl {
        _logger: sys.log.logger("client"),
        sys,
        cert: cert.to_vec(),
        shared_info,
        io_batcher,
        fs,
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
}

struct ClientNativeLoadingImpl {
    _logger: SystemLogGroup,
    sys: System,
    cert: Vec<u8>,
    shared_info: Arc<ServerInfo>,
    io_batcher: TokIOBatcher,
    fs: Arc<FileSystem>,
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
    io_batcher: TokIOBatcher,
    fs: Arc<FileSystem>,
    config: Config,
    cur_time: Duration,
    last_refresh_rate_time: Duration,
    cam: Camera,

    players: Players,
    render: Render,
    cursor_render: RenderCursor,
    hud: RenderHud,
    particles: ParticleManager,

    skin_container: SkinContainer,
    weapon_container: WeaponContainer,
    hook_container: HookContainer,
    ctf_container: CTFContainer,
    pickup_container: PickupContainer,
    entities_container: EntitiesContainer, // TODO:
    hud_container: HudContainer,

    ui_manager: UIWasmManager,

    player_inps_helper: Vec<(TGameElementID, PlayerInput)>,
    player_ids_helper: Pool<Vec<TGameElementID>>,

    global_binds: Binds<BindActions>,

    // put graphics at the end, so it's dropped last
    graphics: Graphics,
    graphics_backend: GraphicsBackend,
}

impl ClientNativeImpl {
    pub fn get_inp_manager<'a>(&'a mut self) -> InputHandling<'a> {
        InputHandling {
            pipe: InputPipe {
                local_players: &mut self.client.client_data.local_players,
                ui: &mut self.ui_manager.ui,
                config: &self.config,
                device_to_local_player: &self.client.client_data.device_to_local_player_index,

                global_binds: &mut self.global_binds,
                graphics: &mut self.graphics,
                fs: &self.fs,
                io_batcher: &self.io_batcher,
            },
        }
    }

    fn render_ingame(
        &mut self,
        ticks_per_second: GameTickType,
        local_player_id: Option<&TGameElementID>,
    ) {
        let map = self.client.map.unwrap();

        let char: Option<(vec2, dvec2)> = match local_player_id {
            Some(id) => {
                let player_id = id.clone();
                if map.game.player_exists(&player_id) {
                    Some((
                        map.game.lerp_core_pos(
                            &player_id,
                            intra_tick_time_to_ratio(
                                self.client.client_data.intra_tick_time,
                                ticks_per_second,
                            ),
                        ),
                        map.game.cursor_vec2(&player_id),
                    ))
                } else {
                    None
                }
            }
            None => None,
        };
        if let Some((char_pos, _)) = char {
            self.cam.pos = char_pos;
            self.cam.animation_start_tick = Default::default(); // TODO!: use animation tick from player
        }

        let mut render_pipe = RenderPipeline::new(
            &map.data.raw,
            &map.data.images,
            &map.data.buffered_map,
            &self.config.map,
            &mut self.graphics,
            &self.sys,
            &self.client.client_data.intra_tick_time,
            &map.game,
            &self.cam,
            &mut self.entities_container,
            &self.fs,
            &self.io_batcher,
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
            collision: &map.data.collision,
            fs: &self.fs,
            io_batcher: &self.io_batcher,
            camera: &self.cam,
        });
        self.render.render(&mut RenderPipe {
            effects: &mut self.particles,
            sys: &self.sys,
            graphics: &mut self.graphics,
            client_data: &self.client.client_data,
            cur_tick: map.game.cur_monotonic_tick(),
            map: map,

            ctf_container: &mut self.ctf_container,
            pickup_container: &mut self.pickup_container,
            weapon_container: &mut self.weapon_container,

            camera: &self.cam,

            fs: &self.fs,
            io_batcher: &self.io_batcher,
            runtime_thread_pool: &self.thread_pool,
        });
        let mut render_pipe = RenderPipeline::new(
            &map.data.raw,
            &map.data.images,
            &map.data.buffered_map,
            &self.config.map,
            &mut self.graphics,
            &self.sys,
            &self.client.client_data.intra_tick_time,
            &map.game,
            &self.cam,
            &mut self.entities_container,
            &self.fs,
            &self.io_batcher,
            &self.thread_pool,
            false,
        );
        map.data.render.render_foreground(&mut render_pipe);
        if let Some((_, mouse_cursor)) = char {
            self.cursor_render.render(&mut RenderCursorPipe {
                graphics: &mut self.graphics,
                fs: &self.fs,
                io_batcher: &self.io_batcher,
                runtime_thread_pool: &self.thread_pool,
                mouse_cursor,
                weapon_container: &mut self.weapon_container,
            });
            self.hud.render(&mut RenderHudPipe {
                graphics: &mut self.graphics,
                fs: &self.fs,
                io_batcher: &self.io_batcher,
                runtime_thread_pool: &self.thread_pool,
                hud_container: &mut self.hud_container,
            });
        }
    }

    fn render(
        &mut self,
        is_ingame: bool,
        native: &mut dyn NativeImpl,
        ticks_per_second: GameTickType,
    ) {
        if is_ingame {
            //graphics.switch_to_dual_pass();

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
                        self.render_ingame(ticks_per_second, ids.iter().next());
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
                            self.render_ingame(ticks_per_second, Some(id));
                        });
                        self.graphics.reset_viewport();
                    }
                }
                None => self.render_ingame(ticks_per_second, None),
            };
        }
        // render components that want to be rendered
        self.client.client_stats.render(&mut ClientStatsRenderPipe {
            graphics: &mut self.graphics,
            sys: &self.sys,
            runtime_thread_pool: &self.thread_pool,
            config: &mut self.config,
        });

        /*let mut quads = graphics.quads_begin();
        quads.set_colors_from_single(1.0, 1.0, 1.0, 1.0);
        quads.quads_set_subset_free(0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0);
        quads.map_canvas(0.0, 0.0, 1.0, 1.0);
        quads.quads_draw_tl(&[CQuadItem::new(0.0, 0.0, 0.5, 0.75)]);
        drop(quads);
        graphics.last_render_call_as_second_pass_transition();*/

        // render ui last
        self.ui_manager.render_if_open(
            &self.config.ui.path.name.clone(),
            &self.fs,
            &self.io_batcher,
            &mut self.graphics,
            &self.graphics_backend,
            &mut UIPipe {
                cur_time: self.sys.time_get_nanoseconds(),
                ui_feedback: &mut ClientUIFeedback::new(
                    &mut self.network_client,
                    &self.fs,
                    &self.io_batcher,
                    &mut self.client.map,
                    &mut self.client.client_data,
                ),
                config: &mut self.config,
                raw_inp_generator: &UIWinitWrapperPipe {
                    window: native.borrow_window(),
                },
            },
        );

        self.graphics.swap();
    }
}

impl FromNativeLoadingImpl<ClientNativeLoadingImpl> for ClientNativeImpl {
    fn new(mut loading: ClientNativeLoadingImpl, native: &mut dyn NativeImpl) -> Self {
        let loading_page = Box::new(LoadingPage::new());
        let page_404 = Box::new(Error404Page::new());
        let mut ui_manager = UIWasmManager::new(
            native,
            &loading.fs,
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

        benchmark!(
            loading.config.dbg.bench,
            loading.sys,
            "init_while_io of graphics and component",
            || {
                let mut pipe = GraphicsBackendLoadWhileIOPipe {
                    runtime_threadpool: &thread_pool,
                    config: &loading.config,
                    sys: &loading.sys,
                    window_handling: BackendWindow::Winit {
                        window: native.borrow_window(),
                    },
                };
                loading.graphics_backend.init_while_io(&mut pipe);
                let mut pipe = ComponentLoadWhileIOPipe {
                    runtime_threadpool: &thread_pool,
                    config: &loading.config,
                    sys: &loading.sys,
                };
                loading.client.network_logic.init_while_io(&mut pipe);
                loading.client.client_stats.init_while_io(&mut pipe);
            }
        );

        let default_skin = SkinContainer::load(
            "default",
            &loading.fs,
            &loading.io_batcher,
            thread_pool.clone(),
        );
        let default_weapon = WeaponContainer::load(
            "default",
            &loading.fs,
            &loading.io_batcher,
            thread_pool.clone(),
        );
        let default_hook = HookContainer::load(
            "default",
            &loading.fs,
            &loading.io_batcher,
            thread_pool.clone(),
        );
        let default_ctf = CTFContainer::load(
            "default",
            &loading.fs,
            &loading.io_batcher,
            thread_pool.clone(),
        );
        let default_pickup = PickupContainer::load(
            "default",
            &loading.fs,
            &loading.io_batcher,
            thread_pool.clone(),
        );
        let default_entities = EntitiesContainer::load(
            "default",
            &loading.fs,
            &loading.io_batcher,
            thread_pool.clone(),
        );
        let default_hud = HudContainer::load(
            "default",
            &loading.fs,
            &loading.io_batcher,
            thread_pool.clone(),
        );

        let stream_data = benchmark!(
            loading.config.dbg.bench,
            loading.sys,
            "init of graphics",
            || { loading.graphics_backend.init().unwrap() }
        );

        let window_props = *loading.graphics_backend.get_window_props();
        let graphics_backend = GraphicsBackend::new(loading.graphics_backend);
        let mut graphics = Graphics::new(graphics_backend.clone(), stream_data, window_props);
        benchmark!(
            loading.config.dbg.bench,
            loading.sys,
            "init of component",
            || {
                // at last, fulfill the initialization of the component
                let mut load_pipe = ComponentLoadPipe {
                    graphics: &mut graphics,
                    config: &loading.config,
                };
                loading.client.network_logic.init(&mut load_pipe).unwrap();
                loading.client.client_stats.init(&mut load_pipe).unwrap();
            }
        );

        let skin_container = SkinContainer::new(default_skin);
        let weapon_container = WeaponContainer::new(default_weapon);
        let hook_container = HookContainer::new(default_hook);
        let ctf_container = CTFContainer::new(default_ctf);
        let pickup_container = PickupContainer::new(default_pickup);
        let entities_container = EntitiesContainer::new(default_entities);
        let hud_container = HudContainer::new(default_hud);

        let players = Players::new(&mut graphics);
        let render = Render::new(&loading.sys, &mut graphics);
        let cursor_render = RenderCursor::new(&mut graphics);
        let hud = RenderHud::new(&mut graphics);
        let particles = ParticleManager::new(&mut graphics);

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

        Self {
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
            io_batcher: loading.io_batcher,
            fs: loading.fs,
            config: loading.config,
            cur_time,
            last_refresh_rate_time,
            cam: loading.cam,

            players,
            render,
            cursor_render,
            hud,
            particles,

            skin_container,
            weapon_container,
            hook_container,
            ctf_container,
            pickup_container,
            entities_container,
            hud_container,

            ui_manager,

            global_binds,

            player_inps_helper: Default::default(),
            player_ids_helper: Pool::with_sized(1, || Vec::with_capacity(4)),
        }
    }
}

impl InputEventHandler for ClientNativeImpl {
    fn key_down(&mut self, device: &winit::event::DeviceId, key: KeyCode) {
        self.get_inp_manager().key_down(device, &key)
    }

    fn key_up(&mut self, device: &winit::event::DeviceId, key: KeyCode) {
        self.get_inp_manager().key_up(device, &key)
    }

    fn mouse_down(
        &mut self,
        device: &winit::event::DeviceId,
        x: f64,
        y: f64,
        btn: &winit::event::MouseButton,
    ) {
        self.get_inp_manager().mouse_down(device, x, y, btn)
    }

    fn mouse_up(
        &mut self,
        device: &winit::event::DeviceId,
        x: f64,
        y: f64,
        btn: &winit::event::MouseButton,
    ) {
        self.get_inp_manager().mouse_up(device, x, y, btn)
    }

    fn mouse_move(
        &mut self,
        device: &winit::event::DeviceId,
        x: f64,
        y: f64,
        xrel: f64,
        yrel: f64,
    ) {
        self.get_inp_manager().mouse_move(device, x, y, xrel, yrel)
    }

    fn raw_window_event<'a>(
        &mut self,
        window: &winit::window::Window,
        event: &WindowEvent<'a>,
    ) -> bool {
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
            io_batcher: &mut self.io_batcher,
            fs: &self.fs,
            config: &mut self.config,
            ui: &mut self.ui_manager.ui.ui_state,
            sys: &sys,
            skin_container: &mut self.skin_container,
        });
        self.cur_time = sys.time_get_nanoseconds();
        // update components that want to be updated
        let pipe = &mut ComponentUpdatePipe {
            fs: &self.fs,
            batcher: &mut self.io_batcher,
            config: &self.config,
            map: &self.client.map,
            network: &mut self.network_client,
            sys: sys,
            client_data: &mut self.client.client_data,
        };
        self.client.network_logic.update(pipe);
        self.client.client_stats.update(pipe);

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
            .continue_loading(
                &self.io_batcher,
                &self.fs,
                &mut self.graphics,
                &self.config,
                &sys,
            )
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
        config_fs::save(&self.config, &self.fs, &self.io_batcher);

        self.network_client.close();
    }
}
