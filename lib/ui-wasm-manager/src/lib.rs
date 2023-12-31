#![allow(clippy::all)]

use std::{collections::HashMap, rc::Rc, sync::Arc, time::Duration};

use anyhow::anyhow;
use base_io::{io::IO, io_batcher::IOBatcherTask};
use base_io_traits::fs_traits::{FileSystemInterface, FileSystemWatcherItemInterface};
use cache::Cache;

use config::config::ConfigPath;
use egui::{FontData, FontDefinitions, FontFamily};
use graphics::{
    graphics::Graphics,
    utils::{render_blur, render_swapped_frame, DEFAULT_BLUR_MIX_LENGTH, DEFAULT_BLUR_RADIUS},
};
use graphics_backend::backend::GraphicsBackend;

use math::math::vector::vec4;
use native::native::NativeImpl;
use ui_base::{
    style::default_style,
    types::{
        RawInputWrapper, RawOutputWrapper, UIFonts, UINativePipe, UINativeState, UIPipe,
        UIRawInputGenerator,
    },
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

    fn call_main_frame(
        &mut self,
        cur_time: &Duration,
        graphics: &mut Graphics,
        zoom_level: f32,
    ) -> anyhow::Result<()> {
        self.wasm_runtime.add_param(0, cur_time);
        self.wasm_runtime
            .add_param(1, &graphics.canvas_handle.window_props());
        self.wasm_runtime.add_param(2, &zoom_level);
        self.wasm_runtime.run_by_name::<()>("ui_main_frame")
    }

    #[must_use]
    fn call(
        &mut self,
        cur_time: &Duration,
        graphics: &mut Graphics,
        input: RawInputWrapper,
        zoom_level: f32,
    ) -> anyhow::Result<RawOutputWrapper> {
        self.wasm_runtime.add_param(0, cur_time);
        self.wasm_runtime
            .add_param(1, &graphics.canvas_handle.window_props());
        self.wasm_runtime.add_param(2, &input);
        self.wasm_runtime.add_param(3, &zoom_level);
        self.wasm_runtime.run_by_name::<()>("ui_run")?;
        let res = self.wasm_runtime.get_result_as::<RawOutputWrapper>();
        Ok(res)
    }
}

enum UIRenderCallback {
    Wasm(UIWasmEntry),
    Native(Box<dyn UIRenderCallbackFunc<()>>),
}

pub struct UIWinitWrapper {
    pub state: egui_winit::State,
}

pub struct UIWinitWrapperPipe<'a> {
    pub window: &'a winit::window::Window,
}

impl<'a> UIRawInputGenerator<UIWinitWrapper> for UIWinitWrapperPipe<'a> {
    fn get_raw_input(&self, state: &mut UINativeState<UIWinitWrapper>) -> egui::RawInput {
        state.native_state.state.take_egui_input(self.window)
    }
    fn process_output(
        &self,
        state: &mut UINativeState<UIWinitWrapper>,
        ctx: &egui::Context,
        output: egui::PlatformOutput,
    ) {
        state
            .native_state
            .state
            .handle_platform_output(self.window, ctx, output)
    }
}

pub struct UIWinitWrapperDummyPipe {}

impl UIRawInputGenerator<UIWinitWrapper> for UIWinitWrapperDummyPipe {
    fn get_raw_input(&self, _state: &mut UINativeState<UIWinitWrapper>) -> egui::RawInput {
        Default::default()
    }
    fn process_output(
        &self,
        _state: &mut UINativeState<UIWinitWrapper>,
        _ctx: &egui::Context,
        _output: egui::PlatformOutput,
    ) {
    }
}

pub struct UIWasmManager {
    ui_paths: HashMap<String, UIRenderCallback>,
    ui_paths_loading: HashMap<String, Option<IOBatcherTask<Vec<u8>>>>,
    cache: Arc<Cache<202306060000>>,
    show_cur_page_during_load: bool,

    pub ui: UI<UIWinitWrapper>,
    last_path: String,

    fs_change_watcher: Box<dyn FileSystemWatcherItemInterface>,

    /// id offset for textures buffers etc. that come from the wasm module's graphics
    id_offset: u128,

    fonts: UIFonts,
}

pub enum UIWasmRunReturn {
    Loading,
    Success,
    Error404,
    RuntimeError(anyhow::Error),
}

pub enum UIWasmLoadingType {
    ShowCurrentPage,
    ShowLoadingPage(Box<dyn UIRenderCallbackFunc<()>>),
}

