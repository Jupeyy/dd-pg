use std::{
    num::NonZeroUsize,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::Duration,
};

use base::{
    benchmark,
    system::{LogLevel, System, SystemLogGroup, SystemLogInterface, SystemTimeInterface},
};
use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};

use config::config::Config;
use graphics_base::streaming::DrawScopeImpl;
use graphics_render_traits::GraphicsRenderGeometry;
use graphics_types::{rendering::State, types::CQuadItem};
use hashlink::LinkedHashMap;
use math::math::vector::{dvec2, vec2};
use native::{
    input::InputEventHandler,
    native::{FromNativeImpl, FromNativeLoadingImpl, Native, NativeCreateOptions, NativeImpl},
};
use network::network::{
    network::{NetworkClientInitOptions, NetworkConnectionID},
    quinn_network::QuinnNetwork,
};
use rayon::ThreadPool;
use server::server::ServerInfo;
use shared_game::{player::player::PlayerInput, state::state::GameStateInterface};
use sound::sound::SoundManager;
use ui_base::types::UIPipe;
use ui_wasm_manager::{UIWasmLoadingType, UIWasmManager, UIWinitWrapperPipe};
use winit::{event::WindowEvent, keyboard::KeyCode};

use crate::{
    client::{
        component::{ComponentDestroyPipe, ComponentLoadable},
        components::{
            players::{PlayerRenderPipe, Players},
            render::Render,
        },
    },
    client_map::ClientMap,
    containers::{
        ctf::CTFContainer, entities::EntitiesContainer, hooks::HookContainer,
        pickups::PickupContainer, skins::SkinContainer, weapons::WeaponContainer,
    },
    localplayer::LocalPlayers,
    ui::pages::{
        demo::DemoPage,
        editor::tee::TeeEditor,
        loading::LoadingPage,
        menu::{main_menu::MainMenu, ClientUIFeedback},
        not_found::Error404Page,
        test::ColorTest,
    },
};

use shared_base::{
    game_types::{intra_tick_time, intra_tick_time_to_ratio, is_next_tick, TGameElementID},
    network::messages::MsgClInput,
    types::GameTickType,
};

use shared_network::{
    game_event_generator::GameEventGenerator,
    messages::{ClientToServerMessage, GameMessage},
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
    render_pipe::{Camera, ClientInterface, RenderPipeline},
};

use graphics::{
    self,
    graphics::Graphics,
    traits::{GraphicsLoadIOPipe, GraphicsLoadWhileIOPipe},
};

pub struct ClientData {
    pub cur_server: NetworkConnectionID,
    pub server_connect_time: Duration,

    // the ping between the client and the server
    pub ping: Duration,

    pub local_players: LocalPlayers,
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

            local_players: LocalPlayers::new(),
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

impl ClientInterface for Client {}

pub fn ddnet_main(mut sys: System, cert: &[u8], shared_info: Arc<ServerInfo>) {
    let cam = Camera {
        pos: vec2::default(),
        zoom: 1.0,
        animation_start_tick: Default::default(),
    };

    let fs = Arc::new(FileSystem::new(&sys.log));

    // tokio runtime for client side tasks
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2) // should be at least 2
        .max_blocking_threads(2) // must be at least 2
        .build()
        .unwrap();

    let io_batcher = Arc::new(std::sync::Mutex::new(TokIOBatcher::new(rt, &sys.log)));

    let mut config = config_fs::load(&fs, &io_batcher);
    config.dbg_bench = true;

    let network_logic = NetworkLogic::new();
    let client_stats = ClientStats::new(&mut sys);
    let mut client = Client {
        network_logic,
        client_stats,

        map: ClientMap::None,

        client_data: Default::default(),
    };

