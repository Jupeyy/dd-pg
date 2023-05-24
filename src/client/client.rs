use std::{
    collections::{BTreeMap, HashMap},
    num::NonZeroUsize,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use base::{
    benchmark,
    config::Config,
    filesys::FileSystem,
    io_batcher::IOBatcher,
    system::{System, SystemLogInterface, SystemTimeInterface},
};
use native::{input::Input, native::Native};
use network::network::{network::NetworkConnectionID, quinn_network::QuinnNetwork};
use wasm_runtime::WasmManager;

use crate::{
    client::input::{InputHandling, InputPipe},
    client_map::ClientMap,
    game::{
        simulation_pipe::{LocalPlayerInput, LocalPlayers, SimulationPipe, SimulationPlayerInput},
        snapshot::SnapshotManager,
        state::GameState,
        TGameElementID, INVALID_GAME_ELEMENT_ID,
    },
    id_gen::IDGeneratorIDType,
    network::{
        game_event_generator::GameEventGenerator,
        messages::{ClientToServerMessage, GameMessage},
    },
    ui::{
        pages::menu::main_menu::{MainMenu, MainMenuUIFeedback},
        types::UIPipe,
        ui::UI,
        ui_manager::UIManager,
    },
    worker::Worker,
};

use tokio::sync::Mutex;

use super::{
    component::{
        ComponentComponent, ComponentLoadIOPipe, ComponentLoadPipe, ComponentLoadWhileIOPipe,
        ComponentLoadable, ComponentRenderPipe, ComponentUpdatePipe,
    },
    components::{client_stats::ClientStats, network_logic::NetworkLogic, skins::Skins},
    game_events::{GameEventPipeline, GameEventsClient},
    input::{self},
    render_pipe::{Camera, ClientInterface, RenderPipeline},
};

use graphics::{
    self,
    graphics::Graphics,
    traits::{GraphicsLoadIOPipe, GraphicsLoadWhileIOPipe},
    window_handling::{WindowEventPipe, WindowHandling},
};

pub struct ClientData {
    pub cur_server: NetworkConnectionID,
    pub server_connect_time: Duration,

    // the ping between the client and the server
    pub ping: Duration,

    pub player_id_on_server: TGameElementID,
    pub snapshot_timestamp: Duration,
}

impl Default for ClientData {
    fn default() -> Self {
        Self {
            cur_server: Default::default(),
            server_connect_time: Duration::default(),

            ping: Duration::default(),

            player_id_on_server: INVALID_GAME_ELEMENT_ID,
            snapshot_timestamp: Duration::ZERO,
        }
    }
}

pub struct ClientPlayerInputPerTick {
    pub inp: HashMap<TGameElementID, LocalPlayerInput>,
}

impl SimulationPlayerInput for ClientPlayerInputPerTick {
    fn get_input(&self, player_id: TGameElementID) -> Option<&LocalPlayerInput> {
        self.inp.get(&player_id)
    }
}

impl ClientPlayerInputPerTick {
    pub fn new() -> Self {
        Self {
            inp: HashMap::new(),
        }
    }
}

pub struct Client<'a> {
    pub components: Vec<&'a mut dyn ComponentComponent>,
    pub components_that_update: Vec<usize>,
    pub components_that_render: Vec<usize>,
    pub components_that_handle_msgs: Vec<usize>,

    pub map: ClientMap,
    // client local calculated game
    pub game: GameState,
    // server game state
    pub server_game: GameState,

    pub input_per_tick: BTreeMap<u64, ClientPlayerInputPerTick>,

    pub snap_builder: SnapshotManager,

    pub client_data: ClientData,
}

impl<'a> ClientInterface for Client<'a> {}

