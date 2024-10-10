use std::time::Duration;

use api_ui_game::render::{create_flags_container, create_skin_container};
use client_containers::{flags::FlagsContainer, skins::SkinContainer};
use client_render_base::render::tee::RenderTee;
use game_interface::types::{
    character_info::{NetworkCharacterInfo, NetworkSkinInfo},
    game::GameEntityId,
    id_gen::IdGenerator,
    network_stats::PlayerNetworkStats,
    network_string::NetworkString,
    render::{
        character::{CharacterInfo, CharacterPlayerInfo, PlayerCameraMode, TeeEye},
        scoreboard::{
            ScoreboardCharacterInfo, ScoreboardConnectionType, ScoreboardGameOptions,
            ScoreboardGameType, ScoreboardStageInfo,
        },
    },
};
use graphics::{
    graphics::graphics::Graphics,
    handles::{canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle},
};
use hashlink::LinkedHashMap;
use math::math::vector::ubvec4;
use pool::{
    datatypes::{PoolLinkedHashMap, PoolString, PoolVec},
    rc::PoolRc,
};
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

pub struct Scoreboard {
    stream_handle: GraphicsStreamHandle,
    canvas_handle: GraphicsCanvasHandle,
    skin_container: SkinContainer,
    render_tee: RenderTee,
    flags_container: FlagsContainer,
}

