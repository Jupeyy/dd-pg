use api_ui_game::render::{create_ctf_container, create_skin_container};
use client_containers::{ctf::CtfContainer, skins::SkinContainer};
use client_render_base::render::tee::RenderTee;
use game_interface::types::{
    character_info::{NetworkCharacterInfo, NetworkSkinInfo},
    game::GameEntityId,
    id_gen::IdGenerator,
    render::{
        character::{CharacterInfo, TeeEye},
        game::GameRenderInfo,
    },
};
use graphics::{
    graphics::graphics::Graphics,
    handles::{canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle},
};
use hashlink::LinkedHashMap;
use pool::{datatypes::PoolString, rc::PoolRc};
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

pub struct HudPage {
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    skin_container: SkinContainer,
    render_tee: RenderTee,
    ctf_container: CtfContainer,
    character_infos: LinkedHashMap<GameEntityId, CharacterInfo>,
}

impl HudPage {
    pub fn new(graphics: &Graphics) -> Self {
        let mut character_infos: LinkedHashMap<GameEntityId, CharacterInfo> = Default::default();
        let id_gen = IdGenerator::new();
        character_infos.insert(
            id_gen.next_id(),
            CharacterInfo {
                info: PoolRc::from_item_without_pool(NetworkCharacterInfo::explicit_default()),
                skin_info: NetworkSkinInfo::Original,
                stage_id: Some(id_gen.next_id()),
                player_info: None,
                browser_score: PoolString::new_without_pool(),
                browser_eye: TeeEye::Happy,
            },
        );
        Self {
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            skin_container: create_skin_container(),
            render_tee: RenderTee::new(graphics),
            ctf_container: create_ctf_container(),
            character_infos,
        }
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        ui_state: &mut UiState,
        main_frame_only: bool,
    ) {
        client_ui::hud::main_frame::render(
            ui,
            &mut UiRenderPipe::new(
                pipe.cur_time,
                &mut client_ui::hud::user_data::UserData {
                    race_timer_counter: &456156,
                    ticks_per_second: &50.try_into().unwrap(),
                    /*game: Some(&GameRenderInfo::Match {
                        standings: MatchStandings::Solo {
                            leading_characters: [
                                Some(LeadingCharacter {
                                    character_id: *self.character_infos.front().unwrap().0,
                                    score: 999,
                                }),
                                None,
                            ],
                        },
                    }),*/
                    /*game: Some(&GameRenderInfo::Match {
                        standings: MatchStandings::Sided {
                            score_red: 999,
                            score_blue: -999,
                        },
                    }),*/
                    game: Some(&GameRenderInfo::Race {}),
                    skin_container: &mut self.skin_container,
                    skin_renderer: &self.render_tee,
                    ctf_container: &mut self.ctf_container,
                    character_infos: &self.character_infos,
                    canvas_handle: &self.canvas_handle,
                    stream_handle: &self.stream_handle,
                },
            ),
            ui_state,
            main_frame_only,
        );
    }
}

impl UiPageInterface<()> for HudPage {
    fn has_blur(&self) -> bool {
        false
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
