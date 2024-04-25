use std::{rc::Rc, sync::Arc};

use base_io::{io::IO, io_batcher::IOBatcherTask};
use client_render_game::{
    map::render_map_base::{ClientMapRender, RenderMapLoading},
    render_game::RenderGameInterface,
};
use config::config::ConfigEngine;

use graphics::graphics::graphics::Graphics;

use base::{
    hash::{fmt_hash, Hash},
    system::System,
};
use graphics_backend::backend::GraphicsBackend;
use map::map::Map;
use shared::{
    game::state_wasm_manager::GameStateWasmManager,
    render::render_wasm_manager::RenderGameWasmManager,
};

use sound::sound::SoundManager;
use url::Url;

pub struct ClientMapLoadingFile {
    pub task: IOBatcherTask<Vec<u8>>,
    io: IO,
    thread_pool: Arc<rayon::ThreadPool>,
    as_menu_map: bool,
}

impl ClientMapLoadingFile {
    pub fn new(
        map_file: &str,
        map_hash: Option<Hash>,
        resource_download_server: Option<Url>,
        io: &IO,
        thread_pool: &Arc<rayon::ThreadPool>,
        as_menu_map: bool,
    ) -> Self {
        let map_file_name = if let Some(map_hash) = map_hash {
            format!("map/maps/{}_{}.twmap", map_file, fmt_hash(&map_hash))
        } else {
            format!("map/maps/{}.twmap", map_file)
        };
        let file_system = io.fs.clone();
        let http = io.http.clone();
        Self {
            task: io.io_batcher.spawn(async move {
                let file = file_system.open_file(map_file_name.as_ref()).await;

                let file = match file {
                    Ok(file) => Ok(file),
                    Err(err) => {
                        // try to download file
                        if let Some(resource_download_server) = resource_download_server
                            .map(|url| url.join(&map_file_name).ok())
                            .flatten()
                        {
                            let file = http
                                .download_binary(
                                    resource_download_server,
                                    &map_hash.unwrap_or_default().into(),
                                )
                                .await?
                                .to_vec();
                            anyhow::ensure!(
                                Map::validate_twmap_header(&file),
                                "not a twmap file or variant of it."
                            );
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
        }
    }
}

pub enum ClientMapComponentLoadingType {
    Game(RenderGameWasmManager),
    Menu(ClientMapRender),
}

pub struct ClientMapComponentLoading {
    ty: ClientMapComponentLoadingType,
    io: IO,
    thread_pool: Arc<rayon::ThreadPool>,
}

impl ClientMapComponentLoading {
    pub fn new(
        thread_pool: Arc<rayon::ThreadPool>,
        file: Vec<u8>,
        io: IO,
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
    },
    /// finished loading
    Map(ClientMapFile),
    /// map not loading
    None,
}

impl ClientMapLoading {
    pub fn new(
        map_file: &str,
        map_hash: Option<Hash>,
        resource_download_server: Option<Url>,
        io: &IO,
        thread_pool: &Arc<rayon::ThreadPool>,
        as_menu_map: bool,
    ) -> Self {
        Self::File(ClientMapLoadingFile::new(
            map_file,
            map_hash,
            resource_download_server,
            io,
            thread_pool,
            as_menu_map,
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
                if file.task.is_finished() {
                    let map_file = file.task.get_storage().unwrap();

                    let loading = ClientMapComponentLoading::new(
                        file.thread_pool.clone(),
                        map_file.clone(),
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
                    }
                } else {
                    *self = Self::File(file)
                }
            }
            Self::PrepareComponents { render, map } => {
                match render.ty {
                    ClientMapComponentLoadingType::Game(mut render_game) => {
                        if render_game.continue_map_loading(&config.dbg) {
                            let game =
                                GameStateWasmManager::new(map, Default::default(), &render.io);

                            // finished loading
                            *self = Self::Map(ClientMapFile::Game(GameMap {
                                render: render_game,
                                game,
                            }));
                        } else {
                            *self = Self::PrepareComponents {
                                render: ClientMapComponentLoading {
                                    ty: ClientMapComponentLoadingType::Game(render_game),
                                    io: render.io,
                                    thread_pool: render.thread_pool,
                                },
                                map,
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