impl Scoreboard {
    pub fn new(graphics: &Graphics) -> Self {
        Self {
            stream_handle: graphics.stream_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            skin_container: create_skin_container(),
            render_tee: RenderTee::new(graphics),
            flags_container: create_flags_container(),
        }
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        ui_state: &mut UiState,
        main_frame_only: bool,
    ) {
        let mut red_stages = PoolLinkedHashMap::new_without_pool();
        let mut blue_stages = PoolLinkedHashMap::new_without_pool();

        let mut red_players = PoolVec::new_without_pool();
        let mut character_infos: LinkedHashMap<GameEntityId, CharacterInfo> = Default::default();
        let gen = IdGenerator::new();
        for i in 0..64 {
            let id = gen.next_id();
            character_infos.insert(
                id,
                CharacterInfo {
                    info: PoolRc::from_item_without_pool({
                        let mut info = NetworkCharacterInfo::explicit_default();

                        info.skin = "WWWWWWWWWWWWWWW".try_into().unwrap();
                        info.skin_info = NetworkSkinInfo::Custom {
                            body_color: ubvec4::new(255, 255, 255, 255),
                            feet_color: ubvec4::new(0, 255, 255, 255),
                        };
                        info.name = NetworkString::new("WWWWWWWWWWWWWWW").unwrap();
                        info.clan = NetworkString::new("MWWWWWWWWWWW").unwrap();
                        info.flag = NetworkString::new("CH").unwrap();

                        info
                    }),
                    skin_info: NetworkSkinInfo::Custom {
                        body_color: ubvec4::new(255, 255, 255, 255),
                        feet_color: ubvec4::new(0, 255, 255, 255),
                    },
                    stage_id: None,
                    player_info: Some(CharacterPlayerInfo {
                        cam_mode: PlayerCameraMode::Default,
                    }),
                    browser_score: PoolString::new_str_without_pool("999"),
                    browser_eye: TeeEye::Normal,
                },
            );

            red_players.push(ScoreboardCharacterInfo {
                id,
                score: 999,
                ping: ScoreboardConnectionType::Network(PlayerNetworkStats {
                    ping: Duration::from_millis(999),
                    ..Default::default()
                }),
            });

            if i % 3 == 0 {
                red_stages.insert(
                    gen.next_id(),
                    ScoreboardStageInfo {
                        characters: std::mem::replace(
                            &mut red_players,
                            PoolVec::new_without_pool(),
                        ),
                        max_size: 0,
                        name: PoolString::new_str_without_pool("TEST"),
                        color: ubvec4::new(
                            (i % 256) as u8,
                            255 - (i % 256) as u8,
                            255 * (i % 2) as u8,
                            20,
                        ),
                    },
                );
            }
        }
        let mut blue_players = PoolVec::new_without_pool();
        for i in 0..12 {
            let id = gen.next_id();
            character_infos.insert(
                id,
                CharacterInfo {
                    info: PoolRc::from_item_without_pool({
                        let mut info = NetworkCharacterInfo::explicit_default();

                        info.skin = "WWWWWWWWWWWWWWW".try_into().unwrap();
                        info.skin_info = NetworkSkinInfo::Original;
                        info.name = NetworkString::new("WWWWWWWWWWWWWWW").unwrap();
                        info.clan = NetworkString::new("MWWWWWWWWWWW").unwrap();
                        info.flag = NetworkString::new("GB").unwrap();

                        info
                    }),
                    skin_info: NetworkSkinInfo::Original,
                    stage_id: None,
                    player_info: Some(CharacterPlayerInfo {
                        cam_mode: PlayerCameraMode::Default,
                    }),
                    browser_score: PoolString::new_str_without_pool("999"),
                    browser_eye: TeeEye::Normal,
                },
            );
            blue_players.push(ScoreboardCharacterInfo {
                id,
                score: 999,
                ping: ScoreboardConnectionType::Network(PlayerNetworkStats {
                    ping: Duration::from_millis(999),
                    ..Default::default()
                }),
            });
            if i % 3 == 0 {
                blue_stages.insert(
                    gen.next_id(),
                    ScoreboardStageInfo {
                        characters: std::mem::replace(
                            &mut blue_players,
                            PoolVec::new_without_pool(),
                        ),
                        max_size: 0,
                        name: PoolString::new_str_without_pool("TEST"),
                        color: ubvec4::new(
                            (i % 256) as u8,
                            255 - (i % 256) as u8,
                            255 * (i % 2) as u8,
                            20,
                        ),
                    },
                );
            }
        }
        let mut spectator_players = PoolVec::new_without_pool();
        for _ in 0..12 {
            let id = gen.next_id();
            character_infos.insert(
                id,
                CharacterInfo {
                    info: PoolRc::from_item_without_pool({
                        let mut info = NetworkCharacterInfo::explicit_default();

                        info.skin = "WWWWWWWWWWWWWWW".try_into().unwrap();
                        info.skin_info = NetworkSkinInfo::Original;
                        info.name = NetworkString::new("WWWWWWWWWWWWWWW").unwrap();
                        info.clan = NetworkString::new("MWWWWWWWWWWW").unwrap();
                        info.flag = NetworkString::new("DE").unwrap();

                        info
                    }),
                    skin_info: NetworkSkinInfo::Original,
                    stage_id: None,
                    player_info: Some(CharacterPlayerInfo {
                        cam_mode: PlayerCameraMode::Default,
                    }),
                    browser_score: PoolString::new_str_without_pool("999"),
                    browser_eye: TeeEye::Angry,
                },
            );
            spectator_players.push(ScoreboardCharacterInfo {
                id,
                score: 999,
                ping: ScoreboardConnectionType::Network(PlayerNetworkStats {
                    ping: Duration::from_millis(999),
                    ..Default::default()
                }),
            });
        }
        client_ui::scoreboard::main_frame::render(
            ui,
            &mut UiRenderPipe::new(
                pipe.cur_time,
                &mut client_ui::scoreboard::user_data::UserData {
                    scoreboard: &game_interface::types::render::scoreboard::Scoreboard {
                        game: ScoreboardGameType::SidedPlay {
                            ignore_stage: *red_stages.front().unwrap().0,
                            red_stages,
                            blue_stages,
                            spectator_players,
                            red_side_name: PoolString::new_str_without_pool("Red Team"),
                            blue_side_name: PoolString::new_str_without_pool("Blue Team"),
                        },
                        options: ScoreboardGameOptions {
                            map_name: PoolString::new_str_without_pool("A_Map"),
                            score_limit: 50,
                        },
                    },
                    character_infos: &character_infos,
                    canvas_handle: &self.canvas_handle,
                    stream_handle: &self.stream_handle,
                    skin_container: &mut self.skin_container,
                    render_tee: &self.render_tee,
                    flags_container: &mut self.flags_container,
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
            &mut UiRenderPipe::new(
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

impl UiPageInterface<()> for Scoreboard {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        ui_state: &mut UiState,
    ) {
        self.render_impl(ui, pipe, ui_state, true)
    }

    fn render(&mut self, ui: &mut egui::Ui, pipe: &mut UiRenderPipe<()>, ui_state: &mut UiState) {
        self.render_impl(ui, pipe, ui_state, false)
    }
}
