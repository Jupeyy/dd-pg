use std::{path::Path, rc::Rc, sync::Arc};

use anyhow::anyhow;
use base_io::{io::Io, io_batcher::IoBatcherTask};
use client_render_game::{
    map::render_map_base::{ClientMapRender, RenderMapLoading},
    render_game::{RenderGameCreateOptions, RenderGameInterface},
};
use config::config::ConfigDebug;

use game_database::dummy::DummyDb;
use game_interface::interface::GameStateCreateOptions;
use graphics::graphics::graphics::Graphics;

use base::{
    hash::{fmt_hash, Hash},
    reduced_ascii_str::ReducedAsciiString,
    system::System,
};
use graphics_backend::backend::GraphicsBackend;
use map::map::Map;
use rayon::ThreadPool;
use shared::{
    game::state_wasm_manager::{GameStateMod, GameStateWasmManager, STATE_MODS_PATH},
    render::render_wasm_manager::RenderGameWasmManager,
};

use shared_base::network::messages::GameModification;
use sound::sound::SoundManager;

pub enum ClientGameStateModTask {
    Native,
    Ddnet,
    Wasm { file: IoBatcherTask<Vec<u8>> },
}

impl ClientGameStateModTask {
    pub fn is_finished(&self) -> bool {
        match self {
            ClientGameStateModTask::Native => true,
            ClientGameStateModTask::Ddnet => true,
            ClientGameStateModTask::Wasm { file } => file.is_finished(),
        }
    }

    pub fn to_game_state_mod(self) -> GameStateMod {
        match self {
            ClientGameStateModTask::Native => GameStateMod::Native,
            ClientGameStateModTask::Ddnet => GameStateMod::Ddnet,
            ClientGameStateModTask::Wasm { file } => GameStateMod::Wasm {
                file: file.get_storage().unwrap(),
            },
        }
    }
}

pub struct ClientMapLoadingFile {
    pub task: IoBatcherTask<Vec<u8>>,
    io: Io,
    thread_pool: Arc<rayon::ThreadPool>,
    as_menu_map: bool,
    map_name: String,
    pub game_mod_task: ClientGameStateModTask,
    pub game_options: GameStateCreateOptions,
    props: RenderGameCreateOptions,

    config_debug: ConfigDebug,
    sound: SoundManager,
    graphics: Graphics,
    backend: Rc<GraphicsBackend>,
    sys: System,
}

impl ClientMapLoadingFile {
    pub fn new(
        sound: &SoundManager,
        graphics: &Graphics,
        backend: &Rc<GraphicsBackend>,
        sys: &System,
        base_path: &Path,
        map_file: &ReducedAsciiString,
        map_hash: Option<Hash>,
        io: &Io,
        thread_pool: &Arc<rayon::ThreadPool>,
        game_mod: GameModification,
        as_menu_map: bool,
        config_debug: &ConfigDebug,
        game_options: GameStateCreateOptions,
        props: RenderGameCreateOptions,
    ) -> Self {
        let map_file_name = if let Some(map_hash) = map_hash {
            base_path.join(format!(
                "{}_{}.twmap",
                map_file.as_str(),
                fmt_hash(&map_hash)
            ))
        } else {
            base_path.join(format!("{}.twmap", map_file.as_str()))
        };

        let file_system = io.fs.clone();
        let http = io.http.clone();
        let resource_download_server_thread = props.resource_download_server.clone();
        Self {
            task: io.io_batcher.spawn(async move {
                let file = file_system.read_file(map_file_name.as_ref()).await;

                let file = match file {
                    Ok(file) => Ok(file),
                    Err(err) => {
                        // try to download file
                        if let Some(resource_download_server) = resource_download_server_thread
                            .and_then(|url| {
                                url.join(map_file_name.as_os_str().to_str().unwrap_or(""))
                                    .ok()
                            })
                        {
                            let file = http
                                .download_binary(
                                    resource_download_server,
                                    &map_hash.unwrap_or_default(),
                                )
                                .await
                                .map_err(|err| anyhow!("failed to download map: {err}"))?
                                .to_vec();
                            anyhow::ensure!(
                                Map::validate_twmap_header(&file),
                                "not a twmap file or variant of it."
                            );
                            let file_path: &Path = map_file_name.as_ref();
                            if let Some(dir) = file_path.parent() {
                                file_system.create_dir(dir).await?;
                            }
                            file_system
                                .write_file(map_file_name.as_ref(), file.clone())
                                .await?;
                            Ok(file)
                        } else {
                            Err(err)
                        }
                    }
                }?;

                Ok(file)
            }),
            io: io.clone(),
            thread_pool: thread_pool.clone(),
            as_menu_map,
            map_name: map_file.as_str().to_string(),
            game_mod_task: match game_mod {
                GameModification::Native => ClientGameStateModTask::Native,
                GameModification::Ddnet => ClientGameStateModTask::Ddnet,
                GameModification::Wasm { name, hash } => ClientGameStateModTask::Wasm {
                    file: {
                        let fs = io.fs.clone();
                        let http = io.http.clone();
                        let game_mod_file_name = format!(
                            "{}/{}_{}.wasm",
                            STATE_MODS_PATH,
                            name.as_str(),
                            fmt_hash(&hash)
                        );
                        let resource_download_server_thread =
                            props.resource_download_server.clone();

                        io.io_batcher.spawn(async move {
                            let file = fs.read_file(game_mod_file_name.as_ref()).await;

                            let file = match file {
                                Ok(file) => Ok(file),
                                Err(err) => {
                                    // try to download file
                                    if let Some(resource_download_server) =
                                        resource_download_server_thread
                                            .and_then(|url| url.join(&game_mod_file_name).ok())
                                    {
                                        let file = http
                                            .download_binary(resource_download_server, &hash)
                                            .await
                                            .map_err(|err| {
                                                anyhow!("failed to download mod: {err}")
                                            })?
                                            .to_vec();

                                        // ensure that downloaded file is valid wasm file
                                        wasmparser::validate(&file)?;

                                        let file_path: &Path = game_mod_file_name.as_ref();
                                        if let Some(dir) = file_path.parent() {
                                            fs.create_dir(dir).await?;
                                        }
                                        fs.write_file(game_mod_file_name.as_ref(), file.clone())
                                            .await?;

                                        Ok(file)
                                    } else {
                                        Err(err)
                                    }
                                }
                            }?;

                            let wasm_module = GameStateWasmManager::load_module(&fs, &file).await?;

                            Ok(wasm_module)
                        })
                    },
                },
            },
            config_debug: *config_debug,
            backend: backend.clone(),
            graphics: graphics.clone(),
            sound: sound.clone(),
            sys: sys.clone(),
            props,
            game_options,
        }
    }
}

