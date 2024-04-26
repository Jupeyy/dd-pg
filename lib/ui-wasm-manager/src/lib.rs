#![allow(clippy::all)]

use std::{collections::HashMap, rc::Rc, sync::Arc, time::Duration};

use anyhow::anyhow;
use base_io::{io::IO, io_batcher::IOBatcherTask};
use base_io_traits::fs_traits::{FileSystemInterface, FileSystemWatcherItemInterface};
use cache::Cache;

use config::config::ConfigPath;
use egui::{FontData, FontDefinitions, FontFamily};
use graphics::{
    graphics::graphics::Graphics,
    utils::{render_blur, render_swapped_frame, DEFAULT_BLUR_MIX_LENGTH, DEFAULT_BLUR_RADIUS},
};
use graphics_backend::backend::GraphicsBackend;

use math::math::vector::vec4;
use serde::Serialize;
use ui_base::{
    font_data::UiFontData,
    types::{RawInputWrapper, RawOutputWrapper, UIFonts, UIPipe},
    ui::UI,
    ui_render::render_ui,
};
use ui_traits::traits::UIRenderCallbackFunc;
use wasm_logic_fs::fs::WasmFileSystemLogic;
use wasm_logic_graphics::WasmGraphicsLogic;
use wasm_runtime::{WasmManager, WasmManagerModuleType};
use wasmer::Module;

pub struct UIWasmEntry {
    wasm_runtime: WasmManager,
}

impl UIWasmEntry {
    fn call_new(&mut self, fonts: &UIFonts) -> anyhow::Result<()> {
        self.wasm_runtime.add_param(0, fonts);
        self.wasm_runtime.run_by_name::<()>("ui_new")
    }

    fn call_has_blur(&mut self) -> anyhow::Result<bool> {
        Ok(match self.wasm_runtime.run_by_name::<u8>("ui_has_blur")? {
            0 => false,
            _ => true,
        })
    }

    fn call_main_frame<U: Serialize>(
        &mut self,
        cur_time: &Duration,
        graphics: &mut Graphics,
        zoom_level: Option<f32>,
        user_data: &U,
    ) -> anyhow::Result<()> {
        self.wasm_runtime.add_param(0, cur_time);
        self.wasm_runtime
            .add_param(1, &graphics.canvas_handle.window_props());
        self.wasm_runtime.add_param(2, &zoom_level);
        self.wasm_runtime.add_param(3, user_data);
        self.wasm_runtime.run_by_name::<()>("ui_main_frame")
    }

    #[must_use]
    fn call<U: Serialize>(
        &mut self,
        cur_time: &Duration,
        graphics: &mut Graphics,
        input: RawInputWrapper,
        zoom_level: Option<f32>,
        user_data: &U,
    ) -> anyhow::Result<RawOutputWrapper> {
        self.wasm_runtime.add_param(0, cur_time);
        self.wasm_runtime
            .add_param(1, &graphics.canvas_handle.window_props());
        self.wasm_runtime.add_param(2, &input);
        self.wasm_runtime.add_param(3, &zoom_level);
        self.wasm_runtime.add_param(4, user_data);
        self.wasm_runtime.run_by_name::<()>("ui_run")?;
        let res = self.wasm_runtime.get_result_as::<RawOutputWrapper>();
        Ok(res)
    }
}

enum UIRenderCallback<U> {
    Wasm(UIWasmEntry),
    Native(Box<dyn UIRenderCallbackFunc<U>>),
}

pub struct UIWasmManagerBase<U>
where
    for<'a> U: 'a,
{
    ui_paths: HashMap<String, UIRenderCallback<U>>,
    ui_paths_loading: HashMap<String, Option<IOBatcherTask<Vec<u8>>>>,
    cache: Arc<Cache<202306060000>>,
    show_cur_page_during_load: bool,

    pub ui: UI,
    last_path: String,

    fs_change_watcher: Box<dyn FileSystemWatcherItemInterface>,

    /// id offset for textures buffers etc. that come from the wasm module's graphics
    id_offset: u128,

    fonts: UIFonts,
}

pub enum UIWasmRunReturn {
    Loading,
    Success(egui::PlatformOutput),
    Error404,
    RuntimeError(anyhow::Error),
}

pub enum UIWasmLoadingType<U> {
    ShowCurrentPage,
    ShowLoadingPage(Box<dyn UIRenderCallbackFunc<U>>),
}

const MODS_PATH: &str = "mods/ui";

const BLUR_COLOR: vec4 = vec4 {
    x: 0.5,
    y: 0.5,
    z: 0.5,
    w: 0.3,
};

