use base::system::{self, SystemTimeInterface};
use client_containers::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use client_ui::scoreboard::{page::ScoreboardUI, user_data::UserData};
use config::config::ConfigEngine;
use egui::Color32;
use graphics::graphics::Graphics;

use shared_game::types::types::ScoreboardGameType;
use ui_base::{
    types::{UIFeedbackInterface, UINativePipe, UIPipe},
    ui::{UIDummyRawInputGenerator, UIDummyState, UI},
};

use crate::generic_ui_renderer;

pub struct ScoreboardRenderPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub sys: &'a system::System,
    pub config: &'a mut ConfigEngine,
    pub entries: &'a ScoreboardGameType,
    pub skin_container: &'a mut SkinContainer,
    pub tee_render: &'a mut RenderTee,
}

pub struct ClientStatsUIFeedbackDummy {}

impl UIFeedbackInterface for ClientStatsUIFeedbackDummy {}

pub struct ScoreboardRender {
    ui: UI<UIDummyState>,
    scoreboard_ui: ScoreboardUI,
}

impl ScoreboardRender {
    pub fn new() -> Self {
        let mut ui = UI::new(UIDummyState::default(), None);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            ui,
            scoreboard_ui: ScoreboardUI::new(),
        }
    }

    pub fn render(&mut self, pipe: &mut ScoreboardRenderPipe) {
        generic_ui_renderer::render(
            pipe.graphics,
            &mut self.ui,
            &mut self.scoreboard_ui,
            pipe.skin_container,
            pipe.tee_render,
            &mut UIPipe::new(
                &mut ClientStatsUIFeedbackDummy {},
                pipe.sys.time_get_nanoseconds(),
                pipe.config,
                UserData {
                    game_data: pipe.entries,
                },
            ),
            &mut UINativePipe {
                raw_inp_generator: &UIDummyRawInputGenerator {},
            },
            &mut UINativePipe {
                raw_inp_generator: &UIDummyRawInputGenerator {},
            },
        );
    }
}