pub struct GameCreateProps {
    sound: SoundManager,
    graphics: Graphics,
    backend: Rc<GraphicsBackend>,
    io: Io,
    thread_pool: Arc<ThreadPool>,
    sys: System,
    map_file: Vec<u8>,
    props: RenderGameCreateOptions,
    config: ConfigDebug,
}

pub enum GameLoading {
    Task {
        task: IoBatcherTask<Vec<u8>>,
        props: Box<GameCreateProps>,
    },
    Game(RenderGameWasmManager),
}

pub enum ClientMapComponentLoadingType {
    Game(GameLoading),
    Menu(ClientMapRender),
}

pub struct ClientMapComponentLoading {
    ty: ClientMapComponentLoadingType,
    io: Io,
    thread_pool: Arc<rayon::ThreadPool>,
}

impl ClientMapComponentLoading {
    pub fn new(
        thread_pool: Arc<rayon::ThreadPool>,
        file: Vec<u8>,
        io: Io,
        sound: &SoundManager,
        graphics: &Graphics,
        backend: &Rc<GraphicsBackend>,
        sys: &System,
        config: &ConfigDebug,
        as_menu_map: bool,
        props: RenderGameCreateOptions,
    ) -> Self {
        Self {
            ty: if as_menu_map {
                ClientMapComponentLoadingType::Menu(ClientMapRender::new(RenderMapLoading::new(
                    thread_pool.clone(),
                    file,
                    props.resource_download_server,
                    io.clone(),
                    sound,
                    props.sound_props,
                    graphics,
                    config,
                )))
            } else {
                let fs = io.fs.clone();
                ClientMapComponentLoadingType::Game(GameLoading::Task {
                    task: io
                        .io_batcher
                        .spawn(async move { RenderGameWasmManager::load_module(&fs).await }),
                    props: Box::new(GameCreateProps {
                        sound: sound.clone(),
                        graphics: graphics.clone(),
                        backend: backend.clone(),
                        io: io.clone(),
                        thread_pool: thread_pool.clone(),
                        sys: sys.clone(),
                        map_file: file,
                        config: *config,
                        props,
                    }),
                })
            },
            io,
            thread_pool,
        }
    }
}

pub struct GameMap {
    pub render: RenderGameWasmManager,
    // client local calculated game
    pub game: GameStateWasmManager,
}

pub enum ClientMapFile {
    Menu { render: ClientMapRender },
    Game(GameMap),
}

pub enum ClientMapLoading {
    /// load the "raw" map file
    File(ClientMapLoadingFile),
    /// wait for the individual components to finish parsing the map file
    /// physics and graphics independently
    PrepareComponents {
        render: ClientMapComponentLoading,
        map: Vec<u8>,
        map_name: String,
        game_mod: GameStateMod,
        game_options: GameStateCreateOptions,
    },
    /// finished loading
    Map(ClientMapFile),
    /// map not loading
    None,
}

