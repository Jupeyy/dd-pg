use game_config::config::Config;
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

use crate::events::UiEvents;

use super::{
    main_frame,
    user_data::{ConnectMode, UserData},
};

pub struct ConnectingUi {
    mode: ConnectMode,
    events: UiEvents,
}

impl ConnectingUi {
    pub fn new(mode: ConnectMode, events: UiEvents) -> Self {
        Self { mode, events }
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<Config>,
        main_frame_only: bool,
    ) {
        main_frame::render(
            ui,
            &mut UiRenderPipe {
                cur_time: pipe.cur_time,
                user_data: &mut UserData {
                    mode: &self.mode,
                    config: pipe.user_data,
                    events: &self.events,
                },
            },
            main_frame_only,
        );
    }
}

impl UiPageInterface<Config> for ConnectingUi {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<Config>,
        _ui_state: &mut UiState,
    ) {
        self.render_impl(ui, pipe, true)
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<Config>,
        _ui_state: &mut UiState,
    ) {
        self.render_impl(ui, pipe, false)
    }
}