pub fn ddnet_main(mut sys: System, cert: &[u8]) {
    let mut local_players = LocalPlayers::new();

    let native = Native::new();
    let mut graphics = Graphics::new(native.clone());
    let input = Input::new(native.clone());

    let mut cam = Camera {
        x: 0.0,
        y: 0.0,
        zoom: 1.0,
    };

    let mut config = Config::load();
    config.dbg_bench = true;

    let mut network_logic = NetworkLogic::new();
    let mut skins = Skins::new();
    let mut client_stats = ClientStats::new(&sys);
    let components: Vec<&mut dyn ComponentComponent> =
        vec![&mut network_logic, &mut skins, &mut client_stats];
    let mut client = Client {
        components: components,
        components_that_update: Vec::new(),
        components_that_render: Vec::new(),
        components_that_handle_msgs: Vec::new(),

        map: ClientMap::None,

        game: GameState::new(),
        server_game: GameState::new(),

        input_per_tick: BTreeMap::new(),

        snap_builder: SnapshotManager::new(),

        client_data: Default::default(),
    };

    // tokio runtime for client side tasks
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .build()
        .unwrap();

    let fs = Arc::new(FileSystem::new());
    let mut io_batcher = Arc::new(std::sync::Mutex::new(IOBatcher::new(rt)));

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
            for comp in &mut client.components {
                comp.load_io(&mut io_pipe);
            }
        }
    );

    // if benchmarking, finish all io batching directly after they are started to know how fast the file reading actually was
    if config.dbg_bench {
        benchmark!(config.dbg_bench, sys, "io_batcher's finish_all", || {
            io_batcher.lock().unwrap().finish_all();
        });
    }

    let mut ui_manager = UIManager::new();
    let mut ui = UI::new(1.5);

    // prepare network stuff while waiting for io
    let mut game_events = GameEventsClient::new();
    let has_new_events_client = Arc::new(AtomicBool::new(false));
    let game_event_generator_client = Arc::new(Mutex::new(GameEventGenerator::new(
        has_new_events_client.clone(),
        sys.time.clone(),
    )));
    let mut network_client = QuinnNetwork::init_client(
        "0.0.0.0:0",
        cert,
        game_event_generator_client.clone(),
        sys.time.clone(),
    );

    // then prepare components allocations etc.
    let mut thread_pool = Arc::new(
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
        config.dbg_bench,
        sys,
        "init_while_io of graphics and component",
        || {
            let mut pipe = GraphicsLoadWhileIOPipe {
                runtime_threadpool: &thread_pool,
                config: &config,
                sys: &sys,
            };
            graphics.init_while_io(&mut pipe);
            let mut pipe = ComponentLoadWhileIOPipe {
                runtime_threadpool: &thread_pool,
                config: &config,
                sys: &sys,
            };
            for comp in client.components.iter_mut() {
                comp.init_while_io(&mut pipe);
            }
        }
    );
    let mut worker = Worker::new();

    // if not benchmarking, finish all io batching after CPU intensive tasks
    if !config.dbg_bench {
        benchmark!(config.dbg_bench, sys, "io_batcher's finish_all", || {
            io_batcher.lock().unwrap().finish_all();
        });
    }

    benchmark!(
        config.dbg_bench,
        sys,
        "init of graphics and component",
        || {
            // at last, fulfill the initialization of the component
            if let Err(err) = graphics.init_graphics() {
                sys.log("client").msg(err.as_str());
            }
            let mut load_pipe = ComponentLoadPipe {
                graphics: &mut graphics,
                config: &config,
            };
            for comp in &mut client.components {
                if let Err(err) = comp.init(&mut load_pipe) {
                    sys.log("client").msg(err.as_str());
                }
            }
        }
    );

    // prepare components for future use
    for (index, comp) in &mut client.components.iter_mut().enumerate() {
        if comp.does_update() {
            client.components_that_update.push(index);
        }
        if comp.does_render() {
            client.components_that_render.push(index);
        }
        if comp.handles_msgs() {
            client.components_that_handle_msgs.push(index);
        }
    }

    //network_client.connect("127.0.0.1:8305");
    let mut main_menu = MainMenu::new(&mut graphics);

    let mut cur_time = sys.time_get_nanoseconds();
    let mut last_tick_time = cur_time;
    let mut last_inp_time = cur_time;
    let time_until_tick = Duration::from_secs(1).as_nanos() / 50;
    let time_until_inp = Duration::from_secs(1).as_nanos() / 100;

    while game_events.update(&mut GameEventPipeline {
        event_generator: &*game_event_generator_client,
        event_generator_has_events: &has_new_events_client,
        network: &mut network_client,
        graphics: &mut graphics,
        client: &mut client,
        runtime_thread_pool: &mut thread_pool,
        io_batcher: &mut io_batcher,
        worker: &mut worker,
        fs: &fs,
        config: &config,
        sys: &sys,
    }) && input.run(
        &mut InputHandling {
            pipe: InputPipe {
                local_players: &mut local_players.players,
                ui: &mut ui,
            },
        },
        &mut WindowHandling {
            pipe: WindowEventPipe {
                graphics: &mut graphics,
            },
        },
    ) {
        cur_time = sys.time_get_nanoseconds();
        // update components that want to be updated
        for comp_update in &client.components_that_update {
            client.components[*comp_update].update(&mut ComponentUpdatePipe {
                fs: &fs,
                batcher: &mut io_batcher,
                config: &config,
                map: &client.map,
                network: &mut network_client,
                sys: &mut sys,
                client_data: &mut client.client_data,
            });
        }

        while (cur_time - last_inp_time).as_nanos() > time_until_inp {
            let net_inp = local_players.players[0].input.to_net_obj();
            network_client.send_to_server(&GameMessage::ClientToServer(
                ClientToServerMessage::Input(net_inp),
            ));
            last_inp_time += Duration::from_nanos(time_until_inp as u64);
        }

        let has_map = client
            .map
            .get(
                &thread_pool,
                &mut worker,
                &io_batcher,
                &fs,
                &mut graphics,
                &config,
                &sys.time,
            )
            .is_some();
        if has_map {
            local_players.players[0].player_id = client.client_data.player_id_on_server;
            let map = client.map.unwrap();
            let mut sim_pipe = SimulationPipe::new(&local_players, &map.collision);
            while (cur_time - last_tick_time).as_nanos() > time_until_tick {
                client.game.tick(&mut sim_pipe);
                // every time a tick is made, save the current input of all users for possible recalculations later
                let mut player_inputs = ClientPlayerInputPerTick::new();
                player_inputs.inp.insert(
                    IDGeneratorIDType(0), /* TODO! */
                    local_players.players[0].input,
                );

                client
                    .input_per_tick
                    .insert(client.game.cur_monotonic_tick - 1, player_inputs);
                last_tick_time += Duration::from_nanos(time_until_tick as u64);
            }

            // after the tick there is always a prediction tick
            client.game.pred_tick(&mut sim_pipe);

            // check if the server tick can be increased
            while (cur_time - client.client_data.snapshot_timestamp).as_nanos() > time_until_tick {
                let monotonic_tick = client.server_game.cur_monotonic_tick;
                // find player inputs of this tick
                let inp = client.input_per_tick.get(&monotonic_tick);
                let dummy = ClientPlayerInputPerTick::new();
                let mut sim_pipe = SimulationPipe::new(inp.unwrap_or(&dummy), &map.collision);
                client.server_game.tick(&mut sim_pipe);
                client.client_data.snapshot_timestamp +=
                    Duration::from_nanos(time_until_tick as u64);
                // drop queued input from 50 ticks ago (1 second)
                let ticks_to_rem: u64 = 50;
                while client.server_game.cur_monotonic_tick >= ticks_to_rem
                    && !client.input_per_tick.is_empty()
                    && *client.input_per_tick.first_entry().unwrap().key()
                        < client.server_game.cur_monotonic_tick - ticks_to_rem
                {
                    client.input_per_tick.pop_first();
                }
            }
        }

        // rendering
        if has_map {
            let map = client.map.unwrap();

            let player_id = client.client_data.player_id_on_server;
            // TODO: optimize this
            let stage = client.game.get_stages().iter().find(|stage| {
                let it = stage
                    .get_world()
                    .get_characters()
                    .iter()
                    .find(|char| char.cores[0].player_id == player_id);
                it.is_some()
            });
            if let Some(stage) = stage {
                let char = stage
                    .get_world()
                    .get_characters()
                    .iter()
                    .find(|char| char.cores[0].player_id == player_id);
                if let Some(char) = char {
                    cam.x = char.cores[1].core.pos.x;
                    cam.y = char.cores[1].core.pos.y;

                    /* sys.log("client")
                    .msg(
                        format!(
                            "pos x: {}, pos y: {}\n",
                            char.cores[0].core.m_Pos.x / 32.0,
                            char.cores[0].core.m_Pos.y / 32.0
                        )
                        .as_str(),
                    )
                    .msg(format!("x: {}, y: {}", cam.x, cam.y).as_str());*/
                }
            }

            let mut render_pipe = RenderPipeline::new(
                &map.raw,
                &map.images,
                Some(&map.buffered_map),
                &config,
                &mut graphics,
                &sys,
                &client,
                &client.game,
                &cam,
            );
            map.render.render(&mut render_pipe);
        }
        // render components that want to be rendered
        for comp_update in &client.components_that_render {
            client.components[*comp_update].render(&mut ComponentRenderPipe {
                graphics: &mut graphics,
                sys: &sys,
                runtime_thread_pool: &thread_pool,
                config: &mut config,
                client_data: &client.client_data,
                game: &client.game,
            });
        }

        // render ui last
        if ui.ui_state.is_ui_open {
            ui.render(
                |egui_ui, pipe, ui_state| main_menu.render_func(egui_ui, pipe, ui_state),
                &mut UIPipe {
                    graphics: &mut graphics,
                    sys: &sys,
                    ui_feedback: &mut MainMenuUIFeedback::new(&mut network_client),
                    runtime_thread_pool: &thread_pool,
                    config: &mut config,
                },
            );
        }

        ui_manager.run(&mut graphics);

        graphics.swap();

        // time related stuff
        //std::thread::sleep(Duration::from_millis(1000 / 50));
    }
}
