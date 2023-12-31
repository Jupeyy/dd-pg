use std::sync::Arc;

use base_io::io::IO;
use graphics::graphics::Graphics;
use shared_base::network::server_info::ServerInfo;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

use crate::main_menu::page::MainMenuUI;

use super::{main_frame, user_data::UserData};

pub struct IngameMenuUI {
    main_menu: MainMenuUI,
}

impl IngameMenuUI {
    pub fn new(server_info: Arc<ServerInfo>, io: IO) -> Self {
        Self {
            main_menu: MainMenuUI::new(server_info, io),
        }
    }

    fn get_user_data<'a>(&'a mut self) -> UserData<'a> {
        UserData {
            browser_menu: self.main_menu.get_user_data(true),
        }
    }
}

impl<'a> UIRenderCallbackFunc<()> for IngameMenuUI {
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
        main_frame::render(
            ui,
            &mut UIPipe {
                config: pipe.config,
                cur_time: pipe.cur_time,
                ui_feedback: pipe.ui_feedback,
                user_data: self.get_user_data(),
            },
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
        main_frame::render(
            ui,
            &mut UIPipe {
                config: pipe.config,
                cur_time: pipe.cur_time,
                ui_feedback: pipe.ui_feedback,
                user_data: self.get_user_data(),
            },
            ui_state,
            graphics,
            false,
        )
    }
}
