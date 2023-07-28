pub mod ui_render;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::anyhow;
use base_fs::{
    filesys::{FileSystem, FileSystemWatcherItem},
    io_batcher::{TokIOBatcher, TokIOBatcherTask},
};
use cache::Cache;
use graphics::graphics::Graphics;
use graphics_traits::GraphicsSizeQuery;
use native::native::NativeImpl;
use ui_base::{
    types::{UIPipe, UIRawInputGenerator, UIState},
    ui::{UIInterface, UI},
};
use ui_render::{destroy_ui, render_ui};
use wasm_logic_graphics::WasmGraphicsLogic;
use wasm_runtime::{WasmManager, WasmManagerModuleType};
use wasmer::Module;

pub struct UIWasmEntry {
    wasm_runtime: WasmManager,
    graphics_logic: WasmGraphicsLogic,
}

impl UIWasmEntry {
    fn call(&mut self, graphics: &mut Graphics) -> anyhow::Result<()> {
        self.graphics_logic.0.graphics.store(graphics);
        let res = self.wasm_runtime.run();
        self.graphics_logic.0.graphics.store(std::ptr::null_mut());
        res
    }
}

pub trait UIRenderCallbackFunc {
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<UIWinitWrapper>,
        ui_state: &mut UIState<UIWinitWrapper>,
        graphics: &mut Graphics,
        fs: &Arc<FileSystem>,
        io_batcher: &Arc<Mutex<TokIOBatcher>>,
    );

    fn destroy(self: Box<Self>, graphics: &mut Graphics);
}

enum UIRenderCallback {
    Wasm(UIWasmEntry),
    Native(Box<dyn UIRenderCallbackFunc>),
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
    ui_paths_loading: HashMap<String, TokIOBatcherTask<Vec<u8>>>,
    cache: Arc<Cache<202306060000>>,
    show_cur_page_during_load: bool,

    pub ui: UI<UIWinitWrapper>,
    last_path: String,

    fs_change_watcher: FileSystemWatcherItem,
}

pub enum UIWasmRunReturn {
    Loading,
    Success,
    Error404,
    RuntimeError(anyhow::Error),
}

pub enum UIWasmLoadingType {
    ShowCurrentPage,
    ShowLoadingPage(Box<dyn UIRenderCallbackFunc>),
}

const MODS_PATH: &str = "mods/ui";