    let mut graphics = Graphics::new();
    let sound = SoundManager::new();
    // load all components
    benchmark!(
        config.dbg_bench,
        sys,
        "load_io of graphics and component",
        || {
            // first prepare all io tasks of all components
            let mut io_pipe = GraphicsLoadIOPipe {
                fs: &fs,
                batcher: &io_batcher,
                config: &config,
            };
            graphics.load_io(&mut io_pipe);
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
    let do_bench = config.dbg_bench;
    let client = ClientNativeLoadingImpl {
        logger: sys.log.logger("client"),
        sys,
        cert: cert.to_vec(),
        shared_info,
        io_batcher,
        fs,
        config,
        client,
        graphics,
        sound,
        cam,
    };
    let native = Native::run_loop::<ClientNativeImpl, _>(
        client,
        NativeCreateOptions {
            do_bench: do_bench,
            title: "DDNet".to_string(),
            sys: &sys_time,
        },
    );
}

struct ClientNativeLoadingImpl {
    logger: SystemLogGroup,
    sys: System,
    cert: Vec<u8>,
    shared_info: Arc<ServerInfo>,
    io_batcher: Arc<Mutex<TokIOBatcher>>,
    fs: Arc<FileSystem>,
    config: Config,
    graphics: Graphics,
    sound: SoundManager,
    client: Client,
    cam: Camera,
}

struct ClientNativeImpl {
    sys: System,
    cert: Vec<u8>,
    shared_info: Arc<ServerInfo>,

    game_events: GameEventsClient,
    game_event_generator_client: Arc<GameEventGenerator>,
    has_new_events_client: Arc<AtomicBool>,
    network_client: QuinnNetwork,
    graphics: Graphics,
    sound: SoundManager,
    client: Client,
    thread_pool: Arc<ThreadPool>,
    io_batcher: Arc<Mutex<TokIOBatcher>>,
    fs: Arc<FileSystem>,
    config: Config,
    cur_time: Duration,
    last_refresh_rate_time: Duration,
    cam: Camera,
    players: Players,
    render: Render,
    particles: ParticleManager,

    skin_container: SkinContainer,
    weapon_container: WeaponContainer,
    hook_container: HookContainer,
    ctf_container: CTFContainer,
    pickup_container: PickupContainer,
    entities_container: EntitiesContainer,

    ui_manager: UIWasmManager,

    device_to_local_player_index: DeviceToLocalPlayerIndex, // TODO: keyboard and mouse are different devices

    player_inps_helper: Vec<(TGameElementID, PlayerInput)>,
}

impl ClientNativeImpl {
    pub fn get_inp_manager<'a>(&'a mut self) -> InputHandling<'a> {
        InputHandling {
            pipe: InputPipe {
                local_players: &mut self.client.client_data.local_players,
                ui: &mut self.ui_manager.ui,
                config: &self.config,
                device_to_local_player: &self.device_to_local_player_index,
            },
        }
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
            if loading.config.dbg_untrusted_cert {
                Some(
                    NetworkClientInitOptions::new()
                        .with_skip_cert_check(true)
                        .with_timeout(loading.config.net_timeout),
                )
            } else {
                None
            },
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
            loading.config.dbg_bench,
            loading.sys,
            "init_while_io of graphics and component",
            || {
                let mut pipe = GraphicsLoadWhileIOPipe {
                    runtime_threadpool: &thread_pool,
                    config: &loading.config,
                    sys: &loading.sys,
                    window_handling: native,
                };
                loading.graphics.init_while_io(&mut pipe);
                let mut pipe = ComponentLoadWhileIOPipe {
                    runtime_threadpool: &thread_pool,
                    config: &loading.config,
                    sys: &loading.sys,
                };
                loading.client.network_logic.init_while_io(&mut pipe);
                loading.client.client_stats.init_while_io(&mut pipe);
            }
        );

        let default_skin = SkinContainer::load("default", &loading.fs, &loading.io_batcher);
        let default_weapon = WeaponContainer::load("default", &loading.fs, &loading.io_batcher);
        let default_hook = HookContainer::load("default", &loading.fs, &loading.io_batcher);
        let default_ctf = CTFContainer::load("default", &loading.fs, &loading.io_batcher);
        let default_pickup = PickupContainer::load("default", &loading.fs, &loading.io_batcher);
        let default_entities = EntitiesContainer::load("default", &loading.fs, &loading.io_batcher);

        benchmark!(
            loading.config.dbg_bench,
            loading.sys,
            "init of graphics and component",
            || {
                // at last, fulfill the initialization of the component
                if let Err(err) = loading.graphics.init(&loading.io_batcher) {
                    loading.logger.log(LogLevel::Info).msg(&err.to_string());
                }
                let mut load_pipe = ComponentLoadPipe {
                    graphics: &mut loading.graphics,
                    config: &loading.config,
                };
                loading.client.network_logic.init(&mut load_pipe).unwrap();
                loading.client.client_stats.init(&mut load_pipe).unwrap();
            }
        );