impl<U: Serialize> UIWasmManagerBase<U>
where
    for<'a> U: 'a,
{
    pub fn new(
        fs: &Arc<dyn FileSystemInterface>,
        error_404_page: Box<dyn UIRenderCallbackFunc<U>>,
        loading_page: UIWasmLoadingType<U>,
        shared_fonts: &Arc<UiFontData>,
    ) -> Self {
        let cache = Arc::new(Cache::new(MODS_PATH, fs));
        let mut ui_paths = HashMap::<String, UIRenderCallback<U>>::default();
        let mut show_cur_page_during_load = false;
        match loading_page {
            UIWasmLoadingType::ShowCurrentPage => show_cur_page_during_load = true,
            UIWasmLoadingType::ShowLoadingPage(page) => {
                ui_paths.insert("000".to_string(), UIRenderCallback::Native(page));
            }
        }
        ui_paths.insert("404".to_string(), UIRenderCallback::Native(error_404_page));
        let fs_change_watcher = fs.watch_for_change(MODS_PATH.as_ref(), None);

        let ui = UI::new(None);

        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "default_latin".to_owned(),
            FontData::from_owned(shared_fonts.latin.clone()),
        );
        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .insert(0, "default_latin".to_owned());

        fonts
            .families
            .get_mut(&FontFamily::Monospace)
            .unwrap()
            .push("default_latin".to_owned());

        fonts.font_data.insert(
            "icons".to_owned(),
            FontData::from_owned(shared_fonts.icon.clone()),
        );
        fonts
            .families
            .insert(FontFamily::Name("icons".into()), vec!["icons".into()]);

        ui.context.egui_ctx.set_fonts(fonts.clone());
        ui.stencil_context.egui_ctx.set_fonts(fonts.clone());

        Self {
            ui_paths,
            ui_paths_loading: Default::default(),
            show_cur_page_during_load,
            cache,
            ui,
            last_path: "404".to_string(),
            fs_change_watcher,

            id_offset: u64::MAX as u128,

            fonts: UIFonts { fonts },
        }
    }

    /// returns Some, if the path was already registered
    /// Re-registers/overwrites the path with the new callback in this case
    pub fn register_path(
        &mut self,
        mod_name: &str,
        path: &str,
        cb: Box<dyn UIRenderCallbackFunc<U>>,
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
        io: &IO,
        graphics: &mut Graphics,
        backend: &Rc<GraphicsBackend>,
        pipe: &mut UIPipe<U>,
        inp: egui::RawInput,
        window: &winit::window::Window,
        blur: bool,
    ) -> UIWasmRunReturn {
        match self.ui_paths.get_mut(path) {
            Some(cb) => match cb {
                UIRenderCallback::Wasm(ui) => {
                    if blur && ui.call_has_blur().unwrap_or(false) {
                        graphics.backend_handle.next_switch_pass();
                        match ui.call_main_frame(
                            &pipe.cur_time,
                            graphics,
                            self.ui.ui_state.zoom_level,
                            &pipe.user_data,
                        ) {
                            Ok(_) => {
                                render_blur(
                                    &graphics.backend_handle,
                                    &graphics.stream_handle,
                                    &graphics.canvas_handle,
                                    true,
                                    DEFAULT_BLUR_RADIUS,
                                    DEFAULT_BLUR_MIX_LENGTH,
                                    &BLUR_COLOR,
                                );
                                render_swapped_frame(
                                    &graphics.canvas_handle,
                                    &graphics.stream_handle,
                                );
                            }
                            Err(err) => {
                                render_swapped_frame(
                                    &graphics.canvas_handle,
                                    &graphics.stream_handle,
                                );
                                return UIWasmRunReturn::RuntimeError(err);
                            }
                        }
                    }

                    match ui.call(
                        &pipe.cur_time,
                        graphics,
                        RawInputWrapper { input: inp },
                        self.ui.ui_state.zoom_level,
                        &pipe.user_data,
                    ) {
                        Ok(output) => {
                            self.ui.ui_state.zoom_level = output.zoom_level;
                            UIWasmRunReturn::Success(output.output)
                        }
                        Err(err) => UIWasmRunReturn::RuntimeError(err),
                    }
                }
                UIRenderCallback::Native(cb) => {
                    let window_width = graphics.canvas_handle.window_width();
                    let window_height = graphics.canvas_handle.window_height();
                    let window_pixels_per_point = graphics.canvas_handle.window_pixels_per_point();
                    if blur && cb.has_blur() {
                        graphics.backend_handle.next_switch_pass();
                        let (screen_rect, full_output, zoom_level) = self.ui.render(
                            window_width,
                            window_height,
                            window_pixels_per_point,
                            |egui_ui, pipe, ui_state| {
                                cb.render_main_frame(egui_ui, pipe, ui_state);
                            },
                            pipe,
                            inp.clone(),
                            true,
                        );
                        render_ui(
                            &mut self.ui,
                            full_output,
                            &screen_rect,
                            zoom_level,
                            &graphics.backend_handle,
                            &graphics.texture_handle,
                            &graphics.stream_handle,
                            true,
                        );
                        render_blur(
                            &graphics.backend_handle,
                            &graphics.stream_handle,
                            &graphics.canvas_handle,
                            true,
                            DEFAULT_BLUR_RADIUS,
                            DEFAULT_BLUR_MIX_LENGTH,
                            &BLUR_COLOR,
                        );
                        render_swapped_frame(&graphics.canvas_handle, &graphics.stream_handle);
                    }
                    let (screen_rect, full_output, zoom_level) = self.ui.render(
                        window_width,
                        window_height,
                        window_pixels_per_point,
                        |egui_ui, pipe, ui_state| cb.render(egui_ui, pipe, ui_state),
                        pipe,
                        inp,
                        false,
                    );
                    let res = render_ui(
                        &mut self.ui,
                        full_output,
                        &screen_rect,
                        zoom_level,
                        &graphics.backend_handle,
                        &graphics.texture_handle,
                        &graphics.stream_handle,
                        false,
                    );
                    UIWasmRunReturn::Success(res)
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
                                    let fs_logic = WasmFileSystemLogic::new(io.clone());
                                    self.id_offset += u64::MAX as u128;
                                    let wasm_runtime: WasmManager = WasmManager::new(
                                        WasmManagerModuleType::FromClosure(|store| {
                                            match unsafe { Module::deserialize(store, &item[..]) } {
                                                Ok(module) => Ok(module),
                                                Err(err) => Err(anyhow!(err)),
                                            }
                                        }),
                                        |store, raw_bytes_env| {
                                            let mut imports = graphics_logic
                                                .get_wasm_graphics_logic_imports(
                                                    store,
                                                    raw_bytes_env,
                                                );
                                            imports.extend(
                                                &fs_logic.get_wasm_graphics_logic_imports(
                                                    store,
                                                    raw_bytes_env,
                                                ),
                                            );
                                            Some(imports)
                                        },
                                    )
                                    .unwrap();
                                    let mut entry = UIWasmEntry { wasm_runtime };
                                    entry.call_new(&self.fonts).unwrap();
                                    self.ui_paths
                                        .insert(path.to_string(), UIRenderCallback::Wasm(entry));
                                    self.run_ui_path(
                                        path, io, graphics, backend, pipe, inp, window, blur,
                                    )
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
                    let task = io.io_batcher.spawn(async move {
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

    pub fn render(
        &mut self,
        path: &str,
        io: &IO,
        graphics: &mut Graphics,
        backend: &Rc<GraphicsBackend>,
        pipe: &mut UIPipe<U>,
        inp: egui::RawInput,
        window: &winit::window::Window,
        blur: bool,
    ) -> Option<egui::PlatformOutput> {
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

        let (success, platform_output) =
            match self.run_ui_path(path, io, graphics, backend, pipe, inp.clone(), window, blur) {
                UIWasmRunReturn::Loading => {
                    if self.show_cur_page_during_load {
                        match self.run_ui_path(
                            &self.last_path.clone(),
                            io,
                            graphics,
                            backend,
                            pipe,
                            inp.clone(),
                            window,
                            blur,
                        ) {
                            UIWasmRunReturn::Loading => (
                                false,
                                self.render(
                                    "404",
                                    io,
                                    graphics,
                                    backend,
                                    pipe,
                                    inp.clone(),
                                    window,
                                    blur,
                                ),
                            ),
                            UIWasmRunReturn::Success(output) => (false, Some(output)),
                            UIWasmRunReturn::Error404 => (
                                false,
                                self.render(
                                    "404",
                                    io,
                                    graphics,
                                    backend,
                                    pipe,
                                    inp.clone(),
                                    window,
                                    blur,
                                ),
                            ),
                            UIWasmRunReturn::RuntimeError(_) => (
                                false,
                                self.render("404", io, graphics, backend, pipe, inp, window, blur),
                            ),
                        }
                    } else {
                        match self.run_ui_path(
                            "000",
                            io,
                            graphics,
                            backend,
                            pipe,
                            inp.clone(),
                            window,
                            blur,
                        ) {
                            UIWasmRunReturn::Loading => {
                                panic!("this should never happen")
                            }
                            UIWasmRunReturn::Success(output) => (false, Some(output)),
                            UIWasmRunReturn::Error404 => {
                                panic!("this should never happen")
                            }
                            UIWasmRunReturn::RuntimeError(_) => {
                                panic!("this should never happen")
                            }
                        }
                    }
                }
                UIWasmRunReturn::Success(output) => (true, Some(output)),
                UIWasmRunReturn::Error404 => {
                    match self.run_ui_path("404", io, graphics, backend, pipe, inp, window, blur) {
                        UIWasmRunReturn::Loading => {
                            panic!("this should never happen")
                        }
                        UIWasmRunReturn::Success(output) => (false, Some(output)),
                        UIWasmRunReturn::Error404 => {
                            panic!("this should never happen")
                        }
                        UIWasmRunReturn::RuntimeError(_) => {
                            panic!("this should never happen")
                        }
                    }
                }
                UIWasmRunReturn::RuntimeError(err) => todo!("{err}"),
            };

        if success {
            self.last_path = path.to_string();
        }
        platform_output
    }
}