impl UIWasmManager {
    pub fn new(
        native: &mut dyn NativeImpl,
        fs: &Arc<FileSystem>,
        error_404_page: Box<dyn UIRenderCallbackFunc>,
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
        let fs_change_watcher = fs.watch_for_change(MODS_PATH);
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
        }
    }

    pub fn destroy(mut self, graphics: &mut Graphics, io_batcher: &Arc<Mutex<TokIOBatcher>>) {
        destroy_ui(self.ui, graphics);
        self.ui_paths.drain().for_each(|(_, path)| match path {
            UIRenderCallback::Wasm(wasm_entry) => wasm_entry.wasm_runtime.destroy(),
            UIRenderCallback::Native(cb) => {
                cb.destroy(graphics);
            }
        });
        self.ui_paths_loading.drain().for_each(|(_, mut loading)| {
            io_batcher
                .lock()
                .unwrap()
                .wait_finished_and_drop(&mut loading)
        });
    }

    /**
     * returns Some, if the path was already registered
     * Re-registers/overwrites the path with the new callback in this case
     */
    pub fn register_path(
        &mut self,
        mod_name: &str,
        path: &str,
        cb: Box<dyn UIRenderCallbackFunc>,
        graphics: &mut Graphics,
    ) -> Option<()> {
        if let Some(_) = mod_name.find(|c: char| !c.is_ascii_alphabetic()) {
            panic!("Mod name must only contain ascii characters");
        }
        if let Some(_) = path.find(|c: char| !c.is_ascii_alphabetic()) {
            panic!("Path name must only contain ascii characters");
        }
        match self.ui_paths.insert(
            if !mod_name.is_empty() {
                mod_name.to_string() + "/"
            } else {
                "".to_string()
            } + path,
            UIRenderCallback::Native(cb),
        ) {
            Some(cb) => {
                match cb {
                    UIRenderCallback::Wasm(wasm_entry) => wasm_entry.wasm_runtime.destroy(),
                    UIRenderCallback::Native(cb) => cb.destroy(graphics),
                }
                Some(())
            }
            None => None,
        }
    }

    #[must_use]
    pub fn run_ui_path(
        &mut self,
        path: &str,
        fs: &Arc<FileSystem>,
        io_batcher: &Arc<Mutex<TokIOBatcher>>,
        graphics: &mut Graphics,
        pipe: &mut UIPipe<UIWinitWrapper>,
    ) -> UIWasmRunReturn {
        match self.ui_paths.get_mut(path) {
            Some(cb) => match cb {
                UIRenderCallback::Wasm(ui) => match ui.call(graphics) {
                    Ok(_) => UIWasmRunReturn::Success,
                    Err(err) => UIWasmRunReturn::RuntimeError(err),
                },
                UIRenderCallback::Native(cb) => {
                    let canvas_width = graphics.window_width();
                    let canvas_height = graphics.window_height();
                    let (screen_rect, full_output) = self.ui.render(
                        canvas_width,
                        canvas_height,
                        |egui_ui, pipe, ui_state| {
                            cb.render(egui_ui, pipe, ui_state, graphics, fs, io_batcher)
                        },
                        pipe,
                    );
                    render_ui(&mut self.ui, full_output, &screen_rect, graphics);
                    UIWasmRunReturn::Success
                }
            },
            None => {
                // check if the path is loading
                if let Some(loading_entry) = self.ui_paths_loading.get_mut(path) {
                    // check if loading was finished
                    if !loading_entry.is_finished() {
                        UIWasmRunReturn::Loading
                    } else {
                        match loading_entry.get_storage() {
                            Ok(item) => {
                                let graphics_logic = WasmGraphicsLogic::new();
                                let wasm_runtime: WasmManager =
                                    WasmManager::new(
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
                                    UIRenderCallback::Wasm(UIWasmEntry {
                                        wasm_runtime,
                                        graphics_logic,
                                    }),
                                );
                                self.run_ui_path(path, fs, io_batcher, graphics, pipe)
                            }
                            Err(_) => UIWasmRunReturn::Error404,
                        }
                    }
                } else {
                    let path_str = MODS_PATH.to_string() + "/" + path + ".wasm";
                    let cache = self.cache.clone();
                    let task = io_batcher.lock().unwrap().spawn(async move {
                        cache
                            .load(&path_str, |wasm_bytes| {
                                Ok(WasmManager::compile_module(&wasm_bytes[..])?
                                    .serialize()?
                                    .to_vec())
                            })
                            .await
                    });
                    self.ui_paths_loading.insert(path.to_string(), task);

                    UIWasmRunReturn::Loading
                }
            }
        }
    }

    pub fn render_if_open(
        &mut self,
        path: &str,
        fs: &Arc<FileSystem>,
        io_batcher: &Arc<Mutex<TokIOBatcher>>,
        graphics: &mut Graphics,
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
                destroy_paths.drain().for_each(|(_, item)| {
                    if let UIRenderCallback::Wasm(wasm_entry) = item {
                        wasm_entry.wasm_runtime.destroy();
                    }
                });
                self.ui_paths_loading.clear();
            }

            let success = match self.run_ui_path(path, fs, io_batcher, graphics, pipe) {
                UIWasmRunReturn::Loading => {
                    if self.show_cur_page_during_load {
                        match self.run_ui_path(
                            &self.last_path.clone(),
                            fs,
                            io_batcher,
                            graphics,
                            pipe,
                        ) {
                            UIWasmRunReturn::Loading => {
                                self.render_if_open("404", fs, io_batcher, graphics, pipe);
                                false
                            }
                            UIWasmRunReturn::Success => false,
                            UIWasmRunReturn::Error404 => {
                                self.render_if_open("404", fs, io_batcher, graphics, pipe);
                                false
                            }
                            UIWasmRunReturn::RuntimeError(_) => {
                                self.render_if_open("404", fs, io_batcher, graphics, pipe);
                                false
                            }
                        }
                    } else {
                        match self.run_ui_path("000", fs, io_batcher, graphics, pipe) {
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
                    match self.run_ui_path("404", fs, io_batcher, graphics, pipe) {
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
