use std::sync::Arc;

use base_io::io::Io;

use game_config::config::Config;
use graphics::graphics::graphics::Graphics;
use shared_base::network::server_info::ServerInfo;
use sound::sound::SoundManager;
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

use crate::{
    client_info::ClientInfo,
    events::UiEvents,
    main_menu::{page::MainMenuUi, profiles_interface::ProfilesInterface, user_data::UiMonitors},
};

use super::{
    main_frame, server_info::GameServerInfo, server_players::ServerPlayers, user_data::UserData,
    votes::Votes,
};

pub struct IngameMenuUi {
    main_menu: MainMenuUi,
    server_players: ServerPlayers,
    game_server_info: GameServerInfo,
    votes: Votes,
}

impl IngameMenuUi {
    pub fn new(
        graphics: &Graphics,
        sound: &SoundManager,
        server_info: Arc<ServerInfo>,
        client_info: ClientInfo,
        events: UiEvents,
        io: Io,
        tp: Arc<rayon::ThreadPool>,
        profiles: Arc<dyn ProfilesInterface>,
        monitors: UiMonitors,
        server_players: ServerPlayers,
        game_server_info: GameServerInfo,
        votes: Votes,
    ) -> Self {
        Self {
            main_menu: MainMenuUi::new(
                graphics,
                sound,
                server_info,
                client_info,
                events,
                io,
                tp,
                profiles,
                monitors,
            ),
            server_players,
            game_server_info,
            votes,
        }
    }

    fn get_user_data<'a>(&'a mut self, config: &'a mut Config, ui: &egui::Ui) -> UserData<'a> {
        UserData {
            browser_menu: self.main_menu.get_user_data(config, true, ui),
            server_players: &self.server_players,
            game_server_info: &self.game_server_info,
            votes: &self.votes,
        }
    }
}

impl UiPageInterface<Config> for IngameMenuUi {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<Config>,
        ui_state: &mut UiState,
    ) {
        main_frame::render(
            ui,
            &mut UiRenderPipe {
                cur_time: pipe.cur_time,
                user_data: &mut self.get_user_data(pipe.user_data, ui),
            },
            ui_state,
            true,
        );
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<Config>,
        ui_state: &mut UiState,
    ) {
        main_frame::render(
            ui,
            &mut UiRenderPipe {
                cur_time: pipe.cur_time,
                user_data: &mut self.get_user_data(pipe.user_data, ui),
            },
            ui_state,
            false,
        )
    }

    fn unmount(&mut self) {
        self.main_menu.unmount();
    }
}
