use std::{path::Path, rc::Rc, sync::Arc};

use anyhow::anyhow;
use base_io::{io::Io, io_batcher::IoBatcherTask};
use client_render_game::{
    map::render_map_base::{ClientMapRender, RenderMapLoading},
    render_game::RenderGameInterface,
};
use config::config::ConfigEngine;

use game_interface::{
    interface::GameStateCreateOptions, types::reduced_ascii_str::ReducedAsciiString,
};
use graphics::graphics::graphics::Graphics;

use base::{
    hash::{fmt_hash, Hash},
    system::System,
};
use graphics_backend::backend::GraphicsBackend;
use map::map::Map;
use shared::{
    game::state_wasm_manager::{GameStateMod, GameStateWasmManager, STATE_MODS_PATH},
    render::render_wasm_manager::RenderGameWasmManager,
};

use sound::sound::SoundManager;
use url::Url;

pub enum ClientGameStateModTask {
    Native,
    //Ddnet,
    Wasm { file: IoBatcherTask<Vec<u8>> },
}

impl ClientGameStateModTask {
    pub fn is_finished(&self) -> bool {
        match self {
            ClientGameStateModTask::Native => true,
            ClientGameStateModTask::Wasm { file } => file.is_finished(),
        }
    }

    pub fn to_game_state_mod(self) -> GameStateMod {
        match self {
            ClientGameStateModTask::Native => GameStateMod::Native,
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
    resource_download_server: Option<Url>,
    pub game_mod_task: ClientGameStateModTask,
    pub game_options: GameStateCreateOptions,
}

impl ClientMapLoadingFile {
    pub fn new(
        base_path: &Path,
        map_file: &ReducedAsciiString,
        map_hash: Option<Hash>,
        resource_download_server: Option<Url>,
        io: &Io,
        thread_pool: &Arc<rayon::ThreadPool>,
        game_mod: &ReducedAsciiString,
        game_mod_hash: Option<Hash>,
        as_menu_map: bool,
        game_options: GameStateCreateOptions,
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
        let game_mod_file_name = game_mod_hash.map(|game_mod_hash| {
            format!(
                "{}/{}_{}.wasm",
                STATE_MODS_PATH,
                game_mod.as_str(),
                fmt_hash(&game_mod_hash)
            )
        });

        let file_system = io.fs.clone();
        let http = io.http.clone();
        let resource_download_server_thread = resource_download_server.clone();
        Self {
            task: io.io_batcher.spawn(async move {
                let file = file_system.read_file(map_file_name.as_ref()).await;

                let file = match file {
                    Ok(file) => Ok(file),
                    Err(err) => {
                        // try to download file
                        if let Some(resource_download_server) = resource_download_server_thread
                            .map(|url| {
                                url.join(map_file_name.as_os_str().to_str().unwrap_or(""))
                                    .ok()
                            })
                            .flatten()
                        {
                            let file = http
                                .download_binary(
                                    resource_download_server,
                                    &map_hash.unwrap_or_default().into(),
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
            game_mod_task: match game_mod.as_str() {
                "" | "vanilla" | "native" => ClientGameStateModTask::Native,
                "ddnet" => todo!(),
                _ => ClientGameStateModTask::Wasm {
                    file: {
                        let fs = io.fs.clone();
                        let http = io.http.clone();
                        let game_mod_file_name = game_mod_file_name.unwrap();
                        let resource_download_server_thread = resource_download_server.clone();

                        io.io_batcher.spawn(async move {
                            let file = fs.read_file(game_mod_file_name.as_ref()).await;

                            let file = match file {
                                Ok(file) => Ok(file),
                                Err(err) => {
                                    // try to download file
                                    if let Some(resource_download_server) =
                                        resource_download_server_thread
                                            .map(|url| url.join(&game_mod_file_name).ok())
                                            .flatten()
                                    {
                                        let file = http
                                            .download_binary(
                                                resource_download_server,
                                                &game_mod_hash.unwrap_or_default().into(),
                                            )
                                            .await
                                            .map_err(|err| {
                                                anyhow!("failed to download mod: {err}")
                                            })?
                                            .to_vec();
                                        // TODO: ensure that downloaded file is valid wasm file
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

                            Ok(file)
                        })
                    },
                },
            },
            resource_download_server,
            game_options,
        }
    }
}

pub enum ClientMapComponentLoadingType {
    Game(RenderGameWasmManager),
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
        resource_download_server: Option<Url>,
        io: Io,
        sound: &SoundManager,
        graphics: &Graphics,
        backend: &Rc<GraphicsBackend>,
        sys: &System,
        config: &ConfigEngine,
        as_menu_map: bool,
    ) -> Self {
        Self {
            ty: if as_menu_map {
                ClientMapComponentLoadingType::Menu(ClientMapRender::new(RenderMapLoading::new(
                    thread_pool.clone(),
                    file,
                    resource_download_server,
                    io.clone(),
                    sound,
                    graphics,
                    config,
                )))
            } else {
                ClientMapComponentLoadingType::Game(RenderGameWasmManager::new(
                    sound,
                    graphics,
                    backend,
                    &io,
                    &thread_pool,
                    sys,
                    file,
                    resource_download_server,
                    config,
                ))
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
        base_path: &Path,
        map_file: &ReducedAsciiString,
        map_hash: Option<Hash>,
        resource_download_server: Option<Url>,
        io: &Io,
        thread_pool: &Arc<rayon::ThreadPool>,
        game_mod: &ReducedAsciiString,
        game_mod_hash: Option<Hash>,
        as_menu_map: bool,
        game_options: GameStateCreateOptions,
    ) -> Self {
        Self::File(ClientMapLoadingFile::new(
            base_path,
            map_file,
            map_hash,
            resource_download_server,
            io,
            thread_pool,
            game_mod,
            game_mod_hash,
            as_menu_map,
            game_options,
        ))
    }

    pub fn unwrap_game_mut(&mut self) -> &mut GameStateWasmManager {
        self.try_get_game_mut()
            .ok_or("map file was not loaded yet")
            .unwrap()
    }

    pub fn unwrap(&self) -> &ClientMapFile {
        self.try_get().ok_or("map file was not loaded yet").unwrap()
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

    pub fn try_get_game_mut(&mut self) -> Option<&mut GameStateWasmManager> {
        if let Self::Map(map_file) = self {
            if let ClientMapFile::Game(GameMap { game, .. }) = map_file {
                Some(game)
            } else {
                None
            }
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

    pub fn continue_loading(
        &mut self,
        sound: &SoundManager,
        graphics: &Graphics,
        backend: &Rc<GraphicsBackend>,
        config: &ConfigEngine,
        sys: &System,
    ) -> Option<&ClientMapFile> {
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
                        file.resource_download_server,
                        file.io.clone(),
                        sound,
                        graphics,
                        backend,
                        sys,
                        config,
                        file.as_menu_map,
                    );

                    *self = Self::PrepareComponents {
                        render: loading,
                        map: map_file,
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
                game_mod,
                game_options,
            } => {
                match render.ty {
                    ClientMapComponentLoadingType::Game(mut load_game) => {
                        if load_game.continue_map_loading(&config.dbg) {
                            let game =
                                GameStateWasmManager::new(game_mod, map, game_options, &render.io);

                            // finished loading
                            *self = Self::Map(ClientMapFile::Game(GameMap {
                                render: load_game,
                                game,
                            }));
                        } else {
                            *self = Self::PrepareComponents {
                                render: ClientMapComponentLoading {
                                    ty: ClientMapComponentLoadingType::Game(load_game),
                                    io: render.io,
                                    thread_pool: render.thread_pool,
                                },
                                map,
                                game_mod,
                                game_options,
                            }
                        }
                    }
                    ClientMapComponentLoadingType::Menu(mut map_prepare) => {
                        if map_prepare.continue_loading(&config.dbg).is_some() {
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
