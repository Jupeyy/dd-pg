use std::{collections::VecDeque, thread::ThreadId};

use api::{graphics::graphics::GraphicsBackend, println, GRAPHICS, IO, RUNTIME_THREAD_POOL};
use base_io::{io_batcher::IOBatcherTask, yield_now::yield_now};
use client_containers_new::skins::SkinContainer;
use client_types::{
    chat::{ChatMsg, ChatMsgPlayerChannel, ServerMsg},
    console::ConsoleEntry,
    server_browser::{ServerBrowserData, ServerBrowserFilter, ServerBrowserServer},
};
use client_ui::console::user_data::UserData;
use game_interface::{
    resource_key::PoolResourceKey,
    types::{
        game::GameEntityId,
        id_gen::IdGenerator,
        render::{
            character::CharacterInfo,
            scoreboard::{ScoreboardCharacterInfo, ScoreboardGameType},
        },
    },
};
use graphics::{graphics::graphics::Graphics, handles::canvas::canvas::GraphicsCanvasHandle};
use hashlink::LinkedHashMap;
use pool::datatypes::{PoolString, PoolVec};
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

pub struct Scoreboard {
    canvas_handle: GraphicsCanvasHandle,
}

impl Scoreboard {
    pub fn new(graphics: &Graphics) -> Self {
        Self {
            canvas_handle: graphics.canvas_handle.clone(),
        }
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        main_frame_only: bool,
    ) {
        let mut red_players = PoolVec::new_without_pool();
        let mut character_infos: LinkedHashMap<GameEntityId, CharacterInfo> = Default::default();
        let mut gen = IdGenerator::new();
        for _ in 0..64 {
            let id = gen.get_next();
            character_infos.insert(
                id,
                CharacterInfo {
                    skin: {
                        let mut skin = PoolResourceKey::new_without_pool();
                        *skin = "WWWWWWWWWWWWWWW".into();
                        skin
                    },
                    name: PoolString::new_str_without_pool("WWWWWWWWWWWWWWW"),
                    clan: PoolString::new_str_without_pool("MWWWWWWWWWWW"),
                    country: PoolString::new_str_without_pool("GER"),
                },
            );

            red_players.push(ScoreboardCharacterInfo {
                id,
                score: 999,
                ping: 999,
            });
        }
        let mut blue_players = PoolVec::new_without_pool();
        for _ in 0..12 {
            let id = gen.get_next();
            character_infos.insert(
                id,
                CharacterInfo {
                    skin: {
                        let mut skin = PoolResourceKey::new_without_pool();
                        *skin = "WWWWWWWWWWWWWWW".into();
                        skin
                    },
                    name: PoolString::new_str_without_pool("WWWWWWWWWWWWWWW"),
                    clan: PoolString::new_str_without_pool("MWWWWWWWWWWW"),
                    country: PoolString::new_str_without_pool("GER"),
                },
            );
            blue_players.push(ScoreboardCharacterInfo {
                id,
                score: 999,
                ping: 999,
            });
        }
        let mut spectator_players = PoolVec::new_without_pool();
        for _ in 0..12 {
            let id = gen.get_next();
            character_infos.insert(
                id,
                CharacterInfo {
                    skin: {
                        let mut skin = PoolResourceKey::new_without_pool();
                        *skin = "WWWWWWWWWWWWWWW".into();
                        skin
                    },
                    name: PoolString::new_str_without_pool("WWWWWWWWWWWWWWW"),
                    clan: PoolString::new_str_without_pool("MWWWWWWWWWWW"),
                    country: PoolString::new_str_without_pool("GER"),
                },
            );
            spectator_players.push(ScoreboardCharacterInfo {
                id,
                score: 999,
                ping: 999,
            });
        }
        client_ui::scoreboard::main_frame::render(
            ui,
            &mut UIPipe::new(
                pipe.cur_time,
                &mut client_ui::scoreboard::user_data::UserData {
                    game_data: &ScoreboardGameType::TeamPlay {
                        red_characters: red_players,
                        blue_characters: blue_players,
                        spectator_players,
                    },
                    character_infos: &character_infos,
                    canvas_handle: &self.canvas_handle,
                },
            ),
            ui_state,
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
    ) {
        self.render_impl(ui, pipe, ui_state, true)
    }

    fn render(&mut self, ui: &mut egui::Ui, pipe: &mut UIPipe<()>, ui_state: &mut UIState) {
        self.render_impl(ui, pipe, ui_state, false)
    }
}
