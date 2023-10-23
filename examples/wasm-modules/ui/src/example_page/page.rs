use std::{collections::VecDeque, thread::ThreadId};

use api::{
    graphics::graphics::{Graphics, GraphicsBackend},
    println, GRAPHICS, IO, RUNTIME_THREAD_POOL,
};
use base_io::{io_batcher::TokIOBatcherTask, yield_now::yield_now};
use client_containers::skins::SkinContainer;
use client_types::{
    console::ConsoleEntry,
    scoreboard::ScoreboardGameType,
    server_browser::{ServerBrowserData, ServerBrowserFilter, ServerBrowserServer},
};
use client_ui::console::user_data::UserData;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

pub struct ExamplePage {}

impl ExamplePage {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc<(), GraphicsBackend> for ExamplePage {
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
        //super::main_frame::render(ui, pipe, ui_state, graphics);
        /*client_ui::main_menu::main_frame::render(
            ui,
            &mut UIPipe::new(
                pipe.ui_feedback,
                pipe.cur_time,
                pipe.config,
                client_ui::main_menu::user_data::UserData {
                    browser_data: &mut ServerBrowserData {
                        servers: Vec::new(),
                        filter: ServerBrowserFilter {
                            search: String::new(),
                            exclude: String::new(),
                        },
                        cur_address: String::new(),
                    },
                },
            ),
            ui_state,
            graphics,
            true,
        );*/
        /*client_ui::console::main_frame::render(
            ui,
            &mut UIPipe {
                ui_feedback: pipe.ui_feedback,
                cur_time: pipe.cur_time,
                config: pipe.config,
                user_data: UserData {
                    entries: &Vec::new(),
                    msgs: &mut String::new(),
                    msg: &mut String::new(),
                },
            },
            ui_state,
            graphics,
            true,
        )*/
        client_ui::scoreboard::main_frame::render(
            ui,
            &mut UIPipe::new(
                pipe.ui_feedback,
                pipe.cur_time,
                pipe.config,
                client_ui::scoreboard::user_data::UserData {
                    game_data: &ScoreboardGameType::TeamPlay {
                        red_players: Vec::new(),
                        blue_players: Vec::new(),
                        spectator_players: Vec::new(),
                    },
                },
            ),
            ui_state,
            graphics,
            true,
        );
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
    ) {
        //super::main_frame::render(ui, pipe, ui_state, graphics)
        /*let mut servers = Vec::new();
        for i in 0..100 {
            servers.push(ServerBrowserServer {
                name: format!("demo_server {i}"),
                game_type: format!("demo_server {i}"),
                map: format!("demo_server {i}"),
                map_sha256: format!("demo_server {i}"),
                players: Vec::new(),
            });
        }
        client_ui::main_menu::main_frame::render(
            ui,
            &mut UIPipe::new(
                pipe.ui_feedback,
                pipe.cur_time,
                pipe.config,
                client_ui::main_menu::user_data::UserData {
                    browser_data: &mut ServerBrowserData {
                        servers,
                        filter: ServerBrowserFilter {
                            search: "demo".to_string(),
                            exclude: "demo".to_string(),
                        },
                        cur_address: "127.0.0.1:8303".to_string(),
                    },
                },
            ),
            ui_state,
            graphics,
            false,
        );*/

        /*let mut logs = String::new();
        for i in 0..100 {
            logs.push_str(&format!("test {i}\ntestr2\n"));
        }
        client_ui::console::main_frame::render(
            ui,
            &mut UIPipe {
                ui_feedback: pipe.ui_feedback,
                cur_time: pipe.cur_time,
                config: pipe.config,
                user_data: UserData {
                    entries: &vec![
                        ConsoleEntry {
                            full_name: "test".to_string(),
                            usage: "test".to_string(),
                        },
                        ConsoleEntry {
                            full_name: "test2".to_string(),
                            usage: "test2".to_string(),
                        },
                        ConsoleEntry {
                            full_name: "tset".to_string(),
                            usage: "tset".to_string(),
                        },
                    ],
                    msgs: &mut logs,
                    msg: &mut "te".to_string(),
                },
            },
            ui_state,
            graphics,
            false,
        )*/
        client_ui::scoreboard::main_frame::render(
            ui,
            &mut UIPipe::new(
                pipe.ui_feedback,
                pipe.cur_time,
                pipe.config,
                client_ui::scoreboard::user_data::UserData {
                    game_data: &ScoreboardGameType::TeamPlay {
                        red_players: Vec::new(),
                        blue_players: Vec::new(),
                        spectator_players: Vec::new(),
                    },
                },
            ),
            ui_state,
            graphics,
            false,
        );
    }
}
