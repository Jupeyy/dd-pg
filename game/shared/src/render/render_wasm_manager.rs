use std::{rc::Rc, sync::Arc, time::Duration};

use base::system::{System, SystemTimeInterface};
use base_io::io::Io;
use base_io_traits::fs_traits::{FileSystemInterface, FileSystemWatcherItemInterface};
use cache::Cache;
use client_render_game::render_game::{RenderGame, RenderGameInput, RenderGameInterface};
use config::config::{ConfigDebug, ConfigEngine};
use game_config::config::ConfigMap;
use game_interface::chat_commands::ChatCommands;
use graphics::{graphics::graphics::Graphics, handles::canvas::canvas::GraphicsCanvasHandle};
use graphics_backend::backend::GraphicsBackend;
use graphics_types::types::WindowProps;
use rayon::ThreadPool;
use sound::sound::SoundManager;
use ui_base::font_data::UiFontData;
use url::Url;
use wasm_runtime::WasmManager;

use super::render_wasm::render_wasm::RenderWasm;

pub enum RenderGameWrapper {
    Native(RenderGame),
    Wasm(RenderWasm),
}

impl RenderGameWrapper {
    pub fn as_ref(&self) -> &dyn RenderGameInterface {
        match self {
            RenderGameWrapper::Native(state) => state,
            RenderGameWrapper::Wasm(state) => state,
        }
    }

    pub fn as_mut(&mut self) -> &mut dyn RenderGameInterface {
        match self {
            RenderGameWrapper::Native(state) => state,
            RenderGameWrapper::Wasm(state) => state,
        }
    }
}

pub struct RenderGameWasmManager {
    state: RenderGameWrapper,
    fs_change_watcher: Box<dyn FileSystemWatcherItemInterface>,
    canvas_handle: GraphicsCanvasHandle,
    window_props: WindowProps,
}

const MODS_PATH: &str = "mods/render";

impl RenderGameWasmManager {
    pub async fn load_module(fs: &Arc<dyn FileSystemInterface>) -> anyhow::Result<Vec<u8>> {
        let cache = Arc::new(Cache::<0>::new(MODS_PATH, fs));
        // check if loading was finished
        let path_str = MODS_PATH.to_string() + "/render_game.wasm";

        cache
            .load(&path_str, |wasm_bytes| {
                Ok(WasmManager::compile_module(wasm_bytes)?
                    .serialize()?
                    .to_vec())
            })
            .await
    }

    pub fn new(
        sound: &SoundManager,
        graphics: &Graphics,
        backend: &Rc<GraphicsBackend>,
        io: &Io,
        thread_pool: &Arc<ThreadPool>,
        sys: &System,
        map_file: Vec<u8>,
        resource_download_server: Option<Url>,
        config: &ConfigEngine,
        wasm_module: Option<Vec<u8>>,
        fonts: Arc<UiFontData>,
    ) -> Self {
        let fs_change_watcher = io
            .fs
            .watch_for_change(MODS_PATH.as_ref(), Some("render_game.wasm".as_ref())); // TODO: even tho watching individual files makes more sense, it should still make sure it's the same the server watches

        let state = if let Some(wasm_module) = wasm_module {
            let state = RenderWasm::new(
                sound,
                graphics,
                backend,
                io,
                &wasm_module,
                map_file,
                resource_download_server,
                config,
                fonts,
            );
            RenderGameWrapper::Wasm(state)
        } else {
            let state = RenderGame::new(
                sound,
                graphics,
                io,
                thread_pool,
                &sys.time.time_get_nanoseconds(),
                map_file,
                resource_download_server,
                config,
                fonts,
            );
            RenderGameWrapper::Native(state)
        };
        Self {
            state,
            fs_change_watcher,
            window_props: graphics.canvas_handle.window_props(),
            canvas_handle: graphics.canvas_handle.clone(),
        }
    }

    pub fn should_reload(&self) -> bool {
        self.fs_change_watcher.has_file_change()
    }
}

impl RenderGameInterface for RenderGameWasmManager {
    fn render(
        &mut self,
        config_map: &ConfigMap,
        cur_time: &Duration,
        input: RenderGameInput,
    ) -> client_render_game::render_game::RenderGameResult {
        if let RenderGameWrapper::Wasm(state) = &self.state {
            let window_props = self.canvas_handle.window_props();
            if window_props != self.window_props {
                state.api_update_window_props(&window_props);
                self.window_props = window_props;
            }
        }
        self.state.as_mut().render(config_map, cur_time, input)
    }

    fn continue_map_loading(&mut self, config: &ConfigDebug) -> bool {
        self.state.as_mut().continue_map_loading(config)
    }

    fn set_chat_commands(&mut self, chat_commands: ChatCommands) {
        self.state.as_mut().set_chat_commands(chat_commands)
    }
}
