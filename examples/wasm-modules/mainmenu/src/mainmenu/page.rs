use std::{collections::VecDeque, sync::Arc, thread::ThreadId};

use api::{graphics::graphics::GraphicsBackend, println, GRAPHICS, IO, RUNTIME_THREAD_POOL};
use base_io::{io_batcher::IOBatcherTask, yield_now::yield_now};
use client_containers_new::skins::SkinContainer;
use client_types::{
    chat::{ChatMsg, ChatMsgPlayerChannel, ServerMsg},
    console::ConsoleEntry,
    server_browser::{
        ServerBrowserData, ServerBrowserFilter, ServerBrowserInfo, ServerBrowserServer,
    },
};
use client_ui::{
    client_info::ClientInfo, console::user_data::UserData, events::UiEvents,
    main_menu::user_data::MainMenuInterface,
};
use game_config::config::Config;
use graphics::graphics::graphics::Graphics;
use shared_base::network::server_info::ServerInfo;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

struct MenuImpl {}

impl MainMenuInterface for MenuImpl {
    fn refresh(&mut self) {}
}

pub struct MainMenu {}

impl MainMenu {
    pub fn new() -> Self {
        Self {}
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<Config>,
        ui_state: &mut UIState,
        main_frame_only: bool,
    ) {
        let mut servers = Vec::new();
        for i in 0..100 {
            servers.push(ServerBrowserServer {
                info: ServerBrowserInfo {
                    name: format!("demo_server {i}"),
                    game_type: format!("demo_server {i}"),
                    map: format!("demo_server {i}"),
                    map_sha256: format!("demo_server {i}"),
                    players: Vec::new(),
                },
                address: "127.0.0.1:8303".into(),
            });
        }
        client_ui::main_menu::main_frame::render(
            ui,
            &mut UIPipe::new(
                pipe.cur_time,
                &mut client_ui::main_menu::user_data::UserData {
                    browser_data: &mut ServerBrowserData {
                        servers,
                        filter: ServerBrowserFilter {
                            search: "demo".to_string(),
                            exclude: "demo".to_string(),
                        },
                        cur_address: "127.0.0.1:8303".to_string(),
                    },
                    server_info: &Default::default(),
                    render_options: client_ui::main_menu::user_data::RenderOptions {
                        hide_buttons_right: false,
                    },
                    main_menu: &mut MenuImpl {},
                    config: &mut Default::default(),
                    events: &UiEvents::new(),
                    client_info: &ClientInfo::default(),
                },
            ),
            ui_state,
            main_frame_only,
        );
    }
}

impl UIRenderCallbackFunc<Config> for MainMenu {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<Config>,
        ui_state: &mut UIState,
    ) {
        self.render_impl(ui, pipe, ui_state, true)
    }

    fn render(&mut self, ui: &mut egui::Ui, pipe: &mut UIPipe<Config>, ui_state: &mut UIState) {
        self.render_impl(ui, pipe, ui_state, false)
    }
}
