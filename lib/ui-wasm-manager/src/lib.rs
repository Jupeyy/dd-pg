use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc, time::Duration};

use anyhow::anyhow;
use base_fs::{
    filesys::{FileSystem, FileSystemWatcherItem},
    io_batcher::{TokIOBatcher, TokIOBatcherTask},
};
use cache::Cache;

use config::config::ConfigPath;
use graphics_backend::{backend::GraphicsBackend, types::Graphics};
use graphics_base_traits::traits::GraphicsSizeQuery;
use native::native::NativeImpl;
use ui_base::{
    types::{RawInputWrapper, UIPipe, UIRawInputGenerator, UIState},
    ui::{UIInterface, UI},
    ui_render::render_ui,
};
use ui_traits::traits::UIRenderCallbackFunc;
use wasm_logic_graphics::WasmGraphicsLogic;
use wasm_runtime::{WasmManager, WasmManagerModuleType};
use wasmer::Module;

pub struct UIWasmEntry {
    wasm_runtime: WasmManager,
}

impl UIWasmEntry {
    fn call(
        &mut self,
        cur_time: &Duration,
        graphics: &mut Graphics,
        input: RawInputWrapper,
        zoom_level: f32,
    ) -> anyhow::Result<()> {
        self.wasm_runtime.add_param(0, cur_time);
        self.wasm_runtime.add_param(1, &graphics.window_props());
        self.wasm_runtime.add_param(2, &input);
        self.wasm_runtime.add_param(3, &zoom_level);
        let res = self.wasm_runtime.run_by_name("ui_run");
        res
    }
}

enum UIRenderCallback {
    Wasm(UIWasmEntry),
    Native(Box<dyn UIRenderCallbackFunc<UIWinitWrapper, GraphicsBackend>>),
}

pub struct UIWinitWrapper {
    pub state: egui_winit::State,
}

pub struct UIWinitWrapperPipe<'a> {
    pub window: &'a winit::window::Window,
}

impl<'a> UIRawInputGenerator<UIWinitWrapper> for UIWinitWrapperPipe<'a> {
    fn get_raw_input(&self, state: &mut UIState<UIWinitWrapper>) -> egui::RawInput {
        state.native_state.state.take_egui_input(self.window)
    }
}

pub struct UIWasmManager {
    ui_paths: HashMap<String, UIRenderCallback>,
    ui_paths_loading: HashMap<String, Option<TokIOBatcherTask<Vec<u8>>>>,
    cache: Arc<Cache<202306060000>>,
    show_cur_page_during_load: bool,

    pub ui: UI<UIWinitWrapper>,
    last_path: String,

    fs_change_watcher: FileSystemWatcherItem,

    /// id offset for textures buffers etc. that come from the wasm module's graphics
    id_offset: u128,
}

pub enum UIWasmRunReturn {
    Loading,
    Success,
    Error404,
    RuntimeError(anyhow::Error),
}

pub enum UIWasmLoadingType {
    ShowCurrentPage,
    ShowLoadingPage(Box<dyn UIRenderCallbackFunc<UIWinitWrapper, GraphicsBackend>>),
}

const MODS_PATH: &str = "mods/ui";

impl UIWasmManager {
    pub fn new(
        native: &mut dyn NativeImpl,
        fs: &Arc<FileSystem>,
        error_404_page: Box<dyn UIRenderCallbackFunc<UIWinitWrapper, GraphicsBackend>>,
        loading_page: UIWasmLoadingType,
    ) -> Self {
        let cache = Arc::new(Cache::new(MODS_PATH, fs));
        let mut ui_paths = HashMap::<String, UIRenderCallback>::default();
        let mut show_cur_page_during_load = false;
        match loading_page {
            UIWasmLoadingType::ShowCurrentPage => show_cur_page_during_load = true,
            UIWasmLoadingType::ShowLoadingPage(page) => {
                ui_paths.insert("000".to_string(), UIRenderCallback::Native(page));
            }
        }
        ui_paths.insert("404".to_string(), UIRenderCallback::Native(error_404_page));
        let fs_change_watcher = fs.watch_for_change(MODS_PATH, None);
        Self {
            ui_paths,
            ui_paths_loading: Default::default(),
            show_cur_page_during_load,
            cache,
            ui: UI::new(
                UIWinitWrapper {
                    state: egui_winit::State::new(native.borrow_window()),
                },
                1.5,
            ),
            last_path: "404".to_string(),
            fs_change_watcher,

            id_offset: u64::MAX as u128,
        }
    }