        let skin_container =
            SkinContainer::new(default_skin, &loading.io_batcher, &mut loading.graphics);
        let weapon_container =
            WeaponContainer::new(default_weapon, &loading.io_batcher, &mut loading.graphics);
        let hook_container =
            HookContainer::new(default_hook, &loading.io_batcher, &mut loading.graphics);
        let ctf_container =
            CTFContainer::new(default_ctf, &loading.io_batcher, &mut loading.graphics);
        let pickup_container =
            PickupContainer::new(default_pickup, &loading.io_batcher, &mut loading.graphics);
        let entities_container =
            EntitiesContainer::new(default_entities, &loading.io_batcher, &mut loading.graphics);

        let players = Players::new(&mut loading.graphics);
        let render = Render::new(&loading.sys, &mut loading.graphics);
        let particles = ParticleManager::new(&mut loading.graphics);

        let main_menu = Box::new(MainMenu::new(loading.shared_info.clone()));
        let tee_editor = Box::new(TeeEditor::new(&mut loading.graphics));
        let color_test = Box::new(ColorTest::new());
        let demo_page = Box::new(DemoPage::new());
        ui_manager.register_path("", "", main_menu, &mut loading.graphics);
        ui_manager.register_path("editor", "tee", tee_editor, &mut loading.graphics);
        ui_manager.register_path("", "color", color_test, &mut loading.graphics);
        ui_manager.register_path("", "demo", demo_page, &mut loading.graphics);

        let cur_time = loading.sys.time_get_nanoseconds();
        loading.client.client_data.last_game_tick = cur_time;
        loading.client.client_data.last_inp_tick = cur_time;

        let last_refresh_rate_time = cur_time;

        native.mouse_grab();