impl ClientMapLoading {
    pub fn new(
        sound: &SoundManager,
        graphics: &Graphics,
        backend: &Rc<GraphicsBackend>,
        sys: &System,
        base_path: &Path,
        map_file: &ReducedAsciiString,
        map_hash: Option<Hash>,
        io: &Io,
        thread_pool: &Arc<rayon::ThreadPool>,
        game_mod: GameModification,
        as_menu_map: bool,
        config_debug: &ConfigDebug,
        game_options: GameStateCreateOptions,
        props: RenderGameCreateOptions,
    ) -> Self {
        Self::File(ClientMapLoadingFile::new(
            sound,
            graphics,
            backend,
            sys,
            base_path,
            map_file,
            map_hash,
            io,
            thread_pool,
            game_mod,
            as_menu_map,
            config_debug,
            game_options,
            props,
        ))
    }

    pub fn try_get(&self) -> Option<&ClientMapFile> {
        if let Self::Map(map_file) = self {
            Some(map_file)
        } else {
            None
        }
    }

    pub fn try_get_mut(&mut self) -> Option<&mut ClientMapFile> {
        if let Self::Map(map_file) = self {
            Some(map_file)
        } else {
            None
        }
    }

    pub fn is_fully_loaded(&self) -> bool {
        if let Self::Map(_map_file) = self {
            return true;
        }
        false
    }

    pub fn continue_loading(&mut self) -> Option<&ClientMapFile> {
        let mut self_helper = ClientMapLoading::None;
        std::mem::swap(&mut self_helper, self);
        match self_helper {
            Self::File(file) => {
                if file.task.is_finished() && file.game_mod_task.is_finished() {
                    let map_file = file.task.get_storage().unwrap();
                    let game_mod = file.game_mod_task.to_game_state_mod();

                    let loading = ClientMapComponentLoading::new(
                        file.thread_pool.clone(),
                        map_file.clone(),
                        file.io.clone(),
                        &file.sound,
                        &file.graphics,
                        &file.backend,
                        &file.sys,
                        &file.config_debug,
                        file.as_menu_map,
                        file.props,
                    );

                    *self = Self::PrepareComponents {
                        render: loading,
                        map: map_file,
                        map_name: file.map_name,
                        game_mod,
                        game_options: file.game_options,
                    }
                } else {
                    *self = Self::File(file)
                }
            }
            Self::PrepareComponents {
                render,
                map,
                map_name,
                game_mod,
                game_options,
            } => {
                match render.ty {
                    ClientMapComponentLoadingType::Game(mut load_game) => {
                        if let GameLoading::Task { task, props } = load_game {
                            if task.is_finished() {
                                let file = task.get_storage().ok();
                                load_game = GameLoading::Game(RenderGameWasmManager::new(
                                    &props.sound,
                                    &props.graphics,
                                    &props.backend,
                                    &props.io,
                                    &props.thread_pool,
                                    &props.sys,
                                    props.map_file,
                                    &props.config,
                                    file,
                                    props.props,
                                ));
                            } else {
                                load_game = GameLoading::Task { task, props };
                            }
                        }
                        match load_game {
                            GameLoading::Task { task, props } => {
                                *self = Self::PrepareComponents {
                                    render: ClientMapComponentLoading {
                                        ty: ClientMapComponentLoadingType::Game(
                                            GameLoading::Task { task, props },
                                        ),
                                        io: render.io,
                                        thread_pool: render.thread_pool,
                                    },
                                    map,
                                    map_name,
                                    game_mod,
                                    game_options,
                                }
                            }
                            GameLoading::Game(mut load_game) => {
                                if load_game.continue_map_loading() {
                                    let game = GameStateWasmManager::new(
                                        game_mod,
                                        map,
                                        map_name,
                                        game_options,
                                        &render.io,
                                        Arc::new(DummyDb),
                                    );

                                    load_game.set_chat_commands(game.info.chat_commands.clone());

                                    // finished loading
                                    *self = Self::Map(ClientMapFile::Game(GameMap {
                                        render: load_game,
                                        game,
                                    }));
                                } else {
                                    *self = Self::PrepareComponents {
                                        render: ClientMapComponentLoading {
                                            ty: ClientMapComponentLoadingType::Game(
                                                GameLoading::Game(load_game),
                                            ),
                                            io: render.io,
                                            thread_pool: render.thread_pool,
                                        },
                                        map,
                                        map_name,
                                        game_mod,
                                        game_options,
                                    }
                                }
                            }
                        }
                    }
                    ClientMapComponentLoadingType::Menu(mut map_prepare) => {
                        if map_prepare.continue_loading().is_some() {
                            *self = Self::Map(ClientMapFile::Menu {
                                render: map_prepare,
                            })
                        } else {
                            *self = Self::PrepareComponents {
                                render: ClientMapComponentLoading {
                                    ty: ClientMapComponentLoadingType::Menu(map_prepare),
                                    io: render.io,
                                    thread_pool: render.thread_pool,
                                },
                                map,
                                map_name,
                                game_mod,
                                game_options,
                            }
                        }
                    }
                }
            }
            Self::Map(map) => *self = ClientMapLoading::Map(map),
            Self::None => {}
        }
        self.try_get()
    }
}