    /**
     * returns Some, if the path was already registered
     * Re-registers/overwrites the path with the new callback in this case
     */
    pub fn register_path(
        &mut self,
        mod_name: &str,
        path: &str,
        cb: Box<dyn UIRenderCallbackFunc<UIWinitWrapper, GraphicsBackend>>,
    ) -> Option<()> {
        ConfigPath::is_route_correct(mod_name, path).unwrap();
        self.ui_paths
            .insert(
                if !mod_name.is_empty() {
                    mod_name.to_string() + "/"
                } else {
                    "".to_string()
                } + path,
                UIRenderCallback::Native(cb),
            )
            .map(|_| ())
    }

    #[must_use]
    pub fn run_ui_path(
        &mut self,
        path: &str,
        fs: &Arc<FileSystem>,
        io_batcher: &TokIOBatcher,
        graphics: &mut Graphics,
        backend: &Rc<RefCell<GraphicsBackend>>,
        pipe: &mut UIPipe<UIWinitWrapper>,
    ) -> UIWasmRunReturn {
        match self.ui_paths.get_mut(path) {
            Some(cb) => match cb {
                UIRenderCallback::Wasm(ui) => match ui.call(
                    &pipe.cur_time,
                    graphics,
                    RawInputWrapper {
                        input: pipe.raw_inp_generator.get_raw_input(&mut self.ui.ui_state),
                    },
                    self.ui.ui_state.zoom_level,
                ) {
                    Ok(_) => UIWasmRunReturn::Success,
                    Err(err) => UIWasmRunReturn::RuntimeError(err),
                },
                UIRenderCallback::Native(cb) => {
                    let canvas_width = graphics.window_width();
                    let canvas_height = graphics.window_height();
                    let (screen_rect, full_output) = self.ui.render(
                        canvas_width,
                        canvas_height,
                        |egui_ui, pipe, ui_state| cb.render(egui_ui, pipe, ui_state, graphics),
                        pipe,
                    );
                    render_ui(&mut self.ui, full_output, &screen_rect, graphics);
                    UIWasmRunReturn::Success
                }
            },
            None => {
                // check if the path is loading
                if let Some(loading_entry) = self.ui_paths_loading.remove(path) {
                    // check if loading was finished
                    if loading_entry
                        .as_ref()
                        .is_some_and(|task| !task.is_finished())
                    {
                        self.ui_paths_loading
                            .insert(path.to_string(), loading_entry);
                        UIWasmRunReturn::Loading
                    } else {
                        match loading_entry {
                            Some(loading_entry) => match loading_entry.get_storage() {
                                Ok(item) => {
                                    let graphics_logic = WasmGraphicsLogic::new(
                                        graphics,
                                        backend.clone(),
                                        self.id_offset,
                                    );
                                    self.id_offset += u64::MAX as u128;
                                    let wasm_runtime: WasmManager = WasmManager::new(
                                        WasmManagerModuleType::FromClosure(|store| {
                                            match unsafe { Module::deserialize(store, &item[..]) } {
                                                Ok(module) => Ok(module),
                                                Err(err) => Err(anyhow!(err)),
                                            }
                                        }),
                                        |store, raw_bytes_env| {
                                            Some(graphics_logic.get_wasm_graphics_logic_imports(
                                                store,
                                                raw_bytes_env,
                                            ))
                                        },
                                    )
                                    .unwrap();
                                    self.ui_paths.insert(
                                        path.to_string(),
                                        UIRenderCallback::Wasm(UIWasmEntry { wasm_runtime }),
                                    );
                                    self.run_ui_path(path, fs, io_batcher, graphics, backend, pipe)
                                }
                                Err(_) => {
                                    self.ui_paths_loading.insert(path.to_string(), None);
                                    UIWasmRunReturn::Error404
                                }
                            },
                            None => {
                                self.ui_paths_loading.insert(path.to_string(), None);
                                UIWasmRunReturn::Error404
                            }
                        }
                    }
                } else {
                    let path_str = MODS_PATH.to_string() + "/" + path + ".wasm";
                    let cache = self.cache.clone();
                    let task = io_batcher.spawn(async move {
                        cache
                            .load(&path_str, |wasm_bytes| {
                                Ok(WasmManager::compile_module(&wasm_bytes[..])?
                                    .serialize()?
                                    .to_vec())
                            })
                            .await
                    });
                    self.ui_paths_loading.insert(path.to_string(), Some(task));

                    UIWasmRunReturn::Loading
                }
            }
        }
    }

