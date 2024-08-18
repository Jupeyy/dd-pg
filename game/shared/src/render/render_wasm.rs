use api_wasm_macros::wasm_mod_prepare_render_game;

#[wasm_mod_prepare_render_game]
pub mod render_wasm {
    use std::{rc::Rc, sync::Arc, time::Duration};

    use anyhow::anyhow;
    use api_wasm_macros::wasm_func_auto_call;
    use base_io::io::Io;
    use client_render_game::render_game::{RenderGameInput, RenderGameInterface, RenderGameResult};
    use config::config::{ConfigDebug, ConfigEngine};
    use game_config::config::ConfigMap;
    use game_interface::chat_commands::ChatCommands;
    use graphics::graphics::graphics::Graphics;
    use graphics_backend::backend::GraphicsBackend;
    use graphics_types::types::WindowProps;
    use sound::sound::SoundManager;
    use ui_base::font_data::UiFontData;
    use url::Url;
    use wasm_logic_fs::fs::WasmFileSystemLogic;
    use wasm_logic_graphics::WasmGraphicsLogic;
    use wasm_logic_http::http::WasmHttpLogic;
    use wasm_logic_sound::sound::WasmSoundLogic;
    use wasm_runtime::{WasmManager, WasmManagerModuleType};
    use wasmer::Module;

    pub struct RenderWasm {
        wasm_manager: WasmManager,

        api_update_window_props_name: wasmer::TypedFunction<(), ()>,
    }

    #[constructor]
    impl RenderWasm {
        pub fn new(
            sound: &SoundManager,
            graphics: &Graphics,
            backend: &Rc<GraphicsBackend>,
            io: &Io,
            wasm_module: &[u8],
            map_file: Vec<u8>,
            resource_download_server: Option<Url>,
            config: &ConfigEngine,
            fonts: Arc<UiFontData>,
        ) -> Self {
            let sound_logic = WasmSoundLogic::new(u128::MAX / 2, sound);
            let graphics_logic = WasmGraphicsLogic::new(graphics, backend.clone(), u128::MAX / 2);
            let fs_logic = WasmFileSystemLogic::new(io.clone());
            let http_logic = WasmHttpLogic::new(io.clone());
            let wasm_manager: WasmManager = WasmManager::new(
                WasmManagerModuleType::FromClosure(|store| {
                    match unsafe { Module::deserialize(store, wasm_module) } {
                        Ok(module) => Ok(module),
                        Err(err) => Err(anyhow!(err)),
                    }
                }),
                |store, raw_bytes_env| {
                    let mut imports = graphics_logic.get_wasm_logic_imports(store, raw_bytes_env);
                    imports.extend(&fs_logic.get_wasm_logic_imports(store, raw_bytes_env));
                    imports.extend(&sound_logic.get_wasm_logic_imports(store, raw_bytes_env));
                    imports.extend(&http_logic.get_wasm_logic_imports(store, raw_bytes_env));
                    Some(imports)
                },
            )
            .unwrap();
            wasm_manager.add_param(0, &map_file);
            wasm_manager.add_param(1, &resource_download_server);
            wasm_manager.add_param(2, config);
            wasm_manager.add_param(3, &graphics.canvas_handle.window_props());
            wasm_manager.add_param(4, &fonts);
            wasm_manager.run_by_name::<()>("render_game_new").unwrap();

            let api_update_window_props_name =
                wasm_manager.run_func_by_name("api_update_window_props");

            Self {
                wasm_manager,
                api_update_window_props_name,
            }
        }
    }

    impl RenderWasm {
        #[wasm_func_auto_call]
        pub fn api_update_window_props(&self, window_props: &WindowProps) {}
    }

    impl RenderGameInterface for RenderWasm {
        #[wasm_func_auto_call]
        fn render(
            &mut self,
            config_map: &ConfigMap,
            cur_time: &Duration,
            input: RenderGameInput,
        ) -> RenderGameResult {
        }

        #[wasm_func_auto_call]
        fn continue_map_loading(&mut self, config: &ConfigDebug) -> bool {}

        #[wasm_func_auto_call]
        fn set_chat_commands(&mut self, chat_commands: ChatCommands) {}
    }
}
