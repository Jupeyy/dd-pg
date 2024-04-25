use std::sync::Arc;

use base_io::io::IO;

use game_config::config::Config;
use shared_base::network::server_info::ServerInfo;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

use crate::{client_info::ClientInfo, events::UiEvents, main_menu::page::MainMenuUI};

use super::{main_frame, user_data::UserData};

pub struct IngameMenuUI {
    main_menu: MainMenuUI,
}

impl IngameMenuUI {
    pub fn new(
        server_info: Arc<ServerInfo>,
        client_info: ClientInfo,
        events: UiEvents,
        io: IO,
    ) -> Self {
        Self {
            main_menu: MainMenuUI::new(server_info, client_info, events, io),
        }
    }

    fn get_user_data<'a>(&'a mut self, config: &'a mut Config) -> UserData<'a> {
        UserData {
            browser_menu: self.main_menu.get_user_data(config, true),
        }
    }
}

impl<'a> UIRenderCallbackFunc<Config> for IngameMenuUI {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<Config>,
        ui_state: &mut UIState,
    ) {
        main_frame::render(
            ui,
            &mut UIPipe {
                cur_time: pipe.cur_time,
                user_data: &mut self.get_user_data(pipe.user_data),
            },
            ui_state,
            true,
        );
    }

    fn render(&mut self, ui: &mut egui::Ui, pipe: &mut UIPipe<Config>, ui_state: &mut UIState) {
        main_frame::render(
            ui,
            &mut UIPipe {
                cur_time: pipe.cur_time,
                user_data: &mut self.get_user_data(pipe.user_data),
            },
            ui_state,
            false,
        )
    }
}