    pub fn render_if_open(
        &mut self,
        path: &str,
        fs: &Arc<FileSystem>,
        io_batcher: &TokIOBatcher,
        graphics: &mut Graphics,
        backend: &Rc<RefCell<GraphicsBackend>>,
        pipe: &mut UIPipe<UIWinitWrapper>,
    ) {
        if self.ui.ui_state.is_ui_open {
            // check for changes
            if self.fs_change_watcher.has_file_change() {
                // clear all paths that are dynamically loaded
                let (keep_paths, mut destroy_paths) =
                    self.ui_paths.drain().partition(|(_, item)| match item {
                        UIRenderCallback::Wasm(_) => false,
                        UIRenderCallback::Native(_) => true,
                    });
                self.ui_paths = keep_paths;
                destroy_paths.clear();
                self.ui_paths_loading.clear();
            }

            let success = match self.run_ui_path(path, fs, io_batcher, graphics, backend, pipe) {
                UIWasmRunReturn::Loading => {
                    if self.show_cur_page_during_load {
                        match self.run_ui_path(
                            &self.last_path.clone(),
                            fs,
                            io_batcher,
                            graphics,
                            backend,
                            pipe,
                        ) {
                            UIWasmRunReturn::Loading => {
                                self.render_if_open("404", fs, io_batcher, graphics, backend, pipe);
                                false
                            }
                            UIWasmRunReturn::Success => false,
                            UIWasmRunReturn::Error404 => {
                                self.render_if_open("404", fs, io_batcher, graphics, backend, pipe);
                                false
                            }
                            UIWasmRunReturn::RuntimeError(_) => {
                                self.render_if_open("404", fs, io_batcher, graphics, backend, pipe);
                                false
                            }
                        }
                    } else {
                        match self.run_ui_path("000", fs, io_batcher, graphics, backend, pipe) {
                            UIWasmRunReturn::Loading => {
                                panic!("this should never happen")
                            }
                            UIWasmRunReturn::Success => false,
                            UIWasmRunReturn::Error404 => {
                                panic!("this should never happen")
                            }
                            UIWasmRunReturn::RuntimeError(_) => {
                                panic!("this should never happen")
                            }
                        }
                    }
                }
                UIWasmRunReturn::Success => true,
                UIWasmRunReturn::Error404 => {
                    match self.run_ui_path("404", fs, io_batcher, graphics, backend, pipe) {
                        UIWasmRunReturn::Loading => {
                            panic!("this should never happen")
                        }
                        UIWasmRunReturn::Success => false,
                        UIWasmRunReturn::Error404 => {
                            panic!("this should never happen")
                        }
                        UIWasmRunReturn::RuntimeError(_) => {
                            panic!("this should never happen")
                        }
                    }
                }
                UIWasmRunReturn::RuntimeError(_) => todo!(),
            };

            if success {
                self.last_path = path.to_string();
            }
        }
    }
}
