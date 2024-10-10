use api::read_param_from_host;
use api::GRAPHICS;
use api::GRAPHICS_BACKEND;
use api::SOUND;
use api_wasm_macros::guest_func_call_from_host_auto_dummy;
use api_wasm_macros::{guest_func_call_from_host_auto, impl_guest_functions_render_game};
use client_render_game::render_game::RenderGameCreateOptions;
use client_render_game::render_game::RenderGameInterface;
use config::config::ConfigDebug;
use game_config::config::ConfigMap;

// TODO: remove them
use api::read_param_from_host_ex;
use api::upload_return_val;
use game_interface::chat_commands::ChatCommands;
use graphics_types::types::WindowProps;

extern "Rust" {
    /// returns an instance of the game state and the game tick speed
    fn mod_render_game_new(
        map_file: Vec<u8>,
        config: &ConfigDebug,
        props: RenderGameCreateOptions,
    ) -> Box<dyn RenderGameInterface>;
}

pub struct ApiRenderGame {
    state: Option<Box<dyn RenderGameInterface>>,
}

static mut API_RENDER_GAME: once_cell::unsync::Lazy<ApiRenderGame> =
    once_cell::unsync::Lazy::new(|| ApiRenderGame { state: None });

#[no_mangle]
pub fn render_game_new() {
    let map_file: Vec<u8> = read_param_from_host(0);
    let config: ConfigDebug = read_param_from_host(1);
    let window_props: WindowProps = read_param_from_host(2);
    let props: RenderGameCreateOptions = read_param_from_host(3);
    unsafe { GRAPHICS.borrow().canvas_handle.resized(window_props) };
    unsafe {
        API_RENDER_GAME.create(map_file, &config, props);
    };
}

impl ApiRenderGame {
    fn create(&mut self, map_file: Vec<u8>, config: &ConfigDebug, props: RenderGameCreateOptions) {
        let state = unsafe { mod_render_game_new(map_file, config, props) };
        self.state = Some(state);
    }
}

#[impl_guest_functions_render_game]
impl ApiRenderGame {
    #[guest_func_call_from_host_auto_dummy]
    fn api_update_window_props(&self) {
        let window_props: WindowProps = read_param_from_host(0);
        unsafe { GRAPHICS.borrow_mut().canvas_handle.resized(window_props) };
    }
}

#[impl_guest_functions_render_game]
impl RenderGameInterface for ApiRenderGame {
    #[guest_func_call_from_host_auto(option)]
    fn render(
        &mut self,
        config_map: &ConfigMap,
        cur_time: &std::time::Duration,
        input: client_render_game::render_game::RenderGameInput,
    ) -> client_render_game::render_game::RenderGameResult {
        unsafe {
            GRAPHICS_BACKEND.actual_run_cmds.set(false);
            let graphics = &mut *GRAPHICS;
            graphics
                .borrow()
                .backend_handle
                .run_backend_buffer(graphics.borrow().stream_handle.stream_data());
            GRAPHICS_BACKEND.actual_run_cmds.set(true);
            SOUND.borrow().backend_handle.run_cmds();
        }
    }

    #[guest_func_call_from_host_auto(option)]
    fn continue_map_loading(&mut self) -> bool {}

    #[guest_func_call_from_host_auto(option)]
    fn set_chat_commands(&mut self, chat_commands: ChatCommands) {}

    #[guest_func_call_from_host_auto(option)]
    fn clear_render_state(&mut self) {}

    #[guest_func_call_from_host_auto(option)]
    fn render_offair_sound(&mut self, samples: u32) {}
}
