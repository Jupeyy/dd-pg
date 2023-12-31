use std::{collections::VecDeque, thread::ThreadId};

use api::{graphics::graphics::GraphicsBackend, println, GRAPHICS, IO, RUNTIME_THREAD_POOL};
use base_io::{io_batcher::IOBatcherTask, yield_now::yield_now};
use client_containers::skins::SkinContainer;
use client_types::{
    chat::{ChatMsg, ChatMsgPlayerChannel, ServerMsg},
    console::ConsoleEntry,
    server_browser::{ServerBrowserData, ServerBrowserFilter, ServerBrowserServer},
};
use client_ui::{chat::user_data::ChatInterface, console::user_data::UserData};
use graphics::graphics::Graphics;
use pool::datatypes::PoolString;
use shared_game::types::types::ScoreboardGameType;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

pub struct Scoreboard {}

impl Scoreboard {
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
        let mut red_players = Vec::new();
        for _ in 0..64 {
            red_players.push(shared_game::types::types::PlayerScoreboardInfo {
                skin_name: PoolString::new_str_without_pool("WWWWWWWWWWWWWWW"),
                player_name: PoolString::new_str_without_pool("WWWWWWWWWWWWWWW"),
                clan_name: PoolString::new_str_without_pool("MWWWWWWWWWWW"),
                flag_name: PoolString::new_str_without_pool("GER"),
                score: 999,
                ping: 999,
            });
        }
        let mut blue_players = Vec::new();
        for _ in 0..12 {
            blue_players.push(shared_game::types::types::PlayerScoreboardInfo {
                skin_name: PoolString::new_str_without_pool("WWWWWWWWWWWWWWW"),
                player_name: PoolString::new_str_without_pool("WWWWWWWWWWWWWWW"),
                clan_name: PoolString::new_str_without_pool("MWWWWWWWWWWW"),
                flag_name: PoolString::new_str_without_pool("GER"),
                score: 999,
                ping: 999,
            });
        }
        let mut spectator_players = Vec::new();
        for _ in 0..12 {
            spectator_players.push(shared_game::types::types::PlayerScoreboardInfo {
                skin_name: PoolString::new_str_without_pool("WWWWWWWWWWWWWWW"),
                player_name: PoolString::new_str_without_pool("WWWWWWWWWWWWWWW"),
                clan_name: PoolString::new_str_without_pool("MWWWWWWWWWWW"),
                flag_name: PoolString::new_str_without_pool("GER"),
                score: 999,
                ping: 999,
            });
        }
        client_ui::scoreboard::main_frame::render(
            ui,
            &mut UIPipe::new(
                pipe.ui_feedback,
                pipe.cur_time,
                pipe.config,
                client_ui::scoreboard::user_data::UserData {
                    game_data: &ScoreboardGameType::TeamPlay {
                        red_players,
                        blue_players,
                        spectator_players,
                    },
                },
            ),
            ui_state,
            graphics,
            main_frame_only,
        );
        /*let mut players = Vec::new();
        for _ in 0..128 {
            players.push(());
        }
        let mut spectator_players = Vec::new();
        for _ in 0..12 {
            spectator_players.push(());
        }
        client_ui::scoreboard::main_frame::render(
            ui,
            &mut UIPipe::new(
                pipe.ui_feedback,
                pipe.cur_time,
                pipe.config,
                client_ui::scoreboard::user_data::UserData {
                    game_data: &ScoreboardGameType::SoloPlay {
                        players,
                        spectator_players,
                    },
                },
            ),
            ui_state,
            graphics,
            main_frame_only,
        );*/
    }
}

impl UIRenderCallbackFunc<()> for Scoreboard {
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