const MODS_PATH: &str = "mods/ui";

const BLUR_COLOR: vec4 = vec4 {
    x: 0.5,
    y: 0.5,
    z: 0.5,
    w: 0.3,
};

impl UIWasmManager {
    fn get_font_data() -> (&'static [u8], &'static [u8]) {
        (
            include_bytes!("../../../data/fonts/DejaVuSans.ttf"),
            include_bytes!("../../../data/fonts/Icons.otf"),
        )
    }

    pub fn new(
        native: &mut dyn NativeImpl,
        fs: &Arc<dyn FileSystemInterface>,
        error_404_page: Box<dyn UIRenderCallbackFunc<()>>,
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

        let ui = UI::new(
            UIWinitWrapper {
                state: egui_winit::State::new(native.borrow_window()),
            },
            None,
        );

        let mut fonts = FontDefinitions::default();
        let (default_latin, icons) = Self::get_font_data();
        fonts.font_data.insert(
            "default_latin".to_owned(),
            FontData::from_static(default_latin),
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

        fonts
            .font_data
            .insert("icons".to_owned(), FontData::from_static(icons));
        fonts
            .families
            .insert(FontFamily::Name("icons".into()), vec!["icons".into()]);

        ui.context.egui_ctx.set_fonts(fonts.clone());

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

    /**
     * returns Some, if the path was already registered
     * Re-registers/overwrites the path with the new callback in this case
     */
    pub fn register_path(
        &mut self,
        mod_name: &str,
        path: &str,
        cb: Box<dyn UIRenderCallbackFunc<()>>,
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
        pipe: &mut UIPipe<()>,
        native_pipe: &mut UINativePipe<UIWinitWrapper>,
        window: &winit::window::Window,
        blur: bool,
    ) -> UIWasmRunReturn {
        match self.ui_paths.get_mut(path) {
            Some(cb) => match cb {
                UIRenderCallback::Wasm(ui) => {
                    if blur && ui.call_has_blur().unwrap_or(false) {
                        graphics.next_switch_pass();
                        match ui.call_main_frame(
                            &pipe.cur_time,
                            graphics,
                            self.ui
                                .ui_state
                                .zoom_level
                                .unwrap_or(self.ui.context.egui_ctx.pixels_per_point()),
                        ) {
                            Ok(_) => {
                                render_blur(
                                    graphics,
                                    true,
                                    DEFAULT_BLUR_RADIUS,
                                    DEFAULT_BLUR_MIX_LENGTH,
                                    &BLUR_COLOR,
                                );
                                render_swapped_frame(graphics);
                            }
                            Err(err) => {
                                render_swapped_frame(graphics);
                                return UIWasmRunReturn::RuntimeError(err);
                            }
                        }
                    }

                    self.ui
                        .ui_native_state
                        .native_state
                        .state
                        .set_pixels_per_point(self.ui.context.egui_ctx.pixels_per_point());
                    match ui.call(
                        &pipe.cur_time,
                        graphics,
                        RawInputWrapper {
                            input: native_pipe
                                .raw_inp_generator
                                .get_raw_input(&mut self.ui.ui_native_state),
                        },
                        self.ui
                            .ui_state
                            .zoom_level
                            .unwrap_or(self.ui.context.egui_ctx.pixels_per_point()),
                    ) {
                        Ok(output) => {
                            let old_pixel_per_point = self.ui.context.egui_ctx.pixels_per_point();
                            if let Some(output) = output.output {
                                self.ui
                                    .ui_native_state
                                    .native_state
                                    .state
                                    .handle_platform_output(
                                        window,
                                        &self.ui.context.egui_ctx,
                                        output,
                                    );
                            }
                            if old_pixel_per_point != output.zoom_level
                                || self
                                    .ui
                                    .ui_state
                                    .zoom_level
                                    .is_some_and(|val| val != output.zoom_level)
                            {
                                self.ui.ui_state.zoom_level = Some(output.zoom_level);
                            }
                            UIWasmRunReturn::Success
                        }
                        Err(err) => UIWasmRunReturn::RuntimeError(err),
                    }
                }
                UIRenderCallback::Native(cb) => {
                    self.ui.context.egui_ctx.set_style(default_style());
                    self.ui.stencil_context.egui_ctx.set_style(default_style());
                    let window_width = graphics.canvas_handle.window_width();
                    let window_height = graphics.canvas_handle.window_height();
                    let window_pixels_per_point = graphics.canvas_handle.window_pixels_per_point();
                    let old_pixel_per_point = self.ui.context.egui_ctx.pixels_per_point();
                    if blur && cb.has_blur() {
                        graphics.next_switch_pass();
                        let (screen_rect, full_output, zoom_level) = self.ui.render(
                            window_width,
                            window_height,
                            window_pixels_per_point,
                            |egui_ui, pipe, ui_state| {
                                cb.render_main_frame(egui_ui, pipe, ui_state, graphics);
                            },
                            &mut UIPipe::new(pipe.ui_feedback, pipe.cur_time, pipe.config, ()),
                            &mut UINativePipe {
                                raw_inp_generator: &mut UIWinitWrapperDummyPipe {},
                            },
                            true,
                        );
                        render_ui(
                            &mut self.ui,
                            &mut UINativePipe {
                                raw_inp_generator: &mut UIWinitWrapperDummyPipe {},
                            },
                            full_output,
                            &screen_rect,
                            zoom_level,
                            graphics,
                            true,
                        );
                        render_blur(
                            graphics,
                            true,
                            DEFAULT_BLUR_RADIUS,
                            DEFAULT_BLUR_MIX_LENGTH,
                            &BLUR_COLOR,
                        );
                        render_swapped_frame(graphics);
                    }
                    let (screen_rect, full_output, zoom_level) = self.ui.render(
                        window_width,
                        window_height,
                        window_pixels_per_point,
                        |egui_ui, pipe, ui_state| cb.render(egui_ui, pipe, ui_state, graphics),
                        pipe,
                        native_pipe,
                        false,
                    );
                    render_ui(
                        &mut self.ui,
                        native_pipe,
                        full_output,
                        &screen_rect,
                        zoom_level,
                        graphics,
                        false,
                    );
                    if old_pixel_per_point != zoom_level
                        || self
                            .ui
                            .ui_state
                            .zoom_level
                            .is_some_and(|val| val != zoom_level)
                    {
                        self.ui.ui_state.zoom_level = Some(zoom_level);
                    }
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
                                        path,
                                        io,
                                        graphics,
                                        backend,
                                        pipe,
                                        native_pipe,
                                        window,
                                        blur,
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

    pub fn render_if_open(
        &mut self,
        path: &str,
        io: &IO,
        graphics: &mut Graphics,
        backend: &Rc<GraphicsBackend>,
        pipe: &mut UIPipe<()>,
        native_pipe: &mut UINativePipe<UIWinitWrapper>,
        window: &winit::window::Window,
        blur: bool,
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

            let success = match self.run_ui_path(
                path,
                io,
                graphics,
                backend,
                pipe,
                native_pipe,
                window,
                blur,
            ) {
                UIWasmRunReturn::Loading => {
                    if self.show_cur_page_during_load {
                        match self.run_ui_path(
                            &self.last_path.clone(),
                            io,
                            graphics,
                            backend,
                            pipe,
                            native_pipe,
                            window,
                            blur,
                        ) {
                            UIWasmRunReturn::Loading => {
                                self.render_if_open(
                                    "404",
                                    io,
                                    graphics,
                                    backend,
                                    pipe,
                                    native_pipe,
                                    window,
                                    blur,
                                );
                                false
                            }
                            UIWasmRunReturn::Success => false,
                            UIWasmRunReturn::Error404 => {
                                self.render_if_open(
                                    "404",
                                    io,
                                    graphics,
                                    backend,
                                    pipe,
                                    native_pipe,
                                    window,
                                    blur,
                                );
                                false
                            }
                            UIWasmRunReturn::RuntimeError(_) => {
                                self.render_if_open(
                                    "404",
                                    io,
                                    graphics,
                                    backend,
                                    pipe,
                                    native_pipe,
                                    window,
                                    blur,
                                );
                                false
                            }
                        }
                    } else {
                        match self.run_ui_path(
                            "000",
                            io,
                            graphics,
                            backend,
                            pipe,
                            native_pipe,
                            window,
                            blur,
                        ) {
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
                    match self.run_ui_path(
                        "404",
                        io,
                        graphics,
                        backend,
                        pipe,
                        native_pipe,
                        window,
                        blur,
                    ) {
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
                UIWasmRunReturn::RuntimeError(err) => todo!("{err}"),
            };

            if success {
                self.last_path = path.to_string();
            }
        }
    }
}
