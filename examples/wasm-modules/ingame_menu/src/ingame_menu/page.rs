use std::{collections::VecDeque, sync::Arc, thread::ThreadId};

use api::{graphics::graphics::GraphicsBackend, println, GRAPHICS, IO, RUNTIME_THREAD_POOL};
use base_io::{io_batcher::IOBatcherTask, yield_now::yield_now};
use client_containers::skins::SkinContainer;
use client_types::{
    chat::{ChatMsg, ChatMsgPlayerChannel, ServerMsg},
    console::ConsoleEntry,
    server_browser::{
        ServerBrowserData, ServerBrowserFilter, ServerBrowserInfo, ServerBrowserServer,
    },
};
use client_ui::{
    chat::user_data::ChatInterface, console::user_data::UserData,
    main_menu::user_data::MainMenuInterface,
};
use graphics::graphics::Graphics;
use shared_base::network::server_info::ServerInfo;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

struct MenuImpl {}

impl MainMenuInterface for MenuImpl {
    fn refresh(&mut self) {}
}

pub struct IngameMenu {}

impl IngameMenu {
    pub fn new() -> Self {
        Self {}
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
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
        client_ui::ingame_menu::main_frame::render(
            ui,
            &mut UIPipe::new(
                pipe.ui_feedback,
                pipe.cur_time,
                pipe.config,
                client_ui::ingame_menu::user_data::UserData {
                    browser_menu: client_ui::main_menu::user_data::UserData {
                        browser_data: &mut ServerBrowserData {
                            servers,
                            filter: ServerBrowserFilter {
                                search: "demo".to_string(),
                                exclude: "demo".to_string(),
                            },
                            cur_address: "127.0.0.1:8303".to_string(),
                        },
                        server_info: &Arc::new(ServerInfo {
                            sock_addr: Default::default(),
                        }),
                        render_options: client_ui::main_menu::user_data::RenderOptions {
                            hide_buttons_right: true,
                        },
                        main_menu: &mut MenuImpl {},
                    },
                },
            ),
            ui_state,
            graphics,
            main_frame_only,
        );
    }
}

impl UIRenderCallbackFunc<()> for IngameMenu {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
    ) {
        self.render_impl(ui, pipe, ui_state, graphics, true)
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
    ) {
        self.render_impl(ui, pipe, ui_state, graphics, false)
    }
}