        Self {
            sys: loading.sys,
            cert: loading.cert,
            shared_info: loading.shared_info,

            game_events,
            game_event_generator_client,
            has_new_events_client,
            network_client,
            graphics: loading.graphics,
            sound: loading.sound,
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
            particles,

            skin_container,
            weapon_container,
            hook_container,
            ctf_container,
            pickup_container,
            entities_container,

            ui_manager,
            device_to_local_player_index: Default::default(),

            player_inps_helper: Default::default(),
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
            runtime_thread_pool: &mut self.thread_pool,
            io_batcher: &mut self.io_batcher,
            fs: &self.fs,
            config: &self.config,
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
        for (index, local_player) in self
            .client
            .client_data
            .local_players
            .values_mut()
            .enumerate()
        {
            if local_player.sent_input != local_player.input {
                let net_inp = &mut local_player.input;
                net_inp.version += 1;
                local_player.sent_input = net_inp.clone();
                self.network_client
                    .send_unreliable_to_server(&GameMessage::ClientToServer(
                        ClientToServerMessage::Input(MsgClInput {
                            index: index as u64,
                            version: net_inp.version,
                            inp: net_inp.inp,
                        }),
                    ));
            }
        }

        let has_map = self
            .client
            .map
            .continue_loading(
                &self.thread_pool,
                &self.io_batcher,
                &self.fs,
                &mut self.graphics,
                &self.config,
                &sys.time,
            )
            .is_some();
        if has_map {
            let (map, game) = self.client.map.unwrap_data_and_game_mut();
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
            while is_next_tick(self.cur_time, &mut self.client.client_data.last_game_tick) {
                game.tick();
            }

            // after the tick, there is always a prediction tick
            game.pred_tick();
        }

        self.client.client_data.intra_tick_time =
            intra_tick_time(self.cur_time, self.client.client_data.last_game_tick);

        // rendering
        //graphics.switch_to_dual_pass();

        if has_map {
            let map = self.client.map.unwrap();

            let char: Option<(vec2, dvec2)> = if self.client.client_data.local_players.len() > 0 {
                let local_player = self.client.client_data.local_players.front().unwrap();
                let player_id = local_player.0.clone();
                let cursor = local_player.1.input.inp.cursor;
                if map.game.player_exists(&player_id) {
                    Some((
                        map.game.lerp_core_pos(
                            &player_id,
                            intra_tick_time_to_ratio(self.client.client_data.intra_tick_time),
                        ),
                        map.game.cursor_vec2(&player_id),
                    ))
                } else {
                    None
                }
            } else {
                None
            };
            if let Some((char_pos, _)) = char {
                self.cam.pos = char_pos;
                self.cam.animation_start_tick = Default::default(); // TODO!: use animation tick from player
            }

            let mut render_pipe = RenderPipeline::new(
                &map.data.raw,
                &map.data.images,
                Some(&map.data.buffered_map),
                &self.config,
                &mut self.graphics,
                sys,
                &self.client,
                &map.game,
                &self.cam,
            );
            map.data.render.render(&mut render_pipe);
            self.players.render(&mut PlayerRenderPipe {
                graphics: &mut self.graphics,
                sys: &sys,
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
                sys: sys,
                graphics: &mut self.graphics,
                client_data: &self.client.client_data,
                cur_tick: map.game.cur_monotonic_tick(),
                map: map,

                ctf_container: &mut self.ctf_container,
                pickup_container: &mut self.pickup_container,

                fs: &self.fs,
                io_batcher: &self.io_batcher,
            });

            if let Some((_, mouse_cursor)) = char {
                let cursor = self.weapon_container.get_or_default(
                    "TODO:",
                    &mut self.graphics,
                    &self.fs,
                    &self.io_batcher,
                );
                let mut state = State::default();
                Players::map_canvas_for_players(&self.graphics, &mut state, 0.0, 0.0, 1.0);
                let mut quads = self.graphics.quads_begin();
                quads.set_state(&state);
                quads.set_texture(&cursor.gun.cursor);
                quads.quads_set_subset_free(0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0);
                quads.set_colors_from_single(1.0, 1.0, 1.0, 1.0);
                let c = mouse_cursor;
                let c = vec2::new(c.x as f32, c.y as f32);
                quads.quads_draw_tl(&[CQuadItem::new(c.x - 32.0, c.y - 32.0, 64.0, 64.0)]);
            }
        }
        // render components that want to be rendered
        self.client.client_stats.render(&mut ClientStatsRenderPipe {
            graphics: &mut self.graphics,
            sys: &sys,
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
            &self.config.ui_path.name.clone(),
            &self.fs,
            &self.io_batcher,
            &mut self.graphics,
            &mut UIPipe {
                sys: &sys,
                ui_feedback: &mut ClientUIFeedback::new(
                    &mut self.network_client,
                    &self.fs,
                    &self.io_batcher,
                ),
                runtime_thread_pool: &self.thread_pool,
                config: &mut self.config,
                raw_inp_generator: &UIWinitWrapperPipe {
                    window: native.borrow_window(),
                },
            },
        );

        self.graphics.swap();

        // sleep time related stuff
        let cur_time = sys.time_get_nanoseconds();
        if self.config.cl_refresh_rate > 0 {
            let time_until_tick_nanos =
                Duration::from_secs(1).as_nanos() as u64 / self.config.cl_refresh_rate as u64;

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
        self.graphics.resized(native, new_width, new_height)
    }

    fn destroy(mut self) {
        // destroy everything
        config_fs::save(&self.config, &self.fs, &self.io_batcher);

        self.network_client.close();

        self.ui_manager
            .destroy(&mut self.graphics, &self.io_batcher);

        self.client.map.destroy(
            &self.thread_pool,
            &self.io_batcher,
            &self.fs,
            &mut self.graphics,
            &self.config,
            &self.sys.time,
        );

        // cleanup containers
        self.hook_container
            .destroy(&self.io_batcher, &mut self.graphics);
        self.weapon_container
            .destroy(&self.io_batcher, &mut self.graphics);
        self.skin_container
            .destroy(&self.io_batcher, &mut self.graphics);
        self.ctf_container
            .destroy(&self.io_batcher, &mut self.graphics);
        self.pickup_container
            .destroy(&self.io_batcher, &mut self.graphics);
        self.entities_container
            .destroy(&self.io_batcher, &mut self.graphics);

        self.particles.destroy(&mut self.graphics);

        self.render.destroy(&mut self.graphics);

        let mut destroy_pipe = ComponentDestroyPipe {
            graphics: &mut self.graphics,
            batcher: &self.io_batcher,
        };
        self.players.destroy(&mut destroy_pipe);

        self.client.network_logic.destroy(&mut destroy_pipe);
        self.client.client_stats.destroy(&mut destroy_pipe);

        self.graphics.destroy();
    }
}
